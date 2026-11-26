//! cosign signatures made with a key pair (`cosign sign --key`, `cosign sign-blob --key`),
//! checked with its public key and nothing else: no transparency log, no network.
//!
//! A cosign signature is ECDSA P-256 over SHA-256, DER-encoded, then base64. An image's
//! signature covers a small JSON payload that names the manifest digest it vouches for.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use p256::ecdsa::signature::Verifier as _;
use p256::ecdsa::{Signature, VerifyingKey};
use p256::pkcs8::DecodePublicKey as _;
use serde::Deserialize;

pub struct PublicKey(VerifyingKey);

impl PublicKey {
    /// A key as `cosign generate-key-pair` writes it (`cosign.pub`, PEM).
    pub fn from_pem(text: &str) -> Result<Self, String> {
        VerifyingKey::from_public_key_pem(text.trim())
            .map(Self)
            .map_err(|err| format!("not a cosign public key (ECDSA P-256, PEM): {err}"))
    }

    /// Checks a `cosign sign-blob` signature (base64, as `--output-signature` writes it).
    pub fn verify_blob(&self, data: &[u8], signature: &str) -> Result<(), String> {
        let der = B64
            .decode(signature.trim())
            .map_err(|_| "the signature is not base64".to_owned())?;
        let signature =
            Signature::from_der(&der).map_err(|_| "the signature is malformed".to_owned())?;
        self.0
            .verify(data, &signature)
            .map_err(|_| "the signature does not match this key".to_owned())
    }

    /// Checks an image signature: `payload` signed by this key, naming the manifest `digest`.
    pub fn verify_image(
        &self,
        digest: &str,
        payload: &[u8],
        signature: &str,
    ) -> Result<(), String> {
        self.verify_blob(payload, signature)?;
        let payload: Payload = serde_json::from_slice(payload)
            .map_err(|err| format!("the signed payload is not cosign's: {err}"))?;
        if payload.critical.kind != "cosign container image signature" {
            return Err(format!("the signed payload is a {}", payload.critical.kind));
        }
        if payload.critical.image.digest != digest {
            return Err(format!(
                "the signature is for {}, not {digest}",
                payload.critical.image.digest
            ));
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct Payload {
    critical: Critical,
}

#[derive(Deserialize)]
struct Critical {
    image: Image,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Deserialize)]
struct Image {
    #[serde(rename = "docker-manifest-digest")]
    digest: String,
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use p256::ecdsa::signature::Signer as _;
    use p256::ecdsa::SigningKey;
    use p256::pkcs8::{EncodePublicKey as _, LineEnding};

    pub(crate) fn key_pair(seed: u8) -> (SigningKey, PublicKey) {
        let signing = SigningKey::from_bytes(&[seed; 32].into()).unwrap();
        let pem = signing
            .verifying_key()
            .to_public_key_pem(LineEnding::LF)
            .unwrap();
        (signing, PublicKey::from_pem(&pem).unwrap())
    }

    pub(crate) fn sign(key: &SigningKey, data: &[u8]) -> String {
        let signature: Signature = key.sign(data);
        B64.encode(signature.to_der().as_bytes())
    }

    pub(crate) fn payload(digest: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "critical": {
                "identity": {"docker-reference": "ghcr.io/youmssi/donka-studio"},
                "image": {"docker-manifest-digest": digest},
                "type": "cosign container image signature"
            },
            "optional": null
        }))
        .unwrap()
    }

    #[test]
    fn a_blob_signature_verifies_with_its_key_only() {
        let (signing, public) = key_pair(7);
        let (_, other) = key_pair(8);
        let signature = sign(&signing, b"SHA256SUMS");
        public.verify_blob(b"SHA256SUMS", &signature).unwrap();
        assert!(public
            .verify_blob(b"SHA256SUMS changed", &signature)
            .is_err());
        assert!(other.verify_blob(b"SHA256SUMS", &signature).is_err());
        assert!(public.verify_blob(b"SHA256SUMS", "not base64!").is_err());
    }

    #[test]
    fn an_image_signature_must_name_the_digest_that_runs() {
        let (signing, public) = key_pair(7);
        let payload = payload("sha256:aa");
        let signature = sign(&signing, &payload);
        public
            .verify_image("sha256:aa", &payload, &signature)
            .unwrap();
        let err = public
            .verify_image("sha256:bb", &payload, &signature)
            .unwrap_err();
        assert!(err.contains("not sha256:bb"), "{err}");
    }

    /// Made with `openssl dgst -sha256 -sign key.pem | base64`, the form cosign writes and
    /// the offline bundle's README checks with openssl.
    #[test]
    fn an_openssl_signature_verifies_like_cosigns() {
        let pem = "-----BEGIN PUBLIC KEY-----\n\
            MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEXU/o2HrM2RyOAAlLC6yeQLHdRTon\n\
            Lcd+KLD6Zye+gVRLD9e3GdsdSUD9JoDTpCJ1q5X7vWCAmiYZpsHNqa7A6w==\n\
            -----END PUBLIC KEY-----\n";
        let signature = "MEUCIAdnjhtz/qzpbEhK1QfQj8gDzkzRa8Ny0BONHSQ5ouPLAiEAgiW8XXkewcQdSomPRe2nK24MSgmy7HcncMRUYcA8WBg=";
        let key = PublicKey::from_pem(pem).unwrap();
        key.verify_blob(b"hello\n", signature).unwrap();
        assert!(key.verify_blob(b"hello", signature).is_err());
        assert!(PublicKey::from_pem("ssh-ed25519 AAAA").is_err());
    }
}
