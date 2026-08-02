#![cfg(all(feature = "background-jobs", feature = "relay"))]

use ed25519_dalek::SigningKey;
use rftps::background::config::{BackendConfig, RelayConfig};
use rftps::background::relay::{parse_fetch_payload, RelayClient, RelayError};

fn cfg(url: &str, key: &str) -> RelayConfig {
    RelayConfig {
        url: url.into(),
        device_key: key.into(),
        device_name: "test".into(),
        approval_timeout_secs: 1,
        ca_cert: None,
        danger_disable_cert_verify: false,
        relay_messages: true,
    }
}

#[test]
fn rejects_empty_url() {
    assert!(matches!(
        RelayClient::new(&cfg("", &"00".repeat(32))),
        Err(RelayError::Config(_))
    ));
}

#[test]
fn rejects_non_hex_key() {
    assert!(matches!(
        RelayClient::new(&cfg("http://x", "zzzz")),
        Err(RelayError::Config(_))
    ));
}

#[test]
fn rejects_short_key() {
    assert!(matches!(
        RelayClient::new(&cfg("http://x", "aa")),
        Err(RelayError::Config(_))
    ));
}

#[test]
fn derives_public_key_from_seed() {
    let client = RelayClient::new(&cfg("http://localhost:8700", &"ab".repeat(32))).unwrap();
    assert_eq!(client.public_key().len(), 64);
    let seed = hex::decode("ab".repeat(32)).unwrap();
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&seed);
    let expected = hex::encode(SigningKey::from_bytes(&bytes).verifying_key().to_bytes());
    assert_eq!(client.public_key(), expected);
}

#[test]
fn error_mapping_retryable() {
    assert!(RelayError::PendingApproval.to_storage().is_retryable());
    assert!(RelayError::Http("boom".into()).to_storage().is_retryable());
    assert!(RelayError::Api { code: 500, message: "x".into() }
        .to_storage()
        .is_retryable());
}

#[test]
fn error_mapping_permanent() {
    assert!(!RelayError::Deauthorized.to_storage().is_retryable());
    assert!(!RelayError::Config("bad".into()).to_storage().is_retryable());
}

#[test]
fn parses_legacy_flat_ftp_payload() {
    let resp = serde_json::json!({
        "protocol": "ftps",
        "host": "ftp.example.com",
        "port": 990,
        "user": "alice",
        "password": "secret",
        "root": "/photos",
        "ca_cert": "PEM",
        "device_id": "abc",
    });
    let config = parse_fetch_payload(&resp).unwrap();
    assert_eq!(config.version, 1);
    match config.backend {
        BackendConfig::Ftps(f) => {
            assert_eq!(f.host, "ftp.example.com");
            assert_eq!(f.port, Some(990));
            assert_eq!(f.username, "alice");
            assert_eq!(f.path_prefix, "/photos");
            assert!(f.use_ssl);
            assert_eq!(f.ca_cert_pem.as_deref(), Some("PEM"));
        }
        _ => panic!("expected FTPS"),
    }
}

#[test]
fn parses_versioned_ftps_payload() {
    let resp = serde_json::json!({
        "version": 1,
        "backend": {
            "type": "ftps",
            "host": "ftp.example.com",
            "port": 21,
            "user": "bob",
            "password": "pw",
            "root": null,
            "ca_cert": null,
        }
    });
    let config = parse_fetch_payload(&resp).unwrap();
    match config.backend {
        BackendConfig::Ftps(f) => {
            assert_eq!(f.host, "ftp.example.com");
            assert_eq!(f.port, Some(21));
            assert_eq!(f.username, "bob");
            assert_eq!(f.path_prefix, "");
            assert!(f.use_ssl);
            assert_eq!(f.ca_cert_pem, None);
        }
        _ => panic!("expected FTPS"),
    }
}

#[test]
fn parses_versioned_s3_payload() {
    let resp = serde_json::json!({
        "version": 1,
        "backend": {
            "type": "s3",
            "endpoint": "https://minio.local:9000",
            "region": "us-east-1",
            "bucket": "photos",
            "path_style": true,
            "access_key_id": "AKID",
            "secret_access_key": "SAK",
            "session_token": null,
            "root": "camera-a",
            "ca_cert": "PEM",
        }
    });
    let config = parse_fetch_payload(&resp).unwrap();
    match config.backend {
        BackendConfig::S3(s) => {
            assert_eq!(s.endpoint, "https://minio.local:9000");
            assert_eq!(s.region.as_deref(), Some("us-east-1"));
            assert_eq!(s.bucket, "photos");
            assert!(s.path_style);
            assert_eq!(s.access_key_id, "AKID");
            assert_eq!(s.secret_access_key, "SAK");
            assert_eq!(s.path_prefix, "camera-a");
            assert_eq!(s.ca_cert_pem.as_deref(), Some("PEM"));
            assert_eq!(s.multipart_threshold_bytes, None);
        }
        _ => panic!("expected S3"),
    }
}

#[test]
fn s3_path_style_defaults_true() {
    let resp = serde_json::json!({
        "version": 1,
        "backend": {
            "type": "s3",
            "endpoint": "https://minio.local:9000",
            "bucket": "photos",
            "access_key_id": "AKID",
            "secret_access_key": "SAK",
        }
    });
    match parse_fetch_payload(&resp).unwrap().backend {
        BackendConfig::S3(s) => assert!(s.path_style),
        _ => panic!("expected S3"),
    }
}

#[test]
fn rejects_missing_s3_endpoint() {
    let resp = serde_json::json!({
        "version": 1,
        "backend": {
            "type": "s3",
            "bucket": "photos",
            "access_key_id": "AKID",
            "secret_access_key": "SAK",
        }
    });
    assert!(matches!(
        parse_fetch_payload(&resp),
        Err(RelayError::Config(_))
    ));
}

#[test]
fn rejects_unknown_backend_type() {
    let resp = serde_json::json!({
        "version": 1,
        "backend": {"type": "sftp"}
    });
    assert!(matches!(
        parse_fetch_payload(&resp),
        Err(RelayError::Config(_))
    ));
}

#[test]
fn rejects_future_version() {
    let resp = serde_json::json!({
        "version": 99,
        "backend": {"type": "ftps", "host": "h", "user": "u"}
    });
    assert!(matches!(
        parse_fetch_payload(&resp),
        Err(RelayError::Config(_))
    ));
}
