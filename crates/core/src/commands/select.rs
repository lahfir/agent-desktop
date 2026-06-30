use crate::{
    action::Action,
    adapter::PlatformAdapter,
    commands::helpers::{RefArgs, execute_ref_action_with_context},
    context::CommandContext,
    error::AppError,
};
use serde_json::Value;

pub struct SelectArgs {
    pub ref_id: String,
    pub snapshot_id: Option<String>,
    pub value: String,
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
        },
        adapter,
        request,
        context,
    )
}
