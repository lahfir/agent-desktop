use crate::{adapter::PlatformAdapter, error::AppError};
use serde_json::{Value, json};

pub struct CloseAppArgs {
    pub app: String,
    pub force: bool,
}

pub fn execute(args: CloseAppArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    if adapter.is_protected_process(&args.app) {
        return Err(AppError::invalid_input_with_suggestion(
            format!(
                "'{}' is a protected system process and cannot be closed",
                args.app
            ),
            "Target a regular application; session-critical processes (loginwindow, WindowServer, Dock, Finder, launchd) are never closed.",
        ));
    }
    adapter.close_app(&args.app, args.force)?;
    Ok(json!({
        "app": args.app,
        "method": if args.force { "force" } else { "graceful" },
        "requested": true,
        "closed": args.force
    }))
}

#[cfg(test)]
#[path = "close_app_tests.rs"]
mod tests;
