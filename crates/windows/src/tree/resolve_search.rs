//! The resolution search family: the bounded broad search, the path
//! fast-path, and the geometry promotion.
//!
//! Split from `resolve.rs` to keep both files under the 400-line cap as the
//! resolver grows (the plan's file-size note names this seam). Everything here
//! mirrors a macOS shape: the search's role gate and incomplete tracking
//! (`resolve_search.rs:192-309`), the path fast-path and its eligibility gate
//! (`resolve.rs:280-290`), and the geometry promotion predicate
//! (`resolve_search.rs:330-333`) with one Windows-measured addition - the
//! stored hash must come from a positive-area rectangle (A17-7).

use agent_desktop_core::{
    AdapterError, ErrorCode, LocatorEvidence, RefEntry,
    ref_identity::has_meaningful_identity,
};
use serde_json::json;

use super::element::UIAElement;
use super::resolve_match::{Candidate, bounds_hash_of};
use super::walker::{TreeSource, WalkBudget};
use super::walker_source::UiaTreeSource;

/// The resolve-scoped depth cap.
///
/// Independently bounded from the walk's own ceiling, mirroring macOS's
/// `MAX_RESOLVE_DEPTH` (`crates/macos/src/tree/resolve.rs:15`). Electron
/// elements commonly sit at depth 25+, so the cap is the search bound rather
/// than the walk bound.
pub(crate) const MAX_RESOLVE_DEPTH: u8 = 50;

/// The incomplete-and-retryable answer: a candidate that could not be read is
/// not a non-match. Mirrors macOS's `identity_unknown` shape exactly, a
/// `complete: false, retryable: true` stamp so the caller's loop retries it.
pub(crate) fn identity_unknown_error(entry: &RefEntry) -> AdapterError {
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Strict resolution could not determine candidate identity from the live accessibility evidence",
    )
    .with_suggestion("Retry after the target application finishes updating its accessibility tree")
    .with_details(json!({
        "kind": "resolution_identity_unknown",
        "role": entry.identity.role,
        "complete": false,
        "retryable": true,
    }))
}

/// Whether the stored child-index path may be walked as a fast path.
///
/// Mirrors macOS's `can_use_path_fast_path`: window-rooted (no drill-down
/// root, or an absolute path) and non-empty, and the ref carries something
/// the matcher can verify with (an id, stable text, or a positive-area
/// bounds hash for the geometry tier).
pub(crate) fn can_use_path_fast_path(entry: &RefEntry) -> bool {
    (entry.scope.root_ref.is_none() || entry.scope.path_is_absolute)
        && !entry.scope.path.is_empty()
        && (entry.geometry.bounds_hash.is_some() || has_meaningful_identity(entry))
}

/// Walks the stored child-index path from a root, O(depth) child reads.
///
/// A path step that lands nowhere yields `None`; the caller treats that as
/// a miss and falls back to the broad search, never as a verdict that the
/// target is gone.
pub(crate) fn element_at_path(
    source: &UiaTreeSource,
    root: &UIAElement,
    path: &[usize],
    budget: &WalkBudget,
    incomplete: &mut bool,
) -> Result<Option<UIAElement>, AdapterError> {
    let mut current = root.clone();
    for &index in path {
        let children = enumerate_children(source, &current, budget, incomplete)?;
        let Some(child) = children.get(index) else {
            return Ok(None);
        };
        current = child.clone();
    }
    Ok(Some(current))
}

/// The geometry promotion eligibility predicate.
///
/// Mirrors macOS's `provisional_geometry_candidate` with one
/// Windows-measured addition: the stored bounds hash must come from a
/// positive-area rectangle (A17-7 - offscreen and virtualized elements
/// collapse to shared zero-extent bounds that are structurally non-unique),
/// and the stored bounds must agree. A ref with no meaningful text identity
/// and a zero-extent stored hash is unresolvable by design and never promotes.
pub(crate) fn provisional_geometry_candidate(entry: &RefEntry) -> bool {
    entry.geometry.bounds_hash.is_some()
        && entry
            .geometry
            .bounds
            .is_some_and(|rect| rect.width > 0.0 && rect.height > 0.0)
        && !has_meaningful_identity(entry)
}

