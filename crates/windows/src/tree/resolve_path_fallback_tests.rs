use super::*;
use agent_desktop_core::{LocatorEvidence, refs::RefPath};

/// The fixture control the stored ref names. Its role is shared with the
/// window's title-bar buttons, which is what makes a same-role decoy
/// available without the role gate deciding the case first.
const TARGET_NAME: &str = "fixture-button";

fn catalogue(
    source: &UiaTreeSource,
    element: &UIAElement,
    depth: u8,
    budget: &WalkBudget,
    prefix: &mut Vec<usize>,
    out: &mut Vec<(RefPath, LocatorEvidence)>,
) {
    if depth >= 8 {
        return;
    }
    let (_, evidence, _) = source.evidence(element);
    let mut path = RefPath::default();
    path.extend_from_slice(prefix);
    out.push((path, evidence));
    let mut ignored = false;
    let Ok(children) =
        crate::tree::resolve_search::enumerate_children(source, element, budget, &mut ignored)
    else {
        return;
    };
    for (index, child) in children.iter().enumerate() {
        prefix.push(index);
        catalogue(source, child, depth + 1, budget, prefix, out);
        prefix.pop();
    }
}

fn stored_entry(
    fixture_handle: isize,
    pid: u32,
    evidence: &LocatorEvidence,
    path: RefPath,
) -> RefEntry {
    let pid = agent_desktop_core::ProcessId::from(pid);
    RefEntry {
        process: agent_desktop_core::RefProcess {
            pid,
            process_instance: Some(
                crate::system::process_identity::token_for_pid(pid)
                    .expect("the token read answers")
                    .expect("a live fixture process has a token"),
            ),
        },
        identity: agent_desktop_core::RefEntryIdentity {
            role: evidence.role.known().cloned().unwrap_or_default(),
            name: evidence.name.known().cloned(),
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
            source_app: Some("fixture.exe".into()),
            source_window_id: Some(format!("w-{fixture_handle}")),
            source_window_title: None,
            source_window_bounds_hash: None,
            source_surface: agent_desktop_core::SnapshotSurface::Window,
        },
        scope: agent_desktop_core::RefScope {
            root_ref: None,
            path_is_absolute: true,
            path,
        },
    }
}

/// The path fast-path's fall-through, driven against a live tree.
///
/// The stored child-index path still lands on a live element - so this is
/// not the path-missed case - but the element now occupying that index is a
/// different one of the same role, which the identity evidence refutes. A
/// refutation at the stored index is evidence about that element only, never
/// about the stored one, so resolution must continue into the broad search
/// and find the real target elsewhere in the tree. Settling `STALE_REF` there
/// instead would strand every ref whose siblings were reordered.
#[test]
fn a_refuted_element_at_the_stored_path_falls_through_to_the_broad_search() {
    crate::tree::fixture::ensure_test_apartment();
    let fixture = crate::tree::fixture::HostedFixture::spawn().expect("a fixture host starts");
    let deadline = crate::tree::walker_fake::deadline();
    let root = crate::tree::automation::root_from_hwnd(fixture.handle(), deadline)
        .expect("the fixture window resolves");
    let source = UiaTreeSource::for_root(&root).expect("a tree source");
    let prepared = source.prepare_root(&root).expect("a prepared root");
    let budget = WalkBudget::new(10, deadline);

    let mut prefix = Vec::new();
    let mut catalogued = Vec::new();
    catalogue(&source, &prepared, 0, &budget, &mut prefix, &mut catalogued);

    let (target_path, target) = catalogued
        .iter()
        .find(|(_, evidence)| {
            evidence
                .name
                .known()
                .is_some_and(|name| name == TARGET_NAME)
        })
        .cloned()
        .expect("the fixture exposes its named button");
    let (decoy_path, decoy) = catalogued
        .iter()
        .find(|(path, evidence)| {
            path != &target_path
                && evidence.role.known() == target.role.known()
                && evidence.name.known() != target.name.known()
        })
        .cloned()
        .expect("the fixture exposes a same-role control with a different identity");

    let entry = stored_entry(
        fixture.handle(),
        fixture.process_id(),
        &target,
        decoy_path.clone(),
    );
    assert!(
        crate::tree::resolve_search::can_use_path_fast_path(&entry),
        "the fast path must actually run, or the fall-through is never reached"
    );

    let mut path_incomplete = false;
    let landed = crate::tree::resolve_search::element_at_path(
        &source,
        &prepared,
        &entry.scope.path,
        &budget,
        &mut path_incomplete,
    )
    .expect("the path walk answers")
    .expect("the stored path still lands on a live element");
    let (_, landed_evidence, _) = source.evidence(&landed);
    assert_eq!(
        landed_evidence.role.known(),
        target.role.known(),
        "the landing shares the stored role, so the role gate is not what rejects it"
    );
    assert_eq!(
        crate::tree::resolve_match::candidate_outcome(&entry, &landed_evidence),
        CandidateOutcome::Refuted,
        "the element at the stored index is refuted by the stored identity"
    );

    let handle = resolve_element_strict(&entry, deadline)
        .expect("a refutation at the stored path must not settle the resolution");
    let resolved = handle
        .downcast_ref::<UIAElement>()
        .expect("the resolved handle carries a UI Automation element");
    let (_, resolved_evidence, _) = source.evidence(resolved);

    assert_eq!(
        resolved_evidence.name.known(),
        target.name.known(),
        "the broad search found the stored element"
    );
    assert_eq!(
        resolved_evidence.ref_evidence.bounds.known(),
        target.ref_evidence.bounds.known(),
        "the resolved element is the stored one, not the decoy at the stored path"
    );
    assert_ne!(
        resolved_evidence.ref_evidence.bounds.known(),
        decoy.ref_evidence.bounds.known(),
        "the decoy and the target are genuinely different elements"
    );
}
