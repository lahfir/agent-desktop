use super::*;

fn entry(
    bounds_hash: Option<u64>,
    source_window_id: Option<&str>,
    source_window_title: Option<&str>,
    root_ref: Option<&str>,
) -> RefEntry {
    RefEntry {
        pid: 1,
        role: "cell".into(),
        name: Some("Investors".into()),
        value: None,
        description: None,
        states: vec![],
        bounds: None,
        bounds_hash,
        available_actions: vec![],
        source_app: None,
        source_window_id: source_window_id.map(String::from),
        source_window_title: source_window_title.map(String::from),
        source_surface: agent_desktop_core::adapter::SnapshotSurface::Window,
        root_ref: root_ref.map(String::from),
        path_is_absolute: false,
        path: smallvec::smallvec![0, 1],
    }
}

#[test]
fn path_fast_path_accepts_bounds_or_source_window_identity() {
    assert!(!can_use_path_fast_path(&entry(None, None, None, None)));
    assert!(can_use_path_fast_path(&entry(
        None,
        Some("w-10"),
        None,
        None
    )));
    assert!(can_use_path_fast_path(&entry(
        None,
        None,
        Some("Documents"),
        None
    )));
    assert!(can_use_path_fast_path(&entry(Some(42), None, None, None)));
    assert!(!can_use_path_fast_path(&entry(
        Some(42),
        Some("w-10"),
        Some("Documents"),
        Some("@e1")
    )));
    let mut absolute_drill = entry(Some(42), Some("w-10"), Some("Documents"), Some("@e1"));
    absolute_drill.path_is_absolute = true;
    assert!(can_use_path_fast_path(&absolute_drill));
}

#[test]
fn no_bounds_source_window_refs_require_scoped_path_resolution() {
    assert!(!requires_scoped_path_resolution(&entry(
        None, None, None, None
    )));
    assert!(requires_scoped_path_resolution(&entry(
        None,
        Some("w-10"),
        None,
        None
    )));
    assert!(requires_scoped_path_resolution(&entry(
        None,
        None,
        Some("Documents"),
        None
    )));
    assert!(!requires_scoped_path_resolution(&entry(
        Some(42),
        Some("w-10"),
        Some("Documents"),
        None
    )));
    assert!(!requires_scoped_path_resolution(&entry(
        None,
        Some("w-10"),
        Some("Documents"),
        Some("@e1")
    )));
    let mut absolute_drill = entry(None, Some("w-10"), Some("Documents"), Some("@e1"));
    absolute_drill.path_is_absolute = true;
    assert!(requires_scoped_path_resolution(&absolute_drill));
}

#[test]
fn scoped_path_retry_requires_a_resolved_scope_root() {
    let no_bounds_entry = entry(None, Some("w-10"), Some("Freeform"), None);

    assert!(should_retry_scoped_path_resolution(&no_bounds_entry, true));
    assert!(!should_retry_scoped_path_resolution(
        &no_bounds_entry,
        false
    ));
    assert!(!should_retry_scoped_path_resolution(
        &entry(Some(42), Some("w-10"), Some("Freeform"), None),
        true
    ));
}

#[test]
fn source_window_number_parses_window_ids_only() {
    assert_eq!(
        source_window_number(&entry(None, Some("w-42"), None, None)),
        Some(42)
    );
    assert_eq!(
        source_window_number(&entry(None, Some("42"), None, None)),
        None
    );
    assert_eq!(
        source_window_number(&entry(None, Some("w-bad"), None, None)),
        None
    );
}

#[test]
fn empty_identity_matches_missing_or_empty_ax_text() {
    let mut entry = entry(None, Some("w-10"), Some("Freeform"), None);
    entry.role = "menubutton".into();
    entry.name = Some(String::new());

    assert!(identity_matches(&entry, None, None, None));
    assert!(identity_matches(&entry, Some(""), None, None));
    assert!(identity_matches(&entry, None, Some(""), None));
    assert!(!identity_matches(&entry, Some("Insert Shape"), None, None));
}

#[test]
fn description_identity_matches_blank_title_controls() {
    let mut entry = entry(None, Some("w-10"), Some("Freeform"), None);
    entry.role = "button".into();
    entry.name = None;
    entry.description = Some("Insert Text Box".into());

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
fn meaningful_identity_still_requires_matching_text() {
    let mut entry = entry(None, Some("w-10"), Some("Freeform"), None);
    entry.name = Some("Zoom".into());

    assert!(identity_matches(&entry, Some("Zoom"), None, None));
    assert!(identity_matches(&entry, None, Some("Zoom"), None));
    assert!(!identity_matches(&entry, None, None, None));
    assert!(!identity_matches(&entry, Some(""), None, None));
}
