use super::*;
use crate::{
    adapter::SnapshotSurface,
    refs::{RefEntry, RefMap},
    refs_test_support::HomeGuard,
};

fn entry(name: &str) -> RefEntry {
    RefEntry {
        pid: 7,
        role: "button".into(),
        name: Some(name.into()),
        value: None,
        description: None,
        states: vec![],
        bounds: None,
        bounds_hash: Some(42),
        available_actions: vec![crate::capability::CLICK.into()],
        source_app: Some("TestApp".into()),
        source_window_id: None,
        source_window_title: Some("Test Window".into()),
        source_surface: SnapshotSurface::Window,
        root_ref: None,
        path_is_absolute: false,
        path: smallvec::SmallVec::new(),
    }
}

fn map_with(name: &str) -> RefMap {
    let mut map = RefMap::new();
    map.allocate(entry(name));
    map
}

#[test]
fn snapshot_roundtrip_updates_latest_pointer() {
    let _guard = HomeGuard::new();
    let store = RefStore::new().unwrap();
    let map = map_with("Send");

    let snapshot_id = store.save_new_snapshot(&map).unwrap();

    assert_eq!(
        store.latest_snapshot_id().as_deref(),
        Some(snapshot_id.as_str())
    );
    assert_eq!(store.load(Some(&snapshot_id)).unwrap().len(), 1);
    assert_eq!(store.load(None).unwrap().len(), 1);
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
        default_store
            .load(Some(&default_id))
            .unwrap()
            .get("@e1")
            .unwrap()
            .name
            .as_deref(),
        Some("Default")
    );
    assert_eq!(
        session_a
            .load(Some(&session_id))
            .unwrap()
            .get("@e1")
            .unwrap()
            .name
            .as_deref(),
        Some("Session A")
    );
    assert!(session_b.load(None).is_err());
    assert_ne!(
        default_store.latest_snapshot_id(),
        session_a.latest_snapshot_id()
    );
}

#[test]
fn save_existing_snapshot_does_not_promote_latest_pointer() {
    let _guard = HomeGuard::new();
    let store = RefStore::new().unwrap();

    let mut first = map_with("First");
    let first_id = store.save_new_snapshot(&first).unwrap();
    let second_id = store.save_new_snapshot(&map_with("Second")).unwrap();

    first.allocate(entry("First Child"));
    store.save_existing_snapshot(&first_id, &first).unwrap();

    assert_eq!(
        store.latest_snapshot_id().as_deref(),
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
    assert!(store.latest_snapshot_id().is_some());
}

#[test]
fn session_store_does_not_migrate_global_legacy_refmap() {
    let _guard = HomeGuard::new();
    map_with("Legacy").save().unwrap();

    let store = RefStore::for_session(Some("fresh-agent")).unwrap();
    let err = store.load(None).unwrap_err();

    assert_eq!(err.code(), "SNAPSHOT_NOT_FOUND");
    assert!(store.latest_snapshot_id().is_none());
}
