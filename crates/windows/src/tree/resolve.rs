use agent_desktop_core::{AdapterError, Deadline, NativeHandle, RefEntry};

#[cfg(target_os = "windows")]
use super::element::UIAElement;
#[cfg(target_os = "windows")]
use super::resolve_match::{Candidate, CandidateOutcome, ambiguous_target_error, bounds_hash_of};
use super::resolve_match::stale_ref_error;
#[cfg(target_os = "windows")]
use super::walker::{DEFAULT_MAX_SIBLINGS, TreeSource, WalkBudget};
#[cfg(target_os = "windows")]
use super::walker_source::UiaTreeSource;
#[cfg(target_os = "windows")]
use agent_desktop_core::{ErrorCode, LocatorEvidence};
#[cfg(target_os = "windows")]
use serde_json::json;

/// The resolve-scoped depth cap.
///
/// Independently bounded from the walk's own ceiling, mirroring macOS's
/// `MAX_RESOLVE_DEPTH` (`crates/macos/src/tree/resolve.rs:15`, a distinct
/// constant). Electron elements commonly sit at depth 25+, so the cap is the
/// search bound rather than the walk bound.
const MAX_RESOLVE_DEPTH: u8 = 50;

/// Resolves a stored ref to its live element, fail-closed and three-state.
///
/// The search descends from the stored window's root to a resolve-scoped
/// depth, reading each candidate with the same composition the walk uses
/// (`UiaTreeSource::evidence`), gates on role, and runs core's composed
/// identity rule (`resolve_match::candidate_outcome`). Then:
///
/// - zero candidates and every decision was readable -> `STALE_REF`, settled
/// - zero candidates but one was **unreadable** -> incomplete-and-retryable
///   (`AppUnresponsive`, the three-state discipline: an `Unknown` verdict is
///   never a `NoMatch`)
/// - two or more candidates that all match -> `AMBIGUOUS_TARGET`
/// - exactly one -> a `NativeHandle` wrapping the live element
///
/// Anything short of an exact match fails closed rather than guessing,
/// because A7-3 measured Explorer re-resolving 29 of 29 `AutomationId` keys
/// with 5 landing on a different element - the silent-wrong-target shape
/// strictness exists to prevent.
#[cfg(target_os = "windows")]
pub(crate) fn resolve_element_strict(
    entry: &RefEntry,
    deadline: Deadline,
) -> Result<NativeHandle, AdapterError> {
    let root = resolve_window_root(entry, deadline)?;
    let source = UiaTreeSource::for_root(&root)?;
    let prepared = source.prepare_root(&root)?;

    let mut searched = Vec::new();
    let mut incomplete = false;
    let budget = WalkBudget::new(MAX_RESOLVE_DEPTH, deadline)
        .with_max_raw_depth(MAX_RESOLVE_DEPTH)
        .with_max_siblings(DEFAULT_MAX_SIBLINGS);
    search_under(
        &source,
        &prepared,
        0,
        &budget,
        entry,
        &mut searched,
        &mut incomplete,
    )?;

    match searched.len() {
        0 if incomplete => Err(identity_unknown_error(entry)),
        0 => Err(stale_ref_error(entry)),
        1 => {
            let Some(candidate) = searched.into_iter().next() else {
                return Err(stale_ref_error(entry));
            };
            Ok(candidate.element.into_native_handle())
        }
        _ => {
            let candidate_hashes: Vec<Option<u64>> = searched
                .iter()
                .map(|candidate| candidate.bounds_hash)
                .collect();
            match super::resolve_match::select_by_bounds_hash(
                &candidate_hashes,
                entry.geometry.bounds_hash,
            ) {
                super::resolve_match::Selection::Resolved(index) => {
                    Ok(searched[index].element.clone().into_native_handle())
                }
                super::resolve_match::Selection::Ambiguous => {
                    Err(ambiguous_target_error(entry, searched.len()))
                }
            }
        }
    }
}

/// The non-Windows twin. The crate cross-compiles to the Linux lane with the
/// resolver reachable, but there are no UI Automation elements there, so every
/// stored ref fails closed as stale rather than attempting a search that
/// cannot find anything.
#[cfg(not(target_os = "windows"))]
pub(crate) fn resolve_element_strict(
    entry: &RefEntry,
    _deadline: Deadline,
) -> Result<NativeHandle, AdapterError> {
    Err(stale_ref_error(entry))
}

/// The incomplete-and-retryable answer: a candidate that could not be read is
/// not a non-match. Mirrors macOS's `identity_unknown` shape exactly, a
/// `complete: false, retryable: true` stamp so the caller's loop retries it.
#[cfg(target_os = "windows")]
fn identity_unknown_error(entry: &RefEntry) -> AdapterError {
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

/// Reaches the stored window's root element from the ref's source window id.
///
/// The fail-closed process gate (the A7-3 wrong-target shape exists to
/// prevent): a stored ref must not search the tree of a different process that
/// has since recycled the HWND. The macOS resolver verifies process instance
/// before searching either. A token-less ref (elevated process whose token
/// could not be read) fails closed here rather than searching an unverified
/// window.
#[cfg(target_os = "windows")]
fn resolve_window_root(entry: &RefEntry, deadline: Deadline) -> Result<UIAElement, AdapterError> {
    let window_id = entry
        .source
        .source_window_id
        .as_deref()
        .ok_or_else(|| stale_ref_error(entry))?;
    if let Some(instance) = entry.process.process_instance.as_deref() {
        if !crate::system::process_identity::matches_instance(entry.process.pid, instance)? {
            return Err(stale_ref_error(entry));
        }
    } else {
        return Err(stale_ref_error(entry));
    }
    crate::tree::surfaces::surface_root(
        agent_desktop_core::ObservationRoot::Window(&agent_desktop_core::WindowInfo {
            id: window_id.to_string(),
            title: entry.source.source_window_title.clone().unwrap_or_default(),
            app: entry.source.source_app.clone().unwrap_or_default(),
            pid: entry.process.pid,
            process_instance: entry.process.process_instance.clone(),
            bounds: None,
            state: Default::default(),
        }),
        entry.source.source_surface,
        deadline,
    )
}

/// Searches the subtree under `element` to the resolve depth, collecting the
/// candidates the composed matcher accepted and flagging an unreadable one as
/// incomplete.
#[cfg(target_os = "windows")]
fn search_under(
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
            CandidateOutcome::Matched => out.push(build_candidate(element, &evidence)),
            CandidateOutcome::Incomplete => *incomplete = true,
            CandidateOutcome::Refuted => {}
        }
    } else if evidence.role.is_unknown() {
        *incomplete = true;
    }

    let children = enumerate_children(source, element, budget)?;
    for child in children {
        search_under(source, &child, depth + 1, budget, entry, out, incomplete)?;
    }
    Ok(())
}

