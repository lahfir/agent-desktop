use super::test_support::window;
use super::{
    LocatorMaterialization, LocatorResolveRequest, LocatorSelection, ObservationRequest,
    ObservationRoot, ObservedTree, resolve_query,
};
use crate::{
    AdapterError, ErrorCode,
    adapter::{ActionOps, InputOps, ObservationOps, SystemOps},
    locator::LocatorQuery,
};
use std::sync::atomic::{AtomicUsize, Ordering};

struct HydrationRetryAdapter {
    window_observations: AtomicUsize,
    hydration_observations: AtomicUsize,
}

impl ObservationOps for HydrationRetryAdapter {
    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        _request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        match root {
            ObservationRoot::Window(_) => {
                self.window_observations.fetch_add(1, Ordering::SeqCst);
                Ok(button_tree())
            }
            ObservationRoot::Element { .. } => {
                let attempt = self.hydration_observations.fetch_add(1, Ordering::SeqCst);
                if attempt == 0 {
                    return Err(AdapterError::stale_ref("hydration retry")
                        .with_details(serde_json::json!({ "retryable": true })));
                }
                Ok(button_tree())
            }
        }
    }

    fn resolve_locator_anchor(
        &self,
        _entry: &crate::RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<crate::NativeHandle, AdapterError> {
        Ok(crate::NativeHandle::null())
    }
}

impl ActionOps for HydrationRetryAdapter {}
impl InputOps for HydrationRetryAdapter {}
impl SystemOps for HydrationRetryAdapter {}

fn button_tree() -> ObservedTree {
    super::test_support::tree(
        vec![super::test_support::node(
            0,
            super::test_support::evidence("button", Some("Save")),
            Vec::new(),
            &[],
        )],
        vec![0],
        true,
    )
}

#[test]
fn hydration_retry_preserves_failed_attempt_statistics() {
    let adapter = HydrationRetryAdapter {
        window_observations: AtomicUsize::new(0),
        hydration_observations: AtomicUsize::new(0),
    };
    let request = LocatorResolveRequest {
        selection: LocatorSelection::Strict,
        deadline: crate::Deadline::after(5_000).unwrap(),
        max_raw_depth: 10,
        surface: None,
        materialization: LocatorMaterialization::SelectedMatches,
    };

    let resolution = resolve_query(
        &adapter,
        &LocatorQuery::default(),
        ObservationRoot::Window(&window()),
        &request,
    )
    .unwrap();

    assert_eq!(adapter.window_observations.load(Ordering::SeqCst), 2);
    assert_eq!(adapter.hydration_observations.load(Ordering::SeqCst), 2);
    assert_eq!(resolution.stats.reads.counts.observation_attempts, 3);
}

struct IncompleteHydrationAdapter {
    hydration_observations: AtomicUsize,
}

impl ObservationOps for IncompleteHydrationAdapter {
    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        _request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        if matches!(root, ObservationRoot::Window(_)) {
            return Ok(button_tree());
        }
        self.hydration_observations.fetch_add(1, Ordering::SeqCst);
        let mut evidence = super::test_support::evidence("button", Some("Save"));
        evidence.ref_evidence.bounds = super::LocatorField::Unknown;
        Ok(super::test_support::tree(
            vec![super::test_support::node(0, evidence, Vec::new(), &[])],
            vec![0],
            true,
        ))
    }

    fn resolve_locator_anchor(
        &self,
        _entry: &crate::RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<crate::NativeHandle, AdapterError> {
        Ok(crate::NativeHandle::null())
    }
}

impl ActionOps for IncompleteHydrationAdapter {}
impl InputOps for IncompleteHydrationAdapter {}
impl SystemOps for IncompleteHydrationAdapter {}

#[test]
fn selected_hydration_rejects_incomplete_snapshot_evidence() {
    let adapter = IncompleteHydrationAdapter {
        hydration_observations: AtomicUsize::new(0),
    };
    let request = LocatorResolveRequest {
        selection: LocatorSelection::Strict,
        deadline: crate::Deadline::after(35).unwrap(),
        max_raw_depth: 10,
        surface: None,
        materialization: LocatorMaterialization::SelectedMatches,
    };

    let error = match resolve_query(
        &adapter,
        &LocatorQuery::default(),
        ObservationRoot::Window(&window()),
        &request,
    ) {
        Ok(_) => panic!("incomplete hydration unexpectedly succeeded"),
        Err(error) => error,
    };

    assert_eq!(error.code(), ErrorCode::Timeout.as_str());
    assert!(adapter.hydration_observations.load(Ordering::SeqCst) >= 1);
}
