use crate::{error::AppError, node::AccessibilityNode, snapshot::SnapshotResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const MAX_SNAPSHOT_BYTES: u64 = 5_242_880; // 5 MB

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotRecord {
    pub tree: AccessibilityNode,
    pub app: String,
    pub window_id: String,
    pub window_title: String,
    pub taken_at_ms: u64,
}

pub fn save(record: &SnapshotRecord) -> Result<(), AppError> {
    let path = snapshot_path()?;
    let dir = path
        .parent()
        .ok_or_else(|| AppError::Internal("invalid snapshot path".into()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
    }
    #[cfg(not(unix))]
    std::fs::create_dir_all(dir)?;

    let json = serde_json::to_string(record)?;
    let tmp = path.with_extension("tmp");

    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)?;
        file.write_all(json.as_bytes())?;
        file.flush()?;
    }
    #[cfg(not(unix))]
    std::fs::write(&tmp, json.as_bytes())?;

    std::fs::rename(&tmp, &path)?;
    Ok(())
}

pub fn load() -> Result<Option<SnapshotRecord>, AppError> {
    let path = snapshot_path()?;

    if !path.exists() {
        return Ok(None);
    }

    let metadata = std::fs::metadata(&path)?;
    if metadata.len() > MAX_SNAPSHOT_BYTES {
        return Err(AppError::Internal(
            "Snapshot file exceeds 5MB size limit".into(),
        ));
    }

    let json = std::fs::read_to_string(&path)?;
    let record: SnapshotRecord = serde_json::from_str(&json)?;
    Ok(Some(record))
}

fn snapshot_path() -> Result<PathBuf, AppError> {
    let home =
        home_dir().ok_or_else(|| AppError::Internal("HOME directory not found".into()))?;
    Ok(home.join(".agent-desktop").join("last_snapshot.json"))
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
}

pub fn record_from_result(result: &SnapshotResult) -> SnapshotRecord {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    SnapshotRecord {
        tree: result.tree.clone(),
        app: result.window.app.clone(),
        window_id: result.window.id.clone(),
        window_title: result.window.title.clone(),
        taken_at_ms: now_ms,
    }
}

