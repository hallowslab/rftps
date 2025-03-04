use std::net::SocketAddr;
use clap::Parser;
use std::sync::Arc;
use unftp_sbe_fs::ServerExt;

pub mod config;
pub use config::{validate_directory,validate_username};
pub mod utils;
mod auth;
//mod logger;

use config::Args;
//use logger::ConnectionLogger;


#[tokio::main]
pub async fn main() {
    let mut args = Args::parse(); // make args mutable to update password value in case it's None
    println!("Bootstrapping");

    // Get the parsed local ip address
    let local_ip = match utils::resolve_local_ip() {
        Ok(local_ip) => local_ip,
        Err(err) => {
            eprintln!("Error: {}", err);
            std::process::exit(1);
        }
    };
    
    // Directory name validation happens in clap, check config.rs
    // Other validations like path exists, is directory and resolve full path happens in utils::verify_home
    // Checks user provided or default directory exists, if not creates it
    let user_dir = match utils::verify_home(args.directory) {
        Ok(path) => path,
        Err(err) => {
            eprintln!("Error: {}", err);
            std::process::exit(1);
        }
    };

    // Username validation happens in clap, check config.rs
    let username = args.username;
    // we use get_or_insert_with to get if exists or modify the value of args.password
    let password = args.password.get_or_insert_with(|| utils::generate_random_string(6));
    let authenticator = Arc::new(auth::StaticAuthenticator {
        username: username.clone(),
        password: password.clone(),
    });

    println!("Server Init");
    let server = match libunftp::Server::with_fs(user_dir)
        .greeting("RFTPS server")
        .passive_ports(50000..65535)
        .authenticator(authenticator)
        //.notify_presence(ConnectionLogger)
        .build() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error building FTP server: {}", e);
                std::process::exit(1);
            }
        };
    
    // Set or default listening address based on address and port from Args
    let addr: SocketAddr = match format!("{}:{}", args.address, args.port).parse() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to parse port ({}), defaulting to 0.0.0.0:21212", e);
            "0.0.0.0:21212".parse().unwrap() // default
        }
    };
    println!("\t=> Listening on {}", addr);

    println!("Config:\n\tHost: {}\n\tPort: {}\n\tUsername: {}\n\tPassword: {}", local_ip.ip().to_string(), args.port, username, password);
    // Start the server and handle errors.
    if let Err(e) = server.listen(&addr.to_string()).await {
        eprintln!("Error listening on port: {}", e);
        std::process::exit(1);
    }
}



