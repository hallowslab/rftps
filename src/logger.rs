use std::{fmt::Debug, future::Future, pin::Pin};
use libunftp::notification::{DataEvent, DataListener, EventMeta, PresenceEvent, PresenceListener};

pub struct ConnectionLogger;

impl Debug for ConnectionLogger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ConnectionLogger->")
    }
}

impl PresenceListener for ConnectionLogger {
    fn receive_presence_event<'life0, 'async_trait>(
        &'life0 self,
        e: PresenceEvent,
        m: EventMeta
    ) ->  Pin<Box<dyn Future<Output = ()> + Send + 'async_trait>>
    where 
        Self: 'async_trait,
        'life0: 'async_trait
    {
        Box::pin(async move {
            match e {
                PresenceEvent::LoggedIn => println!("User {} logged in", m.username),
                PresenceEvent::LoggedOut => println!("User {} logged out", m.username),
            }
        })
    }
}

pub struct DataLogger;

impl Debug for DataLogger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DataLogger->")
    }
}

impl DataListener for DataLogger {
    fn receive_data_event<'life0, 'async_trait>(
        &'life0 self,
        e: DataEvent,
        m:EventMeta
    ) ->  Pin<Box<dyn Future<Output = ()> + Send + 'async_trait>>
    where
        Self:'async_trait,
        'life0:'async_trait
    {
        Box::pin(async move {
            match e {
                DataEvent::MadeDir { path } => println!("User {} created directory {}", m.username, path),
                DataEvent::RemovedDir { path } => println!("User {} deleted directory {}", m.username, path),
                // {path: , bytes: }
                DataEvent::Got { path, .. } => println!("User {} downloaded file {}", m.username, path),
                // {path: , bytes: }
                DataEvent::Put { path, .. } => println!("User {} uploaded file {}", m.username, path),
                DataEvent::Renamed { from, to } => println!("User {} renamed {} to {}", m.username, from, to),
                DataEvent::Deleted { path } => println!("User {} deleted {}", m.username, path),
            }
        })    
    }
}
    