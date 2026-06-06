use crate::{
    context::validate_session_id,
    error::{AdapterError, AppError},
    refs::{RefMap, home_dir, new_snapshot_id, validate_snapshot_id, write_private_file},
    refs_lock::RefStoreLock,
};
use std::io::{ErrorKind, Read};
use std::path::{Path, PathBuf};

const LATEST_SNAPSHOT_FILE: &str = "latest_snapshot_id";
const MAX_SAVED_SNAPSHOTS: usize = 512;

#[derive(Debug, Clone)]
pub struct RefStore {
    base_dir: PathBuf,
    allow_legacy_migration: bool,
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
                allow_legacy_migration: false,
            });
        }
        Ok(Self {
            base_dir: home.join(".agent-desktop"),
            allow_legacy_migration: true,
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
        validate_snapshot_id(snapshot_id)?;
        let base_dir = if self.snapshot_path(snapshot_id).is_file() {
            self.base_dir.clone()
        } else {
            self.discover_snapshot_base(snapshot_id)?
                .unwrap_or_else(|| self.base_dir.clone())
        };
        let store = Self {
            base_dir,
            allow_legacy_migration: false,
        };
        store.with_write_lock(|| store.save_snapshot_unlocked(snapshot_id, refmap))
    }

    pub fn load(&self, snapshot_id: Option<&str>) -> Result<RefMap, AppError> {
        match snapshot_id {
            Some(id) => self.load_snapshot(id),
            None => self.load_latest(),
        }
    }

    pub fn load_latest(&self) -> Result<RefMap, AppError> {
        if let Some(id) = self.read_latest_snapshot_id()? {
            return self.load_snapshot_from_base(&self.base_dir, &id);
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
        if let Some(refmap) = self.read_snapshot_if_present(&self.base_dir, snapshot_id)? {
            return Ok(refmap);
        }
        match self.discover_snapshot_base(snapshot_id)? {
            Some(base_dir) => self.load_snapshot_from_base(&base_dir, snapshot_id),
            None => Err(AppError::Adapter(AdapterError::snapshot_not_found(
                snapshot_id,
            ))),
        }
    }

    fn load_snapshot_from_base(
        &self,
        base_dir: &Path,
        snapshot_id: &str,
    ) -> Result<RefMap, AppError> {
        validate_snapshot_id(snapshot_id)?;
        self.read_snapshot_if_present(base_dir, snapshot_id)?
            .ok_or_else(|| AppError::Adapter(AdapterError::snapshot_not_found(snapshot_id)))
    }

    fn read_snapshot_if_present(
        &self,
        base_dir: &Path,
        snapshot_id: &str,
    ) -> Result<Option<RefMap>, AppError> {
        let path = Self::snapshot_path_for_base(base_dir, snapshot_id);
        let mut file = match open_refstore_file(&path) {
            Ok(file) => file,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err.into()),
        };
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
        Ok(Some(serde_json::from_str(&json)?))
    }

    pub fn set_latest(&self, snapshot_id: &str) -> Result<(), AppError> {
        self.with_write_lock(|| self.set_latest_unlocked(snapshot_id))
    }

    pub fn latest_snapshot_id(&self) -> Option<String> {
        self.read_latest_snapshot_id().ok().flatten()
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

    fn read_latest_snapshot_id(&self) -> Result<Option<String>, AppError> {
        let mut file = match open_refstore_file(&self.latest_path()) {
            Ok(file) => file,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err.into()),
        };
        let mut id = String::new();
        file.read_to_string(&mut id)?;
        Ok(Some(id.trim().to_string()).filter(|id| !id.is_empty()))
    }

    fn snapshot_path(&self, snapshot_id: &str) -> PathBuf {
        Self::snapshot_path_for_base(&self.base_dir, snapshot_id)
    }

    fn snapshot_path_for_base(base_dir: &Path, snapshot_id: &str) -> PathBuf {
        base_dir
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

    fn discover_snapshot_base(&self, snapshot_id: &str) -> Result<Option<PathBuf>, AppError> {
        let home =
            home_dir().ok_or_else(|| AppError::Internal("HOME directory not found".into()))?;
        let agent_dir = home.join(".agent-desktop");
        let mut matches = Vec::new();
        if agent_dir != self.base_dir
            && Self::snapshot_path_for_base(&agent_dir, snapshot_id).is_file()
        {
            matches.push(agent_dir.clone());
        }
        let sessions_dir = agent_dir.join("sessions");
        let Ok(entries) = std::fs::read_dir(sessions_dir) else {
            return Ok(matches.into_iter().next());
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path == self.base_dir {
                continue;
            }
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if validate_session_id(name).is_err() {
                continue;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() && Self::snapshot_path_for_base(&path, snapshot_id).is_file() {
                matches.push(path);
            }
        }
        if matches.len() > 1 {
            return Err(AppError::invalid_input_with_suggestion(
                format!("Snapshot '{snapshot_id}' exists in more than one session"),
                "Pass the matching --session for this rare snapshot id collision.",
            ));
        }
        Ok(matches.into_iter().next())
    }

    fn with_write_lock<T>(&self, f: impl FnOnce() -> Result<T, AppError>) -> Result<T, AppError> {
        let _lock = RefStoreLock::acquire(&self.lock_path())?;
        f()
    }

    fn migrate_legacy_latest(&self) -> Result<Option<RefMap>, AppError> {
        if !self.allow_legacy_migration {
            return Ok(None);
        }
        self.with_write_lock(|| {
            if let Some(id) = self.read_latest_snapshot_id()? {
                return self.load_snapshot_from_base(&self.base_dir, &id).map(Some);
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

fn open_refstore_file(path: &Path) -> std::io::Result<std::fs::File> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
    }
    #[cfg(not(unix))]
    {
        if std::fs::symlink_metadata(path)?.file_type().is_symlink() {
            return Err(std::io::Error::new(
                ErrorKind::PermissionDenied,
                "refmap path must not be a symlink",
            ));
        }
        std::fs::File::open(path)
    }
}

#[cfg(test)]
#[path = "refs_store_tests.rs"]
mod tests;
