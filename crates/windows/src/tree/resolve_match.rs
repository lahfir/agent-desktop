use agent_desktop_core::{AdapterError, ElementIdentifier, RefEntry};
use serde_json::json;

use super::element::UIAElement;

/// The identity evidence one element carries, compared against the stored ref.
///
/// Split from the search so the matching decision is a pure function over
/// plain values, testable without a live COM element in scope.
#[derive(Clone)]
pub(crate) struct NodeIdentity {
    pub(crate) native_id: Option<ElementIdentifier>,
    pub(crate) role: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) bounds_hash: Option<u64>,
}

/// A candidate match: the live element plus the identity evidence read from it.
#[derive(Clone)]
pub(crate) struct Candidate {
    pub(crate) element: UIAElement,
    pub(crate) identity: NodeIdentity,
}

/// Whether one searched element matches the stored evidence exactly.
///
/// `native_id` must match by kind **and** value when the ref carries one; role
/// must match when the ref records a role other than `unknown`; name is
/// corroboration only when the ref records a name and the element reports one
/// (a blank live name cannot refute a ref that had no name). The bounds hash,
/// when both sides have one, is a soft signal - it never refutes, it only
/// corroborates, per the identity-against-OS-reorder practice.
pub(crate) fn candidate_matches(
    identity: &NodeIdentity,
    expected_id: Option<&ElementIdentifier>,
    expected_role: &str,
    expected_name: Option<&str>,
) -> bool {
    if let Some(expected) = expected_id {
        let id_matches = identity
            .native_id
            .as_ref()
            .is_some_and(|actual| actual.kind == expected.kind && actual.value == expected.value);
        if !id_matches {
            return false;
        }
    }
    if !expected_role.is_empty() && expected_role != "unknown" {
        if identity.role.as_deref() != Some(expected_role) {
            return false;
        }
    }
    if let Some(name) = expected_name {
        if !name.is_empty() && identity.name.as_deref() != Some(name) {
            return false;
        }
    }
    true
}

pub(crate) fn stale_ref_error(_entry: &RefEntry) -> AdapterError {
    AdapterError::stale_ref("Stored ref does not match any live element").with_details(json!({
        "kind": "resolve_no_candidate",
    }))
}

pub(crate) fn ambiguous_target_error(_entry: &RefEntry, count: usize) -> AdapterError {
    AdapterError::ambiguous_target("Multiple live elements match the stored identity")
        .with_suggestion("Take a fresh snapshot and use its new ref")
        .with_details(json!({
            "kind": "resolve_ambiguous",
            "candidate_count": count,
        }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_desktop_core::{ErrorCode, IdentifierKind};

    fn identity(native: Option<&str>, role: &str, name: Option<&str>) -> NodeIdentity {
        NodeIdentity {
            native_id: native.map(|value| ElementIdentifier {
                kind: IdentifierKind::AutomationId,
                value: value.to_string(),
            }),
            role: if role.is_empty() {
                None
            } else {
                Some(role.to_string())
            },
            name: name.map(str::to_string),
            bounds_hash: None,
        }
    }

    fn id(expected: &str) -> ElementIdentifier {
        ElementIdentifier {
            kind: IdentifierKind::AutomationId,
            value: expected.to_string(),
        }
    }

    /// The A7-3 silent-wrong-target pin: two elements sharing a `native_id`
    /// but with different roles must not both match a ref that records the
    /// role. Weakening the evidence check to id-only would let a wrong element
    /// resolve - the exact shape A7-3 measured on Explorer.
    #[test]
    fn a_matching_native_id_with_a_mismatched_role_does_not_resolve() {
        let stored = identity(Some("row-77"), "row", Some("Alpha"));
        let other_row = identity(Some("row-77"), "option", Some("Beta"));

        assert!(!candidate_matches(
            &other_row,
            Some(&id("row-77")),
            "row",
            Some("Alpha")
        ));
        assert!(candidate_matches(
            &stored,
            Some(&id("row-77")),
            "row",
            Some("Alpha")
        ));
    }

    /// A ref with no id, no role and no name (the Electron shape, A7-1) cannot
    /// be disambiguated: the matching rule says any candidate matches, so the
    /// caller resolves `AMBIGUOUS_TARGET` over the search rather than guessing.
    /// (The role-mismatch narrowing that keeps an ordinary id ref from landing
    /// on the wrong element is pinned separately by
    /// `a_matching_native_id_with_a_mismatched_role_does_not_resolve`.)
    #[test]
    fn a_native_id_less_ref_is_ambiguous_not_a_guess() {
        let blank = identity(None, "", None);
        assert!(
            candidate_matches(&blank, None, "", None),
            "an all-absent ref matches any candidate; the caller's ambiguity check is the guard"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn a_zero_candidate_resolution_is_stale_windows() {
        let entry = RefEntry {
            process: agent_desktop_core::RefProcess {
                pid: agent_desktop_core::ProcessId::new(1),
                process_instance: None,
            },
            identity: agent_desktop_core::RefEntryIdentity {
                role: "button".into(),
                name: None,
                value: None,
                description: None,
                native_id: None,
            },
            geometry: agent_desktop_core::RefGeometry {
                bounds: None,
                bounds_hash: None,
            },
            capabilities: agent_desktop_core::RefCapabilities {
                states: Vec::new(),
                available_actions: Vec::new(),
            },
            source: agent_desktop_core::RefSource {
                source_app: None,
                source_window_id: Some("w-1".into()),
                source_window_title: None,
                source_window_bounds_hash: None,
                source_surface: agent_desktop_core::SnapshotSurface::Window,
            },
            scope: agent_desktop_core::RefScope {
                root_ref: None,
                path_is_absolute: false,
                path: agent_desktop_core::refs::RefPath::default(),
            },
        };
        assert_eq!(stale_ref_error(&entry).code, ErrorCode::StaleRef);
    }
}
