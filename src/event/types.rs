use std::time::SystemTime;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum FtpEvent {
    LoggedIn { username: String },
    LoggedOut { username: String },
    FileUploaded { username: String, path: String, timestamp: SystemTime },
    FileDownloaded { username: String, path: String },
    DirCreated { username: String, path: String },
    DirRemoved { username: String, path: String },
    Renamed { username: String, from: String, to: String },
    Deleted { username: String, path: String },
    RelayStatus { status: String, message: Option<String> },
}

impl FtpEvent {
    pub fn event_name(&self) -> &'static str {
        match self {
            FtpEvent::LoggedIn { .. } => "logged_in",
            FtpEvent::LoggedOut { .. } => "logged_out",
            FtpEvent::FileUploaded { .. } => "file_uploaded",
            FtpEvent::FileDownloaded { .. } => "file_downloaded",
            FtpEvent::DirCreated { .. } => "dir_created",
            FtpEvent::DirRemoved { .. } => "dir_removed",
            FtpEvent::Renamed { .. } => "renamed",
            FtpEvent::Deleted { .. } => "deleted",
            FtpEvent::RelayStatus { .. } => "relay_status",
        }
    }
}
