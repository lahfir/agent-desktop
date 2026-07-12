use crate::{
    AppError,
    action::Action,
    adapter::PlatformAdapter,
    commands::helpers::{RefArgs, execute_ref_action_with_context},
    context::CommandContext,
};
use serde_json::Value;

pub struct SelectArgs {
    pub ref_id: String,
    pub snapshot_id: Option<String>,
    pub value: String,
    pub timeout_ms: Option<u64>,
}

pub fn execute(
    args: SelectArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let request = context.request_base(Action::Select(args.value));
    execute_ref_action_with_context(
        RefArgs {
            ref_id: args.ref_id,
            snapshot_id: args.snapshot_id,
            timeout_ms: args.timeout_ms,
        },
        adapter,
        request,
        context,
    )
}

#[cfg(test)]
#[path = "select_tests.rs"]
mod tests;
