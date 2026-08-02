#![cfg(feature = "background-jobs")]

use rftps::background::FtpsConfig;
use rftps::storage::StorageBackend;
use rustls::pki_types::{CertificateDer, pem::PemObject};
use std::io::Write;
use tempfile::NamedTempFile;

fn generate_self_signed_cert_pem() -> String {
    let mut params = rcgen::CertificateParams::new(vec!["test.local".to_string()]).unwrap();
    params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let key_pair = rcgen::KeyPair::generate().unwrap();
    let cert = params.self_signed(&key_pair).unwrap();
    cert.pem()
}

fn install_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

#[test]
fn test_no_certificate_verification_is_debug() {
    let verifier = rftps::storage::tls_utils::NoCertificateVerification;
    let debug = format!("{:?}", verifier);
    assert!(debug.contains("NoCertificateVerification"));
}

#[test]
fn test_no_certificate_verification_display() {
    let verifier = rftps::storage::tls_utils::NoCertificateVerification;
    let display = format!("{}", verifier);
    assert!(display.contains("DANGER"));
}

#[test]
fn test_load_ca_cert_from_pem_file() {
    let pem = generate_self_signed_cert_pem();
    let mut tmp = NamedTempFile::new().unwrap();
    tmp.write_all(pem.as_bytes()).unwrap();
    let path = tmp.path().to_str().unwrap();

    let certs: Vec<CertificateDer<'static>> = CertificateDer::pem_file_iter(path)
        .expect("failed to open cert file")
        .collect::<Result<Vec<_>, _>>()
        .expect("failed to parse cert PEM");

    assert_eq!(certs.len(), 1);
}

#[test]
fn test_load_ca_cert_missing_file() {
    let result = CertificateDer::pem_file_iter("/nonexistent/path/cert.pem");
    assert!(result.is_err());
}

#[test]
fn test_load_ca_cert_nonexistent_file() {
    let result = std::fs::read_to_string("/nonexistent/path/cert.pem");
    assert!(result.is_err());
}

#[test]
fn test_load_ca_cert_pem_content() {
    let pem = generate_self_signed_cert_pem();
    let certs = CertificateDer::pem_slice_iter(pem.as_bytes())
        .collect::<Result<Vec<_>, _>>()
        .expect("failed to parse cert PEM content");

    assert_eq!(certs.len(), 1);
}

#[test]
fn test_ftps_backend_with_custom_ca_config() {
    let config = FtpsConfig {
        host: "test.local".into(),
        port: Some(990),
        username: "user".into(),
        password: "pass".into(),
        path_prefix: "".into(),
        use_ssl: true,
        ca_cert: Some("/path/to/server.pem".into()),
        ca_cert_pem: None,
        danger_disable_cert_verify: false,
    };

    let backend = rftps::storage::FtpsBackend::new(config);
    assert_eq!(backend.name(), "ftp");
}

#[test]
fn test_ftps_backend_with_danger_mode() {
    let config = FtpsConfig {
        host: "test.local".into(),
        port: Some(21),
        username: "user".into(),
        password: "pass".into(),
        path_prefix: "".into(),
        use_ssl: true,
        ca_cert: None,
        ca_cert_pem: None,
        danger_disable_cert_verify: true,
    };

    let backend = rftps::storage::FtpsBackend::new(config);
    assert_eq!(backend.name(), "ftp");
}

#[test]
fn test_ftps_backend_with_inline_ca_pem() {
    let config = FtpsConfig {
        host: "test.local".into(),
        port: Some(990),
        username: "user".into(),
        password: "pass".into(),
        path_prefix: "".into(),
        use_ssl: true,
        ca_cert: None,
        ca_cert_pem: Some(generate_self_signed_cert_pem()),
        danger_disable_cert_verify: false,
    };

    let backend = rftps::storage::FtpsBackend::new(config);
    assert_eq!(backend.name(), "ftp");
}

#[test]
fn test_build_root_store_with_custom_ca() {
    let pem = generate_self_signed_cert_pem();
    let mut tmp = NamedTempFile::new().unwrap();
    tmp.write_all(pem.as_bytes()).unwrap();
    let path = tmp.path().to_str().unwrap();

    let mut root_store = rustls::RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

    let ca_certs: Vec<CertificateDer<'static>> = CertificateDer::pem_file_iter(path)
        .expect("failed to open cert file")
        .collect::<Result<Vec<_>, _>>()
        .expect("failed to parse cert PEM");

    for cert in ca_certs {
        root_store.add(cert).expect("failed to add CA cert");
    }

    assert!(!root_store.is_empty());
    assert!(root_store.len() > webpki_roots::TLS_SERVER_ROOTS.len());
}

#[test]
fn test_danger_config_with_no_cert_verifier() {
    install_crypto_provider();
    let verifier = std::sync::Arc::new(rftps::storage::tls_utils::NoCertificateVerification);
    let tls_config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();

    let connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(tls_config));
    let _async_connector = suppaftp::tokio::AsyncRustlsConnector::from(connector);

    // If we got here, the connector was built successfully
}

#[test]
fn test_ftps_backend_config_construction() {
    let config = FtpsConfig {
        host: "test.local".into(),
        port: Some(990),
        username: "user".into(),
        password: "pass".into(),
        path_prefix: "backups".into(),
        use_ssl: true,
        ca_cert: Some("/path/to/server.pem".into()),
        ca_cert_pem: None,
        danger_disable_cert_verify: false,
    };

    let _backend = rftps::storage::FtpsBackend::new(config);
}
