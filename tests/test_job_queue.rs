#![cfg(feature = "background-jobs")]

use rftps::job::queue::InMemoryQueue;
use rftps::job::{JobQueue, JobType, JobStatus};

fn test_job() -> rftps::job::Job {
    rftps::job::Job::new(
        JobType::Replication,
        serde_json::json!({"path": "/test.txt"}),
        3,
    )
}

#[tokio::test]
async fn test_push_and_pop() {
    let queue = InMemoryQueue::new(10);
    queue.push(test_job()).await.unwrap();
    let popped = queue.pop().await.unwrap();
    assert!(popped.is_some());
    assert_eq!(popped.unwrap().job_type, JobType::Replication);
}

#[tokio::test]
async fn test_pop_empty_queue() {
    let queue = InMemoryQueue::new(10);
    let result = queue.pop().await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_queue_full() {
    let queue = InMemoryQueue::new(1);
    queue.push(test_job()).await.unwrap();
    let err = queue.push(test_job()).await.unwrap_err();
    assert!(matches!(err, rftps::job::QueueError::Full));
}

#[tokio::test]
async fn test_fifo_order() {
    let queue = InMemoryQueue::new(10);

    let mut job1 = test_job();
    job1.payload = serde_json::json!({"id": 1});
    queue.push(job1).await.unwrap();

    let mut job2 = test_job();
    job2.payload = serde_json::json!({"id": 2});
    queue.push(job2).await.unwrap();

    let popped1 = queue.pop().await.unwrap().unwrap();
    assert_eq!(popped1.payload["id"], 1);

    let popped2 = queue.pop().await.unwrap().unwrap();
    assert_eq!(popped2.payload["id"], 2);
}

#[tokio::test]
async fn test_peek_retryable_empty() {
    let queue = InMemoryQueue::new(10);
    let retryable = queue.peek_retryable().await;
    assert!(retryable.is_empty());
}

#[tokio::test]
async fn test_peek_retryable_with_scheduled_job() {
    use std::time::Duration;

    let queue = InMemoryQueue::new(10);

    let mut retry_job = test_job();
    retry_job.status = JobStatus::RetryScheduled;
    retry_job.metadata.schedule_retry(Duration::from_secs(0));
    queue.push(retry_job).await.unwrap();

    let retryable = queue.peek_retryable().await;
    assert_eq!(retryable.len(), 1);
    assert_eq!(retryable[0].status, JobStatus::RetryScheduled);
}

#[tokio::test]
async fn test_peek_retryable_ignores_pending() {
    let queue = InMemoryQueue::new(10);
    queue.push(test_job()).await.unwrap();

    let retryable = queue.peek_retryable().await;
    assert!(retryable.is_empty());
}
