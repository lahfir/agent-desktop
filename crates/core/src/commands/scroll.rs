use crate::{
    action::{Action, Direction},
    adapter::PlatformAdapter,
    commands::helpers::{RefArgs, execute_ref_action_with_context},
    context::CommandContext,
    error::AppError,
};
use serde_json::Value;

pub struct ScrollArgs {
    pub ref_id: String,
    pub snapshot_id: Option<String>,
    pub direction: Direction,
    pub amount: u32,
}

pub fn execute(
    args: ScrollArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let request = context.request_base(Action::Scroll(args.direction, args.amount));
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
