//! Pins the shared resolver-error payload byte-for-byte against the two
//! platform copies these constructors replaced
//! (`crates/windows/src/tree/resolve_search.rs::identity_unknown_error`,
//! `crates/macos/src/tree/resolve_errors.rs::identity_unknown`, and both
//! platforms' `mark_deadline_elapsed`, captured before deletion). Renaming or
//! dropping a key here is a behaviour change for both adapters at once.

use super::*;

fn entry_with_role(role: &str) -> RefEntry {
    RefEntry {
        process: crate::RefProcess {
            pid: crate::ProcessId::new(1),
            process_instance: Some("test-instance".into()),
        },
        identity: crate::RefEntryIdentity {
            role: role.into(),
            name: None,
            value: None,
            description: None,
            native_id: None,
        },
        geometry: crate::RefGeometry {
            bounds: None,
            bounds_hash: None,
        },
        capabilities: crate::RefCapabilities {
            states: Vec::new(),
            available_actions: Vec::new(),
        },
        source: crate::RefSource {
            source_app: None,
            source_window_id: None,
            source_window_title: None,
            source_window_bounds_hash: None,
            source_surface: crate::SnapshotSurface::Window,
        },
        scope: crate::RefScope {
            root_ref: None,
            path_is_absolute: false,
            path: crate::refs::RefPath::default(),
        },
    }
}

#[test]
fn identity_unknown_error_pins_the_shared_payload() {
    let entry = entry_with_role("button");
    let error = identity_unknown_error(&entry);

    assert_eq!(error.code, ErrorCode::AppUnresponsive);
    assert_eq!(
        error.message,
        "Strict resolution could not determine candidate identity from the live accessibility evidence"
    );
    assert_eq!(
        error.suggestion.as_deref(),
        Some("Retry after the target application finishes updating its accessibility tree")
    );
    assert_eq!(
        error.details,
        Some(json!({
            "kind": "resolution_identity_unknown",
            "role": "button",
            "complete": false,
            "retryable": true,
        }))
    );
    assert!(error.is_explicitly_retryable());
    assert!(error.permits_retry_by_default());
}

#[test]
fn mark_deadline_elapsed_stamps_an_object_details_payload() {
    let error = AdapterError::new(ErrorCode::AppUnresponsive, "incomplete").with_details(json!({
        "kind": "resolution_identity_unknown",
        "complete": false,
        "retryable": true,
    }));

    let stamped = mark_deadline_elapsed(error);

    assert_eq!(
        stamped.details,
        Some(json!({
            "kind": "resolution_identity_unknown",
            "complete": false,
            "retryable": true,
            "deadline_elapsed": true,
        }))
    );
}

#[test]
fn mark_deadline_elapsed_wraps_a_non_object_details_payload() {
    let error =
        AdapterError::new(ErrorCode::Timeout, "bare").with_details(json!("evidence-string"));

    let stamped = mark_deadline_elapsed(error);

    assert_eq!(
        stamped.details,
        Some(json!({
            "evidence": "evidence-string",
            "deadline_elapsed": true,
        }))
    );
}

#[test]
fn mark_deadline_elapsed_fills_missing_details() {
    let error = AdapterError::new(ErrorCode::Timeout, "bare");

    let stamped = mark_deadline_elapsed(error);

    assert_eq!(stamped.details, Some(json!({ "deadline_elapsed": true })));
}
