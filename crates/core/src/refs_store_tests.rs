use super::*;
use crate::{
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
fn concurrent_writers_preserve_all_snapshots() {
    let _guard = HomeGuard::new();
    let store = RefStore::new().unwrap();
    let mut handles = Vec::new();

    for i in 0..8 {
        let store = store.clone();
        handles.push(std::thread::spawn(move || {
            store
                .save_new_snapshot(&map_with(&format!("Snapshot {i}")))
                .unwrap()
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

#[test]
fn sessions_are_isolated_from_default_store() {
    let _guard = HomeGuard::new();
    let default_store = RefStore::new().unwrap();
    let session_a = RefStore::for_session(Some("agent-a")).unwrap();
    let session_b = RefStore::for_session(Some("agent-b")).unwrap();

    let default_id = default_store
        .save_new_snapshot(&map_with("Default"))
        .unwrap();
    let session_id = session_a.save_new_snapshot(&map_with("Session A")).unwrap();

    assert_eq!(default_store.load(None).unwrap().len(), 1);
    assert_eq!(
        ref_name(&default_store.load(Some(&default_id)).unwrap()),
        Some("Default")
    );
    assert_eq!(
        ref_name(&session_a.load(Some(&session_id)).unwrap()),
        Some("Session A")
    );
    assert!(session_b.load(None).is_err());
    assert_ne!(
        default_store.latest_snapshot_id().unwrap(),
        session_a.latest_snapshot_id().unwrap()
    );
}

#[test]
fn explicit_snapshot_id_remains_in_its_session_namespace() {
    let _guard = HomeGuard::new();
    let default_store = RefStore::new().unwrap();
    let session_a = RefStore::for_session(Some("agent-a")).unwrap();
    let session_b = RefStore::for_session(Some("agent-b")).unwrap();

    let snapshot_id = session_a.save_new_snapshot(&map_with("Session A")).unwrap();

    assert!(default_store.load(Some(&snapshot_id)).is_err());
    assert!(session_b.load(Some(&snapshot_id)).is_err());
    assert_eq!(
        ref_name(&session_a.load(Some(&snapshot_id)).unwrap()),
        Some("Session A")
    );
    assert!(default_store.load(None).is_err());
    assert!(session_b.load(None).is_err());
}

#[test]
fn update_existing_snapshot_cannot_cross_session_namespaces() {
    let _guard = HomeGuard::new();
    let default_store = RefStore::new().unwrap();
    let session_a = RefStore::for_session(Some("agent-a")).unwrap();

    let snapshot_id = session_a.save_new_snapshot(&map_with("Session A")).unwrap();
    let err = default_store
        .update_existing_snapshot(&snapshot_id, "@e1", &entry("Session A"), |_| Ok(()))
        .unwrap_err();

    assert_eq!(err.code(), "SNAPSHOT_NOT_FOUND");
    assert_eq!(
        ref_name(&session_a.load(Some(&snapshot_id)).unwrap()),
        Some("Session A")
    );
    assert!(default_store.latest_snapshot_id().unwrap().is_none());
    assert_eq!(
        session_a.latest_snapshot_id().unwrap().as_deref(),
        Some(snapshot_id.as_str())
    );
}

#[test]
fn duplicate_explicit_snapshot_ids_remain_session_scoped() {
    let _guard = HomeGuard::new();
    let default_store = RefStore::new().unwrap();
    let session_a = RefStore::for_session(Some("agent-a")).unwrap();
    let session_b = RefStore::for_session(Some("agent-b")).unwrap();

    session_a
        .save_snapshot("sdup1", &map_with("Session A"))
        .unwrap();
    session_b
        .save_snapshot("sdup1", &map_with("Session B"))
        .unwrap();

    let err = default_store.load(Some("sdup1")).unwrap_err();

    assert_eq!(err.code(), "SNAPSHOT_NOT_FOUND");
    assert_eq!(
        ref_name(&session_a.load(Some("sdup1")).unwrap()),
        Some("Session A")
    );
    assert_eq!(
        ref_name(&session_b.load(Some("sdup1")).unwrap()),
        Some("Session B")
    );
}

#[test]
fn default_store_does_not_discover_session_snapshots() {
    let _guard = HomeGuard::new();
    let default_store = RefStore::new().unwrap();
    let session_a = RefStore::for_session(Some("agent-a")).unwrap();

    session_a
        .save_snapshot("sdup2", &map_with("Session A"))
        .unwrap();
    let invalid_base = default_store.base_dir.join("sessions").join("bad.session");
    let invalid_path = RefStore::snapshot_path_for_base(&invalid_base, "sdup2");
    std::fs::create_dir_all(invalid_path.parent().unwrap()).unwrap();
    std::fs::write(
        invalid_path,
        map_with("Invalid").serialize_with_size_check().unwrap(),
    )
    .unwrap();

    let err = default_store.load(Some("sdup2")).unwrap_err();
    assert_eq!(err.code(), "SNAPSHOT_NOT_FOUND");
    assert_eq!(
        ref_name(&session_a.load(Some("sdup2")).unwrap()),
        Some("Session A")
    );
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
