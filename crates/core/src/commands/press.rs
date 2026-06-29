use crate::{
    action::Action,
    action_request::ActionRequest,
    adapter::PlatformAdapter,
    commands::combo::{ensure_combo_allowed, parse_combo_normalized},
    error::AppError,
};
use serde_json::Value;

pub struct PressArgs {
    pub combo: String,
    pub app: Option<String>,
    pub force: bool,
}

pub fn execute(args: PressArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let combo = parse_combo_normalized(&args.combo)?;
    ensure_combo_allowed(&combo, &args.combo, args.force, adapter)?;

    if let Some(app_name) = &args.app {
        let result = adapter.press_key_for_app(app_name, &combo)?;
        return Ok(serde_json::to_value(result)?);
    }

    let handle = crate::adapter::NativeHandle::null();
    let result = adapter.execute_action(&handle, ActionRequest::headed(Action::PressKey(combo)))?;
    Ok(serde_json::to_value(result)?)
}

#[cfg(test)]
#[path = "press_tests.rs"]
mod tests;
