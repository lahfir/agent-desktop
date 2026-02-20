use crate::{
    action::WindowOp, adapter::PlatformAdapter, commands::helpers::resolve_app_pid, error::AppError,
};
use serde_json::{json, Value};

pub struct RestoreArgs {
    pub app: Option<String>,
}

pub fn execute(args: RestoreArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let pid = resolve_app_pid(args.app.as_deref(), adapter)?;
    let win = find_window(pid, adapter)?;
    adapter.window_op(&win, WindowOp::Restore)?;
    Ok(json!({ "restored": true }))
}

fn find_window(
    pid: i32,
    adapter: &dyn PlatformAdapter,
) -> Result<crate::node::WindowInfo, AppError> {
    let filter = crate::adapter::WindowFilter {
        focused_only: false,
        app: None,
    };
    let windows = adapter.list_windows(&filter)?;
    windows
        .into_iter()
        .find(|w| w.pid == pid)
        .ok_or_else(|| AppError::invalid_input("No window found for this application"))
}
