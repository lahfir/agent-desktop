use crate::{
    action::Action,
    action_request::ActionRequest,
    adapter::PlatformAdapter,
    commands::helpers::{RefArgs, execute_ref_action_with_context},
    context::CommandContext,
    error::AppError,
    interaction_policy::InteractionPolicy,
};
use serde_json::Value;

/// Executes an action addressed by a snapshot ref through the canonical
/// ref-action pipeline: `RefStore` load → `RefMap` lookup → strict element
/// resolution → live actionability preflight → dispatch.
///
/// `snapshot_id` follows CLI `--snapshot` semantics: `None` pins to the
/// latest snapshot for the session; `Some(id)` pins to that specific snapshot.
///
/// The effective `InteractionPolicy` is the join of `caller_policy` and the
/// action's CLI base policy, ensuring the result is always at least as
/// permissive as what the CLI would use for the same action, while allowing
/// FFI callers to opt in to higher-permission policies such as `headed`.
///
/// Note on PressKey: its CLI base is `focus_fallback` (it may need focus to
/// land keystrokes in the right field). This differs from the previous FFI
/// behaviour where PressKey used `headless` as its base. The change is
/// intentional — it aligns the FFI with the full CLI base-policy table.
pub fn execute(
    ref_id: &str,
    snapshot_id: Option<&str>,
    action: Action,
    caller_policy: InteractionPolicy,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let base = action.base_interaction_policy();
    let effective = base.join(caller_policy);
    let request = ActionRequest {
        action,
        policy: effective,
    };
    execute_ref_action_with_context(
        RefArgs {
            ref_id: ref_id.to_owned(),
            snapshot_id: snapshot_id.map(ToOwned::to_owned),
        },
        adapter,
        request,
        context,
    )
}
