use super::*;

#[test]
fn save_new_snapshot_prunes_old_snapshots_without_removing_latest() {
    let _guard = HomeGuard::new();
    let store = RefStore::new().unwrap();
    let first_id = store.save_new_snapshot(&map_with("First")).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5));
    let mut latest_id = first_id.clone();

    for i in 0..=MAX_SAVED_SNAPSHOTS {
        latest_id = store
            .save_new_snapshot(&map_with(&format!("Snapshot {i}")))
            .unwrap();
    }

    let count = std::fs::read_dir(store.snapshots_dir())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .count();

    assert!(count <= MAX_SAVED_SNAPSHOTS);
    assert!(store.snapshot_path(&latest_id).is_file());
    assert!(!store.snapshot_path(&first_id).exists());
    assert_eq!(
        store.latest_snapshot_id().unwrap().as_deref(),
        Some(latest_id.as_str())
    );
}

#[test]
fn update_existing_refuses_to_recreate_a_missing_snapshot() {
    let _guard = HomeGuard::new();
    let store = RefStore::new().unwrap();
    let snapshot_id = store.save_new_snapshot(&map_with("Original")).unwrap();
    std::fs::remove_dir_all(store.snapshots_dir().join(&snapshot_id)).unwrap();

    let err = store
        .update_existing_snapshot(&snapshot_id, "@e1", &entry("Original"), |_| Ok(()))
        .unwrap_err();

    assert_eq!(err.code(), "SNAPSHOT_NOT_FOUND");
    assert!(!store.snapshot_path(&snapshot_id).exists());
}

#[test]
fn duplicate_snapshot_id_across_sessions_remains_isolated() {
    let _guard = HomeGuard::new();
    let store_a = RefStore::for_session(Some("agent-a")).unwrap();
    let store_b = RefStore::for_session(Some("agent-b")).unwrap();
    let snapshot_id = store_a.save_new_snapshot(&map_with("A")).unwrap();
    store_b.save_snapshot(&snapshot_id, &map_with("B")).unwrap();

    let err = RefStore::new()
        .unwrap()
        .load_snapshot(&snapshot_id)
        .unwrap_err();

    assert_eq!(err.code(), "SNAPSHOT_NOT_FOUND");
    assert_eq!(
        ref_name(&store_a.load_snapshot(&snapshot_id).unwrap()),
        Some("A")
    );
    assert_eq!(
        ref_name(&store_b.load_snapshot(&snapshot_id).unwrap()),
        Some("B")
    );
}

#[test]
fn prune_never_removes_trace_segments() {
    let _guard = HomeGuard::new();
    let store = RefStore::for_session(Some("trace-retention")).unwrap();
    let trace_dir = store.trace_dir();
    crate::trace::ensure_trace_dir(&trace_dir).unwrap();
    let segment = trace_dir.join("1234-5678.jsonl");
    std::fs::write(&segment, b"{}\n").unwrap();
    for index in 0..=MAX_SAVED_SNAPSHOTS {
        let snapshot_id = format!("snap-{index:04}");
        store
            .save_snapshot(&snapshot_id, &map_with(&snapshot_id))
            .unwrap();
        store.set_latest(&snapshot_id).unwrap();
    }
    assert!(segment.is_file());
    assert!(trace_dir.is_dir());
}
