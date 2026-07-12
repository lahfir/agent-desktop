use super::{
    LocatorMaterialization, LocatorResolveRequest, LocatorSelection, evaluate_locator_tree,
};
use crate::{locator::LocatorQuery, refs::RefPath};

use super::test_support::{evidence, node, tree};

fn request(selection: LocatorSelection) -> LocatorResolveRequest {
    LocatorResolveRequest {
        selection,
        deadline: crate::Deadline::from_duration(std::time::Duration::from_secs(5)).unwrap(),
        max_raw_depth: 50,
        materialization: LocatorMaterialization::None,
    }
}

#[test]
fn six_thousand_four_hundred_one_nodes_retain_zero_native_handles() {
    let nodes = (0..6_401)
        .map(|order| node(order, evidence("button", Some("match")), vec![], &[]))
        .collect::<Vec<_>>();
    let roots = (0..6_401).collect::<Vec<_>>();
    let observed = tree(nodes, roots, true);

    assert_eq!(observed.node_count(), 6_401);
    assert_eq!(observed.retained_handle_count(), 0);
}

#[test]
fn count_returns_no_target_ownership() {
    let resolution = evaluate_locator_tree(
        tree(
            vec![node(0, evidence("button", Some("match")), vec![], &[])],
            vec![0],
            true,
        ),
        &LocatorQuery::default(),
        &request(LocatorSelection::Count),
    )
    .unwrap();

    assert!(resolution.matches.is_empty());
}

#[test]
fn invalid_tree_fails_before_evaluation() {
    let mut invalid = tree(
        vec![node(0, evidence("button", Some("match")), vec![], &[])],
        vec![0],
        true,
    );
    invalid.nodes[0].path = RefPath::from_slice(&[1]);
    let error = evaluate_locator_tree(
        invalid,
        &LocatorQuery::default(),
        &request(LocatorSelection::Strict),
    )
    .err()
    .unwrap();

    assert_eq!(error.code, crate::ErrorCode::Internal);
}
