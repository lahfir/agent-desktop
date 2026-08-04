use agent_desktop_core::{
    AdapterError, LocatorEvidence, LocatorField, RefEntry,
    ref_identity::{has_stable_text_identity, identity_match, stable_text_match},
};
use serde_json::json;

use super::element::UIAElement;

/// A matched live element plus the tie-break hash read off its evidence.
#[derive(Clone)]
pub(crate) struct Candidate {
    pub(crate) element: UIAElement,
    pub(crate) bounds_hash: Option<u64>,
}

/// The three-state outcome the composed rule demands: a candidate either
/// matched, was refuted, or **could not be read** - and an unreadable
/// candidate makes the attempt incomplete-and-retryable rather than silently
/// non-matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CandidateOutcome {
    /// The candidate is the stored element.
    Matched,
    /// A read field contradicts the stored evidence; the candidate is not it.
    Refuted,
    /// A field that decides the question could not be read. Never a `NoMatch`.
    Incomplete,
}

/// The composed rule over core's verdicts.
///
/// `identity_match` settles the id tier for an id-bearing ref, and by itself
/// would settle on an id hit without consulting text - the exact behaviour
/// A7-3 measured as silently wrong on Windows (Explorer re-resolving 5 of 29
/// index-keyed refs onto a different element). The corroboration tier runs
/// only when the stored ref carries a stable text identity
/// (`has_stable_text_identity`): an id `Match` + stable-text `NoMatch` is
/// refuted, and a `NoMatch` never collapses an `Unknown` - the tier keys the
/// incomplete flag on a live stable-text field reading `LocatorField::Unknown`,
/// never on the verdict alone, because `stable_text_match` returns the same
/// `Unknown` for a structurally-absent live field as for a failed read. An
/// id `Match` on a ref with no stored text identity (an icon-only button)
/// resolves without corroboration, preserving the previous behaviour. An id-less ref
/// is routed through `identity_match`, which performs the text match itself.
pub(crate) fn candidate_outcome(entry: &RefEntry, evidence: &LocatorEvidence) -> CandidateOutcome {
    let id_tier = identity_match(
        entry,
        &evidence.name,
        &evidence.value,
        &evidence.description,
        &evidence.identifiers,
    );
    if entry.identity.native_id.is_some()
        && id_tier == agent_desktop_core::IdentityMatch::Match
        && has_stable_text_identity(entry)
    {
        return match stable_text_match(
            entry,
            &evidence.name,
            &evidence.value,
            &evidence.description,
        ) {
            agent_desktop_core::IdentityMatch::Match => CandidateOutcome::Matched,
            agent_desktop_core::IdentityMatch::NoMatch => CandidateOutcome::Refuted,
            agent_desktop_core::IdentityMatch::Unknown => CandidateOutcome::Incomplete,
        };
    }
    match id_tier {
        agent_desktop_core::IdentityMatch::Match => CandidateOutcome::Matched,
        agent_desktop_core::IdentityMatch::NoMatch => CandidateOutcome::Refuted,
        agent_desktop_core::IdentityMatch::Unknown => CandidateOutcome::Incomplete,
    }
}

/// Derives the tie-break hash from the walk-shaped evidence slot.
pub(crate) fn bounds_hash_of(evidence: &LocatorEvidence) -> Option<u64> {
    match evidence.ref_evidence.bounds {
        LocatorField::Known(rect) => rect.bounds_hash(),
        LocatorField::Absent | LocatorField::Unknown => None,
    }
}

/// What the 2+-candidate branch decided.
pub(crate) enum Selection {
    /// Exactly one candidate matched the stored bounds hash; its index in the
    /// searched candidate list.
    Resolved(usize),
    /// No stored hash, no candidate matched it, or more than one did.
    Ambiguous,
}

/// Breaks a tie among several evidence-equal candidates by bounds hash.
///
/// Pure over the hashes alone, not the elements, so the decision is testable
/// without a live COM element in scope. The bounds hash never refutes an
/// exact match; it only ever picks among several - a stored ref with no hash
/// (bounds failed at capture, or hidden by the requester) cannot be
/// disambiguated this way, so it stays ambiguous rather than guessing.
pub(crate) fn select_by_bounds_hash(
    candidate_hashes: &[Option<u64>],
    expected_hash: Option<u64>,
) -> Selection {
    let Some(expected_hash) = expected_hash else {
        return Selection::Ambiguous;
    };
    let mut sole = None;
    let mut matches = 0;
    for (index, hash) in candidate_hashes.iter().enumerate() {
        if *hash == Some(expected_hash) {
            matches += 1;
            sole = Some(index);
        }
    }
    match (matches, sole) {
        (1, Some(index)) => Selection::Resolved(index),
        _ => Selection::Ambiguous,
    }
}

