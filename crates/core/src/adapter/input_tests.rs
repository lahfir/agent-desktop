use super::*;
use crate::error::ErrorCode;

struct DefaultOnly;
impl InputOps for DefaultOnly {}

#[test]
fn default_clear_clipboard_is_not_supported() {
    let err = DefaultOnly.clear_clipboard().unwrap_err();
    assert_eq!(err.code, ErrorCode::PlatformNotSupported);
}

#[test]
fn default_get_clipboard_content_is_not_supported() {
    let err = DefaultOnly
        .get_clipboard_content(ClipboardFormat::Text)
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::PlatformNotSupported);
}

#[test]
fn default_set_clipboard_content_is_not_supported() {
    let err = DefaultOnly
        .set_clipboard_content(&ClipboardContent::Text("x".into()))
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::PlatformNotSupported);
}

/// KTD13: `get_clipboard`/`set_clipboard` (string-only) were removed, not
/// wrapped — `get_clipboard_content`/`set_clipboard_content` are the only
/// surface. A regression that re-adds either legacy method would compile
/// (they'd just be unused trait members) so this guard reads the source
/// text directly rather than relying on the type system to catch it.
#[test]
fn legacy_string_clipboard_methods_are_gone() {
    let src = include_str!("input.rs");
    assert!(
        !src.contains("fn get_clipboard("),
        "get_clipboard (string-only) must stay removed; use get_clipboard_content"
    );
    assert!(
        !src.contains("fn set_clipboard("),
        "set_clipboard (string-only) must stay removed; use set_clipboard_content"
    );
    assert!(
        src.contains("fn clear_clipboard("),
        "clear_clipboard must remain (KTD13 keeps it unchanged)"
    );
    assert!(
        src.contains("fn get_clipboard_content("),
        "get_clipboard_content must remain the read surface"
    );
    assert!(
        src.contains("fn set_clipboard_content("),
        "set_clipboard_content must remain the write surface"
    );
}
