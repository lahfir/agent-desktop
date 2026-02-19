use crate::{adapter::PlatformAdapter, error::AppError};
use serde_json::Value;

pub struct LaunchArgs {
    pub app: String,
    pub wait: bool,
}

pub fn execute(args: LaunchArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let window = adapter.launch_app(&args.app, args.wait)?;
    Ok(serde_json::to_value(window)?)
}
