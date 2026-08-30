pub mod config;
pub use config::{Args, validate_directory, validate_username};
pub mod utils;
pub use utils::{generate_random_string, resolve_local_ip, validate_certificates, verify_home};
pub mod auth;
pub mod logger;
pub mod event;
pub mod record;

pub use event::FtpEvent;
pub use record::RecordStore;

use libunftp::ServerBuilder;
use unftp_core::auth::DefaultUserDetailProvider;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::oneshot;
use unftp_sbe_fs::Filesystem;

#[cfg(feature = "include-pem-files")]
use std::io::Write;

#[cfg(not(feature = "include-pem-files"))]
use std::path::Path;

#[cfg(feature = "include-pem-files")]
const EMBEDDED_CERT: &[u8] = include_bytes!("../cert.pem");
#[cfg(feature = "include-pem-files")]
const EMBEDDED_KEY: &[u8] = include_bytes!("../key.pem");

pub struct FtpServer {
    addr: SocketAddr,
    user_dir: std::path::PathBuf,
    username: String,
    password: String,
    enable_ftps: bool,
    cert_path: Option<String>,
    key_path: Option<String>,
    passive_ports: Option<(u16, u16)>,
    external_ip: Option<String>,
    #[cfg(feature = "include-pem-files")]
    _temp_certs: Option<(tempfile::NamedTempFile, tempfile::NamedTempFile)>,
    event_bus: Option<event::EventBus>,
    record_store: Option<record::RecordStore>,
}

impl FtpServer {
    pub fn new(args: Args) -> Result<Self, String> {
        let user_dir = utils::verify_home(args.directory).map_err(|e| e.to_string())?;

        let addr: SocketAddr = format!("{}:{}", args.address, args.port)
            .parse()
            .map_err(|e| format!("Failed to parse address: {}", e))?;

        let mut password = args.password.clone();
        if password.is_none() || password.as_ref().unwrap().is_empty() {
            password = Some(utils::generate_random_string(10));
        }

        let enable_ftps = args.enable_ftps.unwrap_or(true);
        let mut cert_path = args.cert_pem;
        let mut key_path = args.key_pem;

        let passive_ports = match args.passive_ports.as_deref() {
            Some(s) => {
                let parts: Vec<&str> = s.split('-').collect();
                if parts.len() == 2 {
                    if let (Ok(a), Ok(b)) = (parts[0].parse::<u16>(), parts[1].parse::<u16>()) {
                        Some((a, b))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            None => None,
        };
        let external_ip = args.external_ip.clone();

        #[cfg(feature = "include-pem-files")]
        let mut _temp_certs = None;

        if enable_ftps {
            let provided_valid = match (&cert_path, &key_path) {
                (Some(c), Some(k)) => validate_certificates(c, k),
                _ => false,
            };

            if !provided_valid {
                #[cfg(feature = "include-pem-files")]
                {
                    println!("Using embedded certificates fallback");
                    let mut t_cert = tempfile::NamedTempFile::new()
                        .map_err(|e| format!("Failed to create temp cert: {}", e))?;
                    let mut t_key = tempfile::NamedTempFile::new()
                        .map_err(|e| format!("Failed to create temp key: {}", e))?;

                    t_cert
                        .write_all(EMBEDDED_CERT)
                        .map_err(|e| format!("Failed to write temp cert: {}", e))?;
                    t_key
                        .write_all(EMBEDDED_KEY)
                        .map_err(|e| format!("Failed to write temp key: {}", e))?;

                    cert_path = Some(t_cert.path().to_string_lossy().into_owned());
                    key_path = Some(t_key.path().to_string_lossy().into_owned());
                    _temp_certs = Some((t_cert, t_key));
                }

                #[cfg(not(feature = "include-pem-files"))]
                {
                    if cert_path.is_none() || key_path.is_none() {
                        if Path::new("cert.pem").exists() && Path::new("key.pem").exists() {
                            cert_path = Some("cert.pem".to_string());
                            key_path = Some("key.pem".to_string());
                        }
                    }
                }
            }
        }

        Ok(Self {
            addr,
            user_dir,
            username: args.username,
            password: password.unwrap(),
            enable_ftps,
            cert_path,
            key_path,
            passive_ports,
            external_ip,
            #[cfg(feature = "include-pem-files")]
            _temp_certs,
            event_bus: None,
            record_store: None,
        })
    }

    pub fn with_event_bus(mut self, bus: event::EventBus) -> Self {
        self.event_bus = Some(bus);
        self
    }

    pub fn with_record_store(mut self, store: record::RecordStore) -> Self {
        self.record_store = Some(store);
        self
    }

    pub fn config(&self) -> (String, String, String) {
        (
            self.addr.to_string(),
            self.username.clone(),
            self.password.clone(),
        )
    }

    pub async fn run(self, mut stop_rx: oneshot::Receiver<()>) -> Result<(), String> {
        let authenticator = auth::StaticAuthenticator {
            username: self.username,
            password: self.password,
        };

        let root = self.user_dir.clone();
        let event_bus = self.event_bus.clone();

        let server_builder = ServerBuilder::new(Box::new(move || {
            Filesystem::new(root.clone()).expect("Failed to create filesystem")
        }))
        .greeting("RFTPS server")
        .passive_ports(50000..=65535)
        .authenticator(Arc::new(authenticator))
        .user_detail_provider(Arc::new(DefaultUserDetailProvider));

        let mut server_builder = server_builder;

        if let Some((start, end)) = self.passive_ports {
            server_builder = server_builder.passive_ports(start..=end);
        }

        if let Some(host) = self.external_ip {
            if let Ok(ip) = host.parse::<std::net::Ipv4Addr>() {
                let o = ip.octets();
                server_builder = server_builder.passive_host([o[0], o[1], o[2], o[3]]);
            } else {
                server_builder = server_builder.passive_host(host.as_str());
            }
        }

        let mut server_builder = server_builder
            .notify_data(logger::DataLogger { event_bus: event_bus.clone() })
            .notify_presence(logger::ConnectionLogger { event_bus });

        if self.enable_ftps {
            if let (Some(cert), Some(key)) = (self.cert_path, self.key_path) {
                println!("FTPS enabled with cert: {} and key: {}", cert, key);
                server_builder = server_builder.ftps(cert, key);
            }
        }

        let server = server_builder
            .build()
            .map_err(|e| format!("Error building FTP server: {}", e))?;

        println!("FTP Server listening on {}", self.addr);

        let addr_str = self.addr.to_string();

        let record_handle = if let (Some(bus), Some(store)) =
            (self.event_bus.as_ref(), self.record_store.as_ref())
        {
            let (_, mut rx) = bus.subscribe();
            let home = self.user_dir.clone();
            let store = store.clone();
            let handle = tokio::spawn(async move {
                while let Some(event) = rx.recv().await {
                    if let event::FtpEvent::FileUploaded { path, .. } = event {
                        let (source, dest) = record::resolve_upload_path(&home, &path);
                        let src = source.to_string_lossy().into_owned();
                        if let Err(e) = store.insert(&src, &dest, &dest) {
                            eprintln!("[Record] failed to record upload {}: {}", path, e);
                        }
                    }
                }
            });
            Some(handle)
        } else {
            None
        };

        let result = tokio::select! {
            result = server.listen(&addr_str) => {
                result.map_err(|e| format!("Error listening: {}", e))
            }
            _ = &mut stop_rx => {
                println!("FTP Server stopping...");
                Ok(())
            }
        };

        if let Some(handle) = record_handle {
            handle.abort();
        }

        result
    }
}
