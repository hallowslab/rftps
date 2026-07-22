use rftps::storage::StorageError;

#[test]
fn test_connection_error_is_retryable() {
    assert!(StorageError::Connection("timeout".into()).is_retryable());
}

#[test]
fn test_transfer_error_is_retryable() {
    assert!(StorageError::Transfer("reset".into()).is_retryable());
}

#[test]
fn test_io_error_is_retryable() {
    let err = StorageError::Io(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe"));
    assert!(err.is_retryable());
}

#[test]
fn test_auth_error_not_retryable() {
    assert!(!StorageError::Auth("bad creds".into()).is_retryable());
}

#[test]
fn test_not_found_not_retryable() {
    assert!(!StorageError::NotFound("gone".into()).is_retryable());
}

#[test]
fn test_config_error_not_retryable() {
    assert!(!StorageError::Config("bad host".into()).is_retryable());
}
