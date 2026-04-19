use clap::Parser;
use rftps::FtpServer;
use rftps::config::Args;
use tokio::sync::oneshot;

#[tokio::main]
pub async fn main() {
    let args = Args::parse();

    let server = match FtpServer::new(args) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    let (addr, username, password) = server.config();
    let local_ip = rftps::resolve_local_ip()
        .map(|ip| ip.ip().to_string())
        .unwrap_or_else(|_| "Unknown".to_string());

    println!("Server Init");
    println!(
        "Config:\n\tHost: {}\n\tAddress: {}\n\tUsername: {}\n\tPassword: {}\n",
        local_ip, addr, username, password
    );

    // For CLI, we don't have a stop mechanism, but we'll use a channel that never completes
    let (_stop_tx, stop_rx) = oneshot::channel();

    if let Err(e) = server.run(stop_rx).await {
        eprintln!("Error running server: {}", e);
        std::process::exit(1);
    }
}
