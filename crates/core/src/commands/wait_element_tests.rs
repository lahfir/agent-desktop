use super::test_support::{
    PredicateAdapter, save_ref_in_session, snapshot_with_one_ref, wait_for_element_test,
};
use super::*;
use crate::{
    adapter::{NativeHandle, PlatformAdapter},
    commands::wait_predicate,
    element_state::ElementState,
    error::AdapterError,
    refs::RefEntry,
    refs_test_support::HomeGuard,
};
use std::sync::atomic::{AtomicU32, Ordering};

struct NoopAdapter;

impl PlatformAdapter for NoopAdapter {}

struct LiveErrorPredicateAdapter {
    releases: AtomicU32,
}

impl PlatformAdapter for LiveErrorPredicateAdapter {
    fn resolve_element_strict_with_timeout(
        &self,
        _entry: &RefEntry,
        _timeout: std::time::Duration,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn get_live_state(&self, _handle: &NativeHandle) -> Result<Option<ElementState>, AdapterError> {
        Err(AdapterError::permission_denied())
    }

    fn release_handle(&self, _handle: &NativeHandle) -> Result<(), AdapterError> {
        self.releases.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn snapshot_pinned_missing_ref_is_invalid_args() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_with_one_ref();

    let err = wait_for_element_test(
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
fn element_wait_explicit_session_snapshot_without_session_context() {
    let _guard = HomeGuard::new();
    let snapshot_id = save_ref_in_session("agent-a", Vec::new());
    let adapter = PredicateAdapter {
        state: Some(ElementState {
            role: "button".into(),
            states: vec![],
            value: None,
        }),
        value: None,
        bounds: None,
    };

    let value = wait_for_element_test(
        "@e1".into(),
        Some(snapshot_id),
        wait_predicate::ElementPredicate::Exists,
        50,
        &adapter,
        &crate::context::CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["found"], true);
    assert_eq!(value["predicate"], "exists");
}

#[test]
fn element_wait_propagates_live_read_errors_after_releasing_handle() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_with_one_ref();
    let adapter = LiveErrorPredicateAdapter {
        releases: AtomicU32::new(0),
    };

    let err = wait_for_element_test(
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

#[test]
fn zero_timeout_returns_timeout_before_any_resolution_attempt() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_with_one_ref();

    let err = wait_for_element_test(
        "@e1".into(),
        Some(snapshot_id),
        wait_predicate::ElementPredicate::Exists,
        0,
        &NoopAdapter,
        &crate::context::CommandContext::default(),
    )
    .unwrap_err();

    let AppError::Adapter(adapter_err) = err else {
        panic!("expected adapter error");
    };
    assert_eq!(adapter_err.code, crate::error::ErrorCode::Timeout);
    let details = adapter_err.details.expect("timeout should carry details");
    assert!(details["last_observed"].is_null());
}
