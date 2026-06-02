use super::*;
use crate::{
    action::ElementState,
    adapter::{NativeHandle, PlatformAdapter},
    error::AdapterError,
    node::Rect,
    refs::{RefEntry, RefMap},
    refs_store::RefStore,
    refs_test_support::HomeGuard,
};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

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

    fn resolve_element_strict_with_timeout(
        &self,
        entry: &RefEntry,
        _timeout: std::time::Duration,
    ) -> Result<NativeHandle, AdapterError> {
        self.resolve_element_strict(entry)
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

struct FlippingPredicateAdapter {
    states: Mutex<Vec<Vec<String>>>,
}

impl PlatformAdapter for FlippingPredicateAdapter {
    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn resolve_element_strict_with_timeout(
        &self,
        entry: &RefEntry,
        _timeout: std::time::Duration,
    ) -> Result<NativeHandle, AdapterError> {
        self.resolve_element_strict(entry)
    }

    fn get_live_state(&self, _handle: &NativeHandle) -> Result<Option<ElementState>, AdapterError> {
        let states = self.states.lock().unwrap().pop().unwrap_or_default();
        Ok(Some(ElementState {
            role: "button".into(),
            states,
            value: None,
        }))
    }
}

struct LiveErrorPredicateAdapter {
    releases: AtomicU32,
}

impl PlatformAdapter for LiveErrorPredicateAdapter {
    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn resolve_element_strict_with_timeout(
        &self,
        entry: &RefEntry,
        _timeout: std::time::Duration,
    ) -> Result<NativeHandle, AdapterError> {
        self.resolve_element_strict(entry)
    }

    fn get_live_state(&self, _handle: &NativeHandle) -> Result<Option<ElementState>, AdapterError> {
        Err(AdapterError::permission_denied())
    }

    fn release_handle(&self, _handle: &NativeHandle) -> Result<(), AdapterError> {
        self.releases.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

fn snapshot_with_one_ref() -> String {
    save_ref(Vec::new())
}

fn snapshot_with_disabled_ref() -> String {
    save_ref(vec!["disabled".into()])
}

fn save_ref(states: Vec<String>) -> String {
    let mut refmap = RefMap::new();
    refmap.allocate(RefEntry {
        pid: 1,
        role: "button".into(),
        name: Some("Run".into()),
        value: None,
        description: None,
        states,
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
        50,
        &adapter,
        &crate::context::CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["predicate"], "enabled");
    assert_eq!(value["observed"]["enabled"], true);
}

#[test]
fn element_wait_value_predicate_matches_live_value_without_leaking_it() {
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
    assert_eq!(value["observed"]["matched"], true);
    assert_eq!(value["observed"]["value_chars"], 5);
    assert!(value["observed"].get("value").is_none());
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
    match err {
        AppError::Adapter(adapter_error) => {
            let details = adapter_error.details.unwrap();
            assert_eq!(details["predicate"], "actionable");
            assert_eq!(details["last_observed"]["actionable"], false);
        }
        _ => panic!("expected adapter error"),
    }
}

#[test]
fn element_wait_actionable_uses_live_state() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_with_disabled_ref();
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
        wait_predicate::ElementPredicate::Actionable,
        1,
        &adapter,
        &crate::context::CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["predicate"], "actionable");
    assert_eq!(value["observed"]["actionable"], true);
}

#[test]
fn element_wait_actionable_retries_until_live_state_converges() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_with_disabled_ref();
    let adapter = FlippingPredicateAdapter {
        states: Mutex::new(vec![vec![], vec!["disabled".into()]]),
    };

    let value = wait_for_element(
        "@e1".into(),
        Some(snapshot_id),
        wait_predicate::ElementPredicate::Actionable,
        250,
        &adapter,
        &crate::context::CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["predicate"], "actionable");
    assert_eq!(value["observed"]["actionable"], true);
}

#[test]
fn element_wait_propagates_live_read_errors_after_releasing_handle() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_with_one_ref();
    let adapter = LiveErrorPredicateAdapter {
        releases: AtomicU32::new(0),
    };

    let err = wait_for_element(
        "@e1".into(),
        Some(snapshot_id),
        wait_predicate::ElementPredicate::Enabled,
        250,
        &adapter,
        &crate::context::CommandContext::default(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "PERM_DENIED");
    assert_eq!(adapter.releases.load(Ordering::SeqCst), 1);
}
