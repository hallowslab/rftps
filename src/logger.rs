use async_trait::async_trait;
use libunftp::notification::PresenceListener;

pub struct ConnectionLogger;

#[async_trait]
impl PresenceListener for ConnectionLogger {
    async fn receive_presence_event<'life0,'async_trait>(&'life0 self,e:libunftp::notification::PresenceEvent,m:libunftp::notification::EventMeta) ->  ::core::pin::Pin<Box<dyn ::core::future::Future<Output = ()> + ::core::marker::Send+'async_trait> >where 'life0:'async_trait,Self:'async_trait {
        
    }
}