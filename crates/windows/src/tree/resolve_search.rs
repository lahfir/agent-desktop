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
    AdapterError, ErrorCode, LocatorEvidence, RefEntry, ref_identity::has_meaningful_identity,
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

/// Bundles the three values that stay invariant across `search_under`'s
/// recursive descent - the tree source, the stored ref being resolved
/// against, and the walk budget - so the recursive signature threads one
/// reference instead of carrying each individually at every call.
pub(crate) struct SearchContext<'a> {
    pub(crate) source: &'a UiaTreeSource,
    pub(crate) entry: &'a RefEntry,
    pub(crate) budget: &'a WalkBudget,
}

/// Whether a child at `depth + 1` would still be within the resolve depth
/// cap and therefore actually get searched.
///
/// Enumeration is gated on this rather than run unconditionally: at
/// `depth + 1 == MAX_RESOLVE_DEPTH`, every child the enumeration would
/// return is immediately discarded by the depth guard on entry to the next
/// call, so running the enumeration first pays a real, cross-process
/// sibling walk (up to the sibling cap) purely for children the search will
/// never evaluate. Skipping the enumeration also means a transport failure
/// among those never-searched children can no longer flag the search
/// incomplete - correctly so: nothing this search will ever look at can be
/// the reason to retry it, so the retry would buy nothing.
fn child_search_is_reachable(depth: u8) -> bool {
    depth + 1 < MAX_RESOLVE_DEPTH
}

/// Searches the subtree under `element` to the resolve depth, collecting the
/// candidates the composed matcher accepted and flagging an unreadable one as
/// incomplete.
pub(crate) fn search_under(
    ctx: &SearchContext<'_>,
    element: &UIAElement,
    depth: u8,
    out: &mut Vec<Candidate>,
    incomplete: &mut bool,
) -> Result<(), AdapterError> {
    if depth >= MAX_RESOLVE_DEPTH {
        return Ok(());
    }
    crate::system::permissions::ensure_budget(ctx.budget.deadline)?;

    let (_, evidence, _) = ctx.source.evidence(element);
    let role_matches = evidence
        .role
        .known()
        .is_some_and(|role| role == &ctx.entry.identity.role);
    if role_matches {
        match super::resolve_match::candidate_outcome(ctx.entry, &evidence) {
            super::resolve_match::CandidateOutcome::Matched => {
                out.push(build_candidate(element, &evidence))
            }
            super::resolve_match::CandidateOutcome::Incomplete
                if geometry_matches(ctx.entry, &evidence) =>
            {
                out.push(build_candidate(element, &evidence));
            }
            super::resolve_match::CandidateOutcome::Incomplete => *incomplete = true,
            super::resolve_match::CandidateOutcome::Refuted => {}
        }
    } else if evidence.role.is_unknown() {
        *incomplete = true;
    }

    if child_search_is_reachable(depth) {
        let children = enumerate_children(ctx.source, element, ctx.budget, incomplete)?;
        for child in children {
            search_under(ctx, &child, depth + 1, out, incomplete)?;
        }
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
        crate::system::hresult::ReadDisposition::Terminal => {
            Err(super::automation::uia_failure_error(failure, context))
        }
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
#[path = "resolve_search_tests.rs"]
mod tests;
