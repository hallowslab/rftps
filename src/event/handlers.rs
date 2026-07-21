use super::types::FtpEvent;
use crate::job::types::Job;
use async_trait::async_trait;

#[async_trait]
pub trait EventHandler: Send + Sync {
    fn name(&self) -> &str;
    fn interested_in(&self, event: &FtpEvent) -> bool;
    async fn handle(&self, event: &FtpEvent) -> Option<Job>;
}

pub struct HandlerRegistry {
    handlers: Vec<Box<dyn EventHandler>>,
}

impl HandlerRegistry {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
        }
    }

    pub fn register(&mut self, handler: Box<dyn EventHandler>) {
        self.handlers.push(handler);
    }

    pub async fn dispatch(&self, event: &FtpEvent) -> Vec<Job> {
        let mut jobs = Vec::new();
        for handler in &self.handlers {
            if handler.interested_in(event) {
                if let Some(job) = handler.handle(event).await {
                    jobs.push(job);
                }
            }
        }
        jobs
    }

    pub fn handlers(&self) -> &[Box<dyn EventHandler>] {
        &self.handlers
    }
}

impl Default for HandlerRegistry {
    fn default() -> Self {
        Self::new()
    }
}
