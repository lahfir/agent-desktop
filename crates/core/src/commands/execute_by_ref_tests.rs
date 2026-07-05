use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{
    action::{Action, KeyCombo},
    action_request::ActionRequest,
    action_result::ActionResult,
    adapter::NativeHandle,
    error::{AdapterError, ErrorCode},
    refs::{RefEntry, RefMap},
    refs_store::RefStore,
    refs_test_support::HomeGuard,
};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

fn snapshot_with_ref(role: &str, available_actions: &[&str]) -> String {
    let mut refmap = RefMap::new();
    refmap.allocate(RefEntry {
        pid: 1,
        role: role.into(),
        name: Some("Target".into()),
        value: None,
        description: None,
        native_id: None,
        states: vec![],
        bounds: None,
        bounds_hash: None,
        available_actions: available_actions.iter().map(|a| (*a).to_string()).collect(),
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
        Ok(ActionResult::new("click"))
    }
}

impl InputOps for StaleThenOkAdapter {}
impl SystemOps for StaleThenOkAdapter {}

struct PolicyCaptureAdapter {
    captured: Mutex<Option<ActionRequest>>,
}

impl PolicyCaptureAdapter {
    fn new() -> Self {
        Self {
            captured: Mutex::new(None),
        }
    }
}

impl ObservationOps for PolicyCaptureAdapter {
    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }
}

impl ActionOps for PolicyCaptureAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        request: ActionRequest,
    ) -> Result<ActionResult, AdapterError> {
        let name = request.action.name().to_string();
        *self.captured.lock().unwrap() = Some(request);
        Ok(ActionResult::new(name))
    }
}

impl InputOps for PolicyCaptureAdapter {}
impl SystemOps for PolicyCaptureAdapter {}

#[test]
fn default_action_timeout_ms_is_five_seconds() {
    assert_eq!(DEFAULT_ACTION_TIMEOUT_MS, 5000);
}

/// `execute` must forward `DEFAULT_ACTION_TIMEOUT_MS` (not `None` or `0`) into
/// the ref-action retry budget, exactly as a direct
/// `execute_with_timeout(args, DEFAULT_ACTION_TIMEOUT_MS, ..)` call would.
/// A transient `STALE_REF` that clears within the budget must therefore be
/// retried until it resolves rather than surfacing on the first attempt.
#[test]
fn execute_forwards_default_timeout_and_retries_transient_stale_ref() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_with_ref("button", &["Click"]);
    let adapter = StaleThenOkAdapter::new(2);

    let value = execute(
        ExecuteByRefArgs {
            ref_id: "@e1",
            snapshot_id: Some(&snapshot_id),
            action: Action::Click,
            caller_policy: InteractionPolicy::headless(),
        },
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["action"], "click");
    assert!(adapter.resolve_calls.load(Ordering::SeqCst) >= 3);
}

/// `execute_with_timeout` must run `normalize_action_timeout_ms` on the raw
/// timeout it is given, so a `0` (the CLI's "no retry budget" sentinel)
/// collapses to `None` and the ref-action pipeline makes exactly one resolve
/// attempt instead of silently retrying anyway.
#[test]
fn execute_with_timeout_zero_normalizes_to_single_attempt() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_with_ref("button", &["Click"]);
    let adapter = StaleThenOkAdapter::new(1);

    let err = execute_with_timeout(
        ExecuteByRefArgs {
            ref_id: "@e1",
            snapshot_id: Some(&snapshot_id),
            action: Action::Click,
            caller_policy: InteractionPolicy::headless(),
        },
        0,
        &adapter,
        &CommandContext::default(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "STALE_REF");
    assert_eq!(adapter.resolve_calls.load(Ordering::SeqCst), 1);
}

/// The effective policy is a join, not a passthrough of `caller_policy`: a
/// caller supplying `headless` for an action whose CLI base is
/// `focus_fallback` (here `PressKey`) must still end up with
/// `allow_focus_steal = true`, because the join can only elevate the caller
/// above the action's base, never downgrade below it.
#[test]
fn effective_policy_never_drops_below_action_base() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_with_ref("textfield", &["PressKey"]);
    let adapter = PolicyCaptureAdapter::new();

    execute(
        ExecuteByRefArgs {
            ref_id: "@e1",
            snapshot_id: Some(&snapshot_id),
            action: Action::PressKey(KeyCombo {
                key: "A".into(),
                modifiers: vec![],
            }),
            caller_policy: InteractionPolicy::headless(),
        },
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    let captured = adapter.captured.lock().unwrap();
    let policy = captured.as_ref().unwrap().policy;
    assert!(policy.allow_focus_steal);
    assert!(!policy.allow_cursor_move);
}

/// Symmetric case: a caller-supplied policy more permissive than the action's
/// base must be honored, not clamped back down to the base. `Click`'s base is
/// `headless` (both flags false); a caller passing `headed` must see both
/// flags come through as `true` on the dispatched request.
#[test]
fn effective_policy_honors_caller_policy_above_action_base() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_with_ref("button", &["Click"]);
    let adapter = PolicyCaptureAdapter::new();

    execute(
        ExecuteByRefArgs {
            ref_id: "@e1",
            snapshot_id: Some(&snapshot_id),
            action: Action::Click,
            caller_policy: InteractionPolicy::headed(),
        },
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    let captured = adapter.captured.lock().unwrap();
    let policy = captured.as_ref().unwrap().policy;
    assert!(policy.allow_focus_steal);
    assert!(policy.allow_cursor_move);
}
