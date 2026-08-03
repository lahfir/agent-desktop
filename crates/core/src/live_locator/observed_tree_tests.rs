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

/// The P2-O8 descriptor group rides the observed-node projection unchanged:
/// evidence-side values reach the projected `AccessibilityNode` verbatim, and
/// an empty group projects as an absent group.
#[test]
fn descriptor_evidence_projects_onto_the_accessibility_node() {
    use crate::{
        IdentifierEvidence, LocatorEvidence, LocatorField, LocatorRefEvidence, NodeDescriptor, Rect,
    };

    let evidence_with = LocatorEvidence {
        role: LocatorField::Known("button".into()),
        name: LocatorField::Known("Save".into()),
        description: LocatorField::Absent,
        value: LocatorField::Absent,
        identifiers: IdentifierEvidence::absent(),
        states: LocatorField::Known(Vec::new()),
        ref_evidence: LocatorRefEvidence {
            bounds: LocatorField::Known(Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            }),
            available_actions: LocatorField::Known(vec!["Click".into()]),
            descriptors: NodeDescriptor {
                subrole: Some("button-icon".into()),
                role_description: Some("Push button".into()),
                placeholder: Some("Enter name".into()),
                dom_classes: vec!["btn".into()],
            },
        },
    };
    let tree = ObservedTree::from_roots(
        vec![ObservedSubtree::new(evidence_with, Vec::new(), true, None)],
        source(),
        Default::default(),
        true,
    )
    .unwrap();
    let node = tree.into_accessibility_tree().unwrap();

    assert_eq!(
        node.presentation.descriptors,
        NodeDescriptor {
            subrole: Some("button-icon".into()),
            role_description: Some("Push button".into()),
            placeholder: Some("Enter name".into()),
            dom_classes: vec!["btn".into()],
        }
    );

    let empty_tree = ObservedTree::from_roots(
        vec![ObservedSubtree::new(
            evidence("button", Some("Save")),
            Vec::new(),
            true,
            None,
        )],
        source(),
        Default::default(),
        true,
    )
    .unwrap();
    let empty_node = empty_tree.into_accessibility_tree().unwrap();
    assert_eq!(
        empty_node.presentation.descriptors,
        NodeDescriptor::default()
    );
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

#[test]
fn partial_projection_returns_the_observed_subtree_instead_of_discarding_it() {
    let incomplete =
        ObservedSubtree::new(evidence("button", Some("Save")), Vec::new(), false, None);
    let tree = ObservedTree::from_roots(
        vec![subtree("window", "Fixture", vec![incomplete])],
        source(),
        Default::default(),
        true,
    )
    .unwrap();
    let observed = tree.node_count();

    assert!(tree.clone().into_accessibility_tree().is_err());

    let (projected, complete, nodes_observed) = tree.into_accessibility_tree_partial().unwrap();

    assert!(!complete);
    assert_eq!(nodes_observed, observed);
    assert_eq!(projected.role, "window");
    assert_eq!(projected.children.len(), 1);
    assert_eq!(projected.children[0].identity.name.as_deref(), Some("Save"));

    assert!(
        projected.children[0].subtree_truncated,
        "the node whose descendants were cut must carry a boundary marker"
    );
    assert!(
        projected.subtree_truncated,
        "truncation must propagate to ancestors so a caller can walk to the cut"
    );
}

#[test]
fn partial_projection_reports_completeness_when_the_walk_finished() {
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

    let strict = tree.clone().into_accessibility_tree().unwrap();
    let (projected, complete, nodes_observed) = tree.into_accessibility_tree_partial().unwrap();

    assert!(complete);
    assert_eq!(nodes_observed, 2);
    assert!(
        !projected.subtree_truncated && !projected.children[0].subtree_truncated,
        "a complete walk must not mark any node truncated"
    );
    assert_eq!(projected.role, strict.role);
    assert_eq!(projected.identity.name, strict.identity.name);
    assert_eq!(projected.children.len(), strict.children.len());
}
