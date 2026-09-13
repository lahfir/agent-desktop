use super::*;
use crate::{
    ErrorCode,
    adapter::SnapshotSurface,
    refs::{RefEntry, RefMap},
    refs_test_support::HomeGuard,
};

fn entry(name: &str) -> RefEntry {
    let bounds = crate::Rect {
        x: 1.0,
        y: 1.0,
        width: 20.0,
        height: 20.0,
    };
    RefEntry {
        process: crate::RefProcess {
            pid: crate::ProcessId::new(7),
            process_instance: Some("test-instance".into()),
        },
        identity: crate::RefEntryIdentity {
            role: "button".into(),
            name: Some(name.into()),
            value: None,
            description: None,
            native_id: None,
        },
        geometry: crate::RefGeometry {
            bounds: Some(bounds),
            bounds_hash: bounds.bounds_hash(),
        },
        capabilities: crate::RefCapabilities {
            states: vec![],
            available_actions: vec![crate::capability::CLICK.into()],
        },
        source: crate::RefSource {
            source_app: Some("TestApp".into()),
            source_window_id: None,
            source_window_title: Some("Test Window".into()),
            source_window_bounds_hash: None,
            source_surface: SnapshotSurface::Window,
        },
        scope: crate::RefScope {
            root_ref: None,
            path_is_absolute: false,
            path: smallvec::SmallVec::new(),
        },
    }
}

fn map_with(name: &str) -> RefMap {
    let mut map = RefMap::new();
    map.allocate(entry(name));
    map
}

fn ref_name(map: &RefMap) -> Option<&str> {
    map.get("@e1")
        .and_then(|entry| entry.identity.name.as_deref())
}

#[test]
fn snapshot_roundtrip_updates_latest_pointer() {
    let _guard = HomeGuard::new();
    let store = RefStore::new().unwrap();
    let map = map_with("Send");

    let snapshot_id = store.save_new_snapshot(&map).unwrap();

    assert_eq!(
        store.latest_snapshot_id().unwrap().as_deref(),
        Some(snapshot_id.as_str())
    );
    assert_eq!(store.load(Some(&snapshot_id)).unwrap().len(), 1);
    assert_eq!(store.load(None).unwrap().len(), 1);
}

#[test]
fn load_ref_uses_qualified_identity_and_never_crosses_sessions() {
    let _guard = HomeGuard::new();
    let store = RefStore::for_session(Some("debug-session")).unwrap();
    let snapshot = store.save_new_snapshot(&map_with("Target")).unwrap();
    let qualified = format!("@{snapshot}:e1");
    assert_eq!(store.load_ref(&qualified, None).unwrap(), entry("Target"));
    assert_eq!(
        store.load_ref("@e1", Some(&snapshot)).unwrap(),
        entry("Target")
    );
    assert_eq!(
        store.load_ref("@e1", None).unwrap_err().code(),
        "INVALID_ARGS"
    );
    assert_eq!(
        store
            .load_ref(&qualified, Some("sother"))
            .unwrap_err()
            .code(),
        "INVALID_ARGS"
    );
    assert_eq!(
        store
            .load_ref(&format!("@{snapshot}:e2"), None)
            .unwrap_err()
            .code(),
        "STALE_REF"
    );
    assert_eq!(
        RefStore::new()
            .unwrap()
            .load_ref(&qualified, None)
            .unwrap_err()
            .code(),
        "SNAPSHOT_NOT_FOUND"
    );
}

/// Saves under contention, retrying only while the store's write lock reports
/// its own `lock_timeout`.
///
/// The lock's budget is a product decision - a caller waiting on a busy store
/// is told to try again rather than blocked indefinitely - and `TIMEOUT`
/// carries `retry: safe` precisely so a caller can. Eight writers racing one
/// lock legitimately exhaust that budget, so a test requiring every writer to
/// win on its first attempt asserts something the contract never promised.
/// Matching on the `kind` keeps that tolerance narrow: a timeout from anywhere
/// else in the save path is still a failure.
fn save_snapshot_after_contention(store: &RefStore, name: &str) -> String {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        match store.save_new_snapshot(&map_with(name)) {
            Ok(id) => return id,
            Err(AppError::Adapter(inner))
                if inner.code == ErrorCode::Timeout
                    && inner
                        .details
                        .as_ref()
                        .and_then(|details| details.get("kind"))
                        .and_then(serde_json::Value::as_str)
                        == Some("lock_timeout")
                    && std::time::Instant::now() < deadline =>
            {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            Err(error) => panic!("contended snapshot save failed: {error:?}"),
        }
    }
}

