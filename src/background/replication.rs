use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use async_trait::async_trait;

use crate::event::handlers::EventHandler;
use crate::event::types::FtpEvent;
use crate::job::traits::{JobExecutor, JobError, JobResult};
use crate::job::types::{Job, JobType};
use crate::storage::traits::StorageBackend;

pub struct ReplicationHandler {
    home_dir: PathBuf,
}

impl ReplicationHandler {
    pub fn new(home_dir: PathBuf) -> Self {
        Self { home_dir }
    }

    /// Resolves the on-disk path of an uploaded file under `home_dir`.
    ///
    /// libunftp reports the raw STOR argument in the event path, which is
    /// relative to the client's current working directory (e.g. just the file
    /// name when a client `CWD`s into a subfolder). The file itself was
    /// written to `cwd.join(path)`, so a direct `home_dir.join(path)` misses
    /// nested uploads. We therefore try the direct path first, then search the
    /// tree for a file whose relative path ends with the reported path
    /// (newest mtime wins on ambiguity). Returns the real path plus the
    /// normalized destination path used as the storage key.
    fn resolve_path(&self, raw: &str) -> (PathBuf, String) {
        let rel = raw.trim_start_matches(['/', '\\']).replace('\\', "/");
        let direct = self.home_dir.join(&rel);
        if direct.is_file() {
            return (direct, rel);
        }

        let needle = rel.clone();
        let mut best: Option<(PathBuf, SystemTime)> = None;
        let mut stack = vec![self.home_dir.clone()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if !path.is_file() {
                    continue;
                }
                let relative = path
                    .strip_prefix(&self.home_dir)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                if relative != needle && !relative.ends_with(&format!("/{}", needle)) {
                    continue;
                }
                let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
                let newer = match (&best, mtime) {
                    (None, _) => true,
                    (Some((_, best_time)), Some(time)) => time > *best_time,
                    (Some(_), None) => false,
                };
                if newer {
                    best = Some((path.clone(), mtime.unwrap_or(SystemTime::UNIX_EPOCH)));
                }
            }
        }

        if let Some((path, _)) = best {
            let relative = path
                .strip_prefix(&self.home_dir)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            (path, relative)
        } else {
            (direct, rel)
        }
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
                let (source_path, dest_path) = self.resolve_path(path);

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
    bus: Option<crate::event::EventBus>,
    print_messages: bool,
}

impl ReplicationExecutor {
    pub fn new(
        backend: Arc<dyn StorageBackend>,
        bus: Option<crate::event::EventBus>,
        print_messages: bool,
    ) -> Self {
        Self {
            backend,
            bus,
            print_messages,
        }
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
            let error = format!("Source file not found: {}", source);
            self.emit(false, dest, Some(&error));
            return Err(JobError::Permanent(error));
        }

        match self.backend.upload(&source_path, dest).await {
            Ok(()) => {
                self.emit(true, dest, None);
                if self.print_messages {
                    println!("[Replication] {} → {}", source, dest);
                }
                Ok(())
            }
            Err(e) => {
                let message = e.to_string();
                self.emit(false, dest, Some(&message));
                if e.is_retryable() {
                    Err(JobError::Transient(message))
                } else {
                    Err(JobError::Storage(message))
                }
            }
        }
    }
}

impl ReplicationExecutor {
    fn emit(&self, ok: bool, path: &str, error: Option<&str>) {
        if let Some(bus) = &self.bus {
            bus.publish(&crate::event::FtpEvent::Replication {
                path: path.to_string(),
                ok,
                error: error.map(|e| e.to_string()),
            });
        }
    }
}
