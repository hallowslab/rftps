use std::sync::Arc;

use async_trait::async_trait;

use crate::event::handlers::EventHandler;
use crate::event::types::FtpEvent;
use crate::job::traits::{JobExecutor, JobError, JobResult};
use crate::job::types::{Job, JobType};
use crate::storage::traits::StorageBackend;

pub struct ReplicationHandler {
    home_dir: std::path::PathBuf,
    _backend: Arc<dyn StorageBackend>,
}

impl ReplicationHandler {
    pub fn new(home_dir: std::path::PathBuf, backend: Arc<dyn StorageBackend>) -> Self {
        Self { home_dir, _backend: backend }
    }
}

#[async_trait]
impl EventHandler for ReplicationHandler {
    fn name(&self) -> &str {
        "replication"
    }

    fn interested_in(&self, event: &FtpEvent) -> bool {
        matches!(event, FtpEvent::FileUploaded { .. })
    }

    async fn handle(&self, event: &FtpEvent) -> Option<Job> {
        match event {
            FtpEvent::FileUploaded {
                username, path, ..
            } => {
                let source_path = self.home_dir.join(path.trim_start_matches('/'));
                let dest_path = path.trim_start_matches('/').to_string();

                let payload = serde_json::json!({
                    "source_path": source_path.to_string_lossy(),
                    "dest_path": dest_path,
                    "username": username,
                });

                Some(Job::new(JobType::Replication, payload, 3))
            }
            _ => None,
        }
    }
}

pub struct ReplicationExecutor {
    backend: Arc<dyn StorageBackend>,
}

impl ReplicationExecutor {
    pub fn new(backend: Arc<dyn StorageBackend>) -> Self {
        Self { backend }
    }
}

#[async_trait]
impl JobExecutor for ReplicationExecutor {
    fn name(&self) -> &str {
        "replication"
    }

    async fn execute(&self, job: &Job) -> JobResult {
        let source = job
            .payload
            .get("source_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JobError::Config("Missing source_path in payload".into()))?;

        let dest = job
            .payload
            .get("dest_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JobError::Config("Missing dest_path in payload".into()))?;

        let source_path = std::path::PathBuf::from(source);

        if !source_path.exists() {
            return Err(JobError::Permanent(format!(
                "Source file not found: {}",
                source
            )));
        }

        self.backend
            .upload(&source_path, dest)
            .await
            .map_err(|e| {
                if e.is_retryable() {
                    JobError::Transient(e.to_string())
                } else {
                    JobError::Storage(e.to_string())
                }
            })?;

        println!("[Replication] {} → {}", source, dest);
        Ok(())
    }
}
