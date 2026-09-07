use crate::{
    AppError,
    action::Action,
    adapter::PlatformAdapter,
    commands::combo::{ensure_combo_allowed, parse_combo_normalized},
    context::CommandContext,
};
use serde_json::Value;

pub struct PressArgs {
    pub combo: String,
    pub app: Option<String>,
    pub force: bool,
}

pub fn execute(
    args: PressArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    super::helpers::validate_post_action_wait(context)?;
    let combo = parse_combo_normalized(&args.combo)?;
    ensure_combo_allowed(&combo, &args.combo, args.force, adapter)?;
    let deadline = crate::Deadline::standard()?;

    let result = if let Some(app_name) = &args.app {
        let expected = crate::commands::helpers::resolve_app(Some(app_name), adapter, deadline)?;
        let lease = adapter.acquire_interaction_lease(deadline)?;
        let live = crate::commands::helpers::revalidate_app_for_mutation(
            adapter,
            &expected,
            lease.deadline(),
        )?;
        let process = crate::commands::helpers::process_identity(&live)?;
        if context.physical_input_policy().is_headed() {
            crate::headed_focus::focus_process_window(process.clone(), adapter, context, &lease)?;
        }
        adapter.press_key_for_app(process, &combo, context.physical_input_policy(), &lease)?
    } else {
        let lease = adapter.acquire_interaction_lease(deadline)?;
        let handle = crate::adapter::NativeHandle::null();
        adapter.execute_action(
            &handle,
            context.request_base(Action::PressKey(combo)),
            &lease,
        )?
    };
    super::helpers::apply_scoped_post_action_wait(
        serde_json::to_value(result)?,
        args.app,
        None,
        adapter,
        context,
    )
}

#[cfg(test)]
#[path = "press_tests.rs"]
mod tests;
