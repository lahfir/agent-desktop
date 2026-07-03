use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{
    action::Direction,
    action_request::ActionRequest,
    action_result::ActionResult,
    adapter::NativeHandle,
    error::{AdapterError, ErrorCode},
    refs::{RefEntry, RefMap},
    refs_store::RefStore,
    refs_test_support::HomeGuard,
};
use std::sync::atomic::{AtomicU32, Ordering};

struct StaleThenOkAdapter {
    resolve_calls: AtomicU32,
    fail_until: u32,
}

impl StaleThenOkAdapter {
    fn new(fail_until: u32) -> Self {
        Self {
            resolve_calls: AtomicU32::new(0),
            fail_until,
        }
    }
}

impl ObservationOps for StaleThenOkAdapter {
    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        let n = self.resolve_calls.fetch_add(1, Ordering::SeqCst) + 1;
        if n <= self.fail_until {
            return Err(AdapterError::new(ErrorCode::StaleRef, "not yet resolvable"));
        }
        Ok(NativeHandle::null())
    }
}

impl ActionOps for StaleThenOkAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
    ) -> Result<ActionResult, AdapterError> {
        Ok(ActionResult::new("scroll"))
    }
}

impl InputOps for StaleThenOkAdapter {}
impl SystemOps for StaleThenOkAdapter {}

fn snapshot_id() -> String {
    let mut refmap = RefMap::new();
    refmap.allocate(RefEntry {
        pid: 1,
        role: "scrollarea".into(),
        name: Some("Target".into()),
        value: None,
        description: None,
        native_id: None,
        states: vec![],
        bounds: None,
        bounds_hash: None,
        available_actions: vec!["Scroll".into()],
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

/// Regression for the F2 fix: before it, `scroll::execute` always built its
/// `RefArgs` with `timeout_ms: None`, so a caller-supplied `--timeout-ms`
/// budget was silently dropped and the command never retried a transient
/// `STALE_REF`.
#[test]
fn transient_stale_ref_retries_then_succeeds_when_timeout_wired() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_id();
    let adapter = StaleThenOkAdapter::new(2);

    let value = execute(
        ScrollArgs {
            ref_id: "@e1".into(),
            snapshot_id: Some(snapshot_id),
            direction: Direction::Down,
            amount: 3,
            timeout_ms: Some(5_000),
        },
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["action"], "scroll");
    assert!(adapter.resolve_calls.load(Ordering::SeqCst) >= 3);
}

#[test]
fn timeout_none_makes_exactly_one_resolve_attempt() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_id();
    let adapter = StaleThenOkAdapter::new(1);

    let err = execute(
        ScrollArgs {
            ref_id: "@e1".into(),
            snapshot_id: Some(snapshot_id),
            direction: Direction::Down,
            amount: 3,
            timeout_ms: None,
        },
        &adapter,
        &CommandContext::default(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "STALE_REF");
    assert_eq!(adapter.resolve_calls.load(Ordering::SeqCst), 1);
}