/// Whether a candidate's live bounds hash matches the stored one under the
/// promotion's eligibility rules - the unique-geometry-match that promotes an
/// otherwise-unreadable candidate to resolved.
pub(crate) fn geometry_matches(entry: &RefEntry, evidence: &LocatorEvidence) -> bool {
    provisional_geometry_candidate(entry) && bounds_hash_of(evidence) == entry.geometry.bounds_hash
}

/// Searches the subtree under `element` to the resolve depth, collecting the
/// candidates the composed matcher accepted and flagging an unreadable one as
/// incomplete.
pub(crate) fn search_under(
    source: &UiaTreeSource,
    element: &UIAElement,
    depth: u8,
    budget: &WalkBudget,
    entry: &RefEntry,
    out: &mut Vec<Candidate>,
    incomplete: &mut bool,
) -> Result<(), AdapterError> {
    if depth >= MAX_RESOLVE_DEPTH {
        return Ok(());
    }
    crate::system::permissions::ensure_budget(budget.deadline)?;

    let (_, evidence, _) = source.evidence(element);
    let role_matches = evidence
        .role
        .known()
        .is_some_and(|role| role == &entry.identity.role);
    if role_matches {
        match super::resolve_match::candidate_outcome(entry, &evidence) {
            super::resolve_match::CandidateOutcome::Matched => {
                out.push(build_candidate(element, &evidence))
            }
            super::resolve_match::CandidateOutcome::Incomplete if geometry_matches(entry, &evidence) => {
                out.push(build_candidate(element, &evidence));
            }
            super::resolve_match::CandidateOutcome::Incomplete => *incomplete = true,
            super::resolve_match::CandidateOutcome::Refuted => {}
        }
    } else if evidence.role.is_unknown() {
        *incomplete = true;
    }

    let children = enumerate_children(source, element, budget, incomplete)?;
    for child in children {
        search_under(source, &child, depth + 1, budget, entry, out, incomplete)?;
    }
    Ok(())
}

/// Enumerates one element's children for the search, honouring the sibling cap
/// as a hard bound on pathological lists.
pub(crate) fn enumerate_children(
    source: &UiaTreeSource,
    element: &UIAElement,
    budget: &WalkBudget,
    incomplete: &mut bool,
) -> Result<Vec<UIAElement>, AdapterError> {
    let mut children = Vec::new();
    let mut current = match source.first_child(element) {
        Ok(first) => first,
        Err(failure) if failure.is_exhaustion() => return Ok(children),
        Err(failure) => {
            return descent_failure(failure, children, incomplete, "descend to a stored ref");
        }
    };
    loop {
        if children.len() >= budget.max_siblings {
            break;
        }
        let next = source.next_sibling(&current);
        children.push(current);
        match next {
            Ok(sibling) => current = sibling,
            Err(failure) if failure.is_exhaustion() => break,
            Err(failure) => {
                return descent_failure(
                    failure,
                    children,
                    incomplete,
                    "walk a stored ref's siblings",
                );
            }
        }
    }
    Ok(children)
}

/// Classifies a descent failure under the read disposition: a settled
/// absence means the node enumerates nothing (a real answer, not
/// incomplete); a transport failure or vanished node marks the search
/// incomplete and the descent continues - a non-target node dying
/// mid-descent under live churn is not evidence the target is gone; a
/// terminal failure propagates.
fn descent_failure(
    failure: super::automation::UiaFailure,
    children: Vec<UIAElement>,
    incomplete: &mut bool,
    context: &str,
) -> Result<Vec<UIAElement>, AdapterError> {
    match super::automation::uia_failure_disposition(failure) {
        crate::system::hresult::ReadDisposition::SettledAbsence => Ok(children),
        crate::system::hresult::ReadDisposition::Retryable
        | crate::system::hresult::ReadDisposition::Unavailable => {
            *incomplete = true;
            Ok(children)
        }
        crate::system::hresult::ReadDisposition::Terminal => Err(
            super::automation::uia_failure_error(failure, context),
        ),
    }
}

