use crate::{adapter::PlatformAdapter, error::AppError};
use serde_json::Value;

pub fn execute(adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let displays = adapter.list_displays()?;
    Ok(serde_json::to_value(displays)?)
}
