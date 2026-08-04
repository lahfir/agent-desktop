//! The locator-anchor resolver (U6): selected-match hydration's path-only
//! variant of `resolve_element_strict`.
//!
//! Core's `live_locator::hydrate` re-observes the matched subtree from the
//! return handle and enforces evidence completeness itself; Windows only has to
//! land the anchor. The anchor's path is exact from the just-walked tree, so
//! there is **no broad-search fallback** - and its classification is the
//! inverse of the strict search's: every step is descent along a stored path
//! that churn makes permanently wrong, so a path step that lands nowhere, a
//! role-refuted candidate, or a vanished node **settles the attempt
//! immediately** (a completed-search `STALE_REF`, one attempt, the deadline
//! intact for core's fresh re-observation). Only the unresponsive/transport
//! class runs through the bounded retry loop, which re-walks the same cheap
//! O(depth) path rather than replaying an expensive search.

use agent_desktop_core::{AdapterError, Deadline, NativeHandle, RefEntry};

#[cfg(target_os = "windows")]
use super::element::UIAElement;
#[cfg(target_os = "windows")]
use super::resolve_match::CandidateOutcome;
use super::resolve_match::stale_ref_error;
#[cfg(target_os = "windows")]
use super::resolve_search::{MAX_RESOLVE_DEPTH, geometry_matches, identity_unknown_error};
#[cfg(target_os = "windows")]
use super::walker::{DEFAULT_MAX_SIBLINGS, TreeSource, WalkBudget};
#[cfg(target_os = "windows")]
use super::walker_source::UiaTreeSource;

/// Resolves a locator anchor, settled-on-churn and bounded on transport.
#[cfg(target_os = "windows")]
pub(crate) fn resolve_locator_anchor(
    entry: &RefEntry,
    deadline: Deadline,
) -> Result<NativeHandle, AdapterError> {
    super::resolve::retry_incomplete_until(deadline, || resolve_locator_anchor_once(entry, deadline))
}

/// The non-Windows twin. No UI Automation elements exist there, so every
/// anchor fails closed as stale instead of faking a landing.
#[cfg(not(target_os = "windows"))]
pub(crate) fn resolve_locator_anchor(
    entry: &RefEntry,
    _deadline: Deadline,
) -> Result<NativeHandle, AdapterError> {
    Err(stale_ref_error(entry))
}

#[cfg(target_os = "windows")]
fn resolve_locator_anchor_once(entry: &RefEntry, deadline: Deadline) -> Result<NativeHandle, AdapterError> {
    let root = super::resolve::resolve_window_root(entry, deadline)?;
    let source = UiaTreeSource::for_root(&root)?;
    let prepared = source.prepare_root(&root)?;
    let budget = WalkBudget::new(MAX_RESOLVE_DEPTH, deadline)
        .with_max_raw_depth(MAX_RESOLVE_DEPTH)
        .with_max_siblings(DEFAULT_MAX_SIBLINGS);

    if !location_identity_present(entry) {
        return Err(stale_ref_error(entry));
    }

    let Some(candidate) = anchor_path_landed(&source, &prepared, &entry.scope.path, &budget)? else {
        return Err(stale_ref_error(entry));
    };
    let (_, evidence, _) = source.evidence(&candidate);
    let role_matches = evidence
        .role
        .known()
        .is_some_and(|role| role == &entry.identity.role);
    if !role_matches {
        return Err(stale_ref_error(entry));
    }
    match super::resolve_match::candidate_outcome(entry, &evidence) {
        CandidateOutcome::Matched => Ok(super::resolve::into_verified_handle(candidate, entry)),
        CandidateOutcome::Incomplete if geometry_matches(entry, &evidence) => {
            Ok(super::resolve::into_verified_handle(candidate, entry))
        }
        CandidateOutcome::Incomplete => Err(identity_unknown_error(entry)),
        CandidateOutcome::Refuted => Err(stale_ref_error(entry)),
    }
}

