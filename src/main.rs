use clap::Parser;
use rftps::FtpServer;
use rftps::config::Args;
use tokio::sync::oneshot;

#[tokio::main]
pub async fn main() {
    let args = Args::parse();
    let record_store_path = args.record_store.clone();

    let mut server = match FtpServer::new(args) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    if let Some(path) = record_store_path {
        let store_path = std::path::PathBuf::from(&path);
        match rftps::RecordStore::open(&store_path) {
            Ok(store) => {
                println!("[Record] Recording uploads to {}", store_path.display());
                server = server.with_record_store(store);
            }
            Err(e) => {
                eprintln!("Error: failed to open record store: {}", e);
                std::process::exit(1);
            }
        }
    }

    let (addr, username, password) = server.config();
    let local_ip = rftps::resolve_local_ip()
        .map(|ip| ip.ip().to_string())
        .unwrap_or_else(|_| "Unknown".to_string());

    println!("Server Init");
    println!(
        "Config:\n\tHost: {}\n\tAddress: {}\n\tUsername: {}\n\tPassword: {}\n",
        local_ip, addr, username, password
    );

    let (_stop_tx, stop_rx) = oneshot::channel();

    if let Err(e) = server.run(stop_rx, None).await {
        eprintln!("Error running server: {}", e);
        std::process::exit(1);
    }
}
