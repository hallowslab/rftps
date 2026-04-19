use crate::FtpEvent;
use libunftp::notification::{DataEvent, DataListener, EventMeta, PresenceEvent, PresenceListener};
use std::{fmt::Debug, future::Future, pin::Pin};
use tokio::sync::mpsc::UnboundedSender;

pub struct ConnectionLogger {
    pub event_tx: Option<UnboundedSender<FtpEvent>>,
}

impl Debug for ConnectionLogger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ConnectionLogger")
    }
}

impl PresenceListener for ConnectionLogger {
    fn receive_presence_event<'life0, 'async_trait>(
        &'life0 self,
        e: PresenceEvent,
        m: EventMeta,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'async_trait>>
    where
        Self: 'async_trait,
        'life0: 'async_trait,
    {
        let event_tx = self.event_tx.clone();
        Box::pin(async move {
            match e {
                PresenceEvent::LoggedIn => {
                    println!("User {} logged in", m.username);
                    if let Some(tx) = event_tx {
                        let _ = tx.send(FtpEvent::LoggedIn {
                            username: m.username,
                        });
                    }
                }
                PresenceEvent::LoggedOut => {
                    println!("User {} logged out", m.username);
                    if let Some(tx) = event_tx {
                        let _ = tx.send(FtpEvent::LoggedOut {
                            username: m.username,
                        });
                    }
                }
            }
        })
    }
}

pub struct DataLogger {
    pub event_tx: Option<UnboundedSender<FtpEvent>>,
}

impl Debug for DataLogger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DataLogger")
    }
}

impl DataListener for DataLogger {
    fn receive_data_event<'life0, 'async_trait>(
        &'life0 self,
        e: DataEvent,
        m: EventMeta,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'async_trait>>
    where
        Self: 'async_trait,
        'life0: 'async_trait,
    {
        let event_tx = self.event_tx.clone();
        Box::pin(async move {
            let event = match e {
                DataEvent::MadeDir { path } => {
                    println!("User {} created directory {}", m.username, path);
                    FtpEvent::DirCreated {
                        username: m.username,
                        path: path.clone(),
                    }
                }
                DataEvent::RemovedDir { path } => {
                    println!("User {} deleted directory {}", m.username, path);
                    FtpEvent::DirRemoved {
                        username: m.username,
                        path: path.clone(),
                    }
                }
                DataEvent::Got { path, .. } => {
                    println!("User {} downloaded file {}", m.username, path);
                    FtpEvent::FileDownload {
                        username: m.username,
                        path: path.clone(),
                    }
                }
                DataEvent::Put { path, .. } => {
                    println!("User {} uploaded file {}", m.username, path);
                    FtpEvent::FileUpload {
                        username: m.username,
                        path: path.clone(),
                    }
                }
                DataEvent::Renamed { from, to } => {
                    println!("User {} renamed {} to {}", m.username, from, to);
                    FtpEvent::Renamed {
                        username: m.username,
                        from: from.clone(),
                        to: to.clone(),
                    }
                }
                DataEvent::Deleted { path } => {
                    println!("User {} deleted {}", m.username, path);
                    FtpEvent::Deleted {
                        username: m.username,
                        path: path.clone(),
                    }
                }
            };

            if let Some(tx) = event_tx {
                let _ = tx.send(event);
            }
        })
    }
}
