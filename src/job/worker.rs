use super::traits::{JobExecutor, JobQueue, JobResult};
use super::types::{JobId, JobStatus};
use std::sync::Arc;
use tokio::sync::Notify;

pub struct Worker {
    id: usize,
    queue: Arc<dyn JobQueue>,
    executors: Arc<Vec<Box<dyn JobExecutor>>>,
    shutdown: Arc<Notify>,
}

impl Worker {
    pub fn new(
        id: usize,
        queue: Arc<dyn JobQueue>,
        executors: Arc<Vec<Box<dyn JobExecutor>>>,
        shutdown: Arc<Notify>,
    ) -> Self {
        Self {
            id,
            queue,
            executors,
            shutdown,
        }
    }

    pub async fn run(&self) {
        loop {
            tokio::select! {
                _ = self.shutdown.notified() => {
                    println!("[Worker {}] shutting down", self.id);
                    break;
                }
                result = self.queue.pop() => {
                    match result {
                        Ok(Some(mut job)) => {
                            println!("[Worker {}] processing job {} ({:?})", self.id, job.id, job.job_type);
                            job.status = JobStatus::Running;
                            let _ = self.queue.update_status(job.id, JobStatus::Running).await;

                            let job_type_name = format!("{:?}", job.job_type).to_lowercase();
                            let executor = self.executors.iter().find(|e| {
                                e.name() == job_type_name
                            });

                            let result = if let Some(executor) = executor {
                                executor.execute(&job).await
                            } else {
                                Err(super::traits::JobError::Config(
                                    format!("No executor for {:?}", job.job_type)
                                ))
                            };

                            self.handle_result(&job.id, result).await;
                        }
                        Ok(None) => {
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        }
                        Err(e) => {
                            eprintln!("[Worker {}] queue error: {}", self.id, e);
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        }
                    }
                }
            }
        }
    }

    async fn handle_result(&self, job_id: &JobId, result: JobResult) {
        match result {
            Ok(()) => {
                println!("[Worker {}] job {} completed", self.id, job_id);
                let _ = self.queue.update_status(*job_id, JobStatus::Completed).await;
            }
            Err(e) => {
                if e.is_retryable() {
                    println!("[Worker {}] job {} failed (retryable): {}", self.id, job_id, e);
                    let _ = self.queue.update_status(*job_id, JobStatus::RetryScheduled).await;
                } else {
                    eprintln!("[Worker {}] job {} failed (permanent): {}", self.id, job_id, e);
                    let _ = self.queue.update_status(*job_id, JobStatus::Failed).await;
                }
            }
        }
    }
}

pub struct WorkerPool {
    workers: Vec<Worker>,
    shutdown: Arc<Notify>,
}

impl WorkerPool {
    pub fn new(
        count: usize,
        queue: Arc<dyn JobQueue>,
        executors: Arc<Vec<Box<dyn JobExecutor>>>,
    ) -> Self {
        let shutdown = Arc::new(Notify::new());
        let workers: Vec<Worker> = (0..count)
            .map(|i| Worker::new(i, Arc::clone(&queue), Arc::clone(&executors), Arc::clone(&shutdown)))
            .collect();

        Self { workers, shutdown }
    }

    pub async fn run(&self) {
        let handles: Vec<_> = self.workers
            .iter()
            .map(|w| {
                let worker = Worker {
                    id: w.id,
                    queue: Arc::clone(&w.queue),
                    executors: Arc::clone(&w.executors),
                    shutdown: Arc::clone(&w.shutdown),
                };
                tokio::spawn(async move { worker.run().await })
            })
            .collect();

        for handle in handles {
            let _ = handle.await;
        }
    }

    pub fn shutdown(&self) {
        self.shutdown.notify_waiters();
    }
}