pub(crate) fn stale_ref_error(_entry: &RefEntry) -> AdapterError {
    let default_retryable = AdapterError::stale_ref("Stored ref does not match any live element")
        .permits_retry_by_default();
    AdapterError::stale_ref("Stored ref does not match any live element").with_details(json!({
        "kind": "resolve_no_candidate",
        "complete": true,
        "retryable": default_retryable,
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
    use agent_desktop_core::{
        ElementIdentifier, ErrorCode, IdentifierEvidence, IdentifierKind, NodeDescriptor,
    };

    fn field(value: Option<&str>) -> LocatorField<String> {
        match value {
            Some(value) => LocatorField::Known(value.to_string()),
            None => LocatorField::Absent,
        }
    }

    fn evidence(
        name: Option<&str>,
        value: Option<&str>,
        description: Option<&str>,
        native_id: Option<&str>,
    ) -> LocatorEvidence {
        LocatorEvidence {
            role: LocatorField::Known("button".to_string()),
            name: field(name),
            value: field(value),
            description: field(description),
            identifiers: IdentifierEvidence::typed(
                native_id.into_iter().map(|value| ElementIdentifier {
                    kind: IdentifierKind::AutomationId,
                    value: value.to_string(),
                }),
                None,
                true,
            ),
            states: LocatorField::Absent,
            ref_evidence: agent_desktop_core::LocatorRefEvidence {
                bounds: LocatorField::Absent,
                available_actions: LocatorField::Absent,
                descriptors: NodeDescriptor::default(),
            },
        }
    }

    fn entry(
        native: Option<&str>,
        role: &str,
        name: Option<&str>,
        value: Option<&str>,
        description: Option<&str>,
    ) -> RefEntry {
        RefEntry {
            process: agent_desktop_core::RefProcess {
                pid: agent_desktop_core::ProcessId::new(1),
                process_instance: None,
            },
            identity: agent_desktop_core::RefEntryIdentity {
                role: role.to_string(),
                name: name.map(str::to_string),
                value: value.map(str::to_string),
                description: description.map(str::to_string),
                native_id: native.map(|value| ElementIdentifier {
                    kind: IdentifierKind::AutomationId,
                    value: value.to_string(),
                }),
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
                source_window_id: None,
                source_window_title: None,
                source_window_bounds_hash: None,
                source_surface: agent_desktop_core::SnapshotSurface::Window,
            },
            scope: agent_desktop_core::RefScope {
                root_ref: None,
                path_is_absolute: false,
                path: agent_desktop_core::refs::RefPath::default(),
            },
        }
    }

    /// The A7-3 silent-wrong-target pin under the composed matcher: a
    /// candidate sharing the stored `native_id` and role whose stable name
    /// drifted is refuted, not matched. Weakening the corroboration to
    /// id-only would let the wrong element resolve - the exact shape A7-3
    /// measured on Explorer.
    #[test]
    fn a_matching_native_id_with_a_stable_name_drift_is_refuted() {
        let stored = entry(Some("row-77"), "row", Some("Alpha"), None, None);
        let drifted = evidence(Some("Beta"), None, None, Some("row-77"));

        assert_eq!(
            candidate_outcome(&stored, &drifted),
            CandidateOutcome::Refuted
        );
        let matching = evidence(Some("Alpha"), None, None, Some("row-77"));
        assert_eq!(
            candidate_outcome(&stored, &matching),
            CandidateOutcome::Matched
        );
    }

    /// A candidate whose stable-text field could not be read is **incomplete**,
    /// never refuted - the test fails if `Unknown` is collapsed into `NoMatch`.
    #[test]
    fn an_unreadable_name_is_incomplete_not_a_mismatch() {
        let stored = entry(Some("btn-1"), "button", Some("Old Label"), None, None);
        let unreadable = LocatorEvidence {
            name: LocatorField::Unknown,
            ..evidence(None, None, None, Some("btn-1"))
        };

        assert_eq!(
            candidate_outcome(&stored, &unreadable),
            CandidateOutcome::Incomplete
        );
    }

    /// An id `Match` on a ref with no stored text identity (an icon-only
    /// button) resolves without corroboration - the previous behaviour preserved.
    #[test]
    fn an_id_match_without_stored_text_resolves_without_corroboration() {
        let stored = entry(Some("icon-1"), "button", None, None, None);
        let live = evidence(None, None, None, Some("icon-1"));

        assert_eq!(
            candidate_outcome(&stored, &live),
            CandidateOutcome::Matched
        );
    }

    /// A mutable-value role whose name mirrors its current value has no stable
    /// text identity by core's own `stable_name` rule (name == value is
    /// dropped as volatile), so an id `Match` resolves without corroboration -
    /// the prior pin, carried through core rather than re-implemented on Windows.
    /// The realistic textfield shape: stored name and value agree, the live
    /// candidate's drifted name mirrors its own new value.
    #[test]
    fn a_mutable_value_role_with_a_drifted_live_name_still_matches_on_native_id() {
        let stored = entry(Some("field-1"), "textfield", Some("Old Value"), Some("Old Value"), None);
        let live = evidence(Some("New Value"), Some("New Value"), None, Some("field-1"));

        assert_eq!(
            candidate_outcome(&stored, &live),
            CandidateOutcome::Matched
        );
    }

    /// The same mutable-value role with a **stable** name (a labelled field
    /// whose name does not mirror the value) keeps the corroboration tier: a
    /// drifted name refutes even with a matching id, exactly as a stable-role
    /// name would. The composition never special-cases mutable roles itself.
    #[test]
    fn a_mutable_value_role_with_a_stable_label_refutes_a_drifted_name() {
        let stored = entry(Some("field-2"), "textfield", Some("First Name"), Some("Alice"), None);
        let drifted = evidence(Some("First Name Is Drifted"), Some("Alice"), None, Some("field-2"));

        assert_eq!(
            candidate_outcome(&stored, &drifted),
            CandidateOutcome::Refuted
        );
    }

    /// A ref with no id and no stored text, against a blank live candidate,
    /// is not-known rather than a confident non-match: core's
    /// `empty_identity_match` reports `Unknown` (incomplete) instead of
    /// inventing a `NoMatch`. The corroboration tier must survive the switch.
    #[test]
    fn a_blank_ref_and_a_blank_candidate_are_incomplete_not_refuted() {
        let stored = entry(None, "", None, None, None);
        let blank_live = evidence(None, None, None, None);

        assert_eq!(
            candidate_outcome(&stored, &blank_live),
            CandidateOutcome::Incomplete
        );
    }

    /// A ref whose stored name was blank cannot be refuted by a blank live
    /// candidate when the id still matches - blank-cannot-refute, pinned on
    /// the id-bearing side.
    #[test]
    fn a_blank_stored_name_does_not_refute_a_matching_id() {
        let stored = entry(Some("row-1"), "listitem", None, None, None);
        let live = evidence(None, None, None, Some("row-1"));

        assert_eq!(
            candidate_outcome(&stored, &live),
            CandidateOutcome::Matched
        );
    }

    /// An id `Match` and an unreadable stable-text field is incomplete-
    /// retryable, never a settled refutation.
    #[test]
    fn an_id_match_with_unreadable_text_is_incomplete() {
        let stored = entry(Some("btn-1"), "button", Some("Old Label"), None, None);
        let unreadable = LocatorEvidence {
            name: LocatorField::Unknown,
            ..evidence(None, None, None, Some("btn-1"))
        };

        assert_eq!(
            candidate_outcome(&stored, &unreadable),
            CandidateOutcome::Incomplete
        );
    }

    /// A candidate whose native identifier cannot be read is incomplete: the
    /// id tier cannot decide it, and an unreadable field is never a `NoMatch`.
    #[test]
    fn an_unreadable_native_id_is_incomplete() {
        let stored = entry(Some("btn-1"), "button", Some("Label"), None, None);
        let unreadable_id = LocatorEvidence {
            identifiers: IdentifierEvidence::unknown(),
            ..evidence(Some("Label"), None, None, None)
        };

        assert_eq!(
            candidate_outcome(&stored, &unreadable_id),
            CandidateOutcome::Incomplete
        );
    }

    #[test]
    fn a_zero_candidate_resolution_is_stale_windows() {
        let stored = entry(Some("btn-1"), "button", Some("Label"), None, None);
        assert_eq!(stale_ref_error(&stored).code, ErrorCode::StaleRef);
    }

    /// Two candidates carrying the same evidence, neither distinguished by
    /// hash, must stay ambiguous - the caller's `AMBIGUOUS_TARGET`, not a
    /// guess at either one.
    #[test]
    fn two_identical_evidence_candidates_stay_ambiguous() {
        let selection = select_by_bounds_hash(&[Some(1), Some(1)], Some(1));

        assert!(matches!(selection, Selection::Ambiguous));
    }

    /// Exactly one of several candidates carries the stored bounds hash: that
    /// one resolves. The expected hash sits at index 1 on purpose, so a
    /// first-match-wins regression (picking index 0 regardless of hash) would
    /// fail this assertion rather than pass it by accident.
    #[test]
    fn the_sole_candidate_matching_the_stored_hash_resolves() {
        let selection = select_by_bounds_hash(&[Some(2), Some(1)], Some(1));

        assert!(matches!(selection, Selection::Resolved(1)));
    }

    /// A stored ref with no bounds hash cannot be disambiguated by hash at
    /// all, so several candidates stay ambiguous rather than one being picked
    /// by a `None == None` coincidence.
    #[test]
    fn no_stored_hash_cannot_disambiguate() {
        let selection = select_by_bounds_hash(&[None, None], None);

        assert!(matches!(selection, Selection::Ambiguous));
    }
}