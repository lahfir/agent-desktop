use super::*;
use crate::tree::automation::UiaFailure;
use crate::tree::walker_enumerate::TreeWalk;
use crate::tree::walker_fake::{E_FAIL, EXHAUSTION, FakeTree, REAL_FAILURE, budget, walk};

/// The fakes encode the measured pair, not a reading of the crate. If this
/// fails, every other fake-driven assertion below is testing the wrong model.
#[test]
fn the_fakes_are_built_from_the_measured_error_pair() {
    assert!(EXHAUSTION.is_exhaustion());
    assert!(!REAL_FAILURE.is_exhaustion());
    assert_eq!(REAL_FAILURE, UiaFailure::Hresult(E_FAIL));
}

/// A genuinely cyclic child graph: the deepest node's child is the root. Only
/// the guard ends this walk within the requested depth.
#[test]
fn a_repeated_identity_is_skipped_once_and_the_walk_terminates() {
    let fake = FakeTree::default()
        .with_chain(&[1, 2, 3])
        .with_children(3, &[1]);

    let outcome = walk(&fake, budget(10));

    assert_eq!(outcome.stats.traversal.cycles_skipped, 1);
    assert_eq!(outcome.stats.traversal.limits.depth_hits, 0);
    assert!(outcome.tree.is_complete());
}

/// The key is the identity the provider publishes, not the element the client
/// happens to hold: UI Automation returns a new proxy per query, so a distinct
/// proxy reporting an ancestor's runtime id is the same element.
#[test]
fn a_cycle_is_detected_by_published_identity_and_not_by_node_value() {
    let fake = FakeTree::default().with_chain(&[1, 2, 3, 4]).aliasing(4, 2);

    let outcome = walk(&fake, budget(10));

    assert_eq!(outcome.stats.traversal.cycles_skipped, 1);
}

#[test]
fn a_provider_without_a_runtime_id_is_guarded_by_element_comparison() {
    let fake = FakeTree::default()
        .with_chain(&[1, 2, 3, 4])
        .aliasing(4, 2)
        .unkeyed(2)
        .unkeyed(4);

    let outcome = walk(&fake, budget(10));

    assert_eq!(outcome.stats.traversal.cycles_skipped, 1);
}

/// A cycle on one branch must not prune an unrelated branch that reaches the
/// same element, which is what a global visited set would do.
#[test]
fn the_guard_is_an_ancestor_path_and_not_a_global_visited_set() {
    let fake = FakeTree::default()
        .with_children(1, &[2, 3])
        .with_children(2, &[4])
        .with_children(3, &[5])
        .aliasing(5, 4);

    let outcome = walk(&fake, budget(10));

    assert_eq!(outcome.stats.traversal.cycles_skipped, 0);
    assert!(outcome.tree.is_complete());
}

#[test]
fn benign_exhaustion_produces_a_complete_subtree() {
    let fake = FakeTree::default()
        .with_children(1, &[2, 3])
        .with_children(2, &[4]);

    let outcome = walk(&fake, budget(10));

    assert!(outcome.tree.is_complete());
    assert!(outcome.failures.is_empty());
}

#[test]
fn a_real_descent_failure_produces_an_incomplete_tree_and_a_structured_error() {
    let fake = FakeTree::default()
        .with_children(1, &[2, 3])
        .with_children(2, &[4])
        .faulting_on_first_child(4);

    let outcome = walk(&fake, budget(10));

    assert!(!outcome.tree.is_complete());
    assert!(!outcome.failures.is_empty());
    assert!(outcome.tree.into_accessibility_tree().is_err());
}

#[test]
fn a_real_sibling_failure_retires_the_prefix_and_still_reports_incomplete() {
    let fake = FakeTree::default()
        .with_children(1, &[2, 3, 4])
        .faulting_on_next_sibling(3);

    let outcome = walk(&fake, budget(10));

    assert!(!outcome.tree.is_complete());
    assert!(!outcome.failures.is_empty());
}

/// The enumeration error carries shape and nothing else: no value, name, class
/// name, window title, or provider description may reach a message that
/// `ref_action.rs` clones into session trace segments.
#[test]
fn an_enumeration_error_carries_shape_only() {
    let fake = FakeTree::default()
        .with_children(1, &[2])
        .faulting_on_first_child(2);

    let outcome = walk(&fake, budget(10));
    let failure = outcome.failures.first().expect("a structured failure");
    let details = failure.details.as_ref().expect("structured details");

    assert_eq!(details["kind"], "child_enumeration_failed");
    let keys: Vec<&String> = details
        .as_object()
        .expect("a details object")
        .keys()
        .collect();
    assert_eq!(keys, ["axis", "child_index", "kind", "raw_depth"]);
    assert!(failure.message.contains("UI Automation"));
    assert!(
        failure
            .platform_detail
            .as_ref()
            .expect("a platform detail")
            .contains("0x80004005")
    );
}

/// The child count this boundary also emits is asserted on the logical
/// boundary below instead: core exposes `children_count` only through
/// `into_accessibility_tree()`, which by contract refuses the incomplete tree
/// raw-depth truncation must produce.
#[test]
fn raw_depth_exhaustion_marks_incomplete_and_core_refuses_the_projection() {
    let fake = FakeTree::default().with_chain(&[1, 2, 3, 4]);

    let outcome = walk(&fake, budget(50).with_max_raw_depth(1));

    assert!(!outcome.tree.is_complete());
    assert!(outcome.stats.traversal.limits.depth_hits > 0);
    assert!(outcome.tree.into_accessibility_tree().is_err());
}

/// A depth boundary emits a child count in place of the children it did not
/// descend into, which is what makes the boundary a drill-down target rather
/// than a silent leaf.
#[test]
fn a_depth_boundary_reports_a_child_count_instead_of_children() {
    let fake = FakeTree::default().with_chain(&[1, 2, 3]);

    let outcome = walk(&fake, budget(1));
    let root = outcome
        .tree
        .into_accessibility_tree()
        .expect("core accepts a complete observation");
    let boundary = root.children.first().expect("the root retained its child");

    assert!(boundary.children.is_empty());
    assert!(boundary.children_count.is_some());
}

/// Without the `is_web_wrapper` seam the two counters can never disagree, so
/// this assertion would be testing behaviour 2.2 ships no mechanism to
/// produce. The control below is what makes that claim checkable.
#[test]
fn logical_and_raw_depth_diverge_when_a_node_is_a_web_wrapper() {
    let wrapped = FakeTree::default().with_chain(&[1, 2, 3]).wrapping(2);

    let outcome = walk(&wrapped, budget(10));

    assert!(outcome.stats.traversal.max_logical_depth < outcome.stats.traversal.max_raw_depth);
    assert_eq!(outcome.stats.traversal.web_wrapper_nodes, 1);
}

#[test]
fn logical_and_raw_depth_agree_when_no_node_is_a_web_wrapper() {
    let plain = FakeTree::default().with_chain(&[1, 2, 3]);

    let outcome = walk(&plain, budget(10));

    assert_eq!(
        outcome.stats.traversal.max_logical_depth,
        outcome.stats.traversal.max_raw_depth
    );
    assert_eq!(outcome.stats.traversal.web_wrapper_nodes, 0);
}

/// The removal this asserts is the one a walk most easily ships without: the
/// exit taken when enumeration faults part-way down a subtree.
#[test]
fn the_cycle_guard_is_unwound_on_the_error_exit_path() {
    let fake = FakeTree::default()
        .with_chain(&[1, 2, 3])
        .faulting_on_first_child(3);
    let mut walk = TreeWalk::new(&fake, budget(10));

    let subtree = walk.visit(1, 0, 0);

    assert!(subtree.is_some());
    assert_eq!(walk.ancestor_depth(), 0);
    assert!(walk.finish().is_ok());
}

#[test]
fn the_cycle_guard_is_unwound_when_the_deadline_expires() {
    let fake = FakeTree::default().with_chain(&[1, 2, 3]);
    let expired = WalkBudget::new(10, Deadline::after(1).expect("a one millisecond deadline"));
    std::thread::sleep(std::time::Duration::from_millis(20));
    let mut walk = TreeWalk::new(&fake, expired);

    let subtree = walk.visit(1, 0, 0);

    assert!(subtree.is_none());
    assert_eq!(walk.ancestor_depth(), 0);
}

/// A sibling list that never terminates is invisible to the ancestor guard, so
/// the sibling bound is what makes the enumeration loop total.
#[test]
fn an_endless_sibling_list_terminates_and_reports_incomplete() {
    let fake = FakeTree::default()
        .with_children(1, &[2])
        .looping_siblings(2);

    let outcome = walk(&fake, budget(10).with_max_siblings(4));

    assert!(!outcome.tree.is_complete());
    assert!(outcome.stats.traversal.limits.child_hits > 0);
}

#[test]
fn the_complete_case_projects_and_the_incomplete_case_is_refused() {
    let complete = FakeTree::default().with_children(1, &[2, 3]);
    let incomplete = FakeTree::default()
        .with_children(1, &[2, 3])
        .faulting_on_first_child(2);

    assert!(
        walk(&complete, budget(10))
            .tree
            .into_accessibility_tree()
            .is_ok()
    );
    assert!(
        walk(&incomplete, budget(10))
            .tree
            .into_accessibility_tree()
            .is_err()
    );
}

/// The failure cap keeps one pathological target from flooding a trace
/// segment, but a consumer that sees the cap and no count would read a
/// systemic failure as a local one. The count travels even though the dropped
/// errors do not.
#[test]
fn faults_beyond_the_reporting_cap_are_counted_rather_than_vanishing() {
    let children: Vec<i32> = (2..=40).collect();
    let mut fake = FakeTree::default().with_children(1, &children);
    for child in &children {
        fake = fake.faulting_on_first_child(*child);
    }

    let outcome = walk(&fake, budget(10));

    assert!(!outcome.tree.is_complete());
    let suppressed = outcome
        .failures
        .last()
        .and_then(|error| error.details.as_ref())
        .and_then(|details| details.get("suppressed_failures"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default();
    assert!(
        suppressed > 0,
        "a walk that dropped faults must say how many"
    );
    assert_eq!(
        outcome.stats.reads.health.cannot_complete,
        suppressed + outcome.failures.len() as u64,
        "the counted total must account for every fault, reported or not"
    );
}
