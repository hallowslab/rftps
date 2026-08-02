use std::sync::Arc;

pub mod config;
pub mod replication;
#[cfg(feature = "relay")]
pub mod relay;

pub use config::{
    BackendConfig, BackgroundJobConfig, FtpsConfig, RemoteStorageConfig, S3Config,
    StorageBackendType, UserMapping, VersionedBackendConfig, BACKEND_CONFIG_VERSION,
};
pub use replication::{ReplicationExecutor, ReplicationHandler};
#[cfg(feature = "relay")]
pub use relay::{RelayClient, RelayError, RelayStorageBackend};

#[cfg(feature = "relay")]
pub use config::RelayConfig;

pub fn build_static_backend(
    config: &BackgroundJobConfig,
) -> Result<Arc<dyn crate::storage::StorageBackend>, String> {
    if let Some(versioned) = &config.storage {
        versioned
            .check_version()
            .map_err(|e| format!("[Background] Replication disabled: {}", e))?;
        let backend = crate::storage::StorageBackendFactory::build(&versioned.backend)
            .map_err(|e| format!("[Background] Replication disabled: {}", e))?;
        return Ok(backend);
    }
    if let Some(remote) = &config.remote_storage {
        let cfg = BackendConfig::try_from(remote.clone())
            .map_err(|e| format!("[Background] Replication disabled: {}", e))?;
        let backend = crate::storage::StorageBackendFactory::build(&cfg)
            .map_err(|e| format!("[Background] Replication disabled: {}", e))?;
        return Ok(backend);
    }
    Err("[Background] Replication disabled: no remote storage configured".into())
}
