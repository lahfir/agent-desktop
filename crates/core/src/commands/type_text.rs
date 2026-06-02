use crate::{
    action::{Action, ActionRequest},
    adapter::PlatformAdapter,
    commands::helpers::{check_actionability_with_trace, resolve_ref_with_context},
    context::CommandContext,
    error::AppError,
};
use serde_json::Value;

const MAX_TEXT_LEN: usize = 10_000;

pub struct TypeArgs {
    pub ref_id: String,
    pub snapshot_id: Option<String>,
    pub text: String,
}

pub fn execute(
    args: TypeArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    if args.text.len() > MAX_TEXT_LEN {
        return Err(AppError::invalid_input(format!(
            "Text exceeds maximum length of {MAX_TEXT_LEN} characters"
        )));
    }

    let (entry, handle) =
        resolve_ref_with_context(&args.ref_id, args.snapshot_id.as_deref(), adapter, context)?;
    let request = ActionRequest::focus_fallback(Action::TypeText(args.text));
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
