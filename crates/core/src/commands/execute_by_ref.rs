use crate::{
    action::Action,
    action_request::ActionRequest,
    adapter::PlatformAdapter,
    commands::helpers::{RefArgs, execute_ref_action_with_context, normalize_action_timeout_ms},
    context::CommandContext,
    error::AppError,
    interaction_policy::InteractionPolicy,
};
use serde_json::Value;

pub const DEFAULT_ACTION_TIMEOUT_MS: u64 = 5000;

/// Config struct bundling the caller-supplied inputs to `execute` /
/// `execute_with_timeout`, keeping both functions within the 5-parameter
/// limit (mirrors the `RefArgs`/`ActionRequest` config-struct pattern used
/// elsewhere in this module).
pub struct ExecuteByRefArgs<'a> {
    pub ref_id: &'a str,
    pub snapshot_id: Option<&'a str>,
    pub action: Action,
    pub caller_policy: InteractionPolicy,
}

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
/// Note on PressKey: its base policy is `focus_fallback` (derived from
/// `Action::base_interaction_policy`, shared with `TypeText`) because a
/// ref-targeted key press may need the target focused for keystrokes to land.
pub fn execute(
    args: ExecuteByRefArgs<'_>,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    execute_with_timeout(args, DEFAULT_ACTION_TIMEOUT_MS, adapter, context)
}

pub fn execute_with_timeout(
    args: ExecuteByRefArgs<'_>,
    timeout_ms: u64,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let ExecuteByRefArgs {
        ref_id,
        snapshot_id,
        action,
        caller_policy,
    } = args;
    let timeout_ms = normalize_action_timeout_ms(timeout_ms);
    let base = action.base_interaction_policy();
    let effective = base.join(caller_policy);
    let request = ActionRequest {
        action,
        policy: effective,
        timeout_ms,
    };
    execute_ref_action_with_context(
        RefArgs {
            ref_id: ref_id.to_owned(),
            snapshot_id: snapshot_id.map(ToOwned::to_owned),
            timeout_ms,
        },
        adapter,
        request,
        context,
    )
}

#[cfg(test)]
#[path = "execute_by_ref_tests.rs"]
mod tests;
