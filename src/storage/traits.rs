use async_trait::async_trait;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Connection failed: {0}")]
    Connection(String),

    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("Transfer failed: {0}")]
    Transfer(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Configuration error: {0}")]
    Config(String),
}

impl StorageError {
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            StorageError::Connection(_) | StorageError::Transfer(_) | StorageError::Io(_)
        )
    }
}

#[async_trait]
pub trait StorageBackend: Send + Sync {
    async fn upload(&self, source_path: &Path, dest_path: &str) -> Result<(), StorageError>;
    async fn delete(&self, path: &str) -> Result<(), StorageError>;
    async fn rename(&self, old_path: &str, new_path: &str) -> Result<(), StorageError>;
    async fn mkdir(&self, path: &str) -> Result<(), StorageError>;
    fn name(&self) -> &str;
}
