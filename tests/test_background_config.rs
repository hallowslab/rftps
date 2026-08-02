use rftps::background::{BackgroundJobConfig, RemoteStorageConfig, UserMapping};
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
