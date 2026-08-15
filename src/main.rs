use clap::Parser;
use rftps::FtpServer;
use rftps::config::Args;
use tokio::sync::oneshot;

#[tokio::main]
pub async fn main() {
    #[cfg(feature = "broker")]
    let argv: Vec<String> = std::env::args().collect();
    #[cfg(not(feature = "broker"))]
    let _argv: Vec<String> = Vec::new();

    #[cfg(feature = "broker")]
    {
        let _ = rustls::crypto::ring::default_provider().install_default();
        if argv.len() >= 3 && argv[1] == "broker" && argv[2] == "keygen" {
            use ed25519_dalek::SigningKey;
            use rand_core::OsRng;
            let key = SigningKey::generate(&mut OsRng);
            println!("{}", hex::encode(key.to_bytes()));
            return;
        }
        if argv.len() >= 3 && argv[1] == "broker" && argv[2] == "init" {
            run_broker_init(&argv);
            return;
        }
        if argv.len() >= 4 && argv[1] == "broker" && argv[2] == "pubkey" {
            use ed25519_dalek::SigningKey;
            let bytes = match hex::decode(argv[3].trim()) {
                Ok(b) => b,
                Err(_) => {
                    eprintln!("invalid device key: must be 64 hex chars");
                    std::process::exit(1);
                }
            };
            if bytes.len() != 32 {
                eprintln!("invalid device key: must be exactly 32 bytes");
                std::process::exit(1);
            }
            let mut seed = [0u8; 32];
            seed.copy_from_slice(&bytes);
            let signing = SigningKey::from_bytes(&seed);
            println!("{}", hex::encode(signing.verifying_key().to_bytes()));
            return;
        }
    }

    let args = Args::parse();

    #[cfg(feature = "background-jobs")]
    let config_path = args.config.clone();

    #[cfg(feature = "background-jobs")]
    let mut server = match FtpServer::new(args) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    #[cfg(not(feature = "background-jobs"))]
    let server = match FtpServer::new(args) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    #[cfg(feature = "background-jobs")]
    if let Some(ref config_path) = config_path {
        match rftps::background::BackgroundJobConfig::load_from_file(config_path) {
            Ok(config) => {
                if config.enabled {
                    println!("[Background] Background jobs enabled from config: {}", config_path);
                    let bus = rftps::event::EventBus::new();
                    server = server
                        .with_event_bus(bus)
                        .with_background_config(config);
                } else {
                    println!("[Background] Config loaded but background jobs disabled");
                }
            }
            Err(e) => {
                eprintln!("Warning: {}", e);
                eprintln!("[Background] Running without background jobs");
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

    if let Err(e) = server.run(stop_rx).await {
        eprintln!("Error running server: {}", e);
        std::process::exit(1);
    }
}

#[cfg(feature = "broker")]
fn run_broker_init(argv: &[String]) {
    use std::io::Write as _;

    fn prompt(label: &str, default: Option<&str>) -> String {
        print!("{}", label);
        let _ = std::io::stdout().flush();
        let mut line = String::new();
        std::io::stdin()
            .read_line(&mut line)
            .expect("failed to read input");
        let value = line.trim().to_string();
        if value.is_empty() {
            default.unwrap_or("").to_string()
        } else {
            value
        }
    }

    let mut output = "bg.json".to_string();
    let mut force = false;
    let mut i = 3;
    while i < argv.len() {
        match argv[i].as_str() {
            "--output" | "-o" => {
                i += 1;
                if i < argv.len() {
                    output = argv[i].clone();
                }
            }
            "--force" | "-f" => force = true,
            other => {
                eprintln!("unknown arg: {}", other);
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let device_key = {
        use ed25519_dalek::SigningKey;
        use rand_core::OsRng;
        hex::encode(SigningKey::generate(&mut OsRng).to_bytes())
    };

    let url = prompt("broker url [http://127.0.0.1:8700]: ", Some("http://127.0.0.1:8700"));
    if url.is_empty() {
        eprintln!("broker url is required");
        std::process::exit(1);
    }

    let hostname = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "rftps".into());
    let device_name = prompt(&format!("device name [{}]: ", hostname), Some(&hostname));

    let timeout_raw = prompt("approval timeout (seconds) [1800]: ", Some("1800"));
    let approval_timeout_secs: u64 = timeout_raw.trim().parse().unwrap_or(1800);

    let ca_cert = prompt("ca cert file for broker TLS [none]: ", Some("none"));
    let ca_cert = if ca_cert.is_empty() || ca_cert.eq_ignore_ascii_case("none") {
        None
    } else {
        Some(ca_cert)
    };

    let danger_raw = prompt("disable broker cert verification? [n]: ", Some("n"));
    let danger = danger_raw.eq_ignore_ascii_case("y") || danger_raw.eq_ignore_ascii_case("yes");
    if danger {
        eprintln!("WARNING: broker TLS certificate verification will be disabled");
    }

    let messages_raw = prompt("print broker messages? [y]: ", Some("y"));
    let broker_messages = messages_raw.eq_ignore_ascii_case("y")
        || messages_raw.eq_ignore_ascii_case("yes")
        || messages_raw.is_empty();

    let config = serde_json::json!({
        "enabled": true,
        "max_parallel_jobs": 2,
        "queue_capacity": 1000,
        "broker": {
            "url": url,
            "device_key": device_key,
            "device_name": device_name,
            "approval_timeout_secs": approval_timeout_secs,
            "ca_cert": ca_cert,
            "danger_disable_cert_verify": danger,
            "broker_messages": broker_messages,
        },
    });

    let path = std::path::Path::new(&output);
    if path.exists() && !force {
        let answer = prompt(&format!("{} already exists. Overwrite? [y/N]: ", output), Some("n"));
        if !(answer.eq_ignore_ascii_case("y") || answer.eq_ignore_ascii_case("yes")) {
            println!("aborted");
            return;
        }
    }

    let pretty = serde_json::to_string_pretty(&config).expect("failed to serialize config");
    if let Err(e) = std::fs::write(&output, pretty) {
        eprintln!("failed to write {}: {}", output, e);
        std::process::exit(1);
    }
    println!("Wrote {}", output);
    println!("Register the device in the broker UI at http://127.0.0.1:8701/dashboard");
}
