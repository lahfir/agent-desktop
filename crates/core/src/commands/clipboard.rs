use crate::{adapter::PlatformAdapter, error::AppError};
use serde_json::{json, Value};

pub fn execute_get(adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let text = adapter.get_clipboard()?;
    Ok(json!({ "text": text }))
}

pub fn execute_set(text: String, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    adapter.set_clipboard(&text)?;
    Ok(json!({ "ok": true }))
}
