use super::types::Job;
use async_trait::async_trait;

pub type JobResult = Result<(), JobError>;

#[derive(Debug, thiserror::Error)]
pub enum JobError {
    #[error("Storage error: {0}")]
    Storage(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Transient error: {0} (retryable)")]
    Transient(String),

    #[error("Permanent error: {0}")]
    Permanent(String),
}

impl JobError {
    pub fn is_retryable(&self) -> bool {
        matches!(self, JobError::Transient(_) | JobError::Io(_))
    }
}

#[async_trait]
pub trait JobExecutor: Send + Sync {
    async fn execute(&self, job: &Job) -> JobResult;
    fn name(&self) -> &str;
}

#[async_trait]
pub trait JobQueue: Send + Sync {
    async fn push(&self, job: Job) -> Result<(), QueueError>;
    async fn pop(&self) -> Result<Option<Job>, QueueError>;
    async fn remove(&self, job_id: super::types::JobId) -> Result<(), QueueError>;
    async fn update_status(
        &self,
        job_id: super::types::JobId,
        status: super::types::JobStatus,
    ) -> Result<(), QueueError>;
    async fn peek_retryable(&self) -> Vec<Job>;
}

#[derive(Debug, thiserror::Error)]
pub enum QueueError {
    #[error("Queue full")]
    Full,

    #[error("Queue empty")]
    Empty,

    #[error("Job not found: {0}")]
    NotFound(String),

    #[error("Internal error: {0}")]
    Internal(String),
}