/// The anchor's eligibility gate: it needs something to verify against, and a
/// path that is window-rooted (empty is valid - the root itself can be the
/// anchor).
#[cfg(target_os = "windows")]
fn location_identity_present(entry: &RefEntry) -> bool {
    (entry.scope.root_ref.is_none() || entry.scope.path_is_absolute)
        && (entry.geometry.bounds_hash.is_some()
            || agent_desktop_core::ref_identity::has_meaningful_identity(entry))
}

/// Walks the stored child-index path, anchor semantics: a step that lands
/// nowhere, an unsupported enumeration, or a vanished node is a settled miss
/// (`None`, never retried); a transport failure propagates so the loop
/// absorbs it.
#[cfg(target_os = "windows")]
fn anchor_path_landed(
    source: &UiaTreeSource,
    root: &UIAElement,
    path: &[usize],
    budget: &WalkBudget,
) -> Result<Option<UIAElement>, AdapterError> {
    let mut current = root.clone();
    for &index in path {
        let children = enumerate_anchor_children(source, &current, budget)?;
        let Some(child) = children.get(index) else {
            return Ok(None);
        };
        current = child.clone();
    }
    Ok(Some(current))
}

#[cfg(target_os = "windows")]
fn enumerate_anchor_children(
    source: &UiaTreeSource,
    element: &UIAElement,
    budget: &WalkBudget,
) -> Result<Vec<UIAElement>, AdapterError> {
    let mut children = Vec::new();
    let mut current = match source.first_child(element) {
        Ok(first) => first,
        Err(failure) if failure.is_exhaustion() => return Ok(children),
        Err(failure) => return anchor_descent(failure, children),
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
            Err(failure) => return anchor_descent(failure, children),
        }
    }
    Ok(children)
}

/// Anchor descent classification: a settled absence or a vanished node means
/// the subtree enumerates nothing (a real settled miss, never retried); a
/// transport failure propagates retryable; a terminal failure propagates as
/// the attempt's error. The stale-produced-by-completion shape (`STALE_REF`
/// with `complete: true`, retryability derived) is what satisfies core's
/// hydration retry predicate so the fresh re-observation fires.
#[cfg(target_os = "windows")]
fn anchor_descent(
    failure: super::automation::UiaFailure,
    children: Vec<UIAElement>,
) -> Result<Vec<UIAElement>, AdapterError> {
    match super::automation::uia_failure_disposition(failure) {
        crate::system::hresult::ReadDisposition::SettledAbsence
        | crate::system::hresult::ReadDisposition::Unavailable => Ok(children),
        crate::system::hresult::ReadDisposition::Retryable
        | crate::system::hresult::ReadDisposition::Terminal => Err(
            super::automation::uia_failure_error(failure, "walk a locator anchor's path"),
        ),
    }
}

#[cfg(all(test, target_os = "windows"))]
mod windows_only {
    use super::*;
    use crate::tree::fixture::{HostedFixture, ensure_test_apartment};
    use crate::tree::walker_fake::deadline;
    use agent_desktop_core::{ErrorCode, ProcessId, RefEntry};

    fn blank_entry_with_path(fixture: &HostedFixture, path: Vec<usize>) -> RefEntry {
        let deadline = deadline();
        let root = crate::tree::automation::root_from_hwnd(fixture.handle(), deadline)
            .expect("a fixture root");
        let source = crate::tree::walker_source::UiaTreeSource::for_root(&root).expect("a source");
        let prepared = source.prepare_root(&root).expect("a prepared root");
        let budget = crate::tree::walker::WalkBudget::new(10, deadline);
        let mut prefix = Vec::new();
        let found = walk_first_secure(&source, &prepared, 0, &budget, &mut prefix)
            .expect("the fixture walk succeeds")
            .expect("a secure element exists");
        let stored_path = found.path;
        let evidence = found.evidence;
        let rect = evidence.ref_evidence.bounds.known().expect("positive-area bounds");
        let hash = rect.bounds_hash().expect("a positive-area hash");
        let token = crate::system::process_identity::token_for_pid(ProcessId::new(fixture.process_id()))
            .unwrap()
            .expect("a live fixture token");
        let chosen_path = if path.is_empty() { stored_path } else { path };
        RefEntry {
            process: agent_desktop_core::RefProcess {
                pid: ProcessId::new(fixture.process_id()),
                process_instance: Some(token),
            },
            identity: agent_desktop_core::RefEntryIdentity {
                role: evidence.role.known().cloned().unwrap_or_default(),
                name: None,
                value: None,
                description: None,
                native_id: None,
            },
            geometry: agent_desktop_core::RefGeometry {
                bounds: Some(*rect),
                bounds_hash: Some(hash),
            },
            capabilities: agent_desktop_core::RefCapabilities {
                states: Vec::new(),
                available_actions: Vec::new(),
            },
            source: agent_desktop_core::RefSource {
                source_app: Some("fixture.exe".into()),
                source_window_id: Some(format!("w-{}", fixture.handle())),
                source_window_title: None,
                source_window_bounds_hash: None,
                source_surface: agent_desktop_core::SnapshotSurface::Window,
            },
            scope: agent_desktop_core::RefScope {
                root_ref: None,
                path_is_absolute: true,
                path: chosen_path.into(),
            },
        }
    }

