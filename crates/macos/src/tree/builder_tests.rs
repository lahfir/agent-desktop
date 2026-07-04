use super::{
    child_attributes, redact_secure_value, reduce_text_name, window_bounds_for_children,
    window_titles_are_exact_match, window_titles_are_partial_match,
};
use agent_desktop_core::node::Rect;

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

/// Regression guard for F7: deleting `query.rs` also deleted the only unit
/// coverage of window-bounds inheritance (`window_bounds_for_children_*`
/// tests there). The rule now lives solely in `builder.rs` and feeds the
/// `offscreen` state predicate for every descendant, so losing either branch
/// silently breaks offscreen detection under real windows. Fails if the
/// `AXWindow`-owns-bounds branch is dropped.
#[test]
fn window_bounds_for_children_captures_window_own_bounds() {
    let own = Some(Rect {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
    });
    let inherited = Some(Rect {
        x: 10.0,
        y: 10.0,
        width: 5.0,
        height: 5.0,
    });

    let result = window_bounds_for_children(Some("AXWindow"), own, inherited);

    assert_eq!(result, own);
}

/// Fails if the inherited-bounds fallback is dropped from the `AXWindow`
/// branch (e.g. replaced with a bare `own_bounds` that would go `None` here).
#[test]
fn window_bounds_for_children_falls_back_to_inherited_without_own_bounds() {
    let inherited = Some(Rect {
        x: 10.0,
        y: 10.0,
        width: 5.0,
        height: 5.0,
    });

    let result = window_bounds_for_children(Some("AXWindow"), None, inherited);

    assert_eq!(result, inherited);
}

/// Fails if non-window roles stop passing their inherited window bounds
/// through unchanged (e.g. someone widens the own-bounds branch to all roles).
#[test]
fn window_bounds_for_children_passes_through_inherited_for_non_window_roles() {
    let own = Some(Rect {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
    });
    let inherited = Some(Rect {
        x: 10.0,
        y: 10.0,
        width: 5.0,
        height: 5.0,
    });

    let result = window_bounds_for_children(Some("AXButton"), own, inherited);

    assert_eq!(result, inherited);
}
