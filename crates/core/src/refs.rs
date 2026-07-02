use crate::adapter::SnapshotSurface;
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub type RefPath = SmallVec<[usize; 8]>;

pub(crate) const MAX_REFMAP_BYTES: u64 = 1_048_576;
static SNAPSHOT_COUNTER: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static HOME_OVERRIDE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefEntry {
    pub pid: i32,
    pub role: String,
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub states: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<crate::node::Rect>,
    pub bounds_hash: Option<u64>,
    pub available_actions: Vec<String>,
    pub source_app: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_window_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_window_title: Option<String>,
    #[serde(default, skip_serializing_if = "SnapshotSurface::is_window")]
    pub source_surface: SnapshotSurface,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_ref: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub path_is_absolute: bool,
    #[serde(default, skip_serializing_if = "SmallVec::is_empty")]
    pub path: RefPath,
}

fn is_false(value: &bool) -> bool {
    !*value
}

pub fn validate_ref_id(ref_id: &str) -> Result<(), AppError> {
    let valid = ref_id.starts_with("@e")
        && ref_id.len() >= 3
        && ref_id.len() <= 12
        && ref_id[2..].chars().all(|c| c.is_ascii_digit())
        && ref_id[2..].parse::<u32>().is_ok_and(|n| n > 0);
    if valid {
        return Ok(());
    }
    Err(AppError::invalid_input(format!(
        "Invalid ref_id '{ref_id}': must match @e{{N}} where N is a positive integer"
    )))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefMap {
    inner: HashMap<String, RefEntry>,
    counter: u32,
}

impl RefMap {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
            counter: 0,
        }
    }

    pub fn allocate(&mut self, entry: RefEntry) -> String {
        self.counter += 1;
        let ref_id = format!("@e{}", self.counter);
        self.inner.insert(ref_id.clone(), entry);
        ref_id
    }

    pub fn get(&self, ref_id: &str) -> Option<&RefEntry> {
        self.inner.get(ref_id)
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn remove_by_root_ref(&mut self, root: &str) {
        self.inner
            .retain(|_, entry| entry.root_ref.as_deref() != Some(root));
    }

    pub(crate) fn serialize_with_size_check(&self) -> Result<String, AppError> {
        let json = serde_json::to_string(self)?;
        if json.len() as u64 > MAX_REFMAP_BYTES {
            return Err(AppError::Internal(
                "RefMap exceeds 1MB size limit on write".into(),
            ));
        }
        Ok(json)
    }

    #[cfg(test)]
    pub(crate) fn save(&self) -> Result<(), AppError> {
        let json = self.serialize_with_size_check()?;
        let path = refmap_path()?;
        write_private_file(&path, json.as_bytes())
    }

    pub fn load() -> Result<Self, AppError> {
        let path = refmap_path()?;

        let metadata = std::fs::metadata(&path)?;
        if metadata.len() > MAX_REFMAP_BYTES {
            return Err(AppError::Internal(
                "RefMap file exceeds 1MB size limit".into(),
            ));
        }

        let json = std::fs::read_to_string(&path)?;
        let map: Self = serde_json::from_str(&json)?;
        Ok(map)
    }
}

impl Default for RefMap {
    fn default() -> Self {
        Self::new()
    }
}

fn refmap_path() -> Result<PathBuf, AppError> {
    let home = home_dir().ok_or_else(|| AppError::Internal("HOME directory not found".into()))?;
    Ok(home.join(".agent-desktop").join("last_refmap.json"))
}

pub(crate) fn new_snapshot_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let counter = SNAPSHOT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let seed = RandomState::new();
    let mixed = seed.hash_one((nanos, std::process::id(), counter));
    format!("s{}", base36(mixed))
}

fn base36(mut value: u64) -> String {
    const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    const MIN_LEN: usize = 4;

    let mut buf = [b'0'; 13];
    let mut i = buf.len();
    if value == 0 {
        i -= 1;
    } else {
        while value > 0 {
            i -= 1;
            buf[i] = DIGITS[(value % 36) as usize];
            value /= 36;
        }
    }

    let digits = buf.len() - i;
    if digits < MIN_LEN {
        let pad = MIN_LEN - digits;
        i -= pad;
    }

    String::from_utf8_lossy(&buf[i..]).into_owned()
}

pub fn validate_snapshot_id(snapshot_id: &str) -> Result<(), AppError> {
    let valid = snapshot_id.len() <= 64
        && snapshot_id.len() >= 3
        && snapshot_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !valid {
        return Err(AppError::invalid_input(format!(
            "Invalid snapshot_id '{snapshot_id}': use the value returned by snapshot"
        )));
    }
    Ok(())
}

pub(crate) fn write_private_file(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let dir = path
        .parent()
        .ok_or_else(|| AppError::Internal("invalid ref store path".into()))?;

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

    static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);
    let unique = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp = path.with_extension(format!("{}.{unique}.tmp", std::process::id()));
    let written = write_tmp_then_rename(&tmp, path, bytes);
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    written
}

fn write_tmp_then_rename(tmp: &Path, path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(tmp)?;
        file.write_all(bytes)?;
        file.flush()?;
    }
    #[cfg(not(unix))]
    std::fs::write(tmp, bytes)?;

    std::fs::rename(tmp, path)?;
    Ok(())
}

pub(crate) fn is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false)
}

pub(crate) fn open_nofollow(path: &Path) -> std::io::Result<std::fs::File> {
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
                std::io::ErrorKind::PermissionDenied,
                "path must not be a symlink",
            ));
        }
        std::fs::File::open(path)
    }
}

pub(crate) fn home_dir() -> Option<PathBuf> {
    let home = HOME_OVERRIDE
        .with(|cell| cell.borrow().clone())
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))?;
    validate_home_dir(&home).then_some(home)
}

fn validate_home_dir(home: &Path) -> bool {
    let Ok(link_meta) = std::fs::symlink_metadata(home) else {
        return false;
    };
    if link_meta.file_type().is_symlink() {
        return false;
    }
    let Ok(meta) = std::fs::metadata(home) else {
        return false;
    };
    if !meta.is_dir() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        meta.uid() == unsafe { libc::getuid() }
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
pub(crate) fn set_home_override(home: Option<PathBuf>) -> Option<PathBuf> {
    HOME_OVERRIDE.with(|cell| std::mem::replace(&mut *cell.borrow_mut(), home))
}

#[cfg(test)]
#[path = "refs_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "refs_serde_tests.rs"]
mod serde_tests;
