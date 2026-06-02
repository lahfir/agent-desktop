use crate::{
    context::validate_session_id,
    error::{AdapterError, AppError},
    refs::{RefMap, home_dir, new_snapshot_id, validate_snapshot_id, write_private_file},
    refs_lock::RefStoreLock,
};
use std::io::Read;
use std::path::PathBuf;

const LATEST_SNAPSHOT_FILE: &str = "latest_snapshot_id";
const MAX_SAVED_SNAPSHOTS: usize = 512;

#[derive(Debug, Clone)]
pub struct RefStore {
    base_dir: PathBuf,
}

impl RefStore {
    pub fn new() -> Result<Self, AppError> {
        Self::for_session(None)
    }

    pub fn for_session(session_id: Option<&str>) -> Result<Self, AppError> {
        let home =
            home_dir().ok_or_else(|| AppError::Internal("HOME directory not found".into()))?;
        if let Some(session_id) = session_id {
            validate_session_id(session_id)?;
            return Ok(Self {
                base_dir: home
                    .join(".agent-desktop")
                    .join("sessions")
                    .join(session_id),
            });
        }
        Ok(Self {
            base_dir: home.join(".agent-desktop"),
        })
    }

    #[cfg(test)]
    pub fn for_tests() -> Result<Self, AppError> {
        Self::new()
    }

    pub fn save_new_snapshot(&self, refmap: &RefMap) -> Result<String, AppError> {
        self.with_write_lock(|| {
            let snapshot_id = new_snapshot_id();
            self.save_snapshot_unlocked(&snapshot_id, refmap)?;
            self.set_latest_unlocked(&snapshot_id)?;
            self.prune_old_snapshots_unlocked(&snapshot_id)?;
            Ok(snapshot_id)
        })
    }

    pub fn save_snapshot(&self, snapshot_id: &str, refmap: &RefMap) -> Result<(), AppError> {
        self.with_write_lock(|| self.save_snapshot_unlocked(snapshot_id, refmap))
    }

    pub fn save_existing_snapshot(
        &self,
        snapshot_id: &str,
        refmap: &RefMap,
    ) -> Result<(), AppError> {
        self.with_write_lock(|| self.save_snapshot_unlocked(snapshot_id, refmap))
    }

    pub fn load(&self, snapshot_id: Option<&str>) -> Result<RefMap, AppError> {
        match snapshot_id {
            Some(id) => self.load_snapshot(id),
            None => self.load_latest(),
        }
    }

    pub fn load_latest(&self) -> Result<RefMap, AppError> {
        if let Ok(id) = std::fs::read_to_string(self.latest_path()) {
            let id = id.trim();
            if !id.is_empty() {
                return self.load_snapshot(id);
            }
        }
        if let Some(refmap) = self.migrate_legacy_latest()? {
            return Ok(refmap);
        }
        Err(AppError::Adapter(AdapterError::snapshot_not_found(
            "latest",
        )))
    }

    pub fn load_snapshot(&self, snapshot_id: &str) -> Result<RefMap, AppError> {
        validate_snapshot_id(snapshot_id)?;
        let path = self.snapshot_path(snapshot_id);
        let mut file = std::fs::File::open(&path)
            .map_err(|_| AppError::Adapter(AdapterError::snapshot_not_found(snapshot_id)))?;
        let metadata = file.metadata()?;
        if metadata.len() > crate::refs::MAX_REFMAP_BYTES {
            return Err(AppError::Internal(
                "RefMap file exceeds 1MB size limit".into(),
            ));
        }
        let mut json = String::with_capacity(metadata.len() as usize);
        file.read_to_string(&mut json)?;
        if json.len() as u64 > crate::refs::MAX_REFMAP_BYTES {
            return Err(AppError::Internal(
                "RefMap file exceeds 1MB size limit".into(),
            ));
        }
        Ok(serde_json::from_str(&json)?)
    }

    pub fn set_latest(&self, snapshot_id: &str) -> Result<(), AppError> {
        self.with_write_lock(|| self.set_latest_unlocked(snapshot_id))
    }

    pub fn latest_snapshot_id(&self) -> Option<String> {
        std::fs::read_to_string(self.latest_path())
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    fn save_snapshot_unlocked(&self, snapshot_id: &str, refmap: &RefMap) -> Result<(), AppError> {
        validate_snapshot_id(snapshot_id)?;
        let json = refmap.serialize_with_size_check()?;
        let path = self.snapshot_path(snapshot_id);
        write_private_file(&path, json.as_bytes())
    }

    fn set_latest_unlocked(&self, snapshot_id: &str) -> Result<(), AppError> {
        validate_snapshot_id(snapshot_id)?;
        write_private_file(&self.latest_path(), snapshot_id.as_bytes())
    }

    fn latest_path(&self) -> PathBuf {
        self.base_dir.join(LATEST_SNAPSHOT_FILE)
    }

    fn snapshot_path(&self, snapshot_id: &str) -> PathBuf {
        self.base_dir
            .join("snapshots")
            .join(snapshot_id)
            .join("refmap.json")
    }

    fn snapshots_dir(&self) -> PathBuf {
        self.base_dir.join("snapshots")
    }

    fn prune_old_snapshots_unlocked(&self, latest_id: &str) -> Result<(), AppError> {
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
        let remove_count = snapshots.len() - MAX_SAVED_SNAPSHOTS;
        for (_, id, path) in snapshots.into_iter().take(remove_count) {
            if id != latest_id {
                let _ = std::fs::remove_dir_all(path);
            }
        }
        Ok(())
    }

    fn lock_path(&self) -> PathBuf {
        self.base_dir.join("refstore.lock")
    }

    fn with_write_lock<T>(&self, f: impl FnOnce() -> Result<T, AppError>) -> Result<T, AppError> {
        let _lock = RefStoreLock::acquire(&self.lock_path())?;
        f()
    }

    fn migrate_legacy_latest(&self) -> Result<Option<RefMap>, AppError> {
        self.with_write_lock(|| {
            if let Ok(id) = std::fs::read_to_string(self.latest_path()) {
                let id = id.trim();
                if !id.is_empty() {
                    return self.load_snapshot(id).map(Some);
                }
            }
            let refmap = match RefMap::load() {
                Ok(refmap) => refmap,
                Err(err) => {
                    tracing::debug!("legacy last_refmap.json migration skipped: {err}");
                    return Ok(None);
                }
            };
            let snapshot_id = new_snapshot_id();
            self.save_snapshot_unlocked(&snapshot_id, &refmap)?;
            self.set_latest_unlocked(&snapshot_id)?;
            Ok(Some(refmap))
        })
    }
}
