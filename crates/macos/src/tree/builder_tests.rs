use super::{
    child_attributes, redact_secure_value, reduce_text_name, window_titles_are_exact_match,
    window_titles_are_partial_match,
};

#[test]
fn test_browser_children_use_columns() {
    assert_eq!(
        child_attributes(Some("AXBrowser")),
        ["AXColumns", "AXContents"]
    );
}

#[test]
fn test_default_children_follow_fallback_order() {
    assert_eq!(
        child_attributes(Some("AXGroup")),
        ["AXChildren", "AXContents", "AXChildrenInNavigationOrder"]
    );
}

#[test]
fn test_secure_text_value_is_redacted() {
    assert_eq!(
        redact_secure_value(Some("AXSecureTextField"), Some("secret".into())),
        None
    );
    assert_eq!(
        redact_secure_value(Some("AXTextField"), Some("visible".into())),
        Some("visible".into())
    );
}

#[test]
fn window_title_matching_rejects_empty_titles() {
    assert!(!window_titles_are_exact_match("", ""));
    assert!(!window_titles_are_exact_match("Inbox", ""));
    assert!(!window_titles_are_exact_match("", "Inbox"));
    assert!(!window_titles_are_partial_match("Inbox", ""));
    assert!(!window_titles_are_partial_match("", "Inbox"));
}

#[test]
fn window_title_matching_accepts_exact_and_truncated_titles() {
    assert!(window_titles_are_exact_match("Inbox", "Inbox"));
    assert!(window_titles_are_partial_match(
        "noy4/agent-desktop: Native desktop automation",
        "noy4/agent-desktop"
    ));
    assert!(window_titles_are_partial_match(
        "noy4/agent-desktop: Native desk...",
        "noy4/agent-desktop: Native desk"
    ));
}

#[test]
fn reduce_text_name_prefers_title_then_description_then_value() {
    assert_eq!(
        reduce_text_name(Some("T"), Some("D"), Some("V")).as_deref(),
        Some("T")
    );
    assert_eq!(
        reduce_text_name(None, Some("D"), Some("V")).as_deref(),
        Some("D")
    );
    assert_eq!(
        reduce_text_name(None, None, Some("V")).as_deref(),
        Some("V")
    );
    assert_eq!(reduce_text_name(None, None, None), None);
}

/// Guards the STALE_REF divergence class: a blank or whitespace-only title must
/// fall through to the next rung so a stored ref name (computed by the builder
/// through this same reducer) equals what strict re-resolution recomputes.
#[test]
fn reduce_text_name_treats_blank_and_whitespace_as_absent() {
    assert_eq!(
        reduce_text_name(Some(""), Some("D"), None).as_deref(),
        Some("D")
    );
    assert_eq!(
        reduce_text_name(Some("   "), Some("D"), None).as_deref(),
        Some("D")
    );
    assert_eq!(
        reduce_text_name(Some("\t "), None, Some("V")).as_deref(),
        Some("V")
    );
    assert_eq!(reduce_text_name(Some("   "), Some("  "), Some(" ")), None);
    // Real content with surrounding whitespace is preserved verbatim (matches
    // how the builder stores it), so stored and recomputed names still agree.
    assert_eq!(
        reduce_text_name(Some("  Recents  "), None, None).as_deref(),
        Some("  Recents  ")
    );
}
