use crate::{
    AppError,
    actions::action::Action,
    adapter::PlatformAdapter,
    commands::helpers::{RefArgs, execute_ref_action_with_context},
    context::CommandContext,
};
use serde_json::Value;

const MAX_TEXT_LEN: usize = 10_000;
const TYPING_BUDGET_PER_CHAR_MS: u64 = 30;

/// The budget in milliseconds for typing `text` when the caller names none:
/// the standard operation budget plus a fixed allowance for each character,
/// so a long payload is not cut off part way through by a budget sized for a
/// single click.
pub fn default_timeout_ms(text: &str) -> u64 {
    let characters = u64::try_from(text.chars().count()).unwrap_or(u64::MAX);
    crate::DEFAULT_OPERATION_TIMEOUT_MS
        .saturating_add(characters.saturating_mul(TYPING_BUDGET_PER_CHAR_MS))
}

pub struct TypeArgs {
    pub ref_id: String,
    pub text: String,
    pub timeout_ms: Option<u64>,
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
            timeout_ms: args.timeout_ms,
        },
        adapter,
        request,
        context,
    )
}

#[cfg(test)]
#[path = "type_text_tests.rs"]
mod tests;
