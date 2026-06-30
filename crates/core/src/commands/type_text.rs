use crate::{
    action::Action,
    adapter::PlatformAdapter,
    commands::helpers::{RefArgs, execute_ref_action_with_context},
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
