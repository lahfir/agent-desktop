use crate::image_buffer::ImageBuffer;

/// Which clipboard representation a read targets. `Auto` prefers the
/// richest representation currently on the pasteboard: file references,
/// then an image, then plain text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardFormat {
    Auto,
    Text,
    Image,
    FileUrls,
}

impl ClipboardFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            ClipboardFormat::Auto => "auto",
            ClipboardFormat::Text => "text",
            ClipboardFormat::Image => "image",
            ClipboardFormat::FileUrls => "file_urls",
        }
    }
}

/// A typed clipboard payload. `FileUrls` carries filesystem paths — the
/// shape `Finder`'s "Copy" leaves on the pasteboard for one or more files —
/// not raw pasteboard bytes.
#[derive(Debug)]
pub enum ClipboardContent {
    Text(String),
    Image(ImageBuffer),
    FileUrls(Vec<String>),
}

impl ClipboardContent {
    pub fn format(&self) -> ClipboardFormat {
        match self {
            ClipboardContent::Text(_) => ClipboardFormat::Text,
            ClipboardContent::Image(_) => ClipboardFormat::Image,
            ClipboardContent::FileUrls(_) => ClipboardFormat::FileUrls,
        }
    }
}

#[cfg(test)]
#[path = "clipboard_content_tests.rs"]
mod tests;
