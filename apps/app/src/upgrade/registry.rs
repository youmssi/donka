//! Just enough of the OCI distribution API to check an image's cosign signatures before Docker
//! pulls it: resolve a tag to its manifest digest, then read the signatures cosign stores under
//! the `sha256-<digest>.sig` tag of the same repository.

use super::signature::PublicKey;
use sha2::{Digest as _, Sha256};

/// Media types a tag may resolve to: a multi-platform index or a single manifest.
const MANIFESTS: &str = "application/vnd.oci.image.index.v1+json, \
    application/vnd.docker.distribution.manifest.list.v2+json, \
    application/vnd.oci.image.manifest.v1+json, \
    application/vnd.docker.distribution.manifest.v2+json";
const SIGNATURE_ANNOTATION: &str = "dev.cosignproject.cosign/signature";

/// `registry/repository:tag`, e.g. `ghcr.io/youmssi/donka-studio:1.5.0`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRef {
    pub registry: String,
    pub repository: String,
    pub tag: String,
}

impl ImageRef {
    pub fn parse(image: &str) -> Result<Self, String> {
        let invalid = || format!("`{image}` is not registry/repository:tag");
        let (name, tag) = image.rsplit_once(':').ok_or_else(invalid)?;
        let (registry, repository) = name.split_once('/').ok_or_else(invalid)?;
        if tag.contains('/') || !(registry.contains('.') || registry.contains(':')) {
            return Err(invalid());
        }
        Ok(Self {
            registry: registry.to_owned(),
            repository: repository.to_owned(),
            tag: tag.to_owned(),
        })
    }

    /// `registry/repository`, without the tag.
    pub fn name(&self) -> String {
        format!("{}/{}", self.registry, self.repository)
    }

    fn base(&self, insecure: &[String]) -> String {
        let scheme = if insecure.contains(&self.registry) {
            "http"
        } else {
            "https"
        };
        format!("{scheme}://{}/v2/{}", self.registry, self.repository)
    }
}

impl std::fmt::Display for ImageRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.name(), self.tag)
    }
}

pub struct Registry {
    http: reqwest::Client,
    credentials: Option<(String, String)>,
    /// Registries reached over plain HTTP, as Docker is configured to.
    insecure: Vec<String>,
}

impl Registry {
    /// `credentials` (user, token) are sent only to the registry's token service; without them
    /// only public repositories can be read.
    pub fn new(
        credentials: Option<(String, String)>,
        insecure: Vec<String>,
    ) -> Result<Self, String> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|err| err.to_string())?;
        Ok(Self {
            http,
            credentials,
            insecure,
        })
    }

    /// The digest `image`'s tag points to now, computed from the manifest's own bytes.
    pub async fn digest(&self, image: &ImageRef) -> Result<String, String> {
        let manifest = self
            .get(image, &format!("manifests/{}", image.tag), MANIFESTS)
            .await?;
        Ok(sha256(&manifest))
    }

    /// Checks that `key` signed `digest` in `image`'s repository. Several signatures may be
    /// attached; one valid signature is enough.
    pub async fn verify(
        &self,
        image: &ImageRef,
        digest: &str,
        key: &PublicKey,
    ) -> Result<(), String> {
        let tag = format!("{}.sig", digest.replace(':', "-"));
        let manifest = self
            .get(image, &format!("manifests/{tag}"), MANIFESTS)
            .await
            .map_err(|err| format!("{} has no cosign signature ({err})", image.name()))?;
        let manifest: Manifest = serde_json::from_slice(&manifest)
            .map_err(|err| format!("the signature manifest of {image} is not valid: {err}"))?;
        let mut reasons = Vec::new();
        for layer in manifest.layers {
            let Some(signature) = layer.annotations.get(SIGNATURE_ANNOTATION) else {
                continue;
            };
            let payload = self
                .get(image, &format!("blobs/{}", layer.digest), "*/*")
                .await?;
            if sha256(&payload) != layer.digest {
                reasons.push("a signature payload does not match its digest".to_owned());
                continue;
            }
            match key.verify_image(digest, &payload, signature) {
                Ok(()) => return Ok(()),
                Err(reason) => reasons.push(reason),
            }
        }
        Err(format!(
            "no signature of {image} ({digest}) verifies with the installation's key: {}",
            if reasons.is_empty() {
                "none found".to_owned()
            } else {
                reasons.join("; ")
            }
        ))
    }

    /// GETs `path` under the repository, answering a bearer-token challenge once.
    async fn get(&self, image: &ImageRef, path: &str, accept: &str) -> Result<Vec<u8>, String> {
        let url = format!("{}/{path}", image.base(&self.insecure));
        let send = |token: Option<&str>| {
            let mut request = self.http.get(&url).header("accept", accept);
            if let Some(token) = token {
                request = request.bearer_auth(token);
            }
            request.send()
        };
        let mut response = send(None).await.map_err(|err| format!("{url}: {err}"))?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            let challenge = response
                .headers()
                .get("www-authenticate")
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default()
                .to_owned();
            let token = self.token(&challenge, image).await?;
            response = send(Some(&token))
                .await
                .map_err(|err| format!("{url}: {err}"))?;
        }
        let status = response.status();
        if !status.is_success() {
            return Err(format!("{url} answered {status}"));
        }
        response
            .bytes()
            .await
            .map(|bytes| bytes.to_vec())
            .map_err(|err| format!("{url}: {err}"))
    }

    /// A pull token from the service a `Bearer realm=…,service=…,scope=…` challenge names.
    async fn token(&self, challenge: &str, image: &ImageRef) -> Result<String, String> {
        let params = challenge_params(challenge).ok_or_else(|| {
            format!(
                "{} asks for an authentication it does not describe",
                image.registry
            )
        })?;
        let realm = params
            .iter()
            .find(|(key, _)| key == "realm")
            .map(|(_, value)| value.clone())
            .ok_or_else(|| format!("{} gives no token service", image.registry))?;
        let scope = format!("repository:{}:pull", image.repository);
        let mut query: Vec<(String, String)> = params
            .into_iter()
            .filter(|(key, _)| key == "service")
            .collect();
        query.push(("scope".to_owned(), scope));
        let mut request = self.http.get(&realm).query(&query);
        if let Some((user, token)) = &self.credentials {
            request = request.basic_auth(user, Some(token));
        }
        let response = request
            .send()
            .await
            .map_err(|err| format!("{realm}: {err}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "{} refused access to {} ({}): set DONKA_REGISTRY_USER and DONKA_REGISTRY_TOKEN",
                image.registry,
                image.repository,
                response.status()
            ));
        }
        let body: TokenResponse = response.json().await.map_err(|err| err.to_string())?;
        body.token
            .or(body.access_token)
            .ok_or_else(|| format!("{} returned no token", image.registry))
    }
}

