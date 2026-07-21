use libunftp::notification::{DataEvent, DataListener, EventMeta, PresenceEvent, PresenceListener};
use std::{fmt::Debug, future::Future, pin::Pin};

#[cfg(feature = "background-jobs")]
use std::time::SystemTime;

#[cfg(not(feature = "background-jobs"))]
use crate::FtpEvent;
#[cfg(not(feature = "background-jobs"))]
use tokio::sync::mpsc::UnboundedSender;

#[cfg(feature = "background-jobs")]
use crate::event::{EventBus, FtpEvent};

pub struct ConnectionLogger {
    #[cfg(not(feature = "background-jobs"))]
    pub event_tx: Option<UnboundedSender<FtpEvent>>,
    #[cfg(feature = "background-jobs")]
    pub event_bus: Option<EventBus>,
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
        #[cfg(not(feature = "background-jobs"))]
        let event_tx = self.event_tx.clone();
        #[cfg(feature = "background-jobs")]
        let event_bus = self.event_bus.clone();

        Box::pin(async move {
            match e {
                PresenceEvent::LoggedIn => {
                    println!("User {} logged in", m.username);
                    let event = FtpEvent::LoggedIn {
                        username: m.username,
                    };
                    #[cfg(not(feature = "background-jobs"))]
                    if let Some(tx) = event_tx {
                        let _ = tx.send(event);
                    }
                    #[cfg(feature = "background-jobs")]
                    if let Some(bus) = event_bus {
                        bus.publish(&event);
                    }
                }
                PresenceEvent::LoggedOut => {
                    println!("User {} logged out", m.username);
                    let event = FtpEvent::LoggedOut {
                        username: m.username,
                    };
                    #[cfg(not(feature = "background-jobs"))]
                    if let Some(tx) = event_tx {
                        let _ = tx.send(event);
                    }
                    #[cfg(feature = "background-jobs")]
                    if let Some(bus) = event_bus {
                        bus.publish(&event);
                    }
                }
            }
        })
    }
}

pub struct DataLogger {
    #[cfg(not(feature = "background-jobs"))]
    pub event_tx: Option<UnboundedSender<FtpEvent>>,
    #[cfg(feature = "background-jobs")]
    pub event_bus: Option<EventBus>,
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
        #[cfg(not(feature = "background-jobs"))]
        let event_tx = self.event_tx.clone();
        #[cfg(feature = "background-jobs")]
        let event_bus = self.event_bus.clone();

        Box::pin(async move {
            let event = match e {
                DataEvent::MadeDir { path } => {
                    println!("User {} created directory {}", m.username, path);
                    FtpEvent::DirCreated {
                        username: m.username,
                        path,
                    }
                }
                DataEvent::RemovedDir { path } => {
                    println!("User {} deleted directory {}", m.username, path);
                    FtpEvent::DirRemoved {
                        username: m.username,
                        path,
                    }
                }
                DataEvent::Got { path, .. } => {
                    println!("User {} downloaded file {}", m.username, path);
                    #[cfg(not(feature = "background-jobs"))]
                    { FtpEvent::FileDownload { username: m.username, path } }
                    #[cfg(feature = "background-jobs")]
                    { FtpEvent::FileDownloaded { username: m.username, path } }
                }
                DataEvent::Put { path, .. } => {
                    println!("User {} uploaded file {}", m.username, path);
                    #[cfg(not(feature = "background-jobs"))]
                    { FtpEvent::FileUpload { username: m.username, path } }
                    #[cfg(feature = "background-jobs")]
                    { FtpEvent::FileUploaded { username: m.username, path, timestamp: SystemTime::now() } }
                }
                DataEvent::Renamed { from, to } => {
                    println!("User {} renamed {} to {}", m.username, from, to);
                    FtpEvent::Renamed {
                        username: m.username,
                        from,
                        to,
                    }
                }
                DataEvent::Deleted { path } => {
                    println!("User {} deleted {}", m.username, path);
                    FtpEvent::Deleted {
                        username: m.username,
                        path,
                    }
                }
            };

            #[cfg(not(feature = "background-jobs"))]
            if let Some(tx) = event_tx {
                let _ = tx.send(event);
            }
            #[cfg(feature = "background-jobs")]
            if let Some(bus) = event_bus {
                bus.publish(&event);
            }
        })
    }
}
