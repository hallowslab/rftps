use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use rusqlite::{Connection, params};

/// Resolves the on-disk path of an uploaded file under `home_dir`, and returns
/// the normalized relative path used as the replication destination.
///
/// libunftp reports the raw STOR argument relative to the client's CWD. The
/// file was written to `cwd.join(path)`, so a direct `home_dir.join(path)` misses
/// nested uploads. We try the direct path first, then search the tree (newest
/// mtime wins on ambiguity).
pub fn resolve_upload_path(home_dir: &Path, raw: &str) -> (PathBuf, String) {
    let rel = raw.trim_start_matches(['/', '\\']).replace('\\', "/");
    let direct = home_dir.join(&rel);
    if direct.is_file() {
        return (direct, rel);
    }

    let needle = rel.clone();
    let mut best: Option<(PathBuf, SystemTime)> = None;
    let mut stack = vec![home_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if !path.is_file() {
                continue;
            }
            let relative = path
                .strip_prefix(home_dir)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            if relative != needle && !relative.ends_with(&format!("/{}", needle)) {
                continue;
            }
            let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
            let newer = match (&best, mtime) {
                (None, _) => true,
                (Some((_, best_time)), Some(time)) => time > *best_time,
                (Some(_), None) => false,
            };
            if newer {
                best = Some((path.clone(), mtime.unwrap_or(SystemTime::UNIX_EPOCH)));
            }
        }
    }

    if let Some((path, _)) = best {
        let relative = path
            .strip_prefix(home_dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        (path, relative)
    } else {
        (direct, rel)
    }
}

/// Durable store writer shared with the replicator. rftps only ever appends
/// `pending` records; the replicator claims/transitions them. The schema must
/// match `replicator-rs`'s store.
#[derive(Clone)]
pub struct RecordStore {
    conn: Arc<Mutex<Connection>>,
}

impl RecordStore {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create store directory '{}': {}", parent.display(), e))?;
        }
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS replication_records (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 source_path TEXT NOT NULL,
                 dest_path TEXT NOT NULL,
                 dest_key TEXT NOT NULL,
                 status TEXT NOT NULL DEFAULT 'pending',
                 attempts INTEGER NOT NULL DEFAULT 0,
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_status ON replication_records(status);",
        )
        .map_err(|e| e.to_string())?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn insert(&self, source_path: &str, dest_path: &str, dest_key: &str) -> Result<i64, String> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        conn.execute(
            "INSERT INTO replication_records
                (source_path, dest_path, dest_key, status, attempts, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'pending', 0, ?4, ?4)",
            params![source_path, dest_path, dest_key, now],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    }
}
