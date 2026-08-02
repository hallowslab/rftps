use async_trait::async_trait;
use rustls::pki_types::{CertificateDer, pem::PemObject};
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

use suppaftp::tokio::{AsyncFtpStream, AsyncRustlsConnector, AsyncRustlsFtpStream};
use suppaftp::types::FileType;

use super::traits::{BackendCapabilities, Capability, StorageBackend, StorageError};
use super::tls_utils::NoCertificateVerification;
use crate::background::config::FtpsConfig;

pub struct FtpsBackend {
    config: FtpsConfig,
}

enum FtpConn {
    Plain(AsyncFtpStream),
    Tls(AsyncRustlsFtpStream),
}

impl FtpConn {
    async fn login(&mut self, user: &str, pass: &str) -> Result<(), StorageError> {
        match self {
            FtpConn::Plain(s) => s
                .login(user, pass)
                .await
                .map_err(|e| StorageError::Auth(e.to_string())),
            FtpConn::Tls(s) => s
                .login(user, pass)
                .await
                .map_err(|e| StorageError::Auth(e.to_string())),
        }
    }

    async fn transfer_type(&mut self, ft: FileType) -> Result<(), StorageError> {
        match self {
            FtpConn::Plain(s) => s
                .transfer_type(ft)
                .await
                .map_err(|e| StorageError::Connection(e.to_string())),
            FtpConn::Tls(s) => s
                .transfer_type(ft)
                .await
                .map_err(|e| StorageError::Connection(e.to_string())),
        }
    }

    async fn put_file<R: tokio::io::AsyncRead + Unpin + Send>(
        &mut self,
        path: &str,
        reader: &mut R,
    ) -> Result<(), StorageError> {
        match self {
            FtpConn::Plain(s) => s
                .put_file(path, reader)
                .await
                .map(|_| ())
                .map_err(|e| StorageError::Transfer(e.to_string())),
            FtpConn::Tls(s) => s
                .put_file(path, reader)
                .await
                .map(|_| ())
                .map_err(|e| StorageError::Transfer(e.to_string())),
        }
    }

    async fn rm(&mut self, path: &str) -> Result<(), StorageError> {
        match self {
            FtpConn::Plain(s) => s
                .rm(path)
                .await
                .map_err(|e| StorageError::Transfer(e.to_string())),
            FtpConn::Tls(s) => s
                .rm(path)
                .await
                .map_err(|e| StorageError::Transfer(e.to_string())),
        }
    }

    async fn rename(&mut self, from: &str, to: &str) -> Result<(), StorageError> {
        match self {
            FtpConn::Plain(s) => s
                .rename(from, to)
                .await
                .map_err(|e| StorageError::Transfer(e.to_string())),
            FtpConn::Tls(s) => s
                .rename(from, to)
                .await
                .map_err(|e| StorageError::Transfer(e.to_string())),
        }
    }

    async fn mkdir(&mut self, path: &str) -> Result<(), StorageError> {
        match self {
            FtpConn::Plain(s) => s
                .mkdir(path)
                .await
                .map_err(|e| StorageError::Transfer(e.to_string())),
            FtpConn::Tls(s) => s
                .mkdir(path)
                .await
                .map_err(|e| StorageError::Transfer(e.to_string())),
        }
    }

    async fn quit(&mut self) {
        match self {
            FtpConn::Plain(s) => { let _ = s.quit().await; }
            FtpConn::Tls(s) => { let _ = s.quit().await; }
        }
    }
}

impl FtpsBackend {
    pub fn new(config: FtpsConfig) -> Self {
        Self { config }
    }

    fn address(&self) -> String {
        let port = self.config.port.unwrap_or(if self.config.use_ssl { 990 } else { 21 });
        format!("{}:{}", self.config.host, port)
    }

