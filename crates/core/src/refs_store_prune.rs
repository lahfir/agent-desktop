use super::{MAX_SAVED_SNAPSHOTS, RefStore, STALE_TMP_MAX_AGE};
use crate::AppError;
use crate::refs::validate_snapshot_id;

impl RefStore {
    /// Removes orphaned `*.tmp` files left behind when a process died between
    /// the temp write and the atomic rename. Runs under the store write lock;
    /// the age threshold keeps any in-flight write from another process safe.
    pub(crate) fn remove_tmp_files_older_than(&self, max_age: std::time::Duration) {
        remove_stale_files_in_dir(&self.base_dir, max_age, is_orphaned_tmp_file);
        let snapshots_dir = self.snapshots_dir();
        remove_stale_files_in_dir(&snapshots_dir, max_age, is_orphaned_tmp_file);
        let Ok(entries) = std::fs::read_dir(snapshots_dir) else {
            return;
        };
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                remove_stale_files_in_dir(&entry.path(), max_age, is_orphaned_tmp_file);
            }
        }
    }

    pub(super) fn prune_old_snapshots_unlocked(&self, latest_id: &str) -> Result<(), AppError> {
        self.remove_tmp_files_older_than(STALE_TMP_MAX_AGE);
        let dir = self.snapshots_dir();
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return Ok(());
        };
        let mut snapshots = Vec::new();
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if !file_type.is_dir() {
                continue;
            }
            let id = entry.file_name().to_string_lossy().to_string();
            if validate_snapshot_id(&id).is_err() {
                continue;
            }
            let modified = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            snapshots.push((modified, id, entry.path()));
        }
        if snapshots.len() <= MAX_SAVED_SNAPSHOTS {
            return Ok(());
        }
        snapshots.sort_by_key(|(modified, id, _)| (*modified, id.clone()));
        let mut remove_count = snapshots.len() - MAX_SAVED_SNAPSHOTS;
        for (_, id, path) in snapshots {
            if remove_count == 0 {
                break;
            }
            if id != latest_id {
                let _ = std::fs::remove_dir_all(path);
                remove_count -= 1;
            }
        }
        Ok(())
    }
}

pub(crate) fn is_orphaned_tmp_file(path: &std::path::Path) -> bool {
    path.extension().is_some_and(|ext| ext == "tmp")
}

/// Removes plain files directly under `dir` that satisfy `matches` and whose
/// mtime is at least `max_age` old. Never descends into subdirectories and
/// never removes a directory itself. Shared by refstore `*.tmp` cleanup and
/// the sessionless clipboard image sweep so both age-based prunes stay on one
/// implementation.
pub(crate) fn remove_stale_files_in_dir(
    dir: &std::path::Path,
    max_age: std::time::Duration,
    matches: impl Fn(&std::path::Path) -> bool,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !matches(&path) {
            continue;
        }
        let is_plain_file = entry.file_type().is_ok_and(|kind| kind.is_file());
        if !is_plain_file {
            continue;
        }
        let stale = entry
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| modified.elapsed().ok())
            .is_some_and(|age| age >= max_age);
        if stale {
            let _ = std::fs::remove_file(&path);
        }
    }
}