/// Builds the candidate from the walk-composed evidence the search already
/// read, projecting the tie-break hash off the same evidence slot.
pub(crate) fn build_candidate(element: &UIAElement, evidence: &LocatorEvidence) -> Candidate {
    Candidate {
        element: element.clone(),
        bounds_hash: bounds_hash_of(evidence),
    }
}


#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    use agent_desktop_core::{ElementIdentifier, IdentifierEvidence, IdentifierKind, LocatorField, NodeDescriptor};
    use crate::tree::fixture::ensure_test_apartment;

    fn entry(bounds: Option<agent_desktop_core::Rect>, hash: Option<u64>, name: Option<&str>, native: Option<&str>) -> RefEntry {
        RefEntry {
            process: agent_desktop_core::RefProcess {
                pid: agent_desktop_core::ProcessId::new(1),
                process_instance: None,
            },
            identity: agent_desktop_core::RefEntryIdentity {
                role: "button".to_string(),
                name: name.map(str::to_string),
                value: None,
                description: None,
                native_id: native.map(|value| ElementIdentifier {
                    kind: IdentifierKind::AutomationId,
                    value: value.to_string(),
                }),
            },
            geometry: agent_desktop_core::RefGeometry { bounds, bounds_hash: hash },
            capabilities: agent_desktop_core::RefCapabilities { states: Vec::new(), available_actions: Vec::new() },
            source: agent_desktop_core::RefSource {
                source_app: None,
                source_window_id: None,
                source_window_title: None,
                source_window_bounds_hash: None,
                source_surface: agent_desktop_core::SnapshotSurface::Window,
            },
            scope: agent_desktop_core::RefScope {
                root_ref: None,
                path_is_absolute: true,
                path: agent_desktop_core::refs::RefPath::default(),
            },
        }
    }

    fn rect(width: f64, height: f64) -> agent_desktop_core::Rect {
        agent_desktop_core::Rect { x: 10.0, y: 10.0, width, height }
    }

    fn evidence(bounds_hash: Option<u64>) -> LocatorEvidence {
        LocatorEvidence {
            role: LocatorField::Known("button".to_string()),
            name: LocatorField::Absent,
            value: LocatorField::Absent,
            description: LocatorField::Absent,
            identifiers: IdentifierEvidence::absent(),
            states: LocatorField::Absent,
            ref_evidence: agent_desktop_core::LocatorRefEvidence {
                bounds: match bounds_hash {
                    Some(_) => LocatorField::Known(rect(40.0, 20.0)),
                    None => LocatorField::Absent,
                },
                available_actions: LocatorField::Absent,
                descriptors: NodeDescriptor::default(),
            },
        }
    }

    #[test]
    fn a_window_rooted_non_empty_path_with_identity_qualifies_for_the_fast_path() {
        let mut absolute = entry(None, Some(1), Some("name"), Some("id"));
        absolute.scope.path.push(2);
        assert!(can_use_path_fast_path(&absolute));
    }

    #[test]
    fn a_relative_drill_down_path_skips_the_fast_path() {
        let mut relative = entry(None, Some(1), None, Some("id"));
        relative.scope.root_ref = Some("root".to_string());
        relative.scope.path_is_absolute = false;
        assert!(!can_use_path_fast_path(&relative));
    }

    #[test]
    fn an_empty_path_skips_the_fast_path() {
        assert!(!can_use_path_fast_path(&entry(None, Some(1), None, None)));
    }

    #[test]
    fn a_ref_with_no_id_text_or_hash_skips_the_fast_path() {
        assert!(!can_use_path_fast_path(&entry(None, None, None, None)));
    }

    #[test]
    fn promotion_requires_a_positive_area_stored_bounds() {
        let positive = entry(Some(rect(40.0, 20.0)), Some(1), None, None);
        assert!(provisional_geometry_candidate(&positive));

        let zero_extent = entry(Some(rect(0.0, 0.0)), Some(1), None, None);
        assert!(!provisional_geometry_candidate(&zero_extent));

        let only_hash = entry(None, Some(1), None, None);
        assert!(!provisional_geometry_candidate(&only_hash));
    }

    #[test]
    fn promotion_never_fires_when_the_entry_has_a_text_identity() {
        let named = entry(Some(rect(40.0, 20.0)), Some(1), Some("name"), None);
        assert!(!provisional_geometry_candidate(&named));

        let native = entry(Some(rect(40.0, 20.0)), Some(1), None, Some("id"));
        assert!(!provisional_geometry_candidate(&native));
    }

    #[test]
    fn geometry_matches_only_on_the_live_bounds_hash() {
        let live_hash = rect(40.0, 20.0).bounds_hash().expect("a positive-area hash");
        let stored = entry(Some(rect(40.0, 20.0)), Some(live_hash), None, None);
        assert!(geometry_matches(&stored, &evidence(Some(live_hash))));
        assert!(!geometry_matches(&stored, &evidence(None)));
    }

    #[test]
    fn geometry_matches_never_promotes_a_zero_extent_stored_hash() {
        let stored = entry(None, Some(0x1234), None, None);
        assert!(!geometry_matches(&stored, &evidence(Some(0x1234))));
    }

    #[test]
    fn the_live_fixture_exposes_a_promotion_eligible_password_edit() {
        ensure_test_apartment();
        let fixture = crate::tree::fixture::HostedFixture::spawn().expect("a fixture host starts");
        let source = UiaTreeSource::for_root(
            &crate::tree::automation::root_from_hwnd(fixture.handle(), crate::tree::walker_fake::deadline())
                .expect("the fixture resolves"),
        )
        .expect("a tree source");
        let prepared = source.prepare_root(
            &crate::tree::automation::root_from_hwnd(fixture.handle(), crate::tree::walker_fake::deadline())
                .expect("the fixture resolves"),
        )
        .expect("a prepared root");
        let budget = WalkBudget::new(10, crate::tree::walker_fake::deadline());
        let mut prefix = Vec::new();
        let found = find_secure(&source, &prepared, 0, &budget, &mut prefix)
            .expect("the fixture walk succeeds")
            .expect("a secure element exists");
        let (_, properties, evidence, _) = found;
        assert!(properties.is_secure());
        assert!(evidence.role.known().is_some());
    }

    fn find_secure(
        source: &UiaTreeSource,
        element: &UIAElement,
        depth: u8,
        budget: &WalkBudget,
        prefix: &mut Vec<usize>,
    ) -> Result<Option<(agent_desktop_core::refs::RefPath, crate::tree::properties::ElementProperties, LocatorEvidence, u64)>, AdapterError> {
        if depth >= 10 {
            return Ok(None);
        }
        let (properties, node_evidence, failed) = source.evidence(element);
        if properties.is_secure() {
            let mut path = agent_desktop_core::refs::RefPath::default();
            path.extend_from_slice(prefix);
            return Ok(Some((path, properties, node_evidence, failed)));
        }
        let mut ignored = false;
        let children = enumerate_children(source, element, budget, &mut ignored)?;
        for (index, child) in children.iter().enumerate() {
            prefix.push(index);
            if let Some(found) = find_secure(source, child, depth + 1, budget, prefix)? {
                return Ok(Some(found));
            }
            prefix.pop();
        }
        Ok(None)
    }
}