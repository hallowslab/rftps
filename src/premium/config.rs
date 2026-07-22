use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BackgroundJobConfig {
    pub enabled: bool,
    pub max_parallel_jobs: usize,
    pub retry_delay_base: Duration,
    pub max_retries: u32,
    pub queue_capacity: usize,
    pub remote_storage: Option<RemoteStorageConfig>,
    pub user_mapping: UserMapping,
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
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RemoteStorageConfig {
    pub backend: StorageBackendType,
    pub host: String,
    pub port: Option<u16>,
    pub username: String,
    pub password: String,
    pub path_prefix: String,
    pub use_ssl: bool,
    pub ca_cert: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum StorageBackendType {
    Ftps,
    Sftp,
    Https,
    S3,
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
