use super::{
    child_attributes, redact_secure_value, window_titles_are_exact_match,
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
