use crate::{adapter::PlatformAdapter, error::AppError};
use serde_json::Value;

pub struct LaunchArgs {
    pub app: String,
    pub timeout_ms: u64,
}

pub fn execute(args: LaunchArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let window = adapter.launch_app(&args.app, args.timeout_ms)?;
    Ok(serde_json::to_value(window)?)
}
