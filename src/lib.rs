pub mod config;
pub use config::{Args, validate_directory, validate_username};
pub mod utils;
pub use utils::{generate_random_string, resolve_local_ip, validate_certificates, verify_home};
pub mod auth;
pub mod logger;

#[cfg(feature = "background-jobs")]
pub mod event;
#[cfg(feature = "background-jobs")]
pub mod job;

#[cfg(feature = "background-jobs")]
pub use event::FtpEvent;
#[cfg(not(feature = "background-jobs"))]
#[derive(Debug, Clone, serde::Serialize)]
pub enum FtpEvent {
    LoggedIn { username: String },
    LoggedOut { username: String },
    FileUpload { username: String, path: String },
    FileDownload { username: String, path: String },
    DirCreated { username: String, path: String },
    DirRemoved { username: String, path: String },
    Renamed { username: String, from: String, to: String },
    Deleted { username: String, path: String },
}

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
    #[cfg(feature = "include-pem-files")]
    _temp_certs: Option<(tempfile::NamedTempFile, tempfile::NamedTempFile)>,
    #[cfg(not(feature = "background-jobs"))]
    event_tx: Option<tokio::sync::mpsc::UnboundedSender<FtpEvent>>,
    #[cfg(feature = "background-jobs")]
    event_bus: Option<event::EventBus>,
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

        #[cfg(feature = "include-pem-files")]
        let mut _temp_certs = None;

        if enable_ftps {
            // Priority 1: Check if provided paths exist
            let provided_valid = match (&cert_path, &key_path) {
                (Some(c), Some(k)) => validate_certificates(c, k),
                _ => false,
            };

            if !provided_valid {
                // Priority 2: Try embedded fallback if feature enabled
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

                // Priority 3: Try local files cert.pem/key.pem in CWD if still no paths
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
            #[cfg(feature = "include-pem-files")]
            _temp_certs,
            #[cfg(not(feature = "background-jobs"))]
            event_tx: None,
            #[cfg(feature = "background-jobs")]
            event_bus: None,
        })
    }

    #[cfg(not(feature = "background-jobs"))]
    pub fn with_event_tx(mut self, tx: tokio::sync::mpsc::UnboundedSender<FtpEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }

    #[cfg(feature = "background-jobs")]
    pub fn with_event_bus(mut self, bus: event::EventBus) -> Self {
        self.event_bus = Some(bus);
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

        #[cfg(not(feature = "background-jobs"))]
        let event_tx = self.event_tx.clone();
        #[cfg(feature = "background-jobs")]
        let event_bus = self.event_bus.clone();

        let server_builder = ServerBuilder::new(Box::new(move || {
            Filesystem::new(root.clone()).expect("Failed to create filesystem")
        }))
        .greeting("RFTPS server")
        .passive_ports(50000..=65535)
        .authenticator(Arc::new(authenticator))
        .user_detail_provider(Arc::new(DefaultUserDetailProvider));

        #[cfg(not(feature = "background-jobs"))]
        let server_builder = server_builder
            .notify_data(logger::DataLogger { event_tx: event_tx.clone() })
            .notify_presence(logger::ConnectionLogger { event_tx });

        #[cfg(feature = "background-jobs")]
        let server_builder = server_builder
            .notify_data(logger::DataLogger { event_bus: event_bus.clone() })
            .notify_presence(logger::ConnectionLogger { event_bus });

        let mut server_builder = server_builder;
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

        tokio::select! {
            result = server.listen(&addr_str) => {
                result.map_err(|e| format!("Error listening: {}", e))
            }
            _ = &mut stop_rx => {
                println!("FTP Server stopping...");
                Ok(())
            }
        }
    }
}
