//! Where release artifacts go: one bucket (S3, MinIO) or folder, written by
//! Studio and read by Donka Runtime.
//!
//! Modules depend on [`ArtifactStore`]; the binary picks [`ObjectStorage`]
//! from a URL (`s3://donka-releases`, `file:///var/lib/donka/releases`), and
//! tests use `MemoryStore` (feature `testing`).

use async_trait::async_trait;
use object_store::path::Path;
use object_store::{ObjectStore, ObjectStoreExt, PutPayload};
use std::sync::Arc;
use url::Url;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// The storage settings cannot be used (startup only).
    #[error("invalid storage settings: {0}")]
    Config(String),
    /// The write failed; it is worth trying again.
    #[error("storage write failed: {0}")]
    Write(String),
}

#[async_trait]
pub trait ArtifactStore: Send + Sync {
    /// Writes `body` at `key` (e.g. `staging/credit-pme`), replacing what was there.
    async fn put(&self, key: &str, body: Vec<u8>) -> Result<(), StoreError>;
}

/// An [`ArtifactStore`] on any `object_store` backend.
pub struct ObjectStorage {
    store: Arc<dyn ObjectStore>,
    /// The path the URL points at inside the bucket or folder.
    root: Path,
}

impl ObjectStorage {
    /// Opens the store a URL names. `options` are the backend's settings, e.g.
    /// `aws_endpoint`, `aws_region`, `aws_access_key_id` for S3 and MinIO.
    pub fn from_url(
        url: &str,
        options: impl IntoIterator<Item = (String, String)>,
    ) -> Result<Self, StoreError> {
        let url = Url::parse(url).map_err(|err| StoreError::Config(err.to_string()))?;
        let (store, root) = object_store::parse_url_opts(&url, options)
            .map_err(|err| StoreError::Config(err.to_string()))?;
        Ok(Self {
            store: Arc::from(store),
            root,
        })
    }
}

#[async_trait]
impl ArtifactStore for ObjectStorage {
    async fn put(&self, key: &str, body: Vec<u8>) -> Result<(), StoreError> {
        let path = key
            .split('/')
            .fold(self.root.clone(), |path, part| path.clone().join(part));
        self.store
            .put(&path, PutPayload::from(body))
            .await
            .map(drop)
            .map_err(|err| StoreError::Write(err.to_string()))
    }
}

#[cfg(any(test, feature = "testing"))]
mod memory {
    use super::{ArtifactStore, StoreError};
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// Keeps artifacts in memory; can be told to fail the next writes.
    #[derive(Default)]
    pub struct MemoryStore {
        objects: Mutex<HashMap<String, Vec<u8>>>,
        failures: Mutex<usize>,
    }

    impl MemoryStore {
        pub fn get(&self, key: &str) -> Option<Vec<u8>> {
            self.objects.lock().ok()?.get(key).cloned()
        }

        /// The next `count` writes fail, as an unreachable bucket would.
        pub fn fail_next(&self, count: usize) {
            if let Ok(mut failures) = self.failures.lock() {
                *failures = count;
            }
        }
    }

    #[async_trait]
    impl ArtifactStore for MemoryStore {
        async fn put(&self, key: &str, body: Vec<u8>) -> Result<(), StoreError> {
            let mut failures = self
                .failures
                .lock()
                .map_err(|err| StoreError::Write(err.to_string()))?;
            if *failures > 0 {
                *failures -= 1;
                return Err(StoreError::Write("storage is unreachable".to_owned()));
            }
            self.objects
                .lock()
                .map_err(|err| StoreError::Write(err.to_string()))?
                .insert(key.to_owned(), body);
            Ok(())
        }
    }
}

#[cfg(any(test, feature = "testing"))]
pub use memory::MemoryStore;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn writes_under_the_url_path_of_a_folder() {
        let dir = std::env::temp_dir().join(format!("donka-storage-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = ObjectStorage::from_url(
            &format!("file://{}", dir.display()),
            Vec::<(String, String)>::new(),
        )
        .unwrap();
        store
            .put("staging/credit-pme", b"zip".to_vec())
            .await
            .unwrap();
        assert_eq!(
            std::fs::read(dir.join("staging/credit-pme")).unwrap(),
            b"zip"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_bad_url_is_a_configuration_error() {
        let err = ObjectStorage::from_url("not a url", Vec::<(String, String)>::new());
        assert!(matches!(err, Err(StoreError::Config(_))));
    }

    #[tokio::test]
    async fn the_memory_store_can_fail_on_purpose() {
        let store = MemoryStore::default();
        store.fail_next(1);
        assert!(store.put("a", vec![1]).await.is_err());
        store.put("a", vec![2]).await.unwrap();
        assert_eq!(store.get("a"), Some(vec![2]));
    }
}