    struct FoundSecure {
        path: Vec<usize>,
        _properties: crate::tree::properties::ElementProperties,
        evidence: agent_desktop_core::LocatorEvidence,
        _failed: u64,
    }

    fn walk_first_secure(
        source: &crate::tree::walker_source::UiaTreeSource,
        element: &UIAElement,
        depth: u8,
        budget: &crate::tree::walker::WalkBudget,
        prefix: &mut Vec<usize>,
    ) -> Result<Option<FoundSecure>, AdapterError> {
        if depth >= 10 {
            return Ok(None);
        }
        let (properties, node_evidence, failed) = source.evidence(element);
        if properties.is_secure() {
            return Ok(Some(FoundSecure {
                path: prefix.clone(),
                _properties: properties,
                evidence: node_evidence,
                _failed: failed,
            }));
        }
        let mut ignored = false;
        let children = crate::tree::resolve_search::enumerate_children(
            source,
            element,
            budget,
            &mut ignored,
        )?;
        for (index, child) in children.iter().enumerate() {
            prefix.push(index);
            if let Some(found) = walk_first_secure(source, child, depth + 1, budget, prefix)? {
                return Ok(Some(found));
            }
            prefix.pop();
        }
        Ok(None)
    }

    /// A stored anchor path that is exact on the unchanged fixture resolves -
    /// the hydration happy path.
    #[test]
    fn an_exact_anchor_path_resolves_on_the_unchanged_fixture() {
        ensure_test_apartment();
        let fixture = HostedFixture::spawn().expect("a fixture host starts");
        let entry = blank_entry_with_path(&fixture, Vec::new());
        let handle = resolve_locator_anchor(&entry, deadline())
            .expect("the exact anchor path resolves the secure element");
        assert!(handle.downcast_ref::<UIAElement>().is_some());
    }

    /// A path that points at a sibling beyond the target settles stale - the
    /// anchor never resolves a neighbour.
    #[test]
    fn a_wrong_child_index_settles_stale_never_a_neighbour() {
        ensure_test_apartment();
        let fixture = HostedFixture::spawn().expect("a fixture host starts");
        let entry = blank_entry_with_path(&fixture, vec![999]);
        let error = match resolve_locator_anchor(&entry, deadline()) {
            Err(error) => error,
            Ok(_) => panic!("a path that lands nowhere settles, it does not resolve"),
        };
        assert_eq!(error.code, ErrorCode::StaleRef);
    }

    /// A role-refuted landing settles stale: the stored ref records the secure
    /// element's role, the path lands on the fixture's button, and the anchor
    /// refuses - the path is a locator, never an identity by itself.
    #[test]
    fn a_role_refuted_landing_settles_stale() {
        ensure_test_apartment();
        let fixture = HostedFixture::spawn().expect("a fixture host starts");
        let entry = blank_entry_with_path(&fixture, vec![0]);
        let error = match resolve_locator_anchor(&entry, deadline()) {
            Err(error) => error,
            Ok(_) => panic!("a role-refuted landing settles stale, it does not resolve"),
        };
        assert_eq!(error.code, ErrorCode::StaleRef);
    }
}