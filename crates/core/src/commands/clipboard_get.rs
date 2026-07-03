use crate::{adapter::PlatformAdapter, clipboard_content::ClipboardFormat, error::AppError};
use serde_json::{Value, json};

pub struct ClipboardGetArgs {
    pub format: Option<ClipboardFormat>,
}

pub fn execute(args: ClipboardGetArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    if let Some(format) = args.format {
        let content = adapter.get_clipboard_content(format)?;
        return Ok(serde_json::to_value(content)?);
    }
    let text = adapter.get_clipboard()?;
    Ok(json!({ "text": text }))
}
