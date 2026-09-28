use std::net::SocketAddr;

/// Settings read from the environment (see `.env.example`).
#[derive(Debug, Clone)]
pub struct Config {
    pub listen: SocketAddr,
    pub engine_workers: Option<usize>,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let listen = std::env::var("DONKA_LISTEN")
            .unwrap_or_else(|_| "0.0.0.0:8080".into())
            .parse()
            .map_err(|err| anyhow::anyhow!("DONKA_LISTEN must be host:port ({err})"))?;
        let engine_workers = match std::env::var("DONKA_ENGINE_WORKERS") {
            Ok(v) => Some(
                v.parse()
                    .map_err(|_| anyhow::anyhow!("DONKA_ENGINE_WORKERS must be a number"))?,
            ),
            Err(_) => None,
        };
        Ok(Self {
            listen,
            engine_workers,
        })
    }
}
