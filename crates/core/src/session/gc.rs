use super::{
    TRACE_LIVENESS_WINDOW, list_sessions, now_millis, read_current_session_pointer, read_manifest,
    session_dir,
};
use crate::{
    context::validate_session_id,
    error::AppError,
    refs_lock::{RefStoreLock, lock_holder_is_live},
    refs_store::RefStore,
};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub struct GcOptions {
    pub ended_only: bool,
    pub older_than: Option<Duration>,
}

#[derive(Debug)]
pub struct GcReport {
    pub removed: Vec<String>,
}

pub fn pointer_references_live_session() -> Result<bool, AppError> {
    let Some(id) = read_current_session_pointer()? else {
        return Ok(false);
    };
    is_live(&id)
}

pub fn is_live(session_id: &str) -> Result<bool, AppError> {
    validate_session_id(session_id)?;
    let store = RefStore::for_session(Some(session_id))?;
    let base = store.base_dir();
    if lock_holder_is_live(&base.join("refstore.lock")) {
        return Ok(true);
    }
    if path_recently_modified(&base.join("snapshots"))
        || trace_dir_recently_written(&store.trace_dir())
    {
        return Ok(true);
    }
    if let Some(manifest) = read_manifest(session_id)? {
        let age = Duration::from_millis(now_millis().saturating_sub(manifest.created_at));
        if manifest.ended_at.is_none() && age < TRACE_LIVENESS_WINDOW {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn gc(options: GcOptions) -> Result<GcReport, AppError> {
    let pointer = read_current_session_pointer()?;
    let mut removed = Vec::new();
    for manifest in list_sessions()? {
        if pointer.as_deref() == Some(manifest.id.as_str()) {
            continue;
        }
        if is_live(&manifest.id)? {
            continue;
        }
        if options.ended_only && manifest.ended_at.is_none() {
            continue;
        }
        if let Some(older_than) = options.older_than {
            let age_reference = manifest.ended_at.unwrap_or(manifest.created_at);
            let age_ms = now_millis().saturating_sub(age_reference);
            if Duration::from_millis(age_ms) < older_than {
                continue;
            }
        } else if manifest.ended_at.is_none() {
            continue;
        }
        let dir = session_dir(&manifest.id)?;
        let lock = match RefStoreLock::acquire(&dir.join("refstore.lock")) {
            Ok(lock) => lock,
            Err(_) => continue,
        };
        let did_remove = remove_session_dir(&dir)?;
        drop(lock);
        if did_remove {
            removed.push(manifest.id);
        }
    }
    Ok(GcReport { removed })
}

fn path_recently_modified(path: &Path) -> bool {
    let cutoff = SystemTime::now()
        .checked_sub(TRACE_LIVENESS_WINDOW)
        .unwrap_or(UNIX_EPOCH);
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .is_some_and(|modified| modified >= cutoff)
}

fn trace_dir_recently_written(trace_dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(trace_dir) else {
        return false;
    };
    let cutoff = SystemTime::now()
        .checked_sub(TRACE_LIVENESS_WINDOW)
        .unwrap_or(UNIX_EPOCH);
    entries.flatten().any(|entry| {
        entry
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .is_some_and(|modified| modified >= cutoff)
    })
}

pub(crate) fn remove_session_dir(dir: &Path) -> Result<bool, AppError> {
    if !dir.is_dir() {
        return Ok(false);
    }
    if std::fs::symlink_metadata(dir)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(true)
    {
        return Err(AppError::invalid_input_with_suggestion(
            "Refusing to remove a symlinked session directory",
            "Remove the symlink manually before running session gc.",
        ));
    }
    std::fs::remove_dir_all(dir)?;
    Ok(true)
}
