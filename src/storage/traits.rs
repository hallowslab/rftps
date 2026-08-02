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
    fn name(&self) -> &str;
    fn capabilities(&self) -> Option<&dyn BackendCapabilities> {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    Rename,
    Mkdir,
}

#[async_trait]
pub trait BackendCapabilities: StorageBackend {
    fn supports(&self, _capability: Capability) -> bool {
        false
    }

    async fn rename(&self, _old_path: &str, _new_path: &str) -> Result<(), StorageError> {
        Err(StorageError::Config(format!(
            "rename not supported by backend '{}'",
            self.name()
        )))
    }

    async fn mkdir(&self, _path: &str) -> Result<(), StorageError> {
        Err(StorageError::Config(format!(
            "mkdir not supported by backend '{}'",
            self.name()
        )))
    }
}

pub async fn rename_on(
    backend: &dyn StorageBackend,
    old_path: &str,
    new_path: &str,
) -> Result<(), StorageError> {
    let caps = backend.capabilities().ok_or_else(|| {
        StorageError::Config(format!("rename not supported by backend '{}'", backend.name()))
    })?;
    if !caps.supports(Capability::Rename) {
        return Err(StorageError::Config(format!(
            "rename not supported by backend '{}'",
            backend.name()
        )));
    }
    caps.rename(old_path, new_path).await
}

pub async fn mkdir_on(backend: &dyn StorageBackend, path: &str) -> Result<(), StorageError> {
    let caps = backend.capabilities().ok_or_else(|| {
        StorageError::Config(format!("mkdir not supported by backend '{}'", backend.name()))
    })?;
    if !caps.supports(Capability::Mkdir) {
        return Err(StorageError::Config(format!(
            "mkdir not supported by backend '{}'",
            backend.name()
        )));
    }
    caps.mkdir(path).await
}
