use super::*;
use crate::{
    AdapterError, ErrorCode,
    action::Action,
    adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps},
    capability,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, Ordering},
};

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
            source_app: None,
            source_window_id: None,
            source_window_title: None,
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

/// Lease-free polling is followed by physical-pointer preflight and final
/// stability validation under the interaction lease before dispatch.
struct DoubleCheckAdapter {
    live_calls: AtomicU32,
    lease_held: Arc<AtomicBool>,
}

struct DoubleCheckLeaseGuard(Arc<AtomicBool>);

impl Drop for DoubleCheckLeaseGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

impl ObservationOps for DoubleCheckAdapter {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn get_live_element(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<crate::adapter::LiveElement, AdapterError> {
        let call = self.live_calls.fetch_add(1, Ordering::SeqCst) + 1;
        assert_eq!(
            self.lease_held.load(Ordering::SeqCst),
            call > 2,
            "poll reads must be lease-free and final validation must be leased"
        );
        let states = if call <= 2 {
            vec![]
        } else {
            vec!["disabled".to_string()]
        };
        Ok(crate::adapter::LiveElement {
            identity: crate::adapter::live_identity("Run"),
            state: crate::element_state::ElementState {
                role: "button".into(),
                states,
                value: None,
                enabled: Some(true),
                hidden: Some(false),
                offscreen: Some(false),
            },
            states_complete: true,
            bounds: Some(crate::Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            }),
            available_actions: vec![capability::CLICK.into()],
        })
    }

    fn get_element_bounds(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<Option<crate::Rect>, AdapterError> {
        Ok(Some(crate::Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        }))
    }

    fn hit_test(
        &self,
        _handle: &NativeHandle,
        _point: crate::Point,
        _deadline: crate::Deadline,
    ) -> Result<crate::hit_test::HitTestResult, AdapterError> {
        Ok(crate::hit_test::HitTestResult::ReachesTarget)
    }
}

impl ActionOps for DoubleCheckAdapter {
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

impl InputOps for DoubleCheckAdapter {}

impl SystemOps for DoubleCheckAdapter {
    fn acquire_interaction_lease(
        &self,
        deadline: crate::Deadline,
    ) -> Result<crate::InteractionLease, AdapterError> {
        assert_eq!(self.live_calls.load(Ordering::SeqCst), 2);
        self.lease_held.store(true, Ordering::SeqCst);
        Ok(crate::InteractionLease::guarded_with_contention(
            deadline,
            DoubleCheckLeaseGuard(Arc::clone(&self.lease_held)),
            3,
        ))
    }
}

#[test]
fn stability_revalidates_once_under_lease_before_dispatch() {
    let adapter = DoubleCheckAdapter {
        live_calls: AtomicU32::new(0),
        lease_held: Arc::new(AtomicBool::new(false)),
    };

    let result = execute_with_auto_wait(
        RefActionWaitCtx {
            adapter: &adapter,
            entry: &entry(),
            ref_id: "@e1",
            context: &CommandContext::default(),
        },
        ActionRequest::headed(Action::DoubleClick).with_timeout_ms(Some(5_000)),
        crate::ref_action::dispatch_resolved,
    )
    .unwrap();

    assert_eq!(result.action, "click");
    assert_eq!(adapter.live_calls.load(Ordering::SeqCst), 4);
    let metrics = &result.details.as_ref().unwrap()["auto_wait"];
    assert_eq!(metrics["read_only_resolve_attempts"], 2);
    assert_eq!(metrics["read_only_preflight_attempts"], 2);
    assert_eq!(metrics["lease_contention_count"], 3);
    assert!(metrics["lease_hold_ms"].as_u64().is_some());
    assert!(!adapter.lease_held.load(Ordering::SeqCst));
}

#[test]
fn final_attempt_reserves_the_mandatory_stability_tail() {
    let shared = crate::Deadline::after(20).expect("shared deadline");
    std::thread::sleep(std::time::Duration::from_millis(10));

    let final_attempt = super::final_attempt_deadline(shared).expect("final attempt deadline");

    assert!(final_attempt.remaining_ms() >= 90);
    assert_eq!(final_attempt.timeout_ms(), 100);
}

#[path = "ref_action_wait_unresponsive_tests.rs"]
mod unresponsive_tests;
