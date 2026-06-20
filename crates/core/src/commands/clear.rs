use crate::{
    action::Action,
    adapter::PlatformAdapter,
    commands::helpers::{RefArgs, execute_ref_action_with_context},
    context::CommandContext,
    error::AppError,
    interaction_policy::InteractionPolicy,
};
use serde_json::Value;

pub fn execute(
    args: RefArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    execute_ref_action_with_context(
        args,
        adapter,
        context.request(Action::Clear, InteractionPolicy::headless()),
        context,
    )
}
