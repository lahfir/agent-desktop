use crate::{adapter::PlatformAdapter, error::AppError};
use serde_json::{json, Value};

pub fn execute(text: String, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    adapter.set_clipboard(&text)?;
    Ok(json!({ "ok": true }))
}
