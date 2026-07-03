use super::*;
use crate::{
    action::Action,
    adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps},
    capability,
    error::{AdapterError, ErrorCode},
};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

#[test]
fn oversized_timeout_budget_is_clamped_and_never_overflows() {
    assert_eq!(budget_from_ms(100), Duration::from_millis(100));
    let clamped = budget_from_ms(u64::MAX);
    assert!(
        std::time::Instant::now().checked_add(clamped).is_some(),
        "deadline construction must not overflow for an oversized --timeout-ms"
    );
}

struct RetryAdapter {
    resolve_calls: AtomicU32,
}

impl ObservationOps for RetryAdapter {
    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        self.resolve_calls.fetch_add(1, Ordering::SeqCst);
        if self.resolve_calls.load(Ordering::SeqCst) < 3 {
            return Err(AdapterError::new(ErrorCode::StaleRef, "not yet"));
        }
        Ok(NativeHandle::null())
    }

    fn resolve_element_strict_with_timeout(
        &self,
        entry: &RefEntry,
        _timeout: Duration,
    ) -> Result<NativeHandle, AdapterError> {
        self.resolve_element_strict(entry)
    }
}

impl ActionOps for RetryAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
    ) -> Result<crate::action_result::ActionResult, AdapterError> {
        Ok(crate::action_result::ActionResult::new("click"))
    }
}

impl InputOps for RetryAdapter {}

impl SystemOps for RetryAdapter {}

struct AmbiguousThenOkAdapter {
    resolve_calls: AtomicU32,
}

impl ObservationOps for AmbiguousThenOkAdapter {
    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        self.resolve_calls.fetch_add(1, Ordering::SeqCst);
        if self.resolve_calls.load(Ordering::SeqCst) == 1 {
            return Err(AdapterError::ambiguous_target("2 candidates"));
        }
        Ok(NativeHandle::null())
    }

    fn resolve_element_strict_with_timeout(
        &self,
        entry: &RefEntry,
        _timeout: Duration,
    ) -> Result<NativeHandle, AdapterError> {
        self.resolve_element_strict(entry)
    }
}

impl ActionOps for AmbiguousThenOkAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
    ) -> Result<crate::action_result::ActionResult, AdapterError> {
        Ok(crate::action_result::ActionResult::new("click"))
    }
}

impl InputOps for AmbiguousThenOkAdapter {}

impl SystemOps for AmbiguousThenOkAdapter {}

fn entry() -> RefEntry {
    RefEntry {
        pid: 1,
        role: "button".into(),
        name: Some("Run".into()),
        value: None,
        description: None,
        native_id: None,
        states: vec![],
        bounds: Some(crate::node::Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        }),
        bounds_hash: Some(1),
        available_actions: vec![capability::CLICK.into()],
        source_app: None,
        source_window_id: None,
        source_window_title: None,
        source_surface: crate::snapshot_surface::SnapshotSurface::Window,
        root_ref: None,
        path_is_absolute: false,
        path: smallvec::SmallVec::new(),
    }
}

fn request_with_timeout(timeout_ms: u64) -> ActionRequest {
    ActionRequest::headless(Action::Click).with_timeout_ms(Some(timeout_ms))
}

#[test]
fn none_timeout_uses_single_resolve_attempt() {
    let adapter = RetryAdapter {
        resolve_calls: AtomicU32::new(0),
    };
    let err = execute_with_auto_wait(
        &adapter,
        &entry(),
        "@e1",
        &CommandContext::default(),
        ActionRequest::headless(Action::Click),
        crate::ref_action::execute_resolved,
    )
    .unwrap_err();
    assert_eq!(adapter.resolve_calls.load(Ordering::SeqCst), 1);
    assert_eq!(err.code, ErrorCode::StaleRef);
}

#[test]
fn budget_timeout_retries_until_success() {
    let adapter = RetryAdapter {
        resolve_calls: AtomicU32::new(0),
    };
    let result = execute_with_auto_wait(
        &adapter,
        &entry(),
        "@e1",
        &CommandContext::default(),
        request_with_timeout(5_000),
        crate::ref_action::execute_resolved,
    )
    .unwrap();
    assert_eq!(result.action, "click");
    assert!(adapter.resolve_calls.load(Ordering::SeqCst) >= 3);
}

#[test]
fn transient_ambiguity_is_recorded_in_result_details() {
    let adapter = AmbiguousThenOkAdapter {
        resolve_calls: AtomicU32::new(0),
    };
    let result = execute_with_auto_wait(
        &adapter,
        &entry(),
        "@e1",
        &CommandContext::default(),
        request_with_timeout(5_000),
        crate::ref_action::execute_resolved,
    )
    .unwrap();
    assert_eq!(
        result
            .details
            .as_ref()
            .and_then(|d| d.get("transient_ambiguity")),
        Some(&serde_json::json!(true))
    );
}

/// Regression for the double `check_live` bug: `execute_poll_loop` used to
/// call `actionability::check_live` itself and then hand off to `dispatch`
/// (`execute_resolved`), which runs its own internal `check_live` — two live
/// reads per iteration instead of one. If a second call observed a
/// transiently different state (as this adapter's second `get_live_element`
/// call does), the failure propagated via a bare `?` that bypassed the
/// retry/permanent classification entirely, so the action never retried it.
/// With the fix there is exactly one `check_live` per iteration (inside
/// `dispatch`), so this succeeds on the first attempt with a single live
/// read. Reintroducing the loop-level check makes `get_live_element`'s
/// second-call "disabled" state observed within the very first iteration,
/// turning this `Ok` into an immediate `Err`.
struct DoubleCheckAdapter {
    live_calls: AtomicU32,
}

impl ObservationOps for DoubleCheckAdapter {
    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn resolve_element_strict_with_timeout(
        &self,
        _entry: &RefEntry,
        _timeout: Duration,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn get_live_element(
        &self,
        _handle: &NativeHandle,
    ) -> Result<crate::adapter::LiveElement, AdapterError> {
        let call = self.live_calls.fetch_add(1, Ordering::SeqCst) + 1;
        let states = if call == 1 {
            vec![]
        } else {
            vec!["disabled".to_string()]
        };
        Ok(crate::adapter::LiveElement {
            state: Some(crate::element_state::ElementState {
                role: "button".into(),
                states,
                value: None,
            }),
            bounds: Some(crate::node::Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            }),
            available_actions: Some(vec![capability::CLICK.into()]),
        })
    }
}

impl ActionOps for DoubleCheckAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
    ) -> Result<crate::action_result::ActionResult, AdapterError> {
        Ok(crate::action_result::ActionResult::new("click"))
    }
}

impl InputOps for DoubleCheckAdapter {}

impl SystemOps for DoubleCheckAdapter {}

#[test]
fn check_live_runs_exactly_once_per_iteration_not_twice() {
    let adapter = DoubleCheckAdapter {
        live_calls: AtomicU32::new(0),
    };

    let result = execute_with_auto_wait(
        &adapter,
        &entry(),
        "@e1",
        &CommandContext::default(),
        request_with_timeout(5_000),
        crate::ref_action::execute_resolved,
    )
    .unwrap();

    assert_eq!(result.action, "click");
    assert_eq!(adapter.live_calls.load(Ordering::SeqCst), 1);
}
