use crate::{adapter::PlatformAdapter, error::AppError};
use serde_json::{json, Value};

pub fn execute(adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let apps = adapter.list_apps()?;
    Ok(json!({ "apps": apps }))
}
