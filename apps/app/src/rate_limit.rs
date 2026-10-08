//! Per-client-address limits on the endpoints anyone can call to reach an account: sign-in and
//! password reset (DNK-32). Lockout protects one account and one pending email protects one
//! inbox; these limits stop one client from trying many accounts or asking for many emails.
//!
//! Each limit is a GCRA (generic cell rate algorithm) per client address: `requests` within any
//! minute, spread evenly once the burst is spent. State lives in memory, so each Studio instance
//! counts its own requests.

use crate::error::ApiError;
use crate::AppState;
use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::request::Parts;
use ipnet::IpNet;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The window a limit's `requests` are counted over.
pub const WINDOW: Duration = Duration::from_secs(60);

/// Addresses tracked before idle ones are dropped.
const PRUNE_ABOVE: usize = 10_000;

/// At most `requests` per client address within any [`WINDOW`].
#[derive(Debug)]
pub struct RateLimiter {
    /// Time one request "costs".
    interval: Duration,
    /// When each address's budget is fully restored (its theoretical arrival time).
    restored_at: Mutex<HashMap<IpAddr, Instant>>,
}

impl RateLimiter {
    pub fn per_minute(requests: u32) -> Self {
        Self {
            interval: WINDOW / requests.max(1),
            restored_at: Mutex::new(HashMap::new()),
        }
    }

    /// Counts one request from `client`, or says how long until it would be allowed.
    pub fn check(&self, client: IpAddr) -> Result<(), Duration> {
        self.check_at(client, Instant::now())
    }

    fn check_at(&self, client: IpAddr, now: Instant) -> Result<(), Duration> {
        let mut restored_at = self
            .restored_at
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if restored_at.len() > PRUNE_ABOVE {
            restored_at.retain(|_, at| *at > now);
        }
        let start = restored_at.get(&client).copied().unwrap_or(now).max(now);
        let next = start + self.interval;
        let used = next - now;
        if used > WINDOW {
            return Err(used - WINDOW);
        }
        restored_at.insert(client, next);
        Ok(())
    }
}

/// The limits on sign-in and password reset, and whose `X-Forwarded-For` to believe.
#[derive(Clone, Debug)]
pub struct AuthLimits {
    pub sign_in: Arc<RateLimiter>,
    pub password_reset: Arc<RateLimiter>,
    /// Reverse proxies in front of Studio. Only their `X-Forwarded-For` is read.
    pub trusted_proxies: Arc<[IpNet]>,
}

impl AuthLimits {
    pub fn new(
        sign_in_per_minute: u32,
        reset_per_minute: u32,
        trusted_proxies: Vec<IpNet>,
    ) -> Self {
        Self {
            sign_in: Arc::new(RateLimiter::per_minute(sign_in_per_minute)),
            password_reset: Arc::new(RateLimiter::per_minute(reset_per_minute)),
            trusted_proxies: trusted_proxies.into(),
        }
    }

    fn trusted(&self, addr: IpAddr) -> bool {
        self.trusted_proxies.iter().any(|net| net.contains(&addr))
    }

    /// The client's address: the peer, unless the peer is a trusted proxy; then the last
    /// `X-Forwarded-For` hop that is not one (proxies append, so earlier hops can be forged).
    pub fn client(&self, peer: IpAddr, forwarded_for: Option<&str>) -> IpAddr {
        if !self.trusted(peer) {
            return peer;
        }
        let Some(header) = forwarded_for else {
            return peer;
        };
        let mut client = peer;
        for hop in header.rsplit(',') {
            let Ok(addr) = hop.trim().parse::<IpAddr>() else {
                return client;
            };
            client = addr;
            if !self.trusted(addr) {
                break;
            }
        }
        client
    }
}

/// The address a request comes from, as [`AuthLimits::client`] decides it.
pub struct ClientAddr(pub IpAddr);

impl ClientAddr {
    /// Counts this request against `limiter`.
    pub fn check(&self, limiter: &RateLimiter) -> Result<(), ApiError> {
        limiter.check(self.0).map_err(ApiError::RateLimited)
    }
}

impl FromRequestParts<AppState> for ClientAddr {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Always there when served (main.rs); absent only when a test calls the router directly.
        let peer = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED), |info| info.0.ip());
        let forwarded_for = parts
            .headers
            .get("x-forwarded-for")
            .and_then(|value| value.to_str().ok());
        Ok(Self(state.auth_limits.client(peer, forwarded_for)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(text: &str) -> IpAddr {
        text.parse().unwrap()
    }

    #[test]
    fn allows_the_burst_then_refuses_until_a_request_is_restored() {
        let limiter = RateLimiter::per_minute(3);
        let now = Instant::now();
        let client = ip("203.0.113.7");
        for _ in 0..3 {
            assert!(limiter.check_at(client, now).is_ok());
        }
        let wait = limiter.check_at(client, now).unwrap_err();
        assert_eq!(wait, Duration::from_secs(20));
        // Another address has its own budget.
        assert!(limiter.check_at(ip("203.0.113.8"), now).is_ok());
        // One request is restored every 20 seconds.
        assert!(limiter.check_at(client, now + wait).is_ok());
        assert!(limiter.check_at(client, now + wait).is_err());
    }

    #[test]
    fn a_refused_request_does_not_cost_anything() {
        let limiter = RateLimiter::per_minute(1);
        let now = Instant::now();
        let client = ip("203.0.113.7");
        assert!(limiter.check_at(client, now).is_ok());
        for _ in 0..5 {
            assert!(limiter.check_at(client, now).is_err());
        }
        assert!(limiter.check_at(client, now + WINDOW).is_ok());
    }

    #[test]
    fn forwarded_for_is_read_only_from_trusted_proxies() {
        let limits = AuthLimits::new(1, 1, vec!["10.0.0.0/8".parse().unwrap()]);
        let header = Some("198.51.100.9, 203.0.113.7, 10.0.0.2");
        // From the proxy: the last hop that is not a proxy (earlier ones can be forged).
        assert_eq!(limits.client(ip("10.0.0.1"), header), ip("203.0.113.7"));
        // From anyone else: the header is ignored.
        assert_eq!(limits.client(ip("192.0.2.1"), header), ip("192.0.2.1"));
        // From the proxy without the header, or with a malformed one.
        assert_eq!(limits.client(ip("10.0.0.1"), None), ip("10.0.0.1"));
        assert_eq!(
            limits.client(ip("10.0.0.1"), Some("not-an-ip")),
            ip("10.0.0.1")
        );
        // No proxy trusted: never read.
        let none = AuthLimits::new(1, 1, Vec::new());
        assert_eq!(none.client(ip("10.0.0.1"), header), ip("10.0.0.1"));
    }
}
