use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::commands::helpers::test_support::save_one_ref_snapshot;
use crate::{
    AdapterError, action_request::ActionRequest, action_result::ActionResult,
    adapter::NativeHandle, commands::stale_retry_test_support::StaleRetryCounter, refs::RefEntry,
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

    crate::adapter::complete_live_observation!(
        "textfield",
        "Target",
        [crate::capability::TYPE_TEXT]
    );
}

impl ActionOps for StaleThenOkAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
        _lease: &crate::InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        Ok(ActionResult::delivered_unverified("type_text"))
    }
}

impl InputOps for StaleThenOkAdapter {}
impl SystemOps for StaleThenOkAdapter {
    crate::adapter::guarded_interaction_lease!();
}

fn snapshot_id() -> String {
    save_one_ref_snapshot("textfield", "SetValue")
}

/// Regression for the F2 fix: before it, `type_text::execute` always built its
/// `RefArgs` with `timeout_ms: None`, so a caller-supplied `--timeout-ms`
/// budget was silently dropped and the command never retried a transient
/// `STALE_REF`. This proves the CLI-supplied budget is actually wired through
/// by observing a real retry-then-succeed round trip via `execute()`.
#[test]
fn transient_stale_ref_retries_then_succeeds_when_timeout_wired() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_id();
    let adapter = StaleThenOkAdapter::new(2);

    let value = execute(
        TypeArgs {
            ref_id: "@e1".into(),
            snapshot_id: Some(snapshot_id),
            text: "hi".into(),
            timeout_ms: Some(5_000),
        },
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["action"], "type_text");
    assert!(adapter.retry.calls() >= 3);
}

#[test]
fn timeout_none_makes_exactly_one_resolve_attempt() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_id();
    let adapter = StaleThenOkAdapter::new(1);

    let err = execute(
        TypeArgs {
            ref_id: "@e1".into(),
            snapshot_id: Some(snapshot_id),
            text: "hi".into(),
            timeout_ms: None,
        },
        &adapter,
        &CommandContext::default(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "STALE_REF");
    assert_eq!(adapter.retry.calls(), 1);
}
