use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClipboardFormat {
    PlainText,
    Html,
    Rtf,
    Png,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClipboardContent {
    pub format: ClipboardFormat,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_base64: Option<String>,
}

impl ClipboardContent {
    pub fn plain_text(text: impl Into<String>) -> Self {
        Self {
            format: ClipboardFormat::PlainText,
            text: Some(text.into()),
            bytes_base64: None,
        }
    }
}
