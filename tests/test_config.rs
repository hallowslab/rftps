// tests/test_config.rs
use rftps::config::{validate_username, validate_directory};


#[test]
fn test_valid_username() {
    let username = "valid123";
    assert_eq!(validate_username(username), Ok(username.to_string()));
}

// Test for invalid username (contains special characters)
#[test]
fn test_invalid_username_special_characters() {
    let username = "invalid@username";
    assert_eq!(validate_username(username), Err("Username must contain only letters and numbers.".to_string()));
}

// Test for invalid username (contains spaces)
#[test]
fn test_invalid_username_with_space() {
    let username = "invalid username";
    assert_eq!(validate_username(username), Err("Username must contain only letters and numbers.".to_string()));
}

#[test]
fn test_valid_directory() {
    assert_eq!(validate_directory("valid_directory"), Ok("valid_directory".to_string()));
    assert_eq!(validate_directory("my-folder_123"), Ok("my-folder_123".to_string()));
    assert_eq!(validate_directory("./valid/dir"), Ok("./valid/dir".to_string()));
    assert_eq!(validate_directory("C:/Users/ValidDir"), Ok("C:/Users/ValidDir".to_string()));
}

#[test]
fn test_invalid_characters() {
    assert!(validate_directory("invalid|dir").is_err());
    assert!(validate_directory("invalid?dir").is_err());
    assert!(validate_directory("invalid*dir").is_err());
    assert!(validate_directory("<invalid>").is_err());
    assert!(validate_directory("\"quotes\"").is_err());
}

#[test]
fn test_reserved_names() {
    assert!(validate_directory("CON").is_err());
    assert!(validate_directory("com1").is_err()); // Case insensitive
    assert!(validate_directory("lpt3").is_err());
    // TODO: Fix validation
    // Should fail because it contains a reserved name, however this is incorrect I belive it should only fail with extensions like AUX.txt since it matches AUX
    // https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file
    assert!(validate_directory("AUX_folder").is_err());
    assert!(validate_directory("backup_LPT2").is_err()); // Should fail because it contains LPT2
}

#[test]
fn test_case_insensitive_reserved_names() {
    assert!(validate_directory("con").is_err()); // Lowercase should still be invalid
    assert!(validate_directory("CoM5").is_err()); // Mixed case should still be invalid
    assert!(validate_directory("lPt7").is_err());
}

#[test]
fn test_valid_special_cases() {
    assert_eq!(validate_directory("normal-folder"), Ok("normal-folder".to_string()));
    assert_eq!(validate_directory("123456789"), Ok("123456789".to_string()));
    assert_eq!(validate_directory("folder.name"), Ok("folder.name".to_string()));
    assert_eq!(validate_directory("dir_with_underscore"), Ok("dir_with_underscore".to_string()));
}