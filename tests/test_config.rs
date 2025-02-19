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

// Test for valid directory
#[test]
fn test_valid_directory() {
    let directory = "my_folder";
    assert_eq!(validate_directory(directory), Ok(directory.to_string()));
}

// Test for directory with invalid characters
#[test]
fn test_directory_with_invalid_characters() {
    let directory = "invalid|folder";
    assert_eq!(validate_directory(directory), Err(format!("Path {} contains invalid characters", directory)));
}

// Test for directory with a reserved name (case-insensitive)
#[test]
fn test_directory_with_reserved_name() {
    let directory = "com1_folder";
    assert_eq!(validate_directory(directory), Err(format!("Path {} contains a reserved name", directory)));
}

// Test for directory with a valid name but uppercased reserved name
#[test]
fn test_directory_with_uppercase_reserved_name() {
    let directory = "COM1_folder";
    assert_eq!(validate_directory(directory), Err(format!("Path {} contains a reserved name", directory)));
}

// Test for directory with an invalid name (exact match with a reserved name)
#[test]
fn test_directory_with_exact_reserved_name() {
    let directory = "CON";
    assert_eq!(validate_directory(directory), Err(format!("Path {} contains a reserved name", directory)));
}

// Test for directory with an invalid name (substring match)
#[test]
fn test_directory_with_reserved_name_as_substring() {
    let directory = "some_COM1_text";
    assert_eq!(validate_directory(directory), Err(format!("Path {} contains a reserved name", directory)));
}