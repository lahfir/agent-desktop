use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{
    AdapterError, Direction,
    action_request::ActionRequest,
    action_result::ActionResult,
    adapter::NativeHandle,
    commands::stale_retry_test_support::StaleRetryCounter,
    refs::{RefEntry, RefMap},
    refs_store::RefStore,
    refs_test_support::HomeGuard,
};

struct StaleThenOkAdapter {
    retry: StaleRetryCounter,
}

impl StaleThenOkAdapter {
    fn new(fail_until: u32) -> Self {
        Self {
            retry: StaleRetryCounter::new(fail_until),
        }
    }
}

impl ObservationOps for StaleThenOkAdapter {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        self.retry.attempt()
    }

    crate::adapter::complete_live_observation!("scrollarea", "Target", [crate::capability::SCROLL]);
}

impl ActionOps for StaleThenOkAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
        _lease: &crate::InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        Ok(ActionResult::delivered_unverified("scroll"))
    }
}

impl InputOps for StaleThenOkAdapter {}
impl SystemOps for StaleThenOkAdapter {
    crate::adapter::guarded_interaction_lease!();
}

fn snapshot_id() -> String {
    let mut refmap = RefMap::new();
    let bounds = crate::Rect {
        x: 1.0,
        y: 1.0,
        width: 20.0,
        height: 20.0,
    };
    refmap.allocate(RefEntry {
        process: crate::RefProcess {
            pid: crate::ProcessId::new(1),
            process_instance: Some("test-instance".into()),
        },
        identity: crate::RefEntryIdentity {
            role: "scrollarea".into(),
            name: Some("Target".into()),
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
            available_actions: vec!["Scroll".into()],
        },
        source: crate::RefSource {
            source_app: None,
            source_window_id: None,
            source_window_title: None,
            source_window_bounds_hash: None,
            source_surface: crate::adapter::SnapshotSurface::Window,
        },
        scope: crate::RefScope {
            root_ref: None,
            path_is_absolute: false,
            path: smallvec::SmallVec::new(),
        },
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
    assert!(adapter.retry.calls() >= 3);
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
    assert_eq!(adapter.retry.calls(), 1);
}
