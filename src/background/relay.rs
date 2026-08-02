use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use ed25519_dalek::{Signer, SigningKey};
use tokio::sync::RwLock;

use crate::background::RemoteStorageConfig;
use crate::storage::traits::{StorageBackend, StorageError};

use super::config::RelayConfig;

#[derive(Debug, thiserror::Error)]
pub enum RelayError {
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("Relay API error ({code}): {message}")]
    Api { code: u16, message: String },
    #[error("device not approved yet")]
    PendingApproval,
    #[error("device deauthorized")]
    Deauthorized,
    #[error("configuration error: {0}")]
    Config(String),
}

impl RelayError {
    fn to_storage(&self) -> StorageError {
        match self {
            RelayError::PendingApproval => {
                StorageError::Connection("device not approved by relay yet".into())
            }
            RelayError::Deauthorized => StorageError::PermissionDenied(
                "device deauthorized by relay".into(),
            ),
            RelayError::Http(_) | RelayError::Api { .. } => {
                StorageError::Connection(self.to_string())
            }
            RelayError::Config(_) => StorageError::Config(self.to_string()),
        }
    }
}

pub struct RelayClient {
    base: String,
    http: reqwest::Client,
    signing: SigningKey,
    public_key_hex: String,
    device_name: String,
    approval_timeout: Duration,
}

impl RelayClient {
    pub fn new(config: &RelayConfig) -> Result<Self, RelayError> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let base = config.url.trim_end_matches('/').to_string();
        if base.is_empty() {
            return Err(RelayError::Config("relay.url is empty".into()));
        }
        let bytes = hex::decode(config.device_key.trim())
            .map_err(|_| RelayError::Config("relay.device_key must be hex".into()))?;
        if bytes.len() != 32 {
            return Err(RelayError::Config(
                "relay.device_key must be exactly 32 bytes".into(),
            ));
        }
        let mut seed = [0u8; 32];
        seed.copy_from_slice(&bytes);
        let signing = SigningKey::from_bytes(&seed);
        let public_key_hex = hex::encode(signing.verifying_key().to_bytes());

        let mut builder = reqwest::Client::builder();
        if config.danger_disable_cert_verify {
            builder = builder.danger_accept_invalid_certs(true);
        }
        if let Some(ca_path) = &config.ca_cert {
            let pem = std::fs::read(ca_path)
                .map_err(|e| RelayError::Config(format!("cannot read CA cert '{}': {}", ca_path, e)))?;
            let cert = reqwest::Certificate::from_pem(&pem)
                .map_err(|e| RelayError::Config(format!("invalid CA cert '{}': {}", ca_path, e)))?;
            builder = builder.add_root_certificate(cert);
        }
        let http = builder
            .build()
            .map_err(|e| RelayError::Http(e.to_string()))?;

