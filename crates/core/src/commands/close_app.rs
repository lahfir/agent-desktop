use crate::{adapter::PlatformAdapter, error::AppError};
use serde_json::{json, Value};

const PROTECTED_PROCESSES: &[&str] = &["loginwindow", "windowserver", "dock", "launchd", "finder"];

pub struct CloseAppArgs {
    pub app: String,
    pub force: bool,
}

pub fn execute(args: CloseAppArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let lower = args.app.to_lowercase();
    if PROTECTED_PROCESSES.iter().any(|p| lower.contains(p)) {
        return Err(AppError::invalid_input(format!(
            "'{}' is a protected system process and cannot be closed",
            args.app
        )));
    }
    adapter.close_app(&args.app, args.force)?;
    Ok(json!({ "app": args.app, "closed": true }))
}
