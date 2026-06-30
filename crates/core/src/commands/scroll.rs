use crate::{
    action::{Action, Direction},
    adapter::PlatformAdapter,
    commands::helpers::{apply_post_action_wait, execute_ref_action_result_with_context},
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
    let (entry, result) = execute_ref_action_result_with_context(
        &args.ref_id,
        args.snapshot_id.as_deref(),
        adapter,
        request,
        context,
    )?;
    apply_post_action_wait(
        serde_json::to_value(result)?,
        entry.source_app.as_deref(),
        adapter,
        context,
    )
}
