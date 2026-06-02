use super::*;
use crate::{
    adapter::SnapshotSurface,
    refs::{RefEntry, RefMap},
    refs_test_support::HomeGuard,
};

fn save_ref(pid: i32, name: Option<&str>) -> String {
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
        available_actions: vec!["Click".into()],
        source_app: None,
        source_window_id: None,
        source_window_title: None,
        source_surface: SnapshotSurface::Window,
        root_ref: None,
        path_is_absolute: false,
        path: smallvec::SmallVec::new(),
    });
    RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap()
}

#[test]
fn latest_ref_cache_picks_up_newer_snapshot_after_refresh() {
    let _guard = HomeGuard::new();
    let first_id = save_ref(1, Some("First"));
    let store = RefStore::new().unwrap();

    let mut cache = LatestRefCache::new(&store).unwrap();
    assert_eq!(cache.snapshot_id.as_deref(), Some(first_id.as_str()));

    let second_id = save_ref(99, Some("Second"));
    assert_ne!(first_id, second_id);

    cache.last_refresh = std::time::Instant::now() - std::time::Duration::from_secs(2);
    cache.refresh_if_due();

    assert_eq!(cache.snapshot_id.as_deref(), Some(second_id.as_str()));
    assert!(cache.entry("@e1").is_some());
}

#[test]
fn latest_ref_cache_debounces_consecutive_refreshes() {
    let _guard = HomeGuard::new();
    let _first_id = save_ref(1, Some("First"));
    let store = RefStore::new().unwrap();

    let mut cache = LatestRefCache::new(&store).unwrap();
    let pinned_snapshot_id = cache.snapshot_id.clone();
    let pinned_refresh = cache.last_refresh;

    let _ = save_ref(2, None);

    cache.last_refresh = std::time::Instant::now();
    cache.refresh_if_due();

    assert_eq!(cache.snapshot_id, pinned_snapshot_id);
    assert_eq!(cache.last_refresh, pinned_refresh.max(cache.last_refresh));
}
