use crate::{adapter::PlatformAdapter, clipboard_content::ClipboardFormat, error::AppError};
use serde_json::{Value, json};

pub struct ClipboardSetArgs {
    pub text: String,
    pub format: Option<ClipboardFormat>,
}

pub fn execute(args: ClipboardSetArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    if let Some(format) = args.format {
        let content = crate::clipboard_content::ClipboardContent {
            format,
            text: Some(args.text),
            bytes_base64: None,
        };
        adapter.set_clipboard_content(&content)?;
        return Ok(json!({ "ok": true }));
    }
    adapter.set_clipboard(&args.text)?;
    Ok(json!({ "ok": true }))
}