/// Enumerates one element's children for the search, honouring the sibling cap
/// as a hard bound on pathological lists.
#[cfg(target_os = "windows")]
fn enumerate_children(
    source: &UiaTreeSource,
    element: &UIAElement,
    budget: &WalkBudget,
) -> Result<Vec<UIAElement>, AdapterError> {
    let mut children = Vec::new();
    let mut current = match source.first_child(element) {
        Ok(first) => first,
        Err(failure) if failure.is_exhaustion() => return Ok(children),
        Err(failure) => {
            return Err(super::automation::uia_failure_error(
                failure,
                "descend to a stored ref",
            ));
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
                return Err(super::automation::uia_failure_error(
                    failure,
                    "walk a stored ref's siblings",
                ));
            }
        }
    }
    Ok(children)
}

/// Builds the candidate from the walk-composed evidence the search already
/// read, projecting the tie-break hash off the same evidence slot.
#[cfg(target_os = "windows")]
fn build_candidate(element: &UIAElement, evidence: &LocatorEvidence) -> Candidate {
    Candidate {
        element: element.clone(),
        bounds_hash: bounds_hash_of(evidence),
    }
}



#[cfg(all(test, target_os = "windows"))]
mod windows_only {
    use super::*;
    use agent_desktop_core::{ElementIdentifier, LocatorEvidence, RefEntry, WindowInfo};

    fn first_identifier(evidence: &LocatorEvidence) -> Option<ElementIdentifier> {
        evidence
            .identifiers
            .identifiers()
            .iter()
            .find(|identifier| {
                matches!(identifier.kind, agent_desktop_core::IdentifierKind::AutomationId)
            })
            .cloned()
    }

    #[test]
    fn a_fixture_ref_resolves_to_the_same_element_end_to_end() {
        crate::tree::fixture::ensure_test_apartment();
        let fixture = crate::tree::fixture::HostedFixture::spawn().expect("a fixture host starts");
        let window = WindowInfo {
            id: format!("w-{}", fixture.handle()),
            title: "agent-desktop fixture".into(),
            app: "fixture.exe".into(),
            pid: agent_desktop_core::ProcessId::from(fixture.process_id()),
            process_instance: Some(
                crate::system::process_identity::token_for_pid(
                    agent_desktop_core::ProcessId::from(fixture.process_id()),
                )
                .unwrap()
                .expect("a live fixture process has a token"),
            ),
            bounds: None,
            state: Default::default(),
        };
        let deadline = crate::tree::walker_fake::deadline();
        let root = crate::tree::automation::root_from_hwnd(fixture.handle(), deadline)
            .expect("the fixture window resolves");
        let token = window.process_instance.clone().unwrap();

        let captured = capture_identified(&root, deadline).expect("a fixture element has an id");

        let entry = RefEntry {
            process: agent_desktop_core::RefProcess {
                pid: window.pid,
                process_instance: Some(token),
            },
            identity: agent_desktop_core::RefEntryIdentity {
                role: captured.1.clone().unwrap_or_default(),
                name: captured.2.clone(),
                value: None,
                description: None,
                native_id: captured.0.clone(),
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
                source_app: Some("fixture.exe".into()),
                source_window_id: Some(window.id.clone()),
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

        let handle = resolve_element_strict(&entry, deadline)
            .expect("the stored identity re-resolves to a live element");

        assert!(
            handle.downcast_ref::<UIAElement>().is_some(),
            "the resolved handle carries a UI Automation element"
        );
    }

    fn capture_identified(
        root: &UIAElement,
        deadline: agent_desktop_core::Deadline,
    ) -> Option<(Option<ElementIdentifier>, Option<String>, Option<String>)> {
        let source = UiaTreeSource::for_root(root).ok()?;
        let prepared = source.prepare_root(root).ok()?;
        let budget = WalkBudget::new(10, deadline);
        walk_for_identity(&source, &prepared, 0, &budget)
    }

    fn walk_for_identity(
        source: &UiaTreeSource,
        element: &UIAElement,
        depth: u8,
        budget: &WalkBudget,
    ) -> Option<(Option<ElementIdentifier>, Option<String>, Option<String>)> {
        if depth >= 10 {
            return None;
        }
        let (_, evidence, _) = source.evidence(element);
        let native_id = first_identifier(&evidence);
        if native_id.is_some() {
            return Some((
                native_id,
                evidence.role.known().cloned(),
                evidence.name.known().cloned(),
            ));
        }
        let children = enumerate_children(source, element, budget).ok()?;
        for child in children {
            if let Some(found) = walk_for_identity(source, &child, depth + 1, budget) {
                return Some(found);
            }
        }
        None
    }
}