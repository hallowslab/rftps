use std::{io, fs};
use std::path::{Path, PathBuf};
use std::net::{SocketAddr, UdpSocket};
use rand::{rng, Rng};
use rand::distr::Alphanumeric;

fn get_local_ip () -> io::Result<SocketAddr> /*convenience function that bubbles? an io::Result to its caller: */ {
    // Bind to an arbitrary local port
    let socket: UdpSocket = UdpSocket::bind("0.0.0.0:0")?;
    
    // "Connect" to an external address; no packets are actually sent.
    socket.connect("8.8.8.8:80")?;
    
    // Retrieve the local socket address
    let local_addr: std::net::SocketAddr = socket.local_addr()?;
    return Ok(local_addr) // socket should be closed here https://doc.rust-lang.org/std/net/struct.UdpSocket.html
}

pub fn resolve_local_ip() -> Result<SocketAddr, String> {
    println!("Resolving local address");
    
    // Try to get the local IP address
    match get_local_ip() {
        Ok(local_ip) => {
            Ok(local_ip)
        }
        Err(e) => {
            Err(format!("Failed to resolve local IP: {}", e))
        }
    }
}

pub fn verify_home(directory: String) -> Result<PathBuf, String> {
    // Create a new Path object from the provided String
    let user_path = Path::new(&directory);
    // Check path exists, create if not
    if !user_path.exists() {
        println!("Path {} does not exist, creating now", directory);
        if let Err(e) = fs::create_dir_all(user_path) {
            return Err(format!("Failed to create directory {}: {}", directory, e));
        }
    }

    // If the path does exist check if it's a directory
    if !user_path.is_dir() {
        return Err(format!("Path {} is not a directory. Please specify a directory.", user_path.display()));
    }

    match user_path.canonicalize() {
        Ok(abs_path) => {
            Ok(abs_path)
        }
        Err(e) => {
            return Err(format!("Failed to resolve absolute path for {}: {}", user_path.display(), e));
        }
    }

    
}

pub fn generate_random_string(length: usize) -> String {
    rng()
        .sample_iter(&Alphanumeric)  // Generate random characters
        .take(length)                // Limit to the desired length
        .map(char::from)             // Convert bytes to chars
        .collect()                    // Collect into a String
}

pub fn validate_certificates(cert: &String, key: &String) -> bool {
    let cert_path = Path::new(&cert);
    let key_path = Path::new(&key);

    match (cert_path.is_file(),key_path.is_file()) {
        (false, _) => {
            eprintln!("Certificate is not a valid file: {:?}", cert_path);
        }
        (_, false) => {
            eprintln!("Key is not a valid file: {:?}", key_path);
        }
        _ => (),
    };

    return true
}