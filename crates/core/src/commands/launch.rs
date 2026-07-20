use crate::{AppError, adapter::PlatformAdapter, launch_options::LaunchOptions};
use serde_json::Value;

pub struct LaunchArgs {
    pub app: String,
    pub options: LaunchOptions,
}

pub fn execute(args: LaunchArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    crate::wait_timeout_duration(args.options.timeout_ms)?;
    let deadline = if args.options.timeout_ms == 0 {
        crate::Deadline::standard()?
    } else {
        crate::Deadline::after(args.options.timeout_ms)?
    };
    let lease = adapter.acquire_interaction_lease(deadline)?;
    let window = adapter.launch_app(&args.app, &args.options, &lease)?;
    Ok(serde_json::to_value(window)?)
}

#[cfg(test)]
#[path = "launch_tests.rs"]
mod tests;
