use super::*;
use crate::{
    AdapterError, ErrorCode,
    action::Action,
    adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps},
    capability,
};
use std::sync::atomic::{AtomicU32, Ordering};

#[test]
fn oversized_timeout_budget_is_clamped_and_never_overflows() {
    assert_eq!(crate::Deadline::after(100).unwrap().timeout_ms(), 100);
    assert_eq!(
        crate::Deadline::after(u64::MAX).unwrap_err().code,
        ErrorCode::InvalidArgs
    );
}

#[test]
fn actionability_timeout_exposes_the_last_report_contract() {
    let error = crate::ref_action_poll::timeout_with_last_report(serde_json::json!({
        "phase": "preflight"
    }));

    assert_eq!(
        error
            .details
            .as_ref()
            .and_then(|details| details.get("last_report")),
        Some(&serde_json::json!({ "phase": "preflight" }))
    );
    assert!(
        error
            .details
            .as_ref()
            .is_none_or(|details| details.get("report").is_none())
    );
}

struct RetryAdapter {
    resolve_calls: AtomicU32,
}

impl ObservationOps for RetryAdapter {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        self.resolve_calls.fetch_add(1, Ordering::SeqCst);
        if self.resolve_calls.load(Ordering::SeqCst) < 3 {
            return Err(AdapterError::new(ErrorCode::StaleRef, "not yet")
                .with_details(serde_json::json!({ "retryable": true })));
        }
        Ok(NativeHandle::null())
    }

    crate::adapter::complete_live_observation!("button", "Run", [capability::CLICK]);
}

impl ActionOps for RetryAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
        _lease: &crate::InteractionLease,
    ) -> Result<crate::action_result::ActionResult, AdapterError> {
        Ok(crate::action_result::ActionResult::delivered_unverified(
            "click",
        ))
    }
}

impl InputOps for RetryAdapter {}

impl SystemOps for RetryAdapter {
    crate::adapter::guarded_interaction_lease!();
}

struct AmbiguousThenOkAdapter {
    resolve_calls: AtomicU32,
}

impl ObservationOps for AmbiguousThenOkAdapter {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        self.resolve_calls.fetch_add(1, Ordering::SeqCst);
        if self.resolve_calls.load(Ordering::SeqCst) == 1 {
            return Err(AdapterError::ambiguous_target("2 candidates")
                .with_details(serde_json::json!({ "retryable": true })));
        }
        Ok(NativeHandle::null())
    }

    crate::adapter::complete_live_observation!("button", "Run", [capability::CLICK]);
}

impl ActionOps for AmbiguousThenOkAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
        _lease: &crate::InteractionLease,
    ) -> Result<crate::action_result::ActionResult, AdapterError> {
        Ok(crate::action_result::ActionResult::delivered_unverified(
            "click",
        ))
    }
}

impl InputOps for AmbiguousThenOkAdapter {}

impl SystemOps for AmbiguousThenOkAdapter {
    crate::adapter::guarded_interaction_lease!();
}

fn entry() -> RefEntry {
    let bounds = crate::Rect {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    RefEntry {
        process: crate::RefProcess {
            pid: crate::ProcessId::new(1),
            process_instance: Some("test-instance".into()),
        },
        identity: crate::RefEntryIdentity {
            role: "button".into(),
            name: Some("Run".into()),
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
            available_actions: vec![capability::CLICK.into()],
        },
        source: crate::RefSource {
            source_app: Some("Fixture".into()),
            source_window_id: Some("w-1".into()),
            source_window_title: Some("Fixture".into()),
            source_window_bounds_hash: None,
            source_surface: crate::snapshot_surface::SnapshotSurface::Window,
        },
        scope: crate::RefScope {
            root_ref: None,
            path_is_absolute: false,
            path: smallvec::SmallVec::new(),
        },
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
        RefActionWaitCtx {
            adapter: &adapter,
            entry: &entry(),
            ref_id: "@e1",
            context: &CommandContext::default(),
        },
        ActionRequest::headless(Action::Click),
        crate::ref_action::dispatch_resolved,
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
        RefActionWaitCtx {
            adapter: &adapter,
            entry: &entry(),
            ref_id: "@e1",
            context: &CommandContext::default(),
        },
        request_with_timeout(5_000),
        crate::ref_action::dispatch_resolved,
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
        RefActionWaitCtx {
            adapter: &adapter,
            entry: &entry(),
            ref_id: "@e1",
            context: &CommandContext::default(),
        },
        request_with_timeout(5_000),
        crate::ref_action::dispatch_resolved,
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

#[path = "ref_action_wait_lease_tests.rs"]
mod lease_tests;

#[path = "ref_action_wait_unresponsive_tests.rs"]
mod unresponsive_tests;
