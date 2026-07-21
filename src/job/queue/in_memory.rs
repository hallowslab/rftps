use crate::job::traits::{JobQueue, QueueError};
use crate::job::types::{Job, JobId, JobStatus};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct InMemoryQueue {
    jobs: Arc<Mutex<VecDeque<Job>>>,
    max_size: usize,
}

impl InMemoryQueue {
    pub fn new(max_size: usize) -> Self {
        Self {
            jobs: Arc::new(Mutex::new(VecDeque::with_capacity(max_size))),
            max_size,
        }
    }
}

#[async_trait::async_trait]
impl JobQueue for InMemoryQueue {
    async fn push(&self, mut job: Job) -> Result<(), QueueError> {
        let mut jobs = self.jobs.lock().await;
        if jobs.len() >= self.max_size {
            return Err(QueueError::Full);
        }
        let id = JobId(jobs.len() as u64 + 1);
        job.id = id;
        jobs.push_back(job);
        Ok(())
    }

    async fn pop(&self) -> Result<Option<Job>, QueueError> {
        let mut jobs = self.jobs.lock().await;
        Ok(jobs.pop_front())
    }

    async fn remove(&self, job_id: JobId) -> Result<(), QueueError> {
        let mut jobs = self.jobs.lock().await;
        let len = jobs.len();
        jobs.retain(|j| j.id != job_id);
        if jobs.len() == len {
            return Err(QueueError::NotFound(job_id.to_string()));
        }
        Ok(())
    }

    async fn update_status(
        &self,
        job_id: JobId,
        status: JobStatus,
    ) -> Result<(), QueueError> {
        let mut jobs = self.jobs.lock().await;
        for job in jobs.iter_mut() {
            if job.id == job_id {
                job.status = status;
                job.updated_at = std::time::SystemTime::now();
                return Ok(());
            }
        }
        Err(QueueError::NotFound(job_id.to_string()))
    }

    async fn peek_retryable(&self) -> Vec<Job> {
        let jobs = self.jobs.lock().await;
        jobs.iter()
            .filter(|j| j.status == JobStatus::RetryScheduled && j.metadata.is_ready_for_retry())
            .cloned()
            .collect()
    }
}
