use crate::{AdapterError, AppError, CommandContext, InteractionPolicy};

pub(crate) fn mutation_policy(context: &CommandContext) -> Result<InteractionPolicy, AppError> {
    let policy = context.physical_input_policy();
    if !policy.allow_focus_steal {
        return Err(AdapterError::policy_denied_for_policy(
            "Notification mutations open and focus the operating system notification surface",
            policy,
        )
        .into());
    }
    Ok(policy)
}
