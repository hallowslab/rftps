#![cfg(feature = "background-jobs")]

use rftps::background::{
    BackendConfig, BackgroundJobConfig, FtpsConfig, RemoteStorageConfig, StorageBackendType,
    UserMapping, VersionedBackendConfig, BACKEND_CONFIG_VERSION,
};
use rftps::storage::{BackendCapabilities, Capability, StorageBackendFactory, mkdir_on, rename_on};
use std::collections::BTreeMap;
use std::io::Write;

#[test]
fn test_default_config() {
    let config = BackgroundJobConfig::default();
    assert!(!config.enabled);
    assert_eq!(config.max_parallel_jobs, 2);
    assert_eq!(config.max_retries, 3);
    assert_eq!(config.queue_capacity, 1000);
    assert!(config.remote_storage.is_none());
}

#[test]
fn test_user_mapping_prefix() {
    let mapping = UserMapping::PrefixUserName;
    assert_eq!(mapping.resolve("alice", "photos/pic.jpg"), "alice/photos/pic.jpg");
}

#[test]
fn test_user_mapping_fixed_path() {
    let mapping = UserMapping::FixedPath("backups".into());
    assert_eq!(mapping.resolve("alice", "photos/pic.jpg"), "backups/photos/pic.jpg");
}

#[test]
fn test_user_mapping_user_map() {
    let mut map = BTreeMap::new();
    map.insert("alice".into(), "team-alice".into());
    map.insert("bob".into(), "team-bob".into());
    let mapping = UserMapping::UserMap(map);

    assert_eq!(mapping.resolve("alice", "data/file.txt"), "team-alice/data/file.txt");
    assert_eq!(mapping.resolve("bob", "data/file.txt"), "team-bob/data/file.txt");
    assert_eq!(mapping.resolve("unknown", "data/file.txt"), "unknown/data/file.txt");
}

#[test]
fn test_remote_storage_config_backward_compat() {
    let json = r#"{
        "backend": "Ftps",
        "host": "example.com",
        "port": 990,
        "username": "user",
        "password": "pass",
        "path_prefix": "backups",
        "use_ssl": true,
        "ca_cert": null
    }"#;

    let config: RemoteStorageConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.host, "example.com");
    assert!(!config.danger_disable_cert_verify);
    assert!(config.ca_cert.is_none());
}

#[test]
fn test_remote_storage_config_with_new_fields() {
    let json = r#"{
        "backend": "Ftps",
        "host": "myserver.local",
        "username": "admin",
        "password": "secret",
        "path_prefix": "",
        "use_ssl": true,
        "ca_cert": "/path/to/server.pem",
        "danger_disable_cert_verify": true
    }"#;

    let config: RemoteStorageConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.ca_cert.as_deref(), Some("/path/to/server.pem"));
    assert!(config.danger_disable_cert_verify);
}

#[test]
fn test_remote_storage_config_default_values() {
    let config = RemoteStorageConfig::default();
    assert_eq!(config.host, "");
    assert!(!config.use_ssl);
    assert!(!config.danger_disable_cert_verify);
    assert!(config.ca_cert.is_none());
}

#[test]
fn test_load_from_file() {
    let json = r#"{
        "enabled": true,
        "max_parallel_jobs": 4,
        "max_retries": 5,
        "queue_capacity": 500,
        "remote_storage": {
            "backend": "Ftps",
            "host": "backup.example.com",
            "port": 990,
            "username": "backup",
            "password": "secret",
            "path_prefix": "uploads",
            "use_ssl": true,
            "ca_cert": "/path/to/server.pem",
            "danger_disable_cert_verify": false
        },
        "user_mapping": "PrefixUserName"
    }"#;

    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(json.as_bytes()).unwrap();
    let path = tmp.path().to_str().unwrap();

    let config = BackgroundJobConfig::load_from_file(path).unwrap();
    assert!(config.enabled);
    assert_eq!(config.max_parallel_jobs, 4);
    assert_eq!(config.max_retries, 5);
    assert_eq!(config.queue_capacity, 500);

    let rs = config.remote_storage.unwrap();
    assert_eq!(rs.host, "backup.example.com");
    assert_eq!(rs.port, Some(990));
    assert!(rs.use_ssl);
    assert_eq!(rs.ca_cert.as_deref(), Some("/path/to/server.pem"));
}

