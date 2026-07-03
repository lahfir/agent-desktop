use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::error::{AdapterError, ErrorCode};
use std::sync::Mutex;

struct LocalDouble {
    seen: Mutex<Option<ClipboardContent>>,
}

impl LocalDouble {
    fn new() -> Self {
        Self {
            seen: Mutex::new(None),
        }
    }
}

impl ObservationOps for LocalDouble {}
impl ActionOps for LocalDouble {}
impl SystemOps for LocalDouble {}

impl InputOps for LocalDouble {
    fn set_clipboard_content(&self, content: &ClipboardContent) -> Result<(), AdapterError> {
        let stored = match content {
            ClipboardContent::Text(text) => ClipboardContent::Text(text.clone()),
            ClipboardContent::Image(image) => ClipboardContent::Image(ImageBuffer {
                data: image.data.clone(),
                format: crate::image_buffer::ImageFormat::Png,
                width: image.width,
                height: image.height,
                scale_factor: image.scale_factor,
            }),
            ClipboardContent::FileUrls(urls) => ClipboardContent::FileUrls(urls.clone()),
        };
        *self.seen.lock().unwrap() = Some(stored);
        Ok(())
    }
}

#[test]
fn default_with_only_text_writes_text_content() {
    let double = LocalDouble::new();
    let out = execute(
        ClipboardSetArgs {
            text: Some("hello clipboard".into()),
            image: None,
            file_urls: vec![],
        },
        &double,
    )
    .unwrap();
    assert_eq!(out["type"], "text");
    match double.seen.lock().unwrap().as_ref().unwrap() {
        ClipboardContent::Text(text) => assert_eq!(text, "hello clipboard"),
        _ => panic!("expected Text content"),
    }
}

#[test]
fn omitted_text_defaults_to_empty_string_not_error() {
    let double = LocalDouble::new();
    let out = execute(
        ClipboardSetArgs {
            text: None,
            image: None,
            file_urls: vec![],
        },
        &double,
    )
    .unwrap();
    assert_eq!(out["type"], "text");
    match double.seen.lock().unwrap().as_ref().unwrap() {
        ClipboardContent::Text(text) => assert_eq!(text, ""),
        _ => panic!("expected Text content"),
    }
}

#[test]
fn missing_file_url_reports_invalid_args_with_count_and_index_only() {
    let existing = std::env::temp_dir().join(format!(
        "agent-desktop-clipboard-set-test-{}.txt",
        std::process::id()
    ));
    std::fs::write(&existing, b"present").unwrap();

    let double = LocalDouble::new();
    let err = execute(
        ClipboardSetArgs {
            text: None,
            image: None,
            file_urls: vec![
                existing.to_string_lossy().into_owned(),
                "/definitely/does/not/exist/secret-project-name.txt".into(),
            ],
        },
        &double,
    )
    .unwrap_err();

    let _ = std::fs::remove_file(&existing);

    assert_eq!(err.code(), ErrorCode::InvalidArgs.as_str());
    let message = err.to_string();
    assert!(
        message.contains("1 of 2"),
        "message should report count, got: {message}"
    );
    assert!(
        message.contains("[1]"),
        "message should report the missing entry's index (1), got: {message}"
    );
    assert!(
        !message.contains("secret-project-name"),
        "message must not leak the missing path's content, got: {message}"
    );
    assert!(
        double.seen.lock().unwrap().is_none(),
        "adapter must not be called on validation failure"
    );
}

#[test]
fn image_flag_reads_bytes_and_dimensions_from_file() {
    let path = std::env::temp_dir().join(format!(
        "agent-desktop-clipboard-set-image-{}.png",
        std::process::id()
    ));
    let mut bytes = vec![0u8; 24];
    bytes[16..20].copy_from_slice(&11u32.to_be_bytes());
    bytes[20..24].copy_from_slice(&8u32.to_be_bytes());
    std::fs::write(&path, &bytes).unwrap();

    let double = LocalDouble::new();
    let out = execute(
        ClipboardSetArgs {
            text: None,
            image: Some(path.clone()),
            file_urls: vec![],
        },
        &double,
    )
    .unwrap();

    let _ = std::fs::remove_file(&path);

    assert_eq!(out["type"], "image");
    match double.seen.lock().unwrap().as_ref().unwrap() {
        ClipboardContent::Image(image) => {
            assert_eq!(image.width, 11);
            assert_eq!(image.height, 8);
            assert_eq!(image.data, bytes);
        }
        _ => panic!("expected Image content"),
    }
}

#[test]
fn file_urls_take_priority_over_text_and_image() {
    let existing = std::env::temp_dir().join(format!(
        "agent-desktop-clipboard-set-priority-{}.txt",
        std::process::id()
    ));
    std::fs::write(&existing, b"present").unwrap();

    let double = LocalDouble::new();
    let out = execute(
        ClipboardSetArgs {
            text: Some("ignored".into()),
            image: None,
            file_urls: vec![existing.to_string_lossy().into_owned()],
        },
        &double,
    )
    .unwrap();

    let _ = std::fs::remove_file(&existing);

    assert_eq!(out["type"], "file_urls");
    match double.seen.lock().unwrap().as_ref().unwrap() {
        ClipboardContent::FileUrls(urls) => assert_eq!(urls.len(), 1),
        _ => panic!("expected FileUrls content"),
    }
}
