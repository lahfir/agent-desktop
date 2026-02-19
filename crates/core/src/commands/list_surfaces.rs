use crate::{adapter::PlatformAdapter, commands::helpers::resolve_app_pid, error::AppError};
use serde_json::{json, Value};

pub struct ListSurfacesArgs {
    pub app: Option<String>,
}

pub fn execute(args: ListSurfacesArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let pid = resolve_app_pid(args.app.as_deref(), adapter)?;
    let surfaces = adapter.list_surfaces(pid).map_err(AppError::Adapter)?;
    Ok(json!({ "pid": pid, "surfaces": surfaces }))
}
