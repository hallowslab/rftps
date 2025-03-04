use rftps::verify_home;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn test_existing_directory() {
    let temp_dir = tempfile::tempdir().unwrap();
    let dir_path = temp_dir.path().to_string_lossy().to_string();
    
    // Call verify_home and check the result
    let result: Result<PathBuf, String> = match verify_home(dir_path.clone()) {
        Ok(res) => {
            // Canonicalize the original path to ensure we're comparing absolute paths
            let canonical_dir_path = Path::new(&dir_path).canonicalize().unwrap();
            assert_eq!(res, canonical_dir_path); // Compare canonical paths
            Ok(res)
        },
        Err(err) => Err(err),
    };
}

#[test]
fn test_create_new_directory() {
    let temp_dir = tempfile::tempdir().unwrap();
    let new_dir = temp_dir.path().join("new_directory");
    
    let result = verify_home(new_dir.to_str().unwrap().to_string());
    assert!(result.is_ok());
    assert!(new_dir.exists());
}

#[test]
fn test_not_a_directory() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("file.txt");
    fs::write(&file_path, "test").unwrap();

    let result = verify_home(file_path.to_str().unwrap().to_string());
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("is not a directory"));
}