use crate::{
    adapter::PlatformAdapter,
    clipboard_content::ClipboardContent,
    error::AppError,
    image_buffer::{ImageBuffer, ImageFormat},
};
use serde_json::{Value, json};
use std::path::PathBuf;

pub struct ClipboardSetArgs {
    pub text: Option<String>,
    pub image: Option<PathBuf>,
    pub file_urls: Vec<String>,
}

pub fn execute(args: ClipboardSetArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let content = build_content(args)?;
    let format = content.format();
    adapter.set_clipboard_content(&content)?;
    Ok(json!({ "ok": true, "type": format.as_str() }))
}

fn build_content(args: ClipboardSetArgs) -> Result<ClipboardContent, AppError> {
    if !args.file_urls.is_empty() {
        return Ok(ClipboardContent::FileUrls(validate_file_urls(
            &args.file_urls,
        )?));
    }
    if let Some(path) = args.image {
        let data = std::fs::read(&path)?;
        let (width, height) = png_dimensions(&data);
        return Ok(ClipboardContent::Image(ImageBuffer {
            data,
            format: ImageFormat::Png,
            width,
            height,
            scale_factor: 1.0,
        }));
    }
    Ok(ClipboardContent::Text(args.text.unwrap_or_default()))
}

/// Reports missing `--file-url` entries by count and index only — the
/// entries themselves may be sensitive paths and this error can reach a
/// trace file.
fn validate_file_urls(urls: &[String]) -> Result<Vec<String>, AppError> {
    let missing: Vec<usize> = urls
        .iter()
        .enumerate()
        .filter(|(_, path)| !std::path::Path::new(path).exists())
        .map(|(index, _)| index)
        .collect();
    if !missing.is_empty() {
        return Err(AppError::invalid_input(format!(
            "{} of {} --file-url entries do not exist on disk (indexes: {missing:?})",
            missing.len(),
            urls.len()
        )));
    }
    Ok(urls.to_vec())
}

fn png_dimensions(data: &[u8]) -> (u32, u32) {
    if data.len() < 24 {
        return (0, 0);
    }
    let w = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
    let h = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
    (w, h)
}

#[cfg(test)]
#[path = "clipboard_set_tests.rs"]
mod tests;
