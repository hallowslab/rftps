use std::{io, fs};
use std::path::{Path, PathBuf};
use std::net::{SocketAddr, UdpSocket};
use rand::{rng, Rng};
use rand::distr::Alphanumeric;

fn get_local_ip () -> io::Result<SocketAddr> {
    // Bind to an arbitrary local port
    let socket: UdpSocket = UdpSocket::bind("0.0.0.0:0")?;
    
    // "Connect" to an external address; no packets are actually sent.
    socket.connect("8.8.8.8:80")?;
    
    // Retrieve the local socket address
    let local_addr: std::net::SocketAddr = socket.local_addr()?;
    return Ok(local_addr) // socket should be closed here https://doc.rust-lang.org/std/net/struct.UdpSocket.html
}

pub fn resolve_local_ip() -> SocketAddr {
    println!("Resolving local address");
    let local_ip = match get_local_ip() {
        Ok(local_ip) => {
            println!("\t=>: {}", local_ip.ip());
            local_ip
        }
        Err(e) => {
            eprintln!("Failed to get local IP address: {}, Terminating...", e);
            // Optionally, handle the error further, like exiting with a non-zero code:
            std::process::exit(1);
        }
    };
    local_ip
}

pub fn verify_home(directory: String) -> PathBuf {
    // Create a new Path object from the provided String
    let user_path = Path::new(&directory);
    // Check path exists, create if not
    if !user_path.exists() {
        println!("Path {} does not exist, creating now", directory);
        if let Err(e) = fs::create_dir_all(user_path) {
            eprintln!("Failed to create directory {}, Error: {}", directory, e);
            std::process::exit(1)
        }
    }

    // If the path does exist check if it's a directory
    if !user_path.is_dir() {
        eprintln!("Path {} is not a directory, you need to specify a directory", user_path.display());
        std::process::exit(1)
    }

    match user_path.canonicalize() {
        Ok(abs_path) => {
            abs_path
        }
        Err(e) => {
            eprintln!("Failed to resolve absolute path: {}, Error: {}", user_path.display(), e);
            std::process::exit(1)
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
