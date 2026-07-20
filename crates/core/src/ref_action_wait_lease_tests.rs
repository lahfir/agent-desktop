use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, Ordering},
};

struct DoubleCheckAdapter {
    live_calls: AtomicU32,
    lease_held: Arc<AtomicBool>,
    prelease_calls: u32,
    timeout_ms: u64,
    expected_deadline: Option<crate::Deadline>,
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
            call > self.prelease_calls,
            "poll reads must be lease-free and final validation must be leased"
        );
        let states = if call <= self.prelease_calls {
            Vec::new()
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
            bounds: Some(bounds()),
            available_actions: vec![capability::CLICK.into()],
        })
    }

    fn get_element_bounds(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<Option<crate::Rect>, AdapterError> {
        Ok(Some(bounds()))
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
    crate::adapter::exact_window_focus!();

    fn acquire_interaction_lease(
        &self,
        deadline: crate::Deadline,
    ) -> Result<crate::InteractionLease, AdapterError> {
        assert_eq!(self.live_calls.load(Ordering::SeqCst), self.prelease_calls);
        assert_eq!(deadline.timeout_ms(), self.timeout_ms);
        if let Some(expected) = self.expected_deadline {
            assert_eq!(deadline, expected);
            return Err(AdapterError::new(ErrorCode::Internal, "lease probe"));
        }
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
    let adapter = adapter(2, 5_000, None);
    let result = run(&adapter, ActionRequest::headed(Action::DoubleClick), 5_000);

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
fn lease_acquisition_receives_the_exact_original_deadline() {
    let deadline = crate::Deadline::after(80).unwrap();
    let adapter = adapter(0, 80, Some(deadline));
    let error = super::super::execute_with_deadline(
        RefActionWaitCtx {
            adapter: &adapter,
            entry: &entry(),
            ref_id: "@e1",
            context: &CommandContext::default(),
        },
        ActionRequest::headless(Action::Click),
        deadline,
        crate::ref_action::dispatch_resolved,
    )
    .err()
    .unwrap();

    assert_eq!(error.code, ErrorCode::Internal);
    assert_eq!(error.message, "lease probe");
    assert_eq!(adapter.live_calls.load(Ordering::SeqCst), 0);
    assert!(!adapter.lease_held.load(Ordering::SeqCst));
}

fn adapter(
    prelease_calls: u32,
    timeout_ms: u64,
    expected_deadline: Option<crate::Deadline>,
) -> DoubleCheckAdapter {
    DoubleCheckAdapter {
        live_calls: AtomicU32::new(0),
        lease_held: Arc::new(AtomicBool::new(false)),
        prelease_calls,
        timeout_ms,
        expected_deadline,
    }
}

fn run(
    adapter: &DoubleCheckAdapter,
    request: ActionRequest,
    timeout_ms: u64,
) -> crate::action_result::ActionResult {
    execute_with_auto_wait(
        RefActionWaitCtx {
            adapter,
            entry: &entry(),
            ref_id: "@e1",
            context: &CommandContext::default(),
        },
        request.with_timeout_ms(Some(timeout_ms)),
        crate::ref_action::dispatch_resolved,
    )
    .unwrap()
}

fn bounds() -> crate::Rect {
    crate::Rect {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    }
}