pub fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

/// The `key="value"` pairs of a `Bearer …` challenge.
fn challenge_params(challenge: &str) -> Option<Vec<(String, String)>> {
    let rest = challenge.trim().strip_prefix("Bearer ")?;
    let mut params = Vec::new();
    let mut rest = rest.trim();
    while !rest.is_empty() {
        let (key, after) = rest.split_once('=')?;
        let after = after.strip_prefix('"')?;
        let (value, after) = after.split_once('"')?;
        params.push((key.trim().to_owned(), value.to_owned()));
        rest = after.trim_start_matches(',').trim();
    }
    Some(params)
}

#[derive(serde::Deserialize)]
struct Manifest {
    #[serde(default)]
    layers: Vec<Layer>,
}

#[derive(serde::Deserialize)]
struct Layer {
    digest: String,
    #[serde(default)]
    annotations: std::collections::HashMap<String, String>,
}

#[derive(serde::Deserialize)]
struct TokenResponse {
    token: Option<String>,
    access_token: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_references_name_their_registry() {
        let image = ImageRef::parse("ghcr.io/youmssi/donka-studio:1.5.0").unwrap();
        assert_eq!(image.registry, "ghcr.io");
        assert_eq!(image.repository, "youmssi/donka-studio");
        let insecure = vec!["registry.test:5000".to_owned()];
        assert_eq!(
            image.base(&insecure),
            "https://ghcr.io/v2/youmssi/donka-studio"
        );
        let local = ImageRef::parse("registry.test:5000/youmssi/donka-runtime:ci").unwrap();
        assert_eq!(
            local.base(&insecure),
            "http://registry.test:5000/v2/youmssi/donka-runtime"
        );
        assert_eq!(
            local.base(&[]),
            "https://registry.test:5000/v2/youmssi/donka-runtime"
        );
        assert_eq!(
            local.to_string(),
            "registry.test:5000/youmssi/donka-runtime:ci"
        );
        for invalid in [
            "postgres:16-alpine",
            "ghcr.io/youmssi/donka-studio",
            "ghcr.io:x/y",
        ] {
            assert!(ImageRef::parse(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn a_bearer_challenge_is_read() {
        let params = challenge_params(
            r#"Bearer realm="https://ghcr.io/token",service="ghcr.io",scope="repository:youmssi/donka-studio:pull""#,
        )
        .unwrap();
        assert_eq!(params[0], ("realm".into(), "https://ghcr.io/token".into()));
        assert_eq!(params[1], ("service".into(), "ghcr.io".into()));
        assert_eq!(params.len(), 3);
        assert!(challenge_params("Basic realm=\"x\"").is_none());
    }
}