        Ok(Self {
            base,
            http,
            signing,
            public_key_hex,
            device_name: config.device_name.clone(),
            approval_timeout: Duration::from_secs(config.approval_timeout_secs),
        })
    }

    pub fn public_key(&self) -> &str {
        &self.public_key_hex
    }

    pub fn approval_timeout(&self) -> Duration {
        self.approval_timeout
    }

    async fn post(
        &self,
        path: &str,
        body: &serde_json::Value,
        token: Option<&str>,
    ) -> Result<serde_json::Value, RelayError> {
        let mut req = self.http.post(format!("{}{}", self.base, path)).json(body);
        if let Some(t) = token {
            req = req.bearer_auth(t);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| RelayError::Http(e.to_string()))?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        let value: serde_json::Value =
            serde_json::from_str(&text).unwrap_or_else(|_| serde_json::Value::Null);
        if !status.is_success() {
            let message = value
                .get("detail")
                .and_then(|d| d.as_str())
                .unwrap_or(&text)
                .to_string();
            return Err(RelayError::Api {
                code: status.as_u16(),
                message,
            });
        }
        Ok(value)
    }

    async fn get(&self, path: &str) -> Result<serde_json::Value, RelayError> {
        let resp = self
            .http
            .get(format!("{}{}", self.base, path))
            .send()
            .await
            .map_err(|e| RelayError::Http(e.to_string()))?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        let value: serde_json::Value =
            serde_json::from_str(&text).unwrap_or_else(|_| serde_json::Value::Null);
        if !status.is_success() {
            let message = value
                .get("detail")
                .and_then(|d| d.as_str())
                .unwrap_or(&text)
                .to_string();
            return Err(RelayError::Api {
                code: status.as_u16(),
                message,
            });
        }
        Ok(value)
    }

    pub async fn register(&self) -> Result<(), RelayError> {
        let body = serde_json::json!({
            "public_key": self.public_key_hex,
            "device_info": {"name": self.device_name},
        });
        self.post("/api/devices/register", &body, None).await?;
        Ok(())
    }

    pub async fn wait_for_approval(&self) -> Result<(), RelayError> {
        let deadline = tokio::time::Instant::now() + self.approval_timeout;
        loop {
            if tokio::time::Instant::now() >= deadline {
                return Err(RelayError::PendingApproval);
            }
            let resp = self
                .get(&format!("/api/devices/status?public_key={}", self.public_key_hex))
                .await?;
            match resp["status"].as_str() {
                Some("approved") => return Ok(()),
                Some("deauthorized") => return Err(RelayError::Deauthorized),
                _ => {}
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    pub async fn authenticate(&self) -> Result<String, RelayError> {
        let challenge = self
            .post(
                "/api/auth/challenge",
                &serde_json::json!({"public_key": self.public_key_hex}),
                None,
            )
            .await?;
        let nonce = challenge
            .get("challenge")
            .and_then(|c| c.as_str())
            .ok_or_else(|| RelayError::Http("missing challenge in response".into()))?;

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let message = format!("{}{}{}", nonce, self.public_key_hex, timestamp);
        let signature = hex::encode(self.signing.sign(message.as_bytes()).to_bytes());

        let verify = self
            .post(
                "/api/auth/verify",
                &serde_json::json!({
                    "public_key": self.public_key_hex,
                    "signature": signature,
                    "timestamp": timestamp,
                }),
                None,
            )
            .await?;
        verify
            .get("session_token")
            .and_then(|t| t.as_str())
            .map(str::to_string)
            .ok_or_else(|| RelayError::Http("missing session_token in response".into()))
    }

    pub async fn fetch_credentials(&self, token: &str) -> Result<RemoteStorageConfig, RelayError> {
        let resp = self
            .post("/api/credentials/fetch", &serde_json::json!({}), Some(token))
            .await?;
        let protocol = resp.get("protocol").and_then(|p| p.as_str()).unwrap_or("ftp");
        Ok(RemoteStorageConfig {
            backend: crate::background::StorageBackendType::Ftps,
            host: resp.get("host").and_then(|h| h.as_str()).unwrap_or("").to_string(),
            port: resp.get("port").and_then(|p| p.as_u64()).map(|p| p as u16),
            username: resp.get("user").and_then(|u| u.as_str()).unwrap_or("").to_string(),
            password: resp
                .get("password")
                .and_then(|p| p.as_str())
                .unwrap_or("")
                .to_string(),
            path_prefix: resp.get("root").and_then(|r| r.as_str()).unwrap_or("").to_string(),
            use_ssl: protocol == "ftps",
            ca_cert: None,
            ca_cert_pem: resp.get("ca_cert").and_then(|c| c.as_str()).map(|s| s.to_string()),
            danger_disable_cert_verify: false,
        })
    }
}

/// Storage backend that obtains its credentials from the relay and keeps them
/// in memory for the process lifetime (ADR-007 / ADR-010).
pub struct RelayStorageBackend {
    client: Arc<RelayClient>,
    cache: RwLock<Option<RemoteStorageConfig>>,
    bus: Option<crate::event::EventBus>,
}

/// Generates a fresh hex-encoded Ed25519 device seed (same as `rftps relay keygen`).
pub fn generate_device_key() -> String {
    use ed25519_dalek::SigningKey;
    use rand_core::OsRng;
    hex::encode(SigningKey::generate(&mut OsRng).to_bytes())
}

impl RelayStorageBackend {
    pub fn new(config: RelayConfig, bus: Option<crate::event::EventBus>) -> Result<Self, RelayError> {
        Ok(Self {
            client: Arc::new(RelayClient::new(&config)?),
            cache: RwLock::new(None),
            bus,
        })
    }

    pub fn client(&self) -> &RelayClient {
        &self.client
    }

    async fn emit(&self, status: &str, message: Option<&str>) {
        if let Some(bus) = &self.bus {
            bus.publish(&crate::event::FtpEvent::RelayStatus {
                status: status.into(),
                message: message.map(|m| m.to_string()),
            });
        }
    }

    async fn credentials(&self) -> Result<RemoteStorageConfig, StorageError> {
        if let Some(creds) = self.cache.read().await.as_ref() {
            return Ok(creds.clone());
        }
        self.emit("registering", None).await;
        self.client
            .register()
            .await
            .map_err(|e| {
                let err = e.to_storage();
                let msg = err.to_string();
                let _ = self.emit("error", Some(&msg));
                err
            })?;
        self.emit("registered", None).await;
        self.client
            .wait_for_approval()
            .await
            .map_err(|e| {
                let err = e.to_storage();
                if err.is_retryable() {
                    let _ = self.emit("pending", Some("waiting for approval"));
                } else {
                    let msg = err.to_string();
                    let _ = self.emit("rejected", Some(&msg));
                }
                err
            })?;
        self.emit("approved", None).await;
        let token = self
            .client
            .authenticate()
            .await
            .map_err(|e| e.to_storage())?;
        let creds = self
            .client
            .fetch_credentials(&token)
            .await
            .map_err(|e| e.to_storage())?;
        self.emit("active", Some("credentials armed")).await;
        *self.cache.write().await = Some(creds.clone());
        Ok(creds)
    }
}

#[async_trait]
impl StorageBackend for RelayStorageBackend {
    async fn upload(&self, source_path: &Path, dest_path: &str) -> Result<(), StorageError> {
        let creds = self.credentials().await?;
        crate::storage::FtpsBackend::new(creds)
            .upload(source_path, dest_path)
            .await
    }

    async fn delete(&self, path: &str) -> Result<(), StorageError> {
        let creds = self.credentials().await?;
        crate::storage::FtpsBackend::new(creds).delete(path).await
    }

    async fn rename(&self, old_path: &str, new_path: &str) -> Result<(), StorageError> {
        let creds = self.credentials().await?;
        crate::storage::FtpsBackend::new(creds)
            .rename(old_path, new_path)
            .await
    }

    async fn mkdir(&self, path: &str) -> Result<(), StorageError> {
        let creds = self.credentials().await?;
        crate::storage::FtpsBackend::new(creds).mkdir(path).await
    }

    fn name(&self) -> &str {
        "relay"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
