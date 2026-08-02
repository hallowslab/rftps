use std::sync::Arc;

use crate::background::config::BackendConfig;

use super::ftps::FtpsBackend;
#[cfg(feature = "s3")]
use super::s3::S3Backend;
use super::traits::{StorageBackend, StorageError};

pub struct StorageBackendFactory;

impl StorageBackendFactory {
    pub fn build(config: &BackendConfig) -> Result<Arc<dyn StorageBackend>, StorageError> {
        match config {
            BackendConfig::Ftps(cfg) => Ok(Arc::new(FtpsBackend::new(cfg.clone()))),
            #[cfg(feature = "s3")]
            BackendConfig::S3(cfg) => S3Backend::new(cfg.clone())
                .map(|backend| Arc::new(backend) as Arc<dyn StorageBackend>),
            #[cfg(not(feature = "s3"))]
            BackendConfig::S3(_) => Err(StorageError::Config(
                "S3 backend requires the 's3' cargo feature".into(),
            )),
        }
    }
}
