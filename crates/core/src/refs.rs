use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

const MAX_REFMAP_BYTES: u64 = 1_048_576; // 1 MB

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefMap {
    inner: HashMap<String, RefEntry>,
    counter: u32,
}

impl RefMap {
    pub fn new() -> Self {
        Self { inner: HashMap::new(), counter: 0 }
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

    pub fn save(&self) -> Result<(), AppError> {
        let path = refmap_path()?;
        let dir = path
            .parent()
            .ok_or_else(|| AppError::Internal("invalid refmap path".into()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
        }
        #[cfg(not(unix))]
        std::fs::create_dir_all(dir)?;

        let json = serde_json::to_string(self)?;
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
    let home = home_dir()
        .ok_or_else(|| AppError::Internal("HOME directory not found".into()))?;
    Ok(home.join(".agent-desktop").join("last_refmap.json"))
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
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
}
