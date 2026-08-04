//! The locator-anchor resolver: selected-match hydration's path-only
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
use super::resolve_search::{
    MAX_RESOLVE_DEPTH, entry_is_locatable, geometry_matches, identity_unknown_error,
    provisional_geometry_candidate,
};
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
    super::resolve::retry_incomplete_until(deadline, || {
        resolve_locator_anchor_once(entry, deadline)
    })
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

/// The pure entry gate runs first, before any of `resolve_window_root`,
/// `UiaTreeSource::for_root` or `prepare_root` - three cross-process calls a
/// verdict `entry_is_locatable` can already settle from the stored ref alone
/// should never pay for. `resolve.rs`'s `resolve_attempt` runs its own
/// equivalent gate (`entry_is_unverifiable`) in the same first position.
///
/// The anchor reuses `entry_is_locatable` directly rather than the fast
/// path's `can_use_path_fast_path`: an empty path is a valid anchor - the
/// root element itself can be the stored target - so the anchor omits the
/// fast path's non-empty-path conjunct.
#[cfg(target_os = "windows")]
fn resolve_locator_anchor_once(
    entry: &RefEntry,
    deadline: Deadline,
) -> Result<NativeHandle, AdapterError> {
    if !entry_is_locatable(entry) {
        return Err(stale_ref_error(entry));
    }

    let root = super::resolve::resolve_window_root(entry, deadline)?;
    let source = UiaTreeSource::for_root(&root)?;
    let prepared = source.prepare_root(&root)?;
    let budget = WalkBudget::new(MAX_RESOLVE_DEPTH, deadline)
        .with_max_raw_depth(MAX_RESOLVE_DEPTH)
        .with_max_siblings(DEFAULT_MAX_SIBLINGS);

    let Some(candidate) = anchor_path_landed(&source, &prepared, &entry.scope.path, &budget)?
    else {
        return Err(stale_ref_error(entry));
    };
    let (_, evidence, _) = source.evidence(&candidate);
    anchor_role_verdict(entry, &evidence)?;
    match anchor_geometry_verdict(entry, &evidence) {
        AnchorGeometryVerdict::Promote => {
            Ok(super::resolve::into_verified_handle(candidate, entry))
        }
        AnchorGeometryVerdict::SettleStale => Err(stale_ref_error(entry)),
        AnchorGeometryVerdict::Retry => Err(identity_unknown_error(entry)),
    }
}

/// The three-state verdict `candidate_outcome`'s `Incomplete` answer resolves
/// to once the geometry tier has had its say.
#[cfg(target_os = "windows")]
enum AnchorGeometryVerdict {
    /// The candidate is the stored element.
    Promote,
    /// The candidate is definitively not the stored element: either the
    /// identity tier refuted it, or the geometry tier ran its comparison and
    /// the hashes disagreed.
    SettleStale,
    /// A field the matcher needed could not be read; this is the only
    /// verdict the caller's retry loop should replay.
    Retry,
}

/// Composes `candidate_outcome` with the geometry promotion tier into the
/// anchor's settle/retry decision.
///
/// A geometry mismatch is only ever a genuinely transient read when the
/// live bounds themselves came back [`agent_desktop_core::LocatorField::Unknown`]
/// (the provider could not answer this attempt); a bounds read that
/// succeeded and simply disagreed with the stored hash is a definitive
/// answer and settles, the same as a refuted identity tier does.
#[cfg(target_os = "windows")]
fn anchor_geometry_verdict(
    entry: &RefEntry,
    evidence: &agent_desktop_core::LocatorEvidence,
) -> AnchorGeometryVerdict {
    match super::resolve_match::candidate_outcome(entry, evidence) {
        CandidateOutcome::Matched => AnchorGeometryVerdict::Promote,
        CandidateOutcome::Incomplete if geometry_matches(entry, evidence) => {
            AnchorGeometryVerdict::Promote
        }
        CandidateOutcome::Incomplete if geometry_comparison_settled(entry, evidence) => {
            AnchorGeometryVerdict::SettleStale
        }
        CandidateOutcome::Incomplete => AnchorGeometryVerdict::Retry,
        CandidateOutcome::Refuted => AnchorGeometryVerdict::SettleStale,
    }
}

/// Whether a failed geometry promotion is a settled mismatch rather than a
/// transient read: the entry must be eligible for the geometry tier at all
/// (`provisional_geometry_candidate`), and the candidate's live bounds must
/// have actually been read rather than come back unreadable this attempt.
#[cfg(target_os = "windows")]
fn geometry_comparison_settled(
    entry: &RefEntry,
    evidence: &agent_desktop_core::LocatorEvidence,
) -> bool {
    provisional_geometry_candidate(entry) && !evidence.ref_evidence.bounds.is_unknown()
}

/// The anchor's role guard: an [`agent_desktop_core::LocatorField::Unknown`]
/// role is a failed read, not evidence the element is some other role, so it
/// stays retryable; anything else is a settled verdict, either a role match
/// (the walk continues) or a role-refuted landing (the path is a locator,
/// never an identity by itself).
#[cfg(target_os = "windows")]
fn anchor_role_verdict(
    entry: &RefEntry,
    evidence: &agent_desktop_core::LocatorEvidence,
) -> Result<(), AdapterError> {
    if evidence.role.is_unknown() {
        return Err(identity_unknown_error(entry));
    }
    let role_matches = evidence
        .role
        .known()
        .is_some_and(|role| role == &entry.identity.role);
    if role_matches {
        Ok(())
    } else {
        Err(stale_ref_error(entry))
    }
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
#[path = "resolve_anchor_selector_wait_tests.rs"]
mod selector_wait_tests;

#[cfg(all(test, target_os = "windows"))]
#[path = "resolve_anchor_count_agreement_tests.rs"]
mod count_agreement_tests;

#[cfg(all(test, target_os = "windows"))]
#[path = "resolve_anchor_tests.rs"]
mod windows_only;
