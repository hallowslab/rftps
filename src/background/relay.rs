use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use ed25519_dalek::{Signer, SigningKey};
use tokio::sync::RwLock;

use crate::background::config::{BackendConfig, FtpsConfig, S3Config, VersionedBackendConfig};
use crate::storage::traits::{StorageBackend, StorageError};
use crate::storage::StorageBackendFactory;

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
    pub fn to_storage(&self) -> StorageError {
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

    pub async fn fetch_backend_config(
        &self,
        token: &str,
    ) -> Result<VersionedBackendConfig, RelayError> {
        let resp = self
            .post("/api/credentials/fetch", &serde_json::json!({}), Some(token))
            .await?;
        parse_fetch_payload(&resp)
    }
}

/// Parses a `/api/credentials/fetch` response into a backend config.
///
/// Accepts both the versioned payload `{"version": 1, "backend": {"type": ...}}`
/// and the legacy flat FTP payload (absent `version`/`backend`) treated as version 1.
pub fn parse_fetch_payload(resp: &serde_json::Value) -> Result<VersionedBackendConfig, RelayError> {
    if let Some(backend) = resp.get("backend") {
        let version = resp
            .get("version")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as u32;
        let config = VersionedBackendConfig {
            version,
            backend: parse_backend(backend)?,
        };
        config
            .check_version()
            .map_err(RelayError::Config)?;
        return Ok(config);
    }
    let protocol = resp
        .get("protocol")
        .and_then(|p| p.as_str())
        .unwrap_or("ftp");
    let config = FtpsConfig {
        host: str_field(resp, "host"),
        port: resp.get("port").and_then(|p| p.as_u64()).map(|p| p as u16),
        username: str_field(resp, "user"),
        password: str_field(resp, "password"),
        path_prefix: str_field(resp, "root"),
        use_ssl: protocol == "ftps",
        ca_cert: None,
        ca_cert_pem: resp.get("ca_cert").and_then(|c| c.as_str()).map(str::to_string),
        danger_disable_cert_verify: false,
    };
    Ok(VersionedBackendConfig::new(BackendConfig::Ftps(config)))
}

fn parse_backend(value: &serde_json::Value) -> Result<BackendConfig, RelayError> {
    match value.get("type").and_then(|t| t.as_str()) {
        Some("s3") => {
            let endpoint = str_field(value, "endpoint");
            if endpoint.is_empty() {
                return Err(RelayError::Config(
                    "relay S3 backend missing endpoint".into(),
                ));
            }
            let bucket = str_field(value, "bucket");
            if bucket.is_empty() {
                return Err(RelayError::Config("relay S3 backend missing bucket".into()));
            }
            let access_key_id = str_field(value, "access_key_id");
            let secret_access_key = str_field(value, "secret_access_key");
            if access_key_id.is_empty() || secret_access_key.is_empty() {
                return Err(RelayError::Config(
                    "relay S3 backend missing credentials".into(),
                ));
            }
            Ok(BackendConfig::S3(S3Config {
                endpoint,
                region: opt_str_field(value, "region"),
                bucket,
                path_style: value
                    .get("path_style")
                    .and_then(|p| p.as_bool())
                    .unwrap_or(true),
                access_key_id,
                secret_access_key,
                session_token: opt_str_field(value, "session_token"),
                path_prefix: str_field(value, "root"),
                ca_cert_pem: opt_str_field(value, "ca_cert"),
                multipart_threshold_bytes: None,
            }))
        }
        Some("ftps") => {
            let host = str_field(value, "host");
            if host.is_empty() {
                return Err(RelayError::Config(
                    "relay FTPS backend missing host".into(),
                ));
            }
            let username = str_field(value, "user");
            if username.is_empty() {
                return Err(RelayError::Config(
                    "relay FTPS backend missing user".into(),
                ));
            }
            let use_ssl = value
                .get("protocol")
                .and_then(|p| p.as_str())
                .map(|p| p == "ftps")
                .unwrap_or(true);
            Ok(BackendConfig::Ftps(FtpsConfig {
                host,
                port: value.get("port").and_then(|p| p.as_u64()).map(|p| p as u16),
                username,
                password: str_field(value, "password"),
                path_prefix: str_field(value, "root"),
                use_ssl,
                ca_cert: None,
                ca_cert_pem: opt_str_field(value, "ca_cert"),
                danger_disable_cert_verify: false,
            }))
        }
        Some(other) => Err(RelayError::Config(format!(
            "unsupported relay backend type '{other}'"
        ))),
        None => Err(RelayError::Config(
            "relay backend record missing 'type'".into(),
        )),
    }
}

fn str_field(value: &serde_json::Value, key: &str) -> String {
    value.get(key).and_then(|v| v.as_str()).unwrap_or("").to_string()
}

fn opt_str_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value.get(key).and_then(|v| v.as_str()).map(str::to_string)
}

/// Storage backend that obtains its credentials from the relay and keeps them
/// in memory for the process lifetime (ADR-007 / ADR-010).
pub struct RelayStorageBackend {
    client: Arc<RelayClient>,
    cache: RwLock<Option<Arc<dyn StorageBackend>>>,
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

    async fn backend(&self) -> Result<Arc<dyn StorageBackend>, StorageError> {
        if let Some(backend) = self.cache.read().await.as_ref() {
            return Ok(Arc::clone(backend));
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
        let config = self
            .client
            .fetch_backend_config(&token)
            .await
            .map_err(|e| e.to_storage())?;
        config
            .check_version()
            .map_err(|e| StorageError::Config(format!("Relay issued invalid backend config: {e}")))?;
        let backend = StorageBackendFactory::build(&config.backend)?;
        self.emit("active", Some("credentials armed")).await;
        *self.cache.write().await = Some(Arc::clone(&backend));
        Ok(backend)
    }
}

#[async_trait]
impl StorageBackend for RelayStorageBackend {
    async fn upload(&self, source_path: &Path, dest_path: &str) -> Result<(), StorageError> {
        self.backend().await?.upload(source_path, dest_path).await
    }

    async fn delete(&self, path: &str) -> Result<(), StorageError> {
        self.backend().await?.delete(path).await
    }

    fn name(&self) -> &str {
        "relay"
    }
}


