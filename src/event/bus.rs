use super::types::FtpEvent;
use tokio::sync::mpsc::{self, UnboundedSender, UnboundedReceiver};
use std::sync::{Arc, RwLock};

pub type SubscriberId = usize;

pub struct EventBus {
    subscribers: Arc<RwLock<Vec<SubscriberId>>>,
    senders: Arc<RwLock<Vec<(SubscriberId, UnboundedSender<FtpEvent>)>>>,
    next_id: std::sync::atomic::AtomicUsize,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(RwLock::new(Vec::new())),
            senders: Arc::new(RwLock::new(Vec::new())),
            next_id: std::sync::atomic::AtomicUsize::new(1),
        }
    }

    pub fn subscribe(&self) -> (SubscriberId, UnboundedReceiver<FtpEvent>) {
        let id = self.next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let (tx, rx) = mpsc::unbounded_channel();

        self.subscribers.write().unwrap().push(id);
        self.senders.write().unwrap().push((id, tx));

        (id, rx)
    }

    pub fn unsubscribe(&self, id: SubscriberId) {
        self.subscribers.write().unwrap().retain(|&s| s != id);
        self.senders.write().unwrap().retain(|&(s, _)| s != id);
    }

    pub fn publish(&self, event: &FtpEvent) {
        let senders_snapshot: Vec<_> = {
            let senders = self.senders.read().unwrap();
            senders.iter().map(|(id, tx)| (*id, tx.clone())).collect()
        };

        let mut failed = Vec::new();
        for (id, tx) in &senders_snapshot {
            if tx.send(event.clone()).is_err() {
                failed.push(*id);
            }
        }

        if !failed.is_empty() {
            let mut senders = self.senders.write().unwrap();
            senders.retain(|(id, _)| !failed.contains(id));
            let mut subscribers = self.subscribers.write().unwrap();
            subscribers.retain(|id| !failed.contains(id));
        }
    }

    pub fn subscriber_count(&self) -> usize {
        self.subscribers.read().unwrap().len()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for EventBus {
    fn clone(&self) -> Self {
        Self {
            subscribers: Arc::clone(&self.subscribers),
            senders: Arc::clone(&self.senders),
            next_id: std::sync::atomic::AtomicUsize::new(
                self.next_id.load(std::sync::atomic::Ordering::Relaxed),
            ),
        }
    }
}
