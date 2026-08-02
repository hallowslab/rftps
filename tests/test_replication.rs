#![cfg(feature = "background-jobs")]

use rftps::background::{ReplicationHandler, ReplicationExecutor};
use rftps::event::{EventHandler, FtpEvent};
use rftps::job::{JobExecutor, JobType, JobError};
use rftps::storage::StorageBackend;
use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

struct MockStorageBackend;

#[async_trait]
impl StorageBackend for MockStorageBackend {
    async fn upload(&self, _source: &Path, _dest: &str) -> Result<(), rftps::storage::StorageError> {
        Ok(())
    }
    async fn delete(&self, _path: &str) -> Result<(), rftps::storage::StorageError> {
        Ok(())
    }
    fn name(&self) -> &str {
        "mock"
    }
}

struct FailingStorageBackend;

#[async_trait]
impl StorageBackend for FailingStorageBackend {
    async fn upload(&self, _source: &Path, _dest: &str) -> Result<(), rftps::storage::StorageError> {
        Err(rftps::storage::StorageError::Connection("mock failure".into()))
    }
    async fn delete(&self, _path: &str) -> Result<(), rftps::storage::StorageError> {
        Ok(())
    }
    fn name(&self) -> &str {
        "failing"
    }
}

#[test]
fn test_handler_interested_in_upload() {
    let handler = ReplicationHandler::new("/home/test".into());
    let event = FtpEvent::FileUploaded {
        username: "alice".into(),
        path: "/photos/pic.jpg".into(),
        timestamp: SystemTime::now(),
    };
    assert!(handler.interested_in(&event));
}

#[test]
fn test_handler_not_interested_in_download() {
    let handler = ReplicationHandler::new("/home/test".into());
    let event = FtpEvent::FileDownloaded {
        username: "alice".into(),
        path: "/photos/pic.jpg".into(),
    };
    assert!(!handler.interested_in(&event));
}

#[tokio::test]
async fn test_handler_creates_replication_job() {
    let handler = ReplicationHandler::new("/home/test".into());
    let event = FtpEvent::FileUploaded {
        username: "alice".into(),
        path: "/photos/pic.jpg".into(),
        timestamp: SystemTime::now(),
    };

    let job = handler.handle(&event).await.unwrap();
    assert_eq!(job.job_type, JobType::Replication);
    assert_eq!(job.payload["username"], "alice");
    assert_eq!(job.payload["dest_path"], "photos/pic.jpg");
    assert!(job.payload["source_path"].as_str().unwrap().contains("photos/pic.jpg"));
}

#[tokio::test]
async fn test_executor_uploads_file() {
    let executor = ReplicationExecutor::new(Arc::new(MockStorageBackend), true);

    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("test.txt");
    std::fs::write(&file_path, "hello").unwrap();

    let job = rftps::job::Job::new(
        JobType::Replication,
        serde_json::json!({
            "source_path": file_path.to_string_lossy(),
            "dest_path": "backup/test.txt",
            "username": "alice",
        }),
        3,
    );

    let result = executor.execute(&job).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_executor_fails_on_missing_source() {
    let executor = ReplicationExecutor::new(Arc::new(MockStorageBackend), true);

    let job = rftps::job::Job::new(
        JobType::Replication,
        serde_json::json!({
            "source_path": "/nonexistent/file.txt",
            "dest_path": "backup/file.txt",
            "username": "alice",
        }),
        3,
    );

    let result = executor.execute(&job).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        JobError::Permanent(msg) => assert!(msg.contains("not found")),
        other => panic!("Expected Permanent error, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_executor_storage_error_is_retryable() {
    let executor = ReplicationExecutor::new(Arc::new(FailingStorageBackend), true);

    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("test.txt");
    std::fs::write(&file_path, "hello").unwrap();

    let job = rftps::job::Job::new(
        JobType::Replication,
        serde_json::json!({
            "source_path": file_path.to_string_lossy(),
            "dest_path": "backup/test.txt",
            "username": "alice",
        }),
        3,
    );

    let result = executor.execute(&job).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().is_retryable());
}

#[tokio::test]
async fn test_executor_missing_payload_field() {
    let executor = ReplicationExecutor::new(Arc::new(MockStorageBackend), true);

    let job = rftps::job::Job::new(
        JobType::Replication,
        serde_json::json!({"username": "alice"}),
        3,
    );

    let result = executor.execute(&job).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        JobError::Config(msg) => assert!(msg.contains("Missing")),
        other => panic!("Expected Config error, got: {:?}", other),
    }
}
