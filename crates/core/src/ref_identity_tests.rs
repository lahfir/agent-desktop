use super::*;

fn entry() -> RefEntry {
    RefEntry {
        pid: 1,
        role: "button".into(),
        name: None,
        value: None,
        description: None,
        states: vec![],
        bounds: None,
        bounds_hash: None,
        available_actions: vec![],
        source_app: None,
        source_window_id: None,
        source_window_title: None,
        source_surface: SnapshotSurface::Window,
        root_ref: None,
        path_is_absolute: false,
        path: smallvec::SmallVec::new(),
    }
}

#[test]
fn empty_identity_matches_missing_or_empty_ax_text() {
    let mut entry = entry();
    entry.role = "menubutton".into();
    entry.name = Some(String::new());

    assert!(!has_meaningful_identity(&entry));
    assert!(identity_matches(&entry, None, None, None));
    assert!(identity_matches(&entry, Some(""), None, None));
    assert!(identity_matches(&entry, None, Some(""), None));
    assert!(!identity_matches(&entry, Some("Insert Shape"), None, None));
}

#[test]
fn description_identity_matches_blank_title_controls() {
    let mut entry = entry();
    entry.description = Some("Insert Text Box".into());

    assert!(has_meaningful_identity(&entry));
    assert!(identity_matches(
        &entry,
        Some(""),
        None,
        Some("Insert Text Box")
    ));
    assert!(identity_matches(
        &entry,
        Some("Insert Text Box"),
        None,
        None
    ));
    assert!(!identity_matches(&entry, Some(""), None, None));
    assert!(!identity_matches(
        &entry,
        Some(""),
        None,
        Some("Insert Shape")
    ));
}

#[test]
fn name_identity_cannot_be_rescued_by_matching_description() {
    let mut entry = entry();
    entry.name = Some("Primary".into());
    entry.description = Some("Generic".into());

    assert!(identity_matches(&entry, Some("Primary"), None, None));
    assert!(identity_matches(&entry, None, Some("Primary"), None));
    assert!(!identity_matches(
        &entry,
        Some("Other"),
        None,
        Some("Primary")
    ));
    assert!(!identity_matches(&entry, Some("Generic"), None, None));
}

#[test]
fn value_identity_cannot_be_rescued_by_matching_name_when_value_mismatches() {
    let mut entry = entry();
    entry.value = Some("On".into());

    assert!(identity_matches(&entry, None, Some("On"), None));
    assert!(identity_matches(&entry, Some("On"), None, None));
    assert!(!identity_matches(&entry, Some("On"), Some("Off"), None));
}

#[test]
fn mutable_value_role_does_not_go_stale_when_value_changes() {
    let mut entry = entry();
    entry.role = "textfield".into();
    entry.value = Some("seed".into());

    assert!(!has_meaningful_identity(&entry));
    assert!(identity_matches(&entry, None, Some("changed"), None));
}

#[test]
fn unnamed_mutable_value_role_does_not_go_stale_when_content_becomes_name() {
    let mut entry = entry();
    entry.role = "textfield".into();

    assert!(!has_meaningful_identity(&entry));
    assert!(identity_matches(
        &entry,
        Some("typed document text"),
        Some("typed document text"),
        None
    ));
}

#[test]
fn mutable_value_text_promoted_to_name_is_not_stable_identity() {
    let mut entry = entry();
    entry.role = "textfield".into();
    entry.name = Some("00:01".into());
    entry.value = Some("00:01".into());

    assert!(!has_meaningful_identity(&entry));
    assert!(identity_matches(&entry, Some("00:06"), Some("00:06"), None));
}

#[test]
fn formatted_numeric_mutable_value_promoted_to_name_is_not_stable_identity() {
    let mut entry = entry();
    entry.role = "slider".into();
    entry.name = Some("50".into());
    entry.value = Some("50.0".into());

    assert!(!has_meaningful_identity(&entry));
    assert!(identity_matches(&entry, Some("51"), Some("51.0"), None));
}

#[test]
fn named_mutable_value_role_still_uses_name_identity() {
    let mut entry = entry();
    entry.role = "textfield".into();
    entry.name = Some("Search".into());
    entry.value = Some("old query".into());

    assert!(has_meaningful_identity(&entry));
    assert!(identity_matches(
        &entry,
        Some("Search"),
        Some("new query"),
        None
    ));
    assert!(!identity_matches(
        &entry,
        Some("Replace"),
        Some("new query"),
        None
    ));
}

#[test]
fn mutable_role_label_different_from_value_remains_stable_identity() {
    let mut entry = entry();
    entry.role = "combobox".into();
    entry.name = Some("Font".into());
    entry.value = Some("Helvetica".into());

    assert!(has_meaningful_identity(&entry));
    assert!(identity_matches(&entry, Some("Font"), Some("Arial"), None));
    assert!(!identity_matches(&entry, Some("Size"), Some("Arial"), None));
}

#[test]
fn bounded_window_fallback_requires_window_source_window_id_and_bounds() {
    let mut entry = entry();
    entry.source_window_id = Some("platform-window-1".into());
    entry.bounds_hash = Some(42);

    assert!(bounded_window_fallback_allowed(&entry));
    entry.source_window_title = Some("Stale Title".into());
    assert!(bounded_window_fallback_allowed(&entry));
    entry.bounds_hash = None;
    assert!(!bounded_window_fallback_allowed(&entry));
    entry.bounds_hash = Some(42);
    entry.source_window_id = None;
    assert!(!bounded_window_fallback_allowed(&entry));
    entry.source_window_id = Some("platform-window-1".into());
    entry.source_surface = SnapshotSurface::Menu;
    assert!(!bounded_window_fallback_allowed(&entry));
}
