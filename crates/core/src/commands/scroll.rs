use crate::{
    action::{Action, ActionRequest, Direction},
    adapter::PlatformAdapter,
    commands::helpers::{check_actionability_with_trace, resolve_ref_with_context},
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
    let (entry, handle) =
        resolve_ref_with_context(&args.ref_id, args.snapshot_id.as_deref(), adapter, context)?;
    let request = ActionRequest::headless(Action::Scroll(args.direction, args.amount));
    check_actionability_with_trace(
        &args.ref_id,
        &entry,
        handle.handle(),
        adapter,
        &request,
        context,
    )?;
    let result = adapter.execute_action(handle.handle(), request)?;
    context.trace(
        "action.dispatch.ok",
        serde_json::json!({ "ref": args.ref_id }),
    )?;
    Ok(serde_json::to_value(result)?)
}