#[test]
fn concurrent_writers_preserve_all_snapshots() {
    let _guard = HomeGuard::new();
    let store = RefStore::new().unwrap();
    let mut handles = Vec::new();

    for i in 0..8 {
        let store = store.clone();
        handles.push(std::thread::spawn(move || {
            save_snapshot_after_contention(&store, &format!("Snapshot {i}"))
        }));
    }

    let ids = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect::<Vec<_>>();

    for id in &ids {
        assert_eq!(store.load_snapshot(id).unwrap().len(), 1);
    }
    let latest = store.latest_snapshot_id().unwrap().unwrap();
    assert!(ids.iter().any(|id| id == &latest));
}

#[cfg(unix)]
#[test]
fn read_snapshot_rejects_symlinked_refmap() {
    let _guard = HomeGuard::new();
    let store = RefStore::new().unwrap();

    store.save_snapshot("ssym1", &map_with("Original")).unwrap();
    let path = store.snapshot_path("ssym1");
    let target = store.base_dir.join("symlink-target-refmap.json");
    std::fs::write(
        target.as_path(),
        map_with("Symlinked").serialize_with_size_check().unwrap(),
    )
    .unwrap();
    std::fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(&target, &path).unwrap();

    let err = store.load(Some("ssym1")).unwrap_err();

    assert_eq!(err.code(), "INTERNAL");
}

#[cfg(unix)]
#[test]
fn read_latest_rejects_symlinked_pointer() {
    let _guard = HomeGuard::new();
    let store = RefStore::new().unwrap();

    let snapshot_id = store.save_new_snapshot(&map_with("Original")).unwrap();
    let latest = store.latest_path();
    let target = store.base_dir.join("latest-target");
    std::fs::write(&target, snapshot_id).unwrap();
    std::fs::remove_file(&latest).unwrap();
    std::os::unix::fs::symlink(&target, &latest).unwrap();

    let err = store.load_latest().unwrap_err();

    assert_eq!(err.code(), "INTERNAL");
    assert!(store.latest_snapshot_id().is_err());
}

#[test]
fn update_existing_snapshot_does_not_promote_latest_pointer() {
    let _guard = HomeGuard::new();
    let store = RefStore::new().unwrap();

    let first_id = store.save_new_snapshot(&map_with("First")).unwrap();
    let second_id = store.save_new_snapshot(&map_with("Second")).unwrap();

    store
        .update_existing_snapshot(&first_id, "@e1", &entry("First"), |map| {
            map.allocate(entry("First Child"));
            Ok(())
        })
        .unwrap();

    assert_eq!(
        store.latest_snapshot_id().unwrap().as_deref(),
        Some(second_id.as_str())
    );
    assert_eq!(store.load(Some(&first_id)).unwrap().len(), 2);
}

#[test]
fn default_store_migrates_legacy_latest_refmap() {
    let _guard = HomeGuard::new();
    map_with("Legacy").save().unwrap();

    let store = RefStore::new().unwrap();
    let loaded = store.load_latest().unwrap();

    assert_eq!(loaded.len(), 1);
    assert!(store.latest_snapshot_id().unwrap().is_some());
}

#[test]
fn session_store_does_not_migrate_global_legacy_refmap() {
    let _guard = HomeGuard::new();
    map_with("Legacy").save().unwrap();

    let store = RefStore::for_session(Some("fresh-agent")).unwrap();
    let err = store.load(None).unwrap_err();

    assert_eq!(err.code(), "SNAPSHOT_NOT_FOUND");
    assert!(store.latest_snapshot_id().unwrap().is_none());
}

#[test]
fn stale_tmp_files_are_swept_and_fresh_ones_kept() {
    let _guard = HomeGuard::new();
    let store = RefStore::new().unwrap();
    let snapshot_id = store.save_new_snapshot(&map_with("Send")).unwrap();
    let base_tmp = store.base_dir.join("latest_snapshot_id.tmp");
    let snapshot_tmp = store.snapshots_dir().join("dead.tmp");
    let refmap_tmp = store.snapshots_dir().join(&snapshot_id).join("refmap.tmp");
    std::fs::write(&base_tmp, b"orphan").unwrap();
    std::fs::write(&snapshot_tmp, b"orphan").unwrap();
    std::fs::write(&refmap_tmp, b"orphan").unwrap();

    store.remove_tmp_files_older_than(std::time::Duration::ZERO);

    assert!(!base_tmp.exists());
    assert!(!snapshot_tmp.exists());
    assert!(!refmap_tmp.exists());
    assert!(store.snapshot_path(&snapshot_id).is_file());

    std::fs::write(&base_tmp, b"fresh").unwrap();
    store.remove_tmp_files_older_than(STALE_TMP_MAX_AGE);

    assert!(base_tmp.exists());
}

#[path = "refs_store_pruning_tests.rs"]
mod pruning_tests;

#[path = "refs_store_session_tests.rs"]
mod session_tests;
