use crate::{
    action::Action, action_request::ActionRequest, adapter::PlatformAdapter,
    commands::helpers::execute_ref_action_result_with_context, context::CommandContext,
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

    let request = ActionRequest::focus_fallback(Action::TypeText(args.text));
    let (_entry, result) = execute_ref_action_result_with_context(
        &args.ref_id,
        args.snapshot_id.as_deref(),
        adapter,
        request,
        context,
    )?;
    Ok(serde_json::to_value(result)?)
}
