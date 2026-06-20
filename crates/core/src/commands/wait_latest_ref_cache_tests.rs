use super::*;
use crate::{
    adapter::SnapshotSurface,
    capability,
    refs::{RefEntry, RefMap},
    refs_test_support::HomeGuard,
};

fn save_ref(pid: i32, name: Option<&str>) -> String {
    RefStore::new()
        .unwrap()
        .save_new_snapshot(&refmap_with_ref(pid, name))
        .unwrap()
}

fn refmap_with_ref(pid: i32, name: Option<&str>) -> RefMap {
    let mut refmap = RefMap::new();
    refmap.allocate(RefEntry {
        pid,
        role: "button".into(),
        name: name.map(String::from),
        value: None,
        description: None,
        states: vec![],
        bounds: None,
        bounds_hash: None,
        available_actions: vec![capability::CLICK.into()],
        source_app: None,
        source_window_id: None,
        source_window_title: None,
        source_surface: SnapshotSurface::Window,
        root_ref: None,
        path_is_absolute: false,
        path: smallvec::SmallVec::new(),
    });
    refmap
}

#[test]
fn latest_ref_cache_pins_starting_snapshot_when_latest_advances() {
    let _guard = HomeGuard::new();
    let first_id = save_ref(1, Some("First"));
    let store = RefStore::new().unwrap();

    let mut cache = LatestRefCache::new(&store).unwrap();
    assert_eq!(cache.snapshot_id.as_deref(), Some(first_id.as_str()));

    let second_id = save_ref(99, Some("Second"));
    assert_ne!(first_id, second_id);

    cache.last_refresh = std::time::Instant::now() - std::time::Duration::from_secs(2);
    cache.refresh_if_due().unwrap();

    assert_eq!(cache.snapshot_id.as_deref(), Some(first_id.as_str()));
    assert_eq!(
        store.latest_snapshot_id().as_deref(),
        Some(second_id.as_str())
    );
    assert_eq!(cache.entry("@e1").unwrap().pid, 1);
}

#[test]
fn latest_ref_cache_debounces_consecutive_refreshes() {
    let _guard = HomeGuard::new();
    let _first_id = save_ref(1, Some("First"));
    let store = RefStore::new().unwrap();

    let mut cache = LatestRefCache::new(&store).unwrap();
    let pinned_snapshot_id = cache.snapshot_id.clone();

    let _ = save_ref(2, None);

    let debounced_refresh = std::time::Instant::now();
    cache.last_refresh = debounced_refresh;
    cache.refresh_if_due().unwrap();

    assert_eq!(cache.snapshot_id, pinned_snapshot_id);
    assert_eq!(cache.last_refresh, debounced_refresh);
}

#[test]
fn latest_ref_cache_keeps_last_good_map_when_pinned_snapshot_disappears() {
    let _guard = HomeGuard::new();
    let first_id = save_ref(1, Some("First"));
    let store = RefStore::new().unwrap();

    let mut cache = LatestRefCache::new(&store).unwrap();
    assert_eq!(cache.snapshot_id.as_deref(), Some(first_id.as_str()));

    let home = crate::refs::home_dir().unwrap();
    let snapshot_dir = home
        .join(".agent-desktop")
        .join("snapshots")
        .join(&first_id);
    std::fs::remove_dir_all(snapshot_dir).unwrap();

    cache.last_refresh = std::time::Instant::now() - std::time::Duration::from_secs(2);
    cache.refresh_if_due().unwrap();

    assert_eq!(cache.snapshot_id.as_deref(), Some(first_id.as_str()));
    assert_eq!(cache.entry("@e1").unwrap().pid, 1);
}

#[test]
fn latest_ref_cache_retries_unreadable_pinned_snapshot_after_refresh_error() {
    let _guard = HomeGuard::new();
    let first_id = save_ref(1, Some("First"));
    let store = RefStore::new().unwrap();

    let mut cache = LatestRefCache::new(&store).unwrap();
    assert_eq!(cache.snapshot_id.as_deref(), Some(first_id.as_str()));

    let refmap_path = crate::refs::home_dir()
        .unwrap()
        .join(".agent-desktop")
        .join("snapshots")
        .join(&first_id)
        .join("refmap.json");
    std::fs::write(&refmap_path, b"{not-json").unwrap();

    cache.last_refresh = std::time::Instant::now() - std::time::Duration::from_secs(2);
    cache.refresh_if_due().unwrap();

    assert_eq!(cache.snapshot_id.as_deref(), Some(first_id.as_str()));
    assert_eq!(cache.entry("@e1").unwrap().pid, 1);

    store
        .save_snapshot(&first_id, &refmap_with_ref(3, Some("Recovered")))
        .unwrap();
    cache.last_refresh = std::time::Instant::now() - std::time::Duration::from_secs(2);
    cache.refresh_if_due().unwrap();

    assert_eq!(cache.snapshot_id.as_deref(), Some(first_id.as_str()));
    assert_eq!(cache.entry("@e1").unwrap().pid, 3);
}
