use crate::{AppError, adapter::PlatformAdapter};
use serde_json::{Value, json};

pub fn execute(adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let lease = crate::commands::helpers::acquire_interaction_lease(adapter)?;
    adapter.clear_clipboard(&lease)?;
    Ok(json!({ "cleared": true }))
}
