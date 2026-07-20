use super::test_support::{
    PredicateAdapter, save_ref_in_session, snapshot_with_one_ref, wait_for_element_test,
};
use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{
    AdapterError, adapter::NativeHandle, commands::wait_predicate, element_state::ElementState,
    refs::RefEntry, refs_test_support::HomeGuard,
};
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

struct NoopAdapter;

impl ObservationOps for NoopAdapter {}

impl ActionOps for NoopAdapter {}

impl InputOps for NoopAdapter {}

impl SystemOps for NoopAdapter {}

struct LiveErrorPredicateAdapter {
    drops: Arc<AtomicU32>,
}

struct DropProbe(Arc<AtomicU32>);

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

impl ObservationOps for LiveErrorPredicateAdapter {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::new(DropProbe(Arc::clone(&self.drops))))
    }

    fn get_live_state(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<Option<ElementState>, AdapterError> {
        Err(AdapterError::permission_denied())
    }
}

impl ActionOps for LiveErrorPredicateAdapter {}

impl InputOps for LiveErrorPredicateAdapter {}

impl SystemOps for LiveErrorPredicateAdapter {}

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
fn element_wait_explicit_session_snapshot_with_matching_session_context() {
    let _guard = HomeGuard::new();
    let snapshot_id = save_ref_in_session("agent-a", Vec::new());
    let adapter = PredicateAdapter {
        state: Some(ElementState {
            role: "button".into(),
            states: vec![],
            value: None,
            enabled: Some(true),
            hidden: Some(false),
            offscreen: Some(false),
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
        &crate::context::CommandContext::new(Some("agent-a".into()), None, false).unwrap(),
    )
    .unwrap();

    assert_eq!(value["found"], true);
    assert_eq!(value["predicate"], "exists");
}

#[test]
fn element_wait_propagates_live_read_errors_after_dropping_handle() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_with_one_ref();
    let adapter = LiveErrorPredicateAdapter {
        drops: Arc::new(AtomicU32::new(0)),
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
    assert_eq!(adapter.drops.load(Ordering::SeqCst), 1);
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
    assert_eq!(adapter_err.code, crate::ErrorCode::Timeout);
    let details = adapter_err.details.expect("timeout should carry details");
    assert!(details["last_observed"].is_null());
}
