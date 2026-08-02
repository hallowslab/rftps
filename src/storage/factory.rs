use std::sync::Arc;

use crate::background::config::BackendConfig;

use super::ftps::FtpsBackend;
use super::traits::{StorageBackend, StorageError};

pub struct StorageBackendFactory;

impl StorageBackendFactory {
    pub fn build(config: &BackendConfig) -> Result<Arc<dyn StorageBackend>, StorageError> {
        match config {
            BackendConfig::Ftps(cfg) => Ok(Arc::new(FtpsBackend::new(cfg.clone()))),
            BackendConfig::S3(_) => Err(StorageError::Config(
                "S3 backend not implemented yet (planned M2)".into(),
            )),
        }
    }
}
