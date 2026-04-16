use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

const MAX_REFMAP_BYTES: u64 = 1_048_576; // 1 MB

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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub states: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<crate::node::Rect>,
    pub bounds_hash: Option<u64>,
    pub available_actions: Vec<String>,
    pub source_app: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_ref: Option<String>,
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

    fn serialize_with_size_check(&self) -> Result<String, AppError> {
        let json = serde_json::to_string(self)?;
        if json.len() as u64 > MAX_REFMAP_BYTES {
            return Err(AppError::Internal(
                "RefMap exceeds 1MB size limit on write".into(),
            ));
        }
        Ok(json)
    }

    pub fn save(&self) -> Result<(), AppError> {
        let json = self.serialize_with_size_check()?;

        let path = refmap_path()?;
        let dir = path
            .parent()
            .ok_or_else(|| AppError::Internal("invalid refmap path".into()))?;

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

fn home_dir() -> Option<PathBuf> {
    if let Some(p) = HOME_OVERRIDE.with(|cell| cell.borrow().clone()) {
        return Some(p);
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
}

#[cfg(test)]
pub(crate) struct HomeGuard {
    _dir: tempdir::TempDir,
    prev: Option<PathBuf>,
}

#[cfg(test)]
mod tempdir {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    pub struct TempDir(PathBuf);

    impl TempDir {
        pub fn new() -> Self {
            let n = COUNTER.fetch_add(1, Ordering::SeqCst);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let path = std::env::temp_dir().join(format!("agent-desktop-test-{nanos}-{n}"));
            fs::create_dir_all(&path).expect("create tempdir");
            Self(path)
        }

        pub fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

#[cfg(test)]
impl HomeGuard {
    pub fn new() -> Self {
        let dir = tempdir::TempDir::new();
        let prev = HOME_OVERRIDE.with(|cell| cell.borrow().clone());
        HOME_OVERRIDE.with(|cell| *cell.borrow_mut() = Some(dir.path().to_path_buf()));
        Self { _dir: dir, prev }
    }
}

#[cfg(test)]
impl Drop for HomeGuard {
    fn drop(&mut self) {
        let prev = self.prev.take();
        HOME_OVERRIDE.with(|cell| *cell.borrow_mut() = prev);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocate_sequential() {
        let mut map = RefMap::new();
        let entry = RefEntry {
            pid: 1,
            role: "button".into(),
            name: Some("OK".into()),
            value: None,
            states: vec![],
            bounds: None,
            bounds_hash: None,
            available_actions: vec!["Click".into()],
            source_app: None,
            root_ref: None,
        };
        let r1 = map.allocate(entry.clone());
        let r2 = map.allocate(entry);
        assert_eq!(r1, "@e1");
        assert_eq!(r2, "@e2");
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn test_get_existing() {
        let mut map = RefMap::new();
        let entry = RefEntry {
            pid: 42,
            role: "textfield".into(),
            name: None,
            value: None,
            states: vec![],
            bounds: None,
            bounds_hash: Some(12345),
            available_actions: vec![],
            source_app: Some("Finder".into()),
            root_ref: None,
        };
        let ref_id = map.allocate(entry);
        let retrieved = map.get(&ref_id).unwrap();
        assert_eq!(retrieved.pid, 42);
        assert_eq!(retrieved.role, "textfield");
    }

    #[test]
    fn test_get_missing() {
        let map = RefMap::new();
        assert!(map.get("@e99").is_none());
    }

    #[test]
    fn test_remove_by_root_ref() {
        let mut map = RefMap::new();
        let base = RefEntry {
            pid: 1,
            role: "button".into(),
            name: Some("OK".into()),
            value: None,
            states: vec![],
            bounds: None,
            bounds_hash: None,
            available_actions: vec!["Click".into()],
            source_app: None,
            root_ref: None,
        };

        map.allocate(base.clone());

        let drilled = RefEntry {
            root_ref: Some("@e1".into()),
            ..base.clone()
        };
        map.allocate(drilled.clone());
        map.allocate(drilled);
        assert_eq!(map.len(), 3);

        map.remove_by_root_ref("@e1");
        assert_eq!(map.len(), 1);
        assert!(map.get("@e1").is_some());
    }

    #[test]
    fn test_counter_continues_after_skeleton_into_drill_down() {
        let mut map = RefMap::new();
        let skeleton_entry = RefEntry {
            pid: 1,
            role: "button".into(),
            name: Some("Skeleton".into()),
            value: None,
            states: vec![],
            bounds: None,
            bounds_hash: None,
            available_actions: vec![],
            source_app: None,
            root_ref: None,
        };

        let last_skeleton = (0..10)
            .map(|_| map.allocate(skeleton_entry.clone()))
            .last()
            .unwrap();
        assert_eq!(last_skeleton, "@e10");

        let drilled = RefEntry {
            root_ref: Some("@e3".into()),
            ..skeleton_entry
        };

        let first_drilled = map.allocate(drilled.clone());
        let second_drilled = map.allocate(drilled);
        assert_eq!(
            first_drilled, "@e11",
            "counter should continue past skeleton ids, not reset"
        );
        assert_eq!(second_drilled, "@e12");
        assert_eq!(map.len(), 12);

        map.remove_by_root_ref("@e3");
        assert_eq!(
            map.len(),
            10,
            "scoped invalidation should drop only the drill-down refs"
        );
        assert!(map.get("@e3").is_some(), "skeleton @e3 must survive");
    }

    #[test]
    fn test_root_ref_serde_roundtrip() {
        let entry = RefEntry {
            pid: 1,
            role: "button".into(),
            name: None,
            value: None,
            states: vec![],
            bounds: None,
            bounds_hash: None,
            available_actions: vec![],
            source_app: None,
            root_ref: Some("@e5".into()),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("root_ref"));
        let back: RefEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.root_ref.as_deref(), Some("@e5"));
    }

    #[test]
    fn test_serialize_with_size_check_rejects_oversized() {
        let mut map = RefMap::new();
        let big_name = "x".repeat(2048);
        for _ in 0..600 {
            map.allocate(RefEntry {
                pid: 1,
                role: "button".into(),
                name: Some(big_name.clone()),
                value: None,
                states: vec![],
                bounds: None,
                bounds_hash: None,
                available_actions: vec!["Click".into()],
                source_app: None,
                root_ref: None,
            });
        }

        let result = map.serialize_with_size_check();
        assert!(result.is_err(), "oversized refmap should be rejected");
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("1MB"),
            "error should mention the 1MB limit, got: {msg}"
        );
    }

    #[test]
    fn test_serialize_with_size_check_accepts_normal() {
        let mut map = RefMap::new();
        for _ in 0..50 {
            map.allocate(RefEntry {
                pid: 1,
                role: "button".into(),
                name: Some("OK".into()),
                value: None,
                states: vec![],
                bounds: None,
                bounds_hash: None,
                available_actions: vec!["Click".into()],
                source_app: None,
                root_ref: None,
            });
        }

        let result = map.serialize_with_size_check();
        assert!(result.is_ok(), "normal-sized refmap should serialize");
    }

    #[test]
    fn test_save_load_roundtrip_with_home_override() {
        let _guard = HomeGuard::new();
        let mut map = RefMap::new();
        map.allocate(RefEntry {
            pid: 7,
            role: "button".into(),
            name: Some("Send".into()),
            value: None,
            states: vec![],
            bounds: None,
            bounds_hash: Some(42),
            available_actions: vec!["Click".into()],
            source_app: Some("TestApp".into()),
            root_ref: None,
        });
        map.save().expect("save should succeed under HomeGuard");

        let loaded = RefMap::load().expect("load should succeed");
        assert_eq!(loaded.len(), 1);
        let entry = loaded.get("@e1").unwrap();
        assert_eq!(entry.pid, 7);
        assert_eq!(entry.name.as_deref(), Some("Send"));
    }

    #[test]
    fn test_save_oversize_preserves_previous_file() {
        let _guard = HomeGuard::new();

        let mut original = RefMap::new();
        original.allocate(RefEntry {
            pid: 1,
            role: "button".into(),
            name: Some("Original".into()),
            value: None,
            states: vec![],
            bounds: None,
            bounds_hash: None,
            available_actions: vec!["Click".into()],
            source_app: None,
            root_ref: None,
        });
        original.save().expect("baseline save");

        let mut oversize = RefMap::new();
        let big = "x".repeat(2048);
        for _ in 0..600 {
            oversize.allocate(RefEntry {
                pid: 1,
                role: "button".into(),
                name: Some(big.clone()),
                value: None,
                states: vec![],
                bounds: None,
                bounds_hash: None,
                available_actions: vec!["Click".into()],
                source_app: None,
                root_ref: None,
            });
        }
        let result = oversize.save();
        assert!(result.is_err(), "oversize save must reject");

        let reloaded = RefMap::load().expect("previous file must still load");
        assert_eq!(reloaded.len(), 1);
        let entry = reloaded.get("@e1").unwrap();
        assert_eq!(entry.name.as_deref(), Some("Original"));
    }

    #[test]
    fn test_root_ref_none_omitted() {
        let entry = RefEntry {
            pid: 1,
            role: "button".into(),
            name: None,
            value: None,
            states: vec![],
            bounds: None,
            bounds_hash: None,
            available_actions: vec![],
            source_app: None,
            root_ref: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(!json.contains("root_ref"));
    }
}
