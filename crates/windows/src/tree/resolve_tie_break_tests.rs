use super::*;

use super::pair_window::{
    DuplicatePairWindow, PairGeometry, TITLE_MARKER, automation_id, collect_marked,
};

/// The bounds tie-break returns the stored candidate, not the first one it
/// collected.
///
/// Two live controls share role, `Name` and `AutomationId`, so every identity
/// tier matches both and the bounds hash is the only tier that separates them.
/// The stored ref carries the hash of the candidate the search collects
/// **second**, so a resolver that took the head of its candidate list instead
/// of the selected index would return the other control and report success -
/// the silent-wrong-target class A7-3 measured, where Explorer re-resolved 5
/// of 29 refs onto a different element without an error. A first-match-wins
/// regression therefore cannot pass here by luck: the correct answer is never
/// at index zero.
#[test]
fn the_bounds_tie_break_resolves_the_stored_candidate_not_the_first_collected() {
    crate::tree::fixture::ensure_test_apartment();
    let hosted =
        DuplicatePairWindow::create(PairGeometry::Separated).expect("a duplicate window hosts");
    let deadline = crate::tree::walker_fake::deadline();
    let root = crate::tree::automation::root_from_hwnd(hosted.handle, deadline)
        .expect("the hosted window resolves");
    let source = UiaTreeSource::for_root(&root).expect("a tree source");
    let prepared = source.prepare_root(&root).expect("a prepared root");
    let budget = WalkBudget::new(10, deadline);

    let mut duplicates = Vec::new();
    collect_marked(&source, &prepared, 0, &budget, &mut duplicates);
    assert_eq!(
        duplicates.len(),
        2,
        "the host must present exactly two marked controls"
    );
    let (head, target) = (&duplicates[0], &duplicates[1]);
    let role = head.role.known().cloned().expect("a read role");
    let name = head.name.known().cloned().expect("a read name");
    let native_id = automation_id(head);
    assert_eq!(target.role.known(), Some(&role), "the roles are equal");
    assert_eq!(target.name.known(), Some(&name), "the names are equal");
    assert_eq!(
        automation_id(target),
        native_id,
        "the identifiers are equal, so no identity tier separates the pair"
    );

    let head_hash = crate::tree::resolve_match::bounds_hash_of(head)
        .expect("the head candidate has a positive-area bounds hash");
    let target_hash = crate::tree::resolve_match::bounds_hash_of(target)
        .expect("the stored candidate has a positive-area bounds hash");
    assert_ne!(
        head_hash, target_hash,
        "the two controls must occupy distinct rectangles, or the tie-break has nothing to pick by"
    );
    let rect = *target
        .ref_evidence
        .bounds
        .known()
        .expect("a read bounds rectangle");

    let pid = agent_desktop_core::ProcessId::from(std::process::id());
    let entry = RefEntry {
        process: agent_desktop_core::RefProcess {
            pid,
            process_instance: Some(
                crate::system::process_identity::token_for_pid(pid)
                    .expect("the token read answers")
                    .expect("a live process has a token"),
            ),
        },
        identity: agent_desktop_core::RefEntryIdentity {
            role,
            name: Some(name),
            value: None,
            description: None,
            native_id,
        },
        geometry: agent_desktop_core::RefGeometry {
            bounds: Some(rect),
            bounds_hash: Some(target_hash),
        },
        capabilities: agent_desktop_core::RefCapabilities {
            states: Vec::new(),
            available_actions: Vec::new(),
        },
        source: agent_desktop_core::RefSource {
            source_app: None,
            source_window_id: Some(format!("w-{}", hosted.handle)),
            source_window_title: Some(TITLE_MARKER.into()),
            source_window_bounds_hash: None,
            source_surface: agent_desktop_core::SnapshotSurface::Window,
        },
        scope: agent_desktop_core::RefScope {
            root_ref: None,
            path_is_absolute: false,
            path: agent_desktop_core::refs::RefPath::default(),
        },
    };

    for evidence in &duplicates {
        assert_eq!(
            crate::tree::resolve_match::candidate_outcome(&entry, evidence),
            CandidateOutcome::Matched,
            "both live controls satisfy every identity tier"
        );
    }
    assert!(
        matches!(
            classify_search(&[Some(head_hash), Some(target_hash)], false, &entry),
            SearchVerdict::Resolved(1)
        ),
        "the stored hash selects the second candidate, so index zero is the wrong answer"
    );

    let handle = resolve_element_strict(&entry, deadline)
        .expect("the stored hash separates the pair, so resolution settles on one of them");

    let resolved = handle
        .downcast_ref::<UIAElement>()
        .expect("the resolved handle carries a UI Automation element");
    let (_, resolved_evidence, _) = source.evidence(resolved);
    assert_eq!(
        crate::tree::resolve_match::bounds_hash_of(&resolved_evidence),
        Some(target_hash),
        "the resolver returned the candidate the stored hash selected, not the first collected"
    );
}
