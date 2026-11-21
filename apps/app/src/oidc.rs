//! Organisation OpenID Connect client and short-lived browser flow state.

use crate::config::OidcSettings;
use openidconnect::core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata};
use openidconnect::{
    reqwest, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointMaybeSet,
    EndpointNotSet, EndpointSet, IssuerUrl, Nonce, PkceCodeChallenge, PkceCodeVerifier,
    RedirectUrl, Scope, TokenResponse,
};
use serde::Deserialize;
use std::sync::Arc;

type DiscoveredClient = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

#[derive(Clone)]
pub struct OidcProvider {
    client: DiscoveredClient,
    http: reqwest::Client,
    issuer: Arc<String>,
    scopes: Arc<Vec<String>>,
    group_claim: Arc<String>,
    administrator_group: Option<Arc<String>>,
}

#[derive(Debug, thiserror::Error)]
pub enum OidcError {
    #[error("OIDC provider initialization failed")]
    Discovery,
    #[error("OIDC authentication failed")]
    Authentication,
}

#[derive(Debug)]
pub struct OidcPerson {
    pub issuer: String,
    pub subject: String,
    pub email: String,
    pub groups: Vec<String>,
}

#[derive(Deserialize)]
struct IdentityClaims {
    sub: String,
    email: Option<String>,
    email_verified: Option<bool>,
}

impl OidcProvider {
    /// Discovers and validates the configured issuer during application startup.
    pub async fn discover(settings: &OidcSettings, callback_url: &str) -> Result<Self, OidcError> {
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|_| OidcError::Discovery)?;
        let issuer =
            IssuerUrl::new(settings.issuer_url.clone()).map_err(|_| OidcError::Discovery)?;
        let metadata = CoreProviderMetadata::discover_async(issuer, &http)
            .await
            .map_err(|_| OidcError::Discovery)?;
        let redirect =
            RedirectUrl::new(callback_url.to_owned()).map_err(|_| OidcError::Discovery)?;
        let client = CoreClient::from_provider_metadata(
            metadata,
            ClientId::new(settings.client_id.clone()),
            Some(ClientSecret::new(settings.client_secret.clone())),
        )
        .set_redirect_uri(redirect);
        Ok(Self {
            client,
            http,
            issuer: Arc::new(settings.issuer_url.clone()),
            scopes: Arc::new(settings.scopes.clone()),
            group_claim: Arc::new(settings.group_claim.clone()),
            administrator_group: settings.administrator_group.clone().map(Arc::new),
        })
    }

    /// Creates the provider redirect and its browser-bound CSRF, nonce and PKCE values.
    pub fn authorization(&self) -> (String, String, String, String) {
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let (url, state, nonce) = self
            .client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scopes(
                self.scopes
                    .iter()
                    .cloned()
                    .map(Scope::new)
                    .collect::<Vec<_>>(),
            )
            .set_pkce_challenge(challenge)
            .url();
        (
            url.to_string(),
            state.secret().to_owned(),
            nonce.secret().to_owned(),
            verifier.secret().to_owned(),
        )
    }

    /// Exchanges an authorization code and verifies signature, issuer, audience and nonce.
    pub async fn authenticate(
        &self,
        code: String,
        nonce: String,
        verifier: String,
    ) -> Result<OidcPerson, OidcError> {
        let token = self
            .client
            .exchange_code(AuthorizationCode::new(code))
            .map_err(|_| OidcError::Authentication)?
            .set_pkce_verifier(PkceCodeVerifier::new(verifier))
            .request_async(&self.http)
            .await
            .map_err(|_| OidcError::Authentication)?;
        let id_token = token.id_token().ok_or(OidcError::Authentication)?;
        let claims = id_token
            .claims(&self.client.id_token_verifier(), &Nonce::new(nonce))
            .map_err(|_| OidcError::Authentication)?;
        let raw: serde_json::Value = serde_json::from_slice(&payload(id_token.to_string())?)
            .map_err(|_| OidcError::Authentication)?;
        let raw_claims: IdentityClaims =
            serde_json::from_value(raw.clone()).map_err(|_| OidcError::Authentication)?;
        let email = raw_claims
            .email
            .filter(|_| raw_claims.email_verified == Some(true))
            .ok_or(OidcError::Authentication)?;
        let subject = claims.subject().as_str().to_owned();
        if subject != raw_claims.sub {
            return Err(OidcError::Authentication);
        }
        let groups = self
            .administrator_group
            .as_ref()
            .and_then(|_| raw.get(self.group_claim.as_str()).cloned())
            .map(group_values)
            .unwrap_or_default();
        Ok(OidcPerson {
            issuer: self.issuer.as_ref().clone(),
            subject,
            email,
            groups,
        })
    }

    pub fn administrator_role(&self, groups: &[String]) -> Option<bool> {
        self.administrator_group
            .as_ref()
            .map(|configured| groups.iter().any(|group| group == configured.as_str()))
    }
}

fn payload(jwt: String) -> Result<Vec<u8>, OidcError> {
    use base64::Engine;
    let encoded = jwt.split('.').nth(1).ok_or(OidcError::Authentication)?;
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| OidcError::Authentication)
}

fn group_values(value: serde_json::Value) -> Vec<String> {
    match value {
        serde_json::Value::Array(values) => values
            .into_iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect(),
        serde_json::Value::String(value) => vec![value],
        _ => Vec::new(),
    }
}
