use crate::{
    action::Action,
    action_request::ActionRequest,
    adapter::PlatformAdapter,
    commands::combo::{check_blocked_combo, parse_combo_normalized},
    error::AppError,
};
use serde_json::Value;

pub struct PressArgs {
    pub combo: String,
    pub app: Option<String>,
}

pub fn execute(args: PressArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    check_blocked_combo(&args.combo)?;
    let combo = parse_combo_normalized(&args.combo)?;

    if let Some(app_name) = &args.app {
        let result = adapter.press_key_for_app(app_name, &combo)?;
        return Ok(serde_json::to_value(result)?);
    }

    let handle = crate::adapter::NativeHandle::null();
    let result = adapter.execute_action(&handle, ActionRequest::headed(Action::PressKey(combo)))?;
    Ok(serde_json::to_value(result)?)
}
