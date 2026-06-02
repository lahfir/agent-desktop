use super::*;
use crate::{
    action::ElementState,
    adapter::{NativeHandle, PlatformAdapter},
    error::{AdapterError, ErrorCode},
    node::Rect,
    notification::{NotificationFilter, NotificationInfo},
    refs::{RefEntry, RefMap},
    refs_store::RefStore,
    refs_test_support::HomeGuard,
};

struct NoopAdapter;

impl PlatformAdapter for NoopAdapter {}

struct PredicateAdapter {
    state: Option<ElementState>,
    value: Option<String>,
    bounds: Option<Rect>,
}

impl PlatformAdapter for PredicateAdapter {
    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn get_live_state(&self, _handle: &NativeHandle) -> Result<Option<ElementState>, AdapterError> {
        Ok(self.state.clone())
    }

    fn get_live_value(&self, _handle: &NativeHandle) -> Result<Option<String>, AdapterError> {
        Ok(self.value.clone())
    }

    fn get_element_bounds(&self, _handle: &NativeHandle) -> Result<Option<Rect>, AdapterError> {
        Ok(self.bounds)
    }
}

struct NotificationErrorAdapter;

impl PlatformAdapter for NotificationErrorAdapter {
    fn list_notifications(
        &self,
        _filter: &NotificationFilter,
    ) -> Result<Vec<NotificationInfo>, AdapterError> {
        Err(AdapterError::new(
            ErrorCode::PlatformNotSupported,
            "notifications unavailable",
        ))
    }
}

fn snapshot_with_one_ref() -> String {
    let mut refmap = RefMap::new();
    refmap.allocate(RefEntry {
        pid: 1,
        role: "button".into(),
        name: Some("Run".into()),
        value: None,
        description: None,
        states: Vec::new(),
        bounds: None,
        bounds_hash: None,
        available_actions: vec!["Click".into()],
        source_app: None,
        source_window_id: None,
        source_window_title: None,
        source_surface: crate::adapter::SnapshotSurface::Window,
        root_ref: None,
        path_is_absolute: false,
        path: smallvec::SmallVec::new(),
    });
    RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap()
}

fn snapshot_with_disabled_ref() -> String {
    let mut refmap = RefMap::new();
    refmap.allocate(RefEntry {
        pid: 1,
        role: "button".into(),
        name: Some("Run".into()),
        value: None,
        description: None,
        states: vec!["disabled".into()],
        bounds: None,
        bounds_hash: None,
        available_actions: vec!["Click".into()],
        source_app: None,
        source_window_id: None,
        source_window_title: None,
        source_surface: crate::adapter::SnapshotSurface::Window,
        root_ref: None,
        path_is_absolute: false,
        path: smallvec::SmallVec::new(),
    });
    RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap()
}

#[test]
fn snapshot_pinned_missing_ref_is_invalid_args() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_with_one_ref();

    let err = wait_for_element(
        "@e2".into(),
        Some(snapshot_id),
        wait_predicate::ElementPredicate::Exists,
        1,
        &NoopAdapter,
        &crate::context::CommandContext::default(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "INVALID_ARGS");
    assert!(err.suggestion().is_some());
}

#[test]
fn notification_wait_propagates_adapter_error() {
    let err = execute(
        WaitArgs {
            ms: None,
            element: None,
            snapshot_id: None,
            predicate: None,
            value: None,
            count: None,
            window: None,
            text: None,
            timeout_ms: 1,
            menu: false,
            menu_closed: false,
            notification: true,
            app: None,
        },
        &NotificationErrorAdapter,
    )
    .unwrap_err();

    assert_eq!(err.code(), "PLATFORM_NOT_SUPPORTED");
}

#[test]
fn rejects_multiple_wait_modes() {
    let err = execute(
        WaitArgs {
            ms: Some(1),
            element: Some("@e1".into()),
            snapshot_id: None,
            predicate: None,
            value: None,
            count: None,
            window: None,
            text: None,
            timeout_ms: 1,
            menu: false,
            menu_closed: false,
            notification: false,
            app: None,
        },
        &NoopAdapter,
    )
    .unwrap_err();

    assert_eq!(err.code(), "INVALID_ARGS");
    assert!(err.suggestion().is_some());
}

#[test]
fn notification_wait_allows_text_filter() {
    let result = validate_wait_mode(&WaitArgs {
        ms: None,
        element: None,
        snapshot_id: None,
        predicate: None,
        value: None,
        count: None,
        window: None,
        text: Some("done".into()),
        timeout_ms: 1,
        menu: false,
        menu_closed: false,
        notification: true,
        app: None,
    });

    assert!(result.is_ok());
}

