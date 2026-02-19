use crate::{
    adapter::{PlatformAdapter, WindowFilter},
    error::AppError,
};
use serde_json::Value;

pub struct ListWindowsArgs {
    pub app: Option<String>,
}

pub fn execute(args: ListWindowsArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let filter = WindowFilter { focused_only: false, app: args.app };
    let windows = adapter.list_windows(&filter)?;
    Ok(serde_json::to_value(windows)?)
}
