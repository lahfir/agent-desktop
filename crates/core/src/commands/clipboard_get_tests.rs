use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::error::AdapterError;
use crate::refs_test_support::HomeGuard;
use std::sync::Mutex;

struct LocalDouble {
    response: Mutex<Option<Result<Option<ClipboardContent>, AdapterError>>>,
    seen_format: Mutex<Option<ClipboardFormat>>,
}

impl LocalDouble {
    fn returning(result: Result<Option<ClipboardContent>, AdapterError>) -> Self {
        Self {
            response: Mutex::new(Some(result)),
            seen_format: Mutex::new(None),
        }
    }
}

impl ObservationOps for LocalDouble {}
impl ActionOps for LocalDouble {}
impl SystemOps for LocalDouble {}

impl InputOps for LocalDouble {
    fn get_clipboard_content(
        &self,
        format: ClipboardFormat,
    ) -> Result<Option<ClipboardContent>, AdapterError> {
        *self.seen_format.lock().unwrap() = Some(format);
        self.response
            .lock()
            .unwrap()
            .take()
            .expect("double invoked more than once in a single test")
    }
}

fn no_session_context() -> CommandContext {
    CommandContext::default()
}

#[test]
fn omitted_format_defaults_to_text_not_auto() {
    let double = LocalDouble::returning(Ok(Some(ClipboardContent::Text("hi there".into()))));
    let out = execute(
        ClipboardGetArgs {
            format: None,
            out: None,
        },
        &double,
        &no_session_context(),
    )
    .unwrap();
    assert_eq!(
        *double.seen_format.lock().unwrap(),
        Some(ClipboardFormat::Text)
    );
    assert_eq!(out["type"], "text");
    assert_eq!(out["text"], "hi there");
}

#[test]
fn missing_content_for_requested_format_is_structured_not_found_not_an_error() {
    let double = LocalDouble::returning(Ok(None));
    let out = execute(
        ClipboardGetArgs {
            format: Some(ClipboardFormat::Image),
            out: None,
        },
        &double,
        &no_session_context(),
    )
    .expect("must return Ok(..) not panic or bubble a hard error");
    assert_eq!(out["type"], "image");
    assert_eq!(out["found"], false);
}

#[test]
fn file_urls_variant_serializes_list() {
    let double = LocalDouble::returning(Ok(Some(ClipboardContent::FileUrls(vec![
        "/tmp/a.txt".into(),
        "/tmp/b.txt".into(),
    ]))));
    let out = execute(
        ClipboardGetArgs {
            format: Some(ClipboardFormat::FileUrls),
            out: None,
        },
        &double,
        &no_session_context(),
    )
    .unwrap();
    assert_eq!(out["type"], "file_urls");
    assert_eq!(
        out["file_urls"],
        serde_json::json!(["/tmp/a.txt", "/tmp/b.txt"])
    );
}

#[test]
fn auto_format_request_is_relayed_to_adapter_unmodified() {
    let double = LocalDouble::returning(Ok(Some(ClipboardContent::Text("auto text".into()))));
    let _ = execute(
        ClipboardGetArgs {
            format: Some(ClipboardFormat::Auto),
            out: None,
        },
        &double,
        &no_session_context(),
    )
    .unwrap();
    assert_eq!(
        *double.seen_format.lock().unwrap(),
        Some(ClipboardFormat::Auto)
    );
}

#[test]
fn image_variant_with_explicit_out_writes_that_path() {
    let _home = HomeGuard::new();
    let dir = std::env::temp_dir().join(format!(
        "agent-desktop-clipboard-get-test-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let out_path = dir.join("explicit.png");

    let double = LocalDouble::returning(Ok(Some(ClipboardContent::Image(ImageBuffer {
        data: vec![1, 2, 3, 4],
        format: crate::image_buffer::ImageFormat::Png,
        width: 10,
        height: 5,
        scale_factor: 1.0,
    }))));
    let out = execute(
        ClipboardGetArgs {
            format: Some(ClipboardFormat::Image),
            out: Some(out_path.clone()),
        },
        &double,
        &no_session_context(),
    )
    .unwrap();

    assert_eq!(out["type"], "image");
    assert_eq!(out["path"], out_path.to_string_lossy().into_owned());
    assert_eq!(out["width"], 10);
    assert_eq!(out["height"], 5);
    assert_eq!(std::fs::read(&out_path).unwrap(), vec![1, 2, 3, 4]);

    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn image_variant_without_out_writes_private_0600_file_under_session_dir() {
    use std::os::unix::fs::PermissionsExt;

    let _home = HomeGuard::new();
    let context = CommandContext::new(Some("clip-get-test-session".into()), None, false).unwrap();

    let double = LocalDouble::returning(Ok(Some(ClipboardContent::Image(ImageBuffer {
        data: vec![9, 9, 9],
        format: crate::image_buffer::ImageFormat::Png,
        width: 3,
        height: 3,
        scale_factor: 1.0,
    }))));
    let out = execute(
        ClipboardGetArgs {
            format: Some(ClipboardFormat::Image),
            out: None,
        },
        &double,
        &context,
    )
    .unwrap();

    let path = out["path"].as_str().expect("path field").to_string();
    let session_dir = session::session_dir("clip-get-test-session")
        .unwrap()
        .join("clipboard");
    assert!(
        std::path::Path::new(&path).starts_with(&session_dir),
        "default image path {path} must live under the session's clipboard dir {}",
        session_dir.display()
    );
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(
        mode, 0o600,
        "clipboard image temp file must be 0600, got {mode:o}"
    );
}
