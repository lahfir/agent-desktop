use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps, WindowFilter};
use crate::locator::live_locator::{ObservationRequest, ObservationRoot, ObservedTree};
use crate::refs::refs_test_support::HomeGuard;
use std::time::Duration;

struct PartialSnapshotAdapter;

impl ObservationOps for PartialSnapshotAdapter {
    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        _request: &ObservationRequest,
    ) -> Result<ObservedTree, crate::AdapterError> {
        crate::wait::deadline::sleep(Duration::from_millis(31));
        let tree = crate::AccessibilityNode {
            ref_id: None,
            role: "window".into(),
            identity: Default::default(),
            presentation: Default::default(),
            children_count: None,
            subtree_truncated: true,
            children: Vec::new(),
        };
        crate::adapter::observed_tree(&root, tree)
    }

    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<crate::WindowInfo>, crate::AdapterError> {
        Ok(vec![crate::WindowInfo {
            id: "w-1".into(),
            title: "Test".into(),
            app: "TestApp".into(),
            pid: crate::ProcessId::new(42),
            process_instance: Some("test-instance".into()),
            bounds: None,
            state: Default::default(),
        }])
    }
}

impl ActionOps for PartialSnapshotAdapter {}
impl InputOps for PartialSnapshotAdapter {}
impl SystemOps for PartialSnapshotAdapter {}

#[test]
fn expired_snapshot_deadline_persists_partial_tree_and_releases_the_store_lock() {
    let _home = HomeGuard::new();
    let _clock = crate::wait::deadline::VirtualClock::freeze();
    let inherited = crate::Deadline::detached_after(30).unwrap();
    let deadline_scope = crate::wait::deadline::enter_scope(Some(inherited));
    let context = crate::CommandContext::default();
    let adapter = PartialSnapshotAdapter;

    let result = run_with_context(
        &adapter,
        &TreeOptions::default(),
        &SnapshotTarget {
            app_name: Some("TestApp"),
            window_id: None,
        },
        &context,
        30,
    )
    .expect("a partial snapshot should persist after its observation deadline");

    assert!(!result.complete);
    let snapshot_id = result.snapshot_id.expect("snapshot id");
    drop(deadline_scope);
    let store = RefStore::new().unwrap();
    assert_eq!(store.latest_snapshot_id().unwrap(), Some(snapshot_id));

    let next = run_with_context(
        &adapter,
        &TreeOptions::default(),
        &SnapshotTarget {
            app_name: Some("TestApp"),
            window_id: None,
        },
        &context,
        1_000,
    )
    .expect("a later snapshot should acquire the released ref-store lock");
    assert!(next.snapshot_id.is_some());
}
