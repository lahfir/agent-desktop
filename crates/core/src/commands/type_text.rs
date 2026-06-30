use crate::{
    action::Action,
    adapter::PlatformAdapter,
    commands::helpers::{apply_post_action_wait, execute_ref_action_result_with_context},
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

    let request = context.request_base(Action::TypeText(args.text));
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
