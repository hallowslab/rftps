use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct JobId(pub u64);

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum JobType {
    Replication,
    VirusScan,
    ThumbnailGeneration,
    MetadataIndexing,
    Notification,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    RetryScheduled,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JobMetadata {
    pub attempt_count: u32,
    pub max_retries: u32,
    pub next_retry_at: Option<SystemTime>,
    pub last_error: Option<String>,
}

impl JobMetadata {
    pub fn new(max_retries: u32) -> Self {
        Self {
            attempt_count: 0,
            max_retries,
            next_retry_at: None,
            last_error: None,
        }
    }

    pub fn can_retry(&self) -> bool {
        self.attempt_count < self.max_retries
    }

    pub fn record_attempt(&mut self, error: Option<String>) {
        self.attempt_count += 1;
        self.last_error = error;
    }

    pub fn schedule_retry(&mut self, delay: std::time::Duration) {
        self.next_retry_at = Some(SystemTime::now() + delay);
    }

    pub fn is_ready_for_retry(&self) -> bool {
        match self.next_retry_at {
            Some(t) => SystemTime::now() >= t,
            None => false,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Job {
    pub id: JobId,
    pub job_type: JobType,
    pub status: JobStatus,
    pub metadata: JobMetadata,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
    pub payload: serde_json::Value,
}

impl Job {
    pub fn new(job_type: JobType, payload: serde_json::Value, max_retries: u32) -> Self {
        let now = SystemTime::now();
        Self {
            id: JobId(0),
            job_type,
            status: JobStatus::Pending,
            metadata: JobMetadata::new(max_retries),
            created_at: now,
            updated_at: now,
            payload,
        }
    }
}