#[test]
fn element_wait_enabled_predicate_uses_live_state() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_with_one_ref();
    let adapter = PredicateAdapter {
        state: Some(ElementState {
            role: "button".into(),
            states: vec![],
            value: None,
        }),
        value: None,
        bounds: None,
    };

    let value = wait_for_element(
        "@e1".into(),
        Some(snapshot_id),
        wait_predicate::ElementPredicate::Enabled,
        1,
        &adapter,
        &crate::context::CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["predicate"], "enabled");
    assert_eq!(value["observed"]["enabled"], true);
}

#[test]
fn element_wait_value_predicate_matches_live_value() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_with_one_ref();
    let adapter = PredicateAdapter {
        state: None,
        value: Some("ready".into()),
        bounds: None,
    };

    let value = wait_for_element(
        "@e1".into(),
        Some(snapshot_id),
        wait_predicate::ElementPredicate::Value("ready".into()),
        1,
        &adapter,
        &crate::context::CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["predicate"], "value");
    assert_eq!(value["observed"]["value"], "ready");
}

#[test]
fn element_wait_timeout_reports_last_actionability_observation() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_with_disabled_ref();
    let adapter = PredicateAdapter {
        state: Some(ElementState {
            role: "button".into(),
            states: vec!["disabled".into()],
            value: None,
        }),
        value: None,
        bounds: None,
    };

    let err = wait_for_element(
        "@e1".into(),
        Some(snapshot_id),
        wait_predicate::ElementPredicate::Actionable,
        1,
        &adapter,
        &crate::context::CommandContext::default(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "TIMEOUT");
    assert!(err.to_string().contains("\"actionable\":false"));
    assert!(err.to_string().contains("entry state contains disabled"));
}

#[test]
fn predicate_requires_element_mode() {
    let err = validate_wait_mode(&WaitArgs {
        ms: None,
        element: None,
        snapshot_id: None,
        predicate: Some("enabled".into()),
        value: None,
        count: None,
        window: None,
        text: None,
        timeout_ms: 1,
        menu: false,
        menu_closed: false,
        notification: false,
        app: None,
    })
    .unwrap_err();

    assert_eq!(err.code(), "INVALID_ARGS");
}

#[test]
fn latest_ref_cache_picks_up_newer_snapshot_after_refresh() {
    let _guard = HomeGuard::new();
    let _ = snapshot_with_one_ref();
    let store = RefStore::new().unwrap();
    let first_id = store.latest_snapshot_id().unwrap();

    let mut cache = LatestRefCache::new(&store).unwrap();
    assert_eq!(cache.snapshot_id.as_deref(), Some(first_id.as_str()));

    let mut second = RefMap::new();
    second.allocate(RefEntry {
        pid: 99,
        role: "button".into(),
        name: Some("Second".into()),
        value: None,
        description: None,
        states: vec![],
        bounds: None,
        bounds_hash: None,
        available_actions: vec!["Click".into()],
        source_app: None,
        source_window_id: None,
        source_window_title: None,
        source_surface: crate::adapter::SnapshotSurface::Window,
        root_ref: None,
        path_is_absolute: false,
        path: smallvec::SmallVec::new(),
    });
    let second_id = store.save_new_snapshot(&second).unwrap();
    assert_ne!(first_id, second_id);

    cache.last_refresh = std::time::Instant::now() - std::time::Duration::from_secs(2);
    cache.refresh_if_due();

    assert_eq!(cache.snapshot_id.as_deref(), Some(second_id.as_str()));
    assert!(cache.entry("@e1").is_some());
}

#[test]
fn latest_ref_cache_debounces_consecutive_refreshes() {
    let _guard = HomeGuard::new();
    let _ = snapshot_with_one_ref();
    let store = RefStore::new().unwrap();
    let first_id = store.latest_snapshot_id().unwrap();

    let mut cache = LatestRefCache::new(&store).unwrap();
    let pinned_snapshot_id = cache.snapshot_id.clone();
    let pinned_refresh = cache.last_refresh;

    let mut other = RefMap::new();
    other.allocate(RefEntry {
        pid: 1,
        role: "button".into(),
        name: None,
        value: None,
        description: None,
        states: vec![],
        bounds: None,
        bounds_hash: None,
        available_actions: vec!["Click".into()],
        source_app: None,
        source_window_id: None,
        source_window_title: None,
        source_surface: crate::adapter::SnapshotSurface::Window,
        root_ref: None,
        path_is_absolute: false,
        path: smallvec::SmallVec::new(),
    });
    let _ = store.save_new_snapshot(&other).unwrap();

    cache.last_refresh = std::time::Instant::now();
    cache.refresh_if_due();

    assert_eq!(cache.snapshot_id, pinned_snapshot_id);
    assert_eq!(cache.last_refresh, pinned_refresh.max(cache.last_refresh));
    let _ = first_id;
}
