use super::traits::{JobExecutor, JobQueue};
use crate::event::handlers::HandlerRegistry;
use crate::event::types::FtpEvent;
use std::sync::Arc;
use tokio::sync::Notify;

pub struct JobScheduler {
    queue: Arc<dyn JobQueue>,
    handlers: HandlerRegistry,
    max_retries: u32,
    retry_base_delay: std::time::Duration,
    shutdown: Arc<Notify>,
}

impl JobScheduler {
    pub fn new(
        queue: Arc<dyn JobQueue>,
        handlers: HandlerRegistry,
        _executors: Arc<Vec<Box<dyn JobExecutor>>>,
        max_retries: u32,
        retry_base_delay: std::time::Duration,
    ) -> Self {
        Self {
            queue,
            handlers,
            max_retries,
            retry_base_delay,
            shutdown: Arc::new(Notify::new()),
        }
    }

    pub async fn process_event(&self, event: &FtpEvent) {
        let jobs = self.handlers.dispatch(event).await;
        for mut job in jobs {
            job.metadata.max_retries = self.max_retries;
            if let Err(e) = self.queue.push(job).await {
                eprintln!("Failed to enqueue job: {}", e);
            }
        }
    }

    pub async fn run_retry_loop(&self) {
        loop {
            tokio::select! {
                _ = self.shutdown.notified() => break,
                _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                    let retryable = self.queue.peek_retryable().await;
                    for mut job in retryable {
                        println!("Retrying job {}", job.id);
                        job.metadata.record_attempt(None);
                        let delay = self.retry_base_delay * 2u32.pow(job.metadata.attempt_count - 1);
                        job.metadata.schedule_retry(delay);
                        let _ = self.queue.push(job).await;
                    }
                }
            }
        }
    }

    pub fn shutdown(&self) {
        self.shutdown.notify_waiters();
    }

    pub fn queue(&self) -> &Arc<dyn JobQueue> {
        &self.queue
    }
}
