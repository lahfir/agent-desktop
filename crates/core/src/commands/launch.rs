use crate::{adapter::PlatformAdapter, error::AppError, launch_options::LaunchOptions};
use serde_json::Value;

pub struct LaunchArgs {
    pub app: String,
    pub timeout_ms: u64,
    pub options: LaunchOptions,
}

pub fn execute(args: LaunchArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let window = adapter.launch_app_with_options(&args.app, &args.options, args.timeout_ms)?;
    Ok(serde_json::to_value(window)?)
}