    async fn connect(&self) -> Result<FtpConn, StorageError> {
        let addr = self.address();

        let mut conn = if self.config.use_ssl {
            let tls_config = if self.config.danger_disable_cert_verify {
                eprintln!("[FTPS] WARNING: certificate verification is disabled — DO NOT use in production");
                rustls::ClientConfig::builder()
                    .dangerous()
                    .with_custom_certificate_verifier(Arc::new(NoCertificateVerification))
                    .with_no_client_auth()
            } else {
                let mut root_store = rustls::RootCertStore::empty();
                root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

                if let Some(ref ca_path) = self.config.ca_cert {
                    let ca_cert = CertificateDer::pem_file_iter(ca_path)
                        .map_err(|e| StorageError::Config(format!("Failed to read CA cert '{}': {}", ca_path, e)))?
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|e| StorageError::Config(format!("Failed to parse CA cert '{}': {}", ca_path, e)))?;

                    for cert in ca_cert {
                        root_store.add(cert)
                            .map_err(|e| StorageError::Config(format!("Failed to add CA cert to trust store: {}", e)))?;
                    }
                }

                if let Some(ref ca_pem) = self.config.ca_cert_pem {
                    let ca_cert = CertificateDer::pem_slice_iter(ca_pem.as_bytes())
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|e| StorageError::Config(format!("Failed to parse CA cert PEM: {}", e)))?;

                    for cert in ca_cert {
                        root_store.add(cert)
                            .map_err(|e| StorageError::Config(format!("Failed to add CA cert to trust store: {}", e)))?;
                    }
                }

                rustls::ClientConfig::builder()
                    .with_root_certificates(root_store)
                    .with_no_client_auth()
            };

            let connector = tokio_rustls::TlsConnector::from(Arc::new(tls_config));
            let async_connector = AsyncRustlsConnector::from(connector);

            let stream = AsyncRustlsFtpStream::connect(&addr)
                .await
                .map_err(|e| StorageError::Connection(e.to_string()))?
                .into_secure(async_connector, &self.config.host)
                .await
                .map_err(|e| StorageError::Connection(format!("TLS handshake failed: {}", e)))?;

            FtpConn::Tls(stream)
        } else {
            FtpConn::Plain(
                AsyncFtpStream::connect(&addr)
                    .await
                    .map_err(|e| StorageError::Connection(e.to_string()))?,
            )
        };

        conn.login(&self.config.username, &self.config.password).await?;
        conn.transfer_type(FileType::Binary).await?;

        Ok(conn)
    }

    fn remote_path(&self, path: &str) -> String {
        if self.config.path_prefix.is_empty() {
            path.to_string()
        } else {
            format!("{}/{}", self.config.path_prefix.trim_end_matches('/'), path)
        }
    }

    async fn mkdir_recursive(&self, conn: &mut FtpConn, path: &str) -> Result<(), StorageError> {
        let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let mut current = String::new();

        for component in &components {
            current.push('/');
            current.push_str(component);
            let _ = conn.mkdir(&current).await;
        }

        Ok(())
    }
}

#[async_trait]
impl StorageBackend for FtpsBackend {
    async fn upload(&self, source_path: &Path, dest_path: &str) -> Result<(), StorageError> {
        let mut conn = self.connect().await?;
        let remote = self.remote_path(dest_path);

        let parent = std::path::Path::new(&remote)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();

        if !parent.is_empty() {
            self.mkdir_recursive(&mut conn, &parent).await?;
        }

        let file_bytes = tokio::fs::read(source_path)
            .await
            .map_err(StorageError::Io)?;

        let mut cursor = Cursor::new(file_bytes);
        conn.put_file(&remote, &mut cursor).await?;

        conn.quit().await;
        Ok(())
    }

    async fn delete(&self, path: &str) -> Result<(), StorageError> {
        let mut conn = self.connect().await?;
        let remote = self.remote_path(path);

        conn.rm(&remote).await?;
        conn.quit().await;
        Ok(())
    }

    fn name(&self) -> &str {
        "ftp"
    }

    fn capabilities(&self) -> Option<&dyn BackendCapabilities> {
        Some(self)
    }
}

#[async_trait]
impl BackendCapabilities for FtpsBackend {
    fn supports(&self, capability: Capability) -> bool {
        matches!(capability, Capability::Rename | Capability::Mkdir)
    }

    async fn rename(&self, old_path: &str, new_path: &str) -> Result<(), StorageError> {
        let mut conn = self.connect().await?;
        let old_remote = self.remote_path(old_path);
        let new_remote = self.remote_path(new_path);

        conn.rename(&old_remote, &new_remote).await?;
        conn.quit().await;
        Ok(())
    }

    async fn mkdir(&self, path: &str) -> Result<(), StorageError> {
        let mut conn = self.connect().await?;
        let remote = self.remote_path(path);

        self.mkdir_recursive(&mut conn, &remote).await?;
        conn.quit().await;
        Ok(())
    }
}
