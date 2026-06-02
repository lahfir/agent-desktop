use super::*;
use crate::tree::resolve_roots::source_window_number;

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
fn scoped_path_retry_fails_closed_when_scope_is_unresolved() {
    let no_bounds_entry = entry(None, Some("w-10"), Some("Freeform"), None);

    assert!(requires_scoped_path_resolution(&no_bounds_entry));
    assert!(requires_scoped_path_resolution(&description_entry()));
    assert!(!requires_scoped_path_resolution(&entry(
        Some(42),
        Some("w-10"),
        Some("Freeform"),
        None
    )));
}

#[test]
fn scoped_path_retry_fails_closed_for_blank_identity_without_bounds() {
    let mut blank = entry(None, Some("w-10"), Some("Freeform"), None);
    blank.name = None;

    assert!(requires_scoped_path_resolution(&blank));
}

#[test]
fn broad_search_requires_bounds_or_meaningful_identity() {
    let mut blank = entry(None, None, None, None);
    blank.name = None;

    assert!(!can_use_broad_search(&blank));
    assert!(can_use_broad_search(&description_entry()));
    assert!(can_use_broad_search(&entry(Some(42), None, None, None)));
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
fn only_element_not_found_is_retryable_resolution_error() {
    assert!(is_retryable_resolution_error(
        &AdapterError::element_not_found("element")
    ));
    assert!(!is_retryable_resolution_error(
        &AdapterError::ambiguous_target("2 candidates matched")
    ));
    assert!(!is_retryable_resolution_error(&AdapterError::new(
        ErrorCode::Timeout,
        "resolution timed out"
    )));
}

#[test]
fn ambiguous_candidate_classification_reports_structured_details() {
    let err = match classify_candidates(
        vec![
            AXElement(std::ptr::null_mut()),
            AXElement(std::ptr::null_mut()),
        ],
        &entry(None, Some("w-42"), Some("Documents"), None),
    ) {
        Ok(_) => panic!("expected ambiguous target"),
        Err(err) => err,
    };

    assert_eq!(err.code, ErrorCode::AmbiguousTarget);
    let details = err.details.unwrap();
    assert_eq!(details["candidate_count"], 2);
    assert_eq!(details["role"], "cell");
    assert_eq!(details["source_window_id"], "w-42");
}

fn description_entry() -> RefEntry {
    let mut entry = entry(None, Some("w-10"), Some("Freeform"), None);
    entry.role = "button".into();
    entry.name = None;
    entry.description = Some("Insert Text Box".into());
    entry
}
