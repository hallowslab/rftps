use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Server address
    #[arg(short, long, default_value = "0.0.0.0")]
    pub address: String,

    /// Port to run the FTP server on
    #[arg(short, long, default_value = "2121")]
    pub port: u16,

    /// Directory where uploaded files will be stored
    #[arg(short, long, value_parser = validate_directory, default_value = "./rftps")]
    pub directory: String,

    /// Username for FTP server
    #[arg(short, long, value_parser = validate_username, default_value= "rftps")]
    pub username: String,

    /// Password for the FTP user
    #[arg(long)]
    pub password: Option<String>,
}

pub fn validate_username(username: &str) -> Result<String, String> {
    // For each? character check if is alphanumeric
    if username.chars().all(|c| c.is_alphanumeric()) {
        Ok(username.to_string()) // Return valid username
    } else {
        Err(String::from("Username must contain only letters and numbers."))
    }
}

pub fn validate_directory(directory: &str) -> Result<String, String> {
    // Statics cannot have dynamic memory allocations and need to be known at compile time,
    // define the arrays and respective sizes, we ignore "/" since the user can specify ./dir which is valid in Windows
    // Validation based on https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file
    static INVALID_CHARS: [&str; 8] = ["<", ">", ":", "\"", "\\", "|", "?", "*"];
    static INVALID_NAMES: [&str; 30] = ["CON", "PRN", "AUX", "NUL", "COM0", "COM1", "COM2", "COM3", "COM4", "COM5",
                                    "COM6", "COM7", "COM8", "COM9", "COM¹", "COM²", "COM³", "LPT0", "LPT1", "LPT2",
                                    "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9", "LPT¹", "LPT²", "LPT³"];
    
    if directory.chars().any(|c| INVALID_CHARS.contains(&c.to_string().as_str())) {
        return Err(format!("Path {} contains invalid characters", directory));
    }

    // Convert directory to uppercase to handle case insensitivity
    let directory_upper = directory.to_uppercase();

    // Check if the directory name contains any reserved names as substrings
    if INVALID_NAMES.iter().any(|&reserved| directory_upper.contains(reserved)) {
        return Err(format!("Path {} contains a reserved name", directory));
    }

    Ok(directory.to_string())
}