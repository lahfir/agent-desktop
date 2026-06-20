use super::*;
use crate::tree::AXElement;
use crate::tree::resolve_classify::classify_candidates;
use crate::tree::resolve_roots::{
    single_window_fallback_allowed, sole_source_window_fallback_allowed, source_window_number,
    unique_fallible_matching_index,
};
use crate::tree::resolve_search::should_stop_collecting;
use agent_desktop_core::adapter::SnapshotSurface;

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
        source_surface: SnapshotSurface::Window,
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
    assert!(!can_use_path_fast_path(&entry(
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
    assert!(!requires_scoped_path_resolution(&entry(
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
fn expired_deadline_fails_before_path_resolution_reads() {
    let err = match find_entry_by_path(
        &[],
        &entry(Some(42), Some("w-42"), Some("Documents"), None),
        false,
        std::time::Instant::now(),
    ) {
        Ok(_) => panic!("expected timeout"),
        Err(err) => err,
    };

    assert_eq!(err.code, ErrorCode::Timeout);
}

#[test]
fn ambiguous_candidate_classification_reports_structured_details() {
    let err = match classify_candidates(
        vec![
            AXElement(std::ptr::null_mut()),
            AXElement(std::ptr::null_mut()),
        ],
        &entry(None, Some("w-42"), Some("Documents"), None),
        true,
    ) {
        Ok(_) => panic!("expected ambiguous target"),
        Err(err) => err,
    };

    assert_eq!(err.code, ErrorCode::AmbiguousTarget);
    assert!(!err.message.contains("Investors"));
    assert!(err.message.contains("name_chars=9"));
    let details = err.details.unwrap();
    assert_eq!(details["candidate_count"], 2);
    assert_eq!(details["role"], "cell");
    assert_eq!(details["source_window_id"], "w-42");
}

#[test]
fn multiple_identity_candidates_without_bounds_match_are_stale_not_ambiguous() {
    let err = match classify_candidates(
        vec![
            AXElement(std::ptr::null_mut()),
            AXElement(std::ptr::null_mut()),
        ],
        &entry(Some(42), Some("w-42"), Some("Documents"), None),
        true,
    ) {
        Ok(_) => panic!("expected stale moved target"),
        Err(err) => err,
    };

    assert_eq!(err.code, ErrorCode::ElementNotFound);
}

#[test]
fn single_meaningful_identity_candidate_resolves_after_bounds_change() {
    let _handle = classify_candidates(
        vec![AXElement(std::ptr::null_mut())],
        &entry(None, Some("w-42"), Some("Documents"), None),
        true,
    )
    .expect("unique identity should resolve within the verified source window");
}

#[test]
fn cross_window_replacement_without_verified_source_window_fails_closed() {
    let err = match classify_candidates(
        vec![AXElement(std::ptr::null_mut())],
        &entry(None, Some("w-42"), Some("Documents"), None),
        false,
    ) {
        Ok(_) => panic!("expected stale candidate to fail closed"),
        Err(err) => err,
    };

    assert_eq!(err.code, ErrorCode::ElementNotFound);
}

#[test]
fn non_window_identity_candidate_without_bounds_fails_closed() {
    let mut menu_entry = entry(None, None, None, None);
    menu_entry.source_surface = SnapshotSurface::Menu;

    let err = match classify_candidates(vec![AXElement(std::ptr::null_mut())], &menu_entry, false) {
        Ok(_) => panic!("expected stale candidate to fail closed"),
        Err(err) => err,
    };

    assert_eq!(err.code, ErrorCode::ElementNotFound);
}

#[test]
fn single_window_fallback_requires_bounds_hash_not_title() {
    assert!(single_window_fallback_allowed(&entry(
        Some(42),
        Some("w-10"),
        None,
        None
    )));
    assert!(!single_window_fallback_allowed(&entry(
        None,
        Some("w-10"),
        Some("Documents"),
        None
    )));
    let mut menu_entry = entry(Some(42), Some("w-10"), Some("Documents"), None);
    menu_entry.source_surface = SnapshotSurface::Menu;
    assert!(!single_window_fallback_allowed(&menu_entry));
}

#[test]
fn sole_window_fallback_requires_missing_title() {
    assert!(sole_source_window_fallback_allowed(&entry(
        Some(42),
        Some("w-10"),
        None,
        None
    )));
    assert!(!sole_source_window_fallback_allowed(&entry(
        Some(42),
        Some("w-10"),
        Some("Documents"),
        None
    )));
}

#[test]
fn unique_fallible_matching_index_fails_closed_on_scan_error() {
    let values = [1, 2, 3];

    assert_eq!(
        unique_fallible_matching_index(&values, |value| Ok::<bool, ()>(*value == 2)),
        Some(1)
    );
    assert_eq!(
        unique_fallible_matching_index(&values, |value| Ok::<bool, ()>(*value > 1)),
        None
    );
    assert_eq!(
        unique_fallible_matching_index(&values, |value| Ok::<bool, ()>(*value == 4)),
        None
    );
    assert_eq!(
        unique_fallible_matching_index(&values, |value| {
            if *value == 3 {
                return Err(());
            }
            Ok(*value == 2)
        }),
        None
    );
}

#[test]
fn bounds_hash_keeps_collecting_to_disambiguate_identity_matches() {
    assert!(!should_stop_collecting(
        2,
        &entry(Some(42), None, None, None)
    ));
    assert!(should_stop_collecting(2, &entry(None, None, None, None)));
}

fn description_entry() -> RefEntry {
    let mut entry = entry(None, Some("w-10"), Some("Freeform"), None);
    entry.role = "button".into();
    entry.name = None;
    entry.description = Some("Insert Text Box".into());
    entry
}
