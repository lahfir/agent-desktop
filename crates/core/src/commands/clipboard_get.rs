use crate::{
    adapter::PlatformAdapter,
    clipboard_content::{ClipboardContent, ClipboardFormat},
    context::CommandContext,
    error::AppError,
    image_buffer::ImageBuffer,
    refs::write_private_file,
    session,
};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct ClipboardGetArgs {
    pub format: Option<ClipboardFormat>,
    pub out: Option<PathBuf>,
}

static IMAGE_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn execute(
    args: ClipboardGetArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let format = args.format.unwrap_or(ClipboardFormat::Text);
    let Some(content) = adapter.get_clipboard_content(format)? else {
        return Ok(json!({ "type": format.as_str(), "found": false }));
    };
    match content {
        ClipboardContent::Text(text) => Ok(json!({ "type": "text", "text": text })),
        ClipboardContent::FileUrls(file_urls) => {
            Ok(json!({ "type": "file_urls", "file_urls": file_urls }))
        }
        ClipboardContent::Image(image) => write_image(image, args.out, context),
    }
}

fn write_image(
    image: ImageBuffer,
    out: Option<PathBuf>,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let path = match out {
        Some(path) => path,
        None => default_clipboard_image_path(context)?,
    };
    write_private_file(&path, &image.data)?;
    Ok(json!({
        "type": "image",
        "path": path.to_string_lossy(),
        "width": image.width,
        "height": image.height,
    }))
}

fn default_clipboard_image_path(context: &CommandContext) -> Result<PathBuf, AppError> {
    let dir = match context.session_id() {
        Some(id) => session::session_dir(id)?.join("clipboard"),
        None => session::agent_desktop_dir()?.join("tmp"),
    };
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seq = IMAGE_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    Ok(dir.join(format!(
        "clipboard-{}-{nanos}-{seq}.png",
        std::process::id()
    )))
}

#[cfg(test)]
#[path = "clipboard_get_tests.rs"]
mod tests;
