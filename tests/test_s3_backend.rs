#![cfg(all(feature = "background-jobs", feature = "s3"))]

use rftps::background::{BackendConfig, S3Config};
use rftps::storage::s3::{plan_part_size, MAX_PART_COUNT, MIN_PART_SIZE};
use rftps::storage::{Capability, S3Backend, StorageBackend, StorageBackendFactory, mkdir_on};

fn s3_config() -> S3Config {
    S3Config {
        endpoint: "https://minio.local".into(),
        region: Some("us-east-1".into()),
        bucket: "photos".into(),
        path_style: true,
        access_key_id: "k".into(),
        secret_access_key: "s".into(),
        session_token: None,
        path_prefix: "".into(),
        ca_cert_pem: None,
        multipart_threshold_bytes: None,
        immutable_naming: false,
    }
}

fn backend() -> S3Backend {
    S3Backend::new(s3_config()).unwrap()
}

#[test]
fn object_key_trims_leading_slash() {
    let b = backend();
    assert_eq!(b.object_key("/alice/pic.jpg"), "alice/pic.jpg");
    assert_eq!(b.object_key("alice/pic.jpg"), "alice/pic.jpg");
}

#[test]
fn object_key_applies_path_prefix() {
    let b = S3Backend::new(S3Config {
        path_prefix: "backups/".into(),
        ..s3_config()
    })
    .unwrap();
    assert_eq!(b.object_key("alice/pic.jpg"), "backups/alice/pic.jpg");
    assert_eq!(b.object_key("/alice/pic.jpg"), "backups/alice/pic.jpg");
}

#[test]
fn object_key_immutable_naming_prepends_uuid() {
    let b = S3Backend::new(S3Config {
        immutable_naming: true,
        ..s3_config()
    })
    .unwrap();
    let key1 = b.object_key("alice/pic.jpg");
    let key2 = b.object_key("alice/pic.jpg");
    
    // Keys should be different (UUID prepended)
    assert_ne!(key1, key2);
    
    // Keys should end with the original filename
    assert!(key1.ends_with("-pic.jpg"));
    assert!(key2.ends_with("-pic.jpg"));
    
    // Keys should preserve directory structure
    assert!(key1.starts_with("alice/"));
    assert!(key2.starts_with("alice/"));
}

#[test]
fn object_key_immutable_naming_with_prefix() {
    let b = S3Backend::new(S3Config {
        path_prefix: "backups".into(),
        immutable_naming: true,
        ..s3_config()
    })
    .unwrap();
    let key = b.object_key("alice/pic.jpg");
    
    // Key should start with prefix
    assert!(key.starts_with("backups/"));
    
    // Key should end with filename
    assert!(key.ends_with("-pic.jpg"));
}

#[test]
fn object_key_immutable_naming_root_file() {
    let b = S3Backend::new(S3Config {
        immutable_naming: true,
        ..s3_config()
    })
    .unwrap();
    let key = b.object_key("pic.jpg");
    
    // Key should end with filename
    assert!(key.ends_with("-pic.jpg"));
    
    // Key should not have directory prefix
    assert!(!key.contains('/'));
}

#[test]
fn copy_source_encodes_key_but_keeps_separators() {
    let b = backend();
    assert_eq!(
        b.copy_source("alice/photo (1).jpg"),
        "photos/alice/photo%20%281%29.jpg"
    );
    assert_eq!(b.copy_source("alice/a-b_c.d"), "photos/alice/a-b_c.d");
}

#[test]
fn name_and_capabilities() {
    let b = backend();
    assert_eq!(b.name(), "s3");
    assert!(b.capabilities().unwrap().supports(Capability::Rename));
    assert!(!b.capabilities().unwrap().supports(Capability::Mkdir));
}

#[test]
fn plan_part_size_floors_at_min_part_size() {
    assert_eq!(plan_part_size(10 * 1024 * 1024), MIN_PART_SIZE);
    assert_eq!(plan_part_size(MIN_PART_SIZE), MIN_PART_SIZE);
}

#[test]
fn plan_part_size_grows_to_cap_part_count() {
    let size = 100 * 1024 * 1024 * 1024u64;
    let part_size = plan_part_size(size);
    assert!(part_size >= MIN_PART_SIZE);
    assert!(size.div_ceil(part_size) <= MAX_PART_COUNT);
}

#[test]
fn multipart_threshold_uses_config_when_set() {
    let b = S3Backend::new(S3Config {
        multipart_threshold_bytes: Some(1024),
        ..s3_config()
    })
    .unwrap();
    assert_eq!(b.multipart_threshold(), 1024);
}

#[test]
fn factory_builds_s3_backend() {
    let backend = StorageBackendFactory::build(&BackendConfig::S3(s3_config())).unwrap();
    assert_eq!(backend.name(), "s3");
}

#[test]
fn s3_declares_rename_but_not_mkdir() {
    let backend = StorageBackendFactory::build(&BackendConfig::S3(s3_config())).unwrap();
    let caps = backend.capabilities().unwrap();
    assert!(caps.supports(Capability::Rename));
    assert!(!caps.supports(Capability::Mkdir));
}

#[tokio::test]
async fn mkdir_on_declines_for_s3() {
    let backend = StorageBackendFactory::build(&BackendConfig::S3(s3_config())).unwrap();
    let err = mkdir_on(backend.as_ref(), "dir").await.unwrap_err();
    assert!(err.to_string().contains("not supported"));
}
