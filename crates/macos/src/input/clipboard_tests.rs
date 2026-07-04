use super::*;
use agent_desktop_core::clipboard_content::{ClipboardContent, ClipboardFormat};
use agent_desktop_core::image_buffer::{ImageBuffer, ImageFormat};
use std::sync::Mutex;

/// The real system pasteboard is process-wide shared state; serialize the
/// tests in this file so they don't interleave writes on separate threads.
static CLIPBOARD_TEST_LOCK: Mutex<()> = Mutex::new(());

/// Captures the real clipboard on construction and restores it on drop, so
/// these tests can exercise `set_content`/`get_content` against the actual
/// `NSPasteboard` (the only receiver `pasteboard()` talks to) without
/// leaving the developer's or CI runner's clipboard mutated afterward.
struct RestoreGuard(ClipboardSnapshot);

impl RestoreGuard {
    fn capture() -> Self {
        Self(ClipboardSnapshot::capture().expect("capture clipboard before test"))
    }
}

impl Drop for RestoreGuard {
    fn drop(&mut self) {
        let _ = self.0.restore();
    }
}

fn fake_png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = vec![0u8; 24];
    bytes[0..8].copy_from_slice(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
    bytes[12..16].copy_from_slice(b"IHDR");
    bytes[16..20].copy_from_slice(&width.to_be_bytes());
    bytes[20..24].copy_from_slice(&height.to_be_bytes());
    bytes
}

#[test]
fn text_round_trips_through_typed_api() {
    let _serial = CLIPBOARD_TEST_LOCK.lock().unwrap();
    let _guard = RestoreGuard::capture();

    set_content(&ClipboardContent::Text(
        "agent-desktop clipboard test".into(),
    ))
    .expect("set text content");
    match get_content(ClipboardFormat::Text).expect("get text content") {
        Some(ClipboardContent::Text(text)) => {
            assert_eq!(text, "agent-desktop clipboard test");
        }
        other => panic!(
            "expected Some(Text(_)), got a different variant: {}",
            describe(&other)
        ),
    }
}

#[test]
fn image_set_then_get_round_trips_dimensions() {
    let _serial = CLIPBOARD_TEST_LOCK.lock().unwrap();
    let _guard = RestoreGuard::capture();

    let bytes = fake_png(37, 21);
    set_content(&ClipboardContent::Image(ImageBuffer {
        data: bytes,
        format: ImageFormat::Png,
        width: 37,
        height: 21,
        scale_factor: 1.0,
    }))
    .expect("set image content");

    match get_content(ClipboardFormat::Image).expect("get image content") {
        Some(ClipboardContent::Image(image)) => {
            assert_eq!(image.width, 37);
            assert_eq!(image.height, 21);
        }
        other => panic!(
            "expected Some(Image(_)), got a different variant: {}",
            describe(&other)
        ),
    }
}

#[test]
fn file_urls_round_trip() {
    let _serial = CLIPBOARD_TEST_LOCK.lock().unwrap();
    let _guard = RestoreGuard::capture();

    let path = std::env::temp_dir().join(format!(
        "agent-desktop-clipboard-test-{}-{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, b"clipboard file-url test").unwrap();
    let path_string = path.to_string_lossy().into_owned();

    set_content(&ClipboardContent::FileUrls(vec![path_string.clone()]))
        .expect("set file-url content");
    let result = get_content(ClipboardFormat::FileUrls).expect("get file-url content");
    let _ = std::fs::remove_file(&path);

    match result {
        Some(ClipboardContent::FileUrls(urls)) => {
            assert_eq!(urls, vec![path_string]);
        }
        other => panic!(
            "expected Some(FileUrls(_)), got a different variant: {}",
            describe(&other)
        ),
    }
}

#[test]
fn requesting_image_format_on_text_only_clipboard_returns_none_not_panic() {
    let _serial = CLIPBOARD_TEST_LOCK.lock().unwrap();
    let _guard = RestoreGuard::capture();

    set_content(&ClipboardContent::Text("just text, no image".into())).expect("set text content");
    let result = get_content(ClipboardFormat::Image).expect("get image format must not panic");
    assert!(
        result.is_none(),
        "text-only clipboard must report no image content"
    );
}

#[test]
fn auto_prefers_file_urls_when_present() {
    let _serial = CLIPBOARD_TEST_LOCK.lock().unwrap();
    let _guard = RestoreGuard::capture();

    let path = std::env::temp_dir().join(format!(
        "agent-desktop-clipboard-auto-{}-{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, b"auto preference test").unwrap();
    set_content(&ClipboardContent::FileUrls(vec![
        path.to_string_lossy().into_owned(),
    ]))
    .expect("set file-url content");

    let result = get_content(ClipboardFormat::Auto).expect("get auto content");
    let _ = std::fs::remove_file(&path);

    assert!(
        matches!(result, Some(ClipboardContent::FileUrls(_))),
        "auto format should surface file URLs over lower-priority representations"
    );
}

fn describe(content: &Option<ClipboardContent>) -> &'static str {
    match content {
        None => "None",
        Some(ClipboardContent::Text(_)) => "Some(Text)",
        Some(ClipboardContent::Image(_)) => "Some(Image)",
        Some(ClipboardContent::FileUrls(_)) => "Some(FileUrls)",
    }
}
