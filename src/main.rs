use std::net::SocketAddr;
use clap::Parser;
use std::sync::Arc;
use unftp_sbe_fs::ServerExt;

mod config;
mod utils;
mod auth;
//mod logger;

use config::Args;
//use logger::ConnectionLogger;


#[tokio::main]
pub async fn main() {
    println!("Bootstrapping");
    let args = Args::parse();

    // Get the parsed local ip address
    let local_ip = utils::resolve_local_ip();
    
    // TODO: Make sure the user didn't provide characters that can't be used in a directory
    // Ideally we also check that the directory does not use any reserved name like CON or AUX
    // https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file
    // Check user provided or default directory exists, if not create it
    let user_dir = utils::verify_home(args.directory);

    // TODO: Enforce alphanumerical username on clap or validate here
    let username = args.username;
    // TODO: Use user password if provided or generate random one
    let password = utils::generate_random_string(6);
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
            eprintln!("Failed to parse port ({}), defaulting to 0.0.0.0:2121", e);
            "0.0.0.0:2121".parse().unwrap() // default
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



