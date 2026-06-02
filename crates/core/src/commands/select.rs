use crate::{
    action::{Action, ActionRequest},
    adapter::PlatformAdapter,
    commands::helpers::resolve_ref_with_context,
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
    let (entry, handle) =
        resolve_ref_with_context(&args.ref_id, args.snapshot_id.as_deref(), adapter, context)?;
    let request = ActionRequest::headless(Action::Select(args.value));
    context.trace(
        "actionability.check.start",
        serde_json::json!({ "ref": args.ref_id, "action": request.action.name() }),
    )?;
    crate::actionability::check(&entry, &request)?;
    context.trace(
        "actionability.check.ok",
        serde_json::json!({ "ref": args.ref_id, "action": request.action.name() }),
    )?;
    let result = adapter.execute_action(handle.handle(), request)?;
    context.trace(
        "action.dispatch.ok",
        serde_json::json!({ "ref": args.ref_id }),
    )?;
    Ok(serde_json::to_value(result)?)
}
