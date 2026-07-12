use super::{
    IdentifierEvidence, LocatorMaterialization, LocatorResolveRequest, LocatorSelection,
    ObservationSource, ObservedSubtree, ObservedTree, evaluate_locator_tree,
};
use crate::{
    WindowInfo,
    locator::{IdentityPredicate, LocatorQuery},
};

use super::test_support::evidence;

fn source() -> ObservationSource {
    ObservationSource::Window(WindowInfo {
        id: "w-1".into(),
        title: "Fixture".into(),
        app: "FixtureApp".into(),
        pid: crate::ProcessId::new(42),
        process_instance: Some("test-instance".into()),
        bounds: None,
        state: crate::WindowState {
            is_focused: true,
            ..Default::default()
        },
    })
}

fn subtree(role: &str, name: &str, children: Vec<ObservedSubtree>) -> ObservedSubtree {
    ObservedSubtree::new(evidence(role, Some(name)), children, true, None)
}

#[test]
fn core_builder_owns_preorder_paths_and_child_indices() {
    let tree = ObservedTree::from_roots(
        vec![subtree(
            "window",
            "Fixture",
            vec![
                subtree("button", "Save", Vec::new()),
                subtree("link", "Help", Vec::new()),
            ],
        )],
        source(),
        Default::default(),
        true,
    )
    .unwrap();

    assert_eq!(tree.roots, [0]);
    assert_eq!(tree.nodes[0].children, [1, 2]);
    assert_eq!(tree.nodes[0].document_order, 0);
    assert_eq!(tree.nodes[1].document_order, 1);
    assert_eq!(tree.nodes[2].document_order, 2);
    assert!(tree.nodes[0].path.is_empty());
    assert_eq!(tree.nodes[1].path.as_slice(), &[0]);
    assert_eq!(tree.nodes[2].path.as_slice(), &[1]);
    assert_eq!(tree.retained_handle_count(), 0);
}

#[test]
fn source_child_indices_survive_omitted_siblings() {
    let target = subtree("scrollarea", "Nested", Vec::new()).with_source_child_index(0);
    let retained = subtree("group", "Wrapper", vec![target])
        .with_source_child_index(2)
        .with_predecessors_complete(false);
    let tree = ObservedTree::from_roots(
        vec![subtree("window", "Fixture", vec![retained])],
        source(),
        Default::default(),
        true,
    )
    .unwrap();

    assert_eq!(tree.nodes[1].path.as_slice(), &[2]);
    assert_eq!(tree.nodes[2].path.as_slice(), &[2, 0]);
    assert!(!tree.nodes[1].completeness.predecessors_complete);
    assert!(tree.nodes[2].completeness.predecessors_complete);
}

#[test]
fn incomplete_descendant_propagates_to_the_observation_root() {
    let incomplete =
        ObservedSubtree::new(evidence("button", Some("Save")), Vec::new(), false, None);
    let tree = ObservedTree::from_roots(
        vec![subtree("window", "Fixture", vec![incomplete])],
        source(),
        Default::default(),
        true,
    )
    .unwrap();

    assert!(!tree.is_complete());
    assert!(tree.into_accessibility_tree().is_err());
}

#[test]
fn snapshot_projection_and_find_share_the_same_observation() {
    let tree = ObservedTree::from_roots(
        vec![subtree(
            "window",
            "Fixture",
            vec![subtree("button", "Save", Vec::new())],
        )],
        source(),
        Default::default(),
        true,
    )
    .unwrap();
    let snapshot = tree.clone().into_accessibility_tree().unwrap();
    let query = LocatorQuery {
        identity: IdentityPredicate {
            name: Some("save".into()),
            ..IdentityPredicate::default()
        },
        ..LocatorQuery::default()
    };
    let resolution = evaluate_locator_tree(
        tree,
        &query,
        &LocatorResolveRequest {
            selection: LocatorSelection::Count,
            deadline: crate::Deadline::from_duration(std::time::Duration::from_secs(1)).unwrap(),
            max_raw_depth: 50,
            materialization: LocatorMaterialization::None,
        },
    )
    .unwrap();

    assert_eq!(resolution.meta.total_matches, 1);
    assert_eq!(
        snapshot
            .children
            .iter()
            .filter(|node| crate::accessibility_node_matches(node, &query))
            .count(),
        1
    );
}

#[test]
fn identifier_evidence_preserves_preference_without_platform_names() {
    let identifiers = IdentifierEvidence::new(
        [
            "automation-id".into(),
            "runtime-id".into(),
            "automation-id".into(),
        ],
        Some(0),
        true,
    );

    assert_eq!(identifiers.values(), ["automation-id", "runtime-id"]);
    assert_eq!(identifiers.preferred_value(), Some("automation-id"));
    assert!(identifiers.is_complete());
}
