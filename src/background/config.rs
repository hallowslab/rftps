use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct BackgroundJobConfig {
    pub enabled: bool,
    pub max_parallel_jobs: usize,
    pub retry_delay_base: Duration,
    pub max_retries: u32,
    pub queue_capacity: usize,
    pub remote_storage: Option<RemoteStorageConfig>,
    pub user_mapping: UserMapping,
    #[cfg(feature = "relay")]
    pub relay: Option<RelayConfig>,
}

impl Default for BackgroundJobConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_parallel_jobs: 2,
            retry_delay_base: Duration::from_secs(5),
            max_retries: 3,
            queue_capacity: 1000,
            remote_storage: None,
            user_mapping: UserMapping::PrefixUserName,
            #[cfg(feature = "relay")]
            relay: None,
        }
    }
}

impl BackgroundJobConfig {
    pub fn load_from_file(path: &str) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read config file '{}': {}", path, e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse config '{}': {}", path, e))
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct RemoteStorageConfig {
    pub backend: StorageBackendType,
    pub host: String,
    pub port: Option<u16>,
    pub username: String,
    pub password: String,
    pub path_prefix: String,
    pub use_ssl: bool,
    pub ca_cert: Option<String>,
    pub ca_cert_pem: Option<String>,
    pub danger_disable_cert_verify: bool,
}

impl Default for RemoteStorageConfig {
    fn default() -> Self {
        Self {
            backend: StorageBackendType::Ftps,
            host: String::new(),
            port: None,
            username: String::new(),
            password: String::new(),
            path_prefix: String::new(),
            use_ssl: false,
            ca_cert: None,
            ca_cert_pem: None,
            danger_disable_cert_verify: false,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum StorageBackendType {
    Ftps,
    Sftp,
    Https,
    S3,
}

pub const BACKEND_CONFIG_VERSION: u32 = 1;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BackendConfig {
    Ftps(FtpsConfig),
    S3(S3Config),
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct FtpsConfig {
    pub host: String,
    pub port: Option<u16>,
    pub username: String,
    pub password: String,
    pub path_prefix: String,
    pub use_ssl: bool,
    pub ca_cert: Option<String>,
    pub ca_cert_pem: Option<String>,
    pub danger_disable_cert_verify: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct S3Config {
    pub endpoint: String,
    pub region: Option<String>,
    pub bucket: String,
    pub path_style: bool,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub session_token: Option<String>,
    pub path_prefix: String,
    pub ca_cert_pem: Option<String>,
}

impl Default for S3Config {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            region: None,
            bucket: String::new(),
            path_style: true,
            access_key_id: String::new(),
            secret_access_key: String::new(),
            session_token: None,
            path_prefix: String::new(),
            ca_cert_pem: None,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VersionedBackendConfig {
    pub version: u32,
    pub backend: BackendConfig,
}

impl VersionedBackendConfig {
    pub fn new(backend: BackendConfig) -> Self {
        Self {
            version: BACKEND_CONFIG_VERSION,
            backend,
        }
    }

    pub fn check_version(&self) -> Result<(), String> {
        if self.version > BACKEND_CONFIG_VERSION {
            Err(format!(
                "backend config version {} is newer than supported version {}",
                self.version, BACKEND_CONFIG_VERSION
            ))
        } else if self.version == 0 {
            Err("backend config version must be at least 1".into())
        } else {
            Ok(())
        }
    }
}

impl TryFrom<RemoteStorageConfig> for BackendConfig {
    type Error = String;

    fn try_from(legacy: RemoteStorageConfig) -> Result<Self, Self::Error> {
        match legacy.backend {
            StorageBackendType::Ftps => Ok(BackendConfig::Ftps(FtpsConfig {
                host: legacy.host,
                port: legacy.port,
                username: legacy.username,
                password: legacy.password,
                path_prefix: legacy.path_prefix,
                use_ssl: legacy.use_ssl,
                ca_cert: legacy.ca_cert,
                ca_cert_pem: legacy.ca_cert_pem,
                danger_disable_cert_verify: legacy.danger_disable_cert_verify,
            })),
            other => Err(format!(
                "legacy remote_storage shape cannot represent backend type {:?}; use a versioned backend config",
                other
            )),
        }
    }
}

#[cfg(feature = "relay")]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct RelayConfig {
    pub url: String,
    pub device_key: String,
    pub device_name: String,
    pub approval_timeout_secs: u64,
    pub ca_cert: Option<String>,
    pub danger_disable_cert_verify: bool,
    pub relay_messages: bool,
}

#[cfg(feature = "relay")]
impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            device_key: String::new(),
            device_name: "rftps".into(),
            approval_timeout_secs: 1800,
            ca_cert: None,
            danger_disable_cert_verify: false,
            relay_messages: true,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum UserMapping {
    PrefixUserName,
    FixedPath(String),
    UserMap(BTreeMap<String, String>),
}

impl UserMapping {
    pub fn resolve(&self, username: &str, path: &str) -> String {
        match self {
            UserMapping::PrefixUserName => format!("{}/{}", username, path),
            UserMapping::FixedPath(prefix) => format!("{}/{}", prefix, path),
            UserMapping::UserMap(map) => {
                let mapped = map.get(username).map(|s| s.as_str()).unwrap_or(username);
                format!("{}/{}", mapped, path)
            }
        }
    }
}
