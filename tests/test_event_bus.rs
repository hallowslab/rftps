#![cfg(feature = "background-jobs")]

use rftps::event::{EventBus, FtpEvent};
use std::time::SystemTime;

#[test]
fn test_publish_subscribe() {
    let bus = EventBus::new();
    let (_, mut rx) = bus.subscribe();

    let event = FtpEvent::FileUploaded {
        username: "test".into(),
        path: "/test.txt".into(),
        timestamp: SystemTime::now(),
    };

    bus.publish(&event);
    let received = rx.try_recv().unwrap();
    assert_eq!(received.event_name(), "file_uploaded");
}

#[test]
fn test_fan_out() {
    let bus = EventBus::new();
    let (_, mut rx1) = bus.subscribe();
    let (_, mut rx2) = bus.subscribe();

    let event = FtpEvent::LoggedIn {
        username: "alice".into(),
    };
    bus.publish(&event);

    assert!(rx1.try_recv().is_ok());
    assert!(rx2.try_recv().is_ok());
}

#[test]
fn test_unsubscribe() {
    let bus = EventBus::new();
    let (id, mut rx) = bus.subscribe();
    bus.unsubscribe(id);

    let event = FtpEvent::LoggedIn {
        username: "alice".into(),
    };
    bus.publish(&event);
    assert!(rx.try_recv().is_err());
}

#[test]
fn test_dead_sender_cleanup() {
    let bus = EventBus::new();
    let (_id, _rx) = bus.subscribe();
    drop(_rx);

    let event = FtpEvent::LoggedIn {
        username: "alice".into(),
    };
    bus.publish(&event);

    assert_eq!(bus.subscriber_count(), 0);
}