#[test]
fn test_load_missing_file() {
    let result = BackgroundJobConfig::load_from_file("/nonexistent/config.json");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Failed to read"));
}

#[test]
fn test_load_invalid_json() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(b"not json").unwrap();
    let path = tmp.path().to_str().unwrap();

    let err = BackgroundJobConfig::load_from_file(path).unwrap_err();
    assert!(err.contains("Failed to parse"), "unexpected error: {}", err);
}

#[test]
fn test_load_partial_json_uses_defaults() {
    let json = r#"{"enabled": true}"#;
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(json.as_bytes()).unwrap();
    let path = tmp.path().to_str().unwrap();

    let config = BackgroundJobConfig::load_from_file(path).unwrap();
    assert!(config.enabled);
    assert_eq!(config.max_parallel_jobs, 2);
    assert_eq!(config.max_retries, 3);
    assert!(config.remote_storage.is_none());
}

fn legacy_ftps() -> RemoteStorageConfig {
    RemoteStorageConfig {
        backend: StorageBackendType::Ftps,
        host: "example.com".into(),
        port: Some(990),
        username: "user".into(),
        password: "pass".into(),
        path_prefix: "backups".into(),
        use_ssl: true,
        ca_cert: None,
        ca_cert_pem: None,
        danger_disable_cert_verify: false,
    }
}

#[test]
fn test_legacy_ftps_maps_to_backend_config() {
    let cfg = BackendConfig::try_from(legacy_ftps()).unwrap();
    match cfg {
        BackendConfig::Ftps(f) => {
            assert_eq!(f.host, "example.com");
            assert_eq!(f.port, Some(990));
            assert!(f.use_ssl);
            assert_eq!(f.path_prefix, "backups");
        }
        other => panic!("expected Ftps, got {:?}", other),
    }
}

#[test]
fn test_legacy_non_ftps_rejected() {
    for backend in [StorageBackendType::S3, StorageBackendType::Sftp, StorageBackendType::Https] {
        let legacy = RemoteStorageConfig {
            backend,
            ..legacy_ftps()
        };
        assert!(BackendConfig::try_from(legacy).is_err());
    }
}

#[test]
fn test_versioned_config_round_trip() {
    let cfg = BackendConfig::Ftps(FtpsConfig::default());
    let versioned = VersionedBackendConfig::new(cfg.clone());
    assert_eq!(versioned.version, BACKEND_CONFIG_VERSION);
    assert!(versioned.check_version().is_ok());

    let json = serde_json::to_string(&versioned).unwrap();
    let parsed: VersionedBackendConfig = serde_json::from_str(&json).unwrap();
    assert!(parsed.check_version().is_ok());
    match parsed.backend {
        BackendConfig::Ftps(_) => {}
        other => panic!("expected Ftps, got {:?}", other),
    }
}

#[test]
fn test_versioned_config_future_version_rejected() {
    let cfg = VersionedBackendConfig {
        version: BACKEND_CONFIG_VERSION + 1,
        backend: BackendConfig::Ftps(FtpsConfig::default()),
    };
    assert!(cfg.check_version().is_err());
}

#[test]
fn test_versioned_config_zero_version_rejected() {
    let cfg = VersionedBackendConfig {
        version: 0,
        backend: BackendConfig::Ftps(FtpsConfig::default()),
    };
    assert!(cfg.check_version().is_err());
}

#[test]
fn test_backend_config_type_tag_parse() {
    let json = r#"{
        "type": "ftps",
        "host": "h",
        "port": 990,
        "username": "u",
        "password": "p",
        "path_prefix": "",
        "use_ssl": true,
        "ca_cert": null,
        "ca_cert_pem": null,
        "danger_disable_cert_verify": false
    }"#;
    let cfg: BackendConfig = serde_json::from_str(json).unwrap();
    assert!(matches!(cfg, BackendConfig::Ftps(_)));
}

#[test]
fn test_s3_config_parse() {
    let json = r#"{
        "type": "s3",
        "endpoint": "https://minio.local",
        "region": null,
        "bucket": "b",
        "path_style": true,
        "access_key_id": "k",
        "secret_access_key": "s",
        "session_token": null,
        "path_prefix": "",
        "ca_cert_pem": null
    }"#;
    let cfg: BackendConfig = serde_json::from_str(json).unwrap();
    match &cfg {
        BackendConfig::S3(s3) => {
            assert_eq!(s3.endpoint, "https://minio.local");
            assert_eq!(s3.bucket, "b");
            assert!(s3.path_style);
        }
        other => panic!("expected S3, got {:?}", other),
    }
}

#[cfg(feature = "s3")]
#[test]
fn test_s3_factory_builds_backend() {
    let json = r#"{
        "type": "s3",
        "endpoint": "https://minio.local",
        "region": null,
        "bucket": "b",
        "path_style": true,
        "access_key_id": "k",
        "secret_access_key": "s",
        "session_token": null,
        "path_prefix": "",
        "ca_cert_pem": null
    }"#;
    let cfg: BackendConfig = serde_json::from_str(json).unwrap();
    let backend = StorageBackendFactory::build(&cfg).unwrap();
    assert_eq!(backend.name(), "s3");
    let caps = backend.capabilities().unwrap();
    assert!(caps.supports(Capability::Rename));
    assert!(!caps.supports(Capability::Mkdir));
}

#[cfg(not(feature = "s3"))]
#[test]
fn test_s3_factory_rejects_without_feature() {
    let json = r#"{
        "type": "s3",
        "endpoint": "https://minio.local",
        "region": null,
        "bucket": "b",
        "path_style": true,
        "access_key_id": "k",
        "secret_access_key": "s",
        "session_token": null,
        "path_prefix": "",
        "ca_cert_pem": null
    }"#;
    let cfg: BackendConfig = serde_json::from_str(json).unwrap();
    let err = StorageBackendFactory::build(&cfg)
        .map(|_| ())
        .unwrap_err();
    assert!(err.to_string().contains("s3"));
}

#[test]
fn test_factory_builds_ftps_from_legacy() {
    let cfg = BackendConfig::try_from(legacy_ftps()).unwrap();
    let backend = StorageBackendFactory::build(&cfg).unwrap();
    assert_eq!(backend.name(), "ftp");
}

#[test]
fn test_ftps_backend_declares_rename_and_mkdir() {
    let ftps = rftps::storage::FtpsBackend::new(FtpsConfig {
        host: "example.com".into(),
        ..Default::default()
    });
    assert!(ftps.supports(Capability::Rename));
    assert!(ftps.supports(Capability::Mkdir));

    let backend: &dyn rftps::storage::StorageBackend = &ftps;
    let caps = backend.capabilities().expect("ftps exposes capabilities");
    assert!(caps.supports(Capability::Rename));
    assert!(caps.supports(Capability::Mkdir));
}

struct NoCapabilityBackend;

#[async_trait::async_trait]
impl rftps::storage::StorageBackend for NoCapabilityBackend {
    async fn upload(
        &self,
        _source: &std::path::Path,
        _dest: &str,
    ) -> Result<(), rftps::storage::StorageError> {
        Ok(())
    }
    async fn delete(&self, _path: &str) -> Result<(), rftps::storage::StorageError> {
        Ok(())
    }
    fn name(&self) -> &str {
        "no-caps"
    }
}

#[tokio::test]
async fn test_capability_helpers_reject_incapable_backend() {
    let backend = NoCapabilityBackend;
    let rename_err = rename_on(&backend, "a", "b").await.unwrap_err();
    assert!(rename_err.to_string().contains("not supported"));
    let mkdir_err = mkdir_on(&backend, "d").await.unwrap_err();
    assert!(mkdir_err.to_string().contains("not supported"));
}
