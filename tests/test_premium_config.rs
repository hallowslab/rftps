use rftps::premium::{BackgroundJobConfig, UserMapping};
use std::collections::BTreeMap;

#[test]
fn test_default_config() {
    let config = BackgroundJobConfig::default();
    assert!(!config.enabled);
    assert_eq!(config.max_parallel_jobs, 2);
    assert_eq!(config.max_retries, 3);
    assert_eq!(config.queue_capacity, 1000);
    assert!(config.remote_storage.is_none());
}

#[test]
fn test_user_mapping_prefix() {
    let mapping = UserMapping::PrefixUserName;
    assert_eq!(mapping.resolve("alice", "photos/pic.jpg"), "alice/photos/pic.jpg");
}

#[test]
fn test_user_mapping_fixed_path() {
    let mapping = UserMapping::FixedPath("backups".into());
    assert_eq!(mapping.resolve("alice", "photos/pic.jpg"), "backups/photos/pic.jpg");
}

#[test]
fn test_user_mapping_user_map() {
    let mut map = BTreeMap::new();
    map.insert("alice".into(), "team-alice".into());
    map.insert("bob".into(), "team-bob".into());
    let mapping = UserMapping::UserMap(map);

    assert_eq!(mapping.resolve("alice", "data/file.txt"), "team-alice/data/file.txt");
    assert_eq!(mapping.resolve("bob", "data/file.txt"), "team-bob/data/file.txt");
    assert_eq!(mapping.resolve("unknown", "data/file.txt"), "unknown/data/file.txt");
}
