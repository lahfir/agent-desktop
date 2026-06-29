use super::{PressArgs, execute};
use crate::action::KeyCombo;
use crate::action_request::ActionRequest;
use crate::action_result::ActionResult;
use crate::adapter::{NativeHandle, PlatformAdapter};
use crate::error::AdapterError;

struct BlockingAdapter;

impl PlatformAdapter for BlockingAdapter {
    fn is_blocked_combo(&self, _combo: &KeyCombo) -> bool {
        true
    }

    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
    ) -> Result<ActionResult, AdapterError> {
        Ok(ActionResult::new("PressKey"))
    }
}

struct AllowingAdapter;

impl PlatformAdapter for AllowingAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
    ) -> Result<ActionResult, AdapterError> {
        Ok(ActionResult::new("PressKey"))
    }
}

fn args(combo: &str, force: bool) -> PressArgs {
    PressArgs {
        combo: combo.to_owned(),
        app: None,
        force,
    }
}

#[test]
fn adapter_blocked_combo_is_refused_when_not_forced() {
    let err = execute(args("cmd+q", false), &BlockingAdapter).unwrap_err();
    assert_eq!(err.code(), "POLICY_DENIED");
    assert!(
        err.to_string().contains("--force"),
        "the refusal must tell the caller how to override, got: {err}"
    );
}

#[test]
fn force_bypasses_the_adapter_block() {
    execute(args("cmd+q", true), &BlockingAdapter)
        .expect("--force must let the agent send a blocked combo");
}

#[test]
fn core_blocks_nothing_by_default() {
    execute(args("cmd+q", false), &AllowingAdapter)
        .expect("core must not hardcode any block; the default adapter allows everything");
}
