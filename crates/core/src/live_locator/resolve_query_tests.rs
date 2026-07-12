use super::{
    LocatorMaterialization, LocatorResolveRequest, LocatorSelection, LocatorStats,
    ObservationRequest, ObservationRoot, ObservedTree, resolve_query,
};
use crate::{
    AdapterError, AppError, ErrorCode, WindowInfo,
    adapter::{ActionOps, InputOps, ObservationOps, SystemOps},
    locator::{LocatorQuery, StatePredicate},
};
use std::sync::atomic::{AtomicUsize, Ordering};

struct CountingAdapter {
    builds: AtomicUsize,
    observe: Option<fn(usize) -> ObservedTree>,
}

impl ObservationOps for CountingAdapter {
    fn observe_tree(
        &self,
        _root: ObservationRoot<'_>,
        _request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        let attempt = self.builds.fetch_add(1, Ordering::SeqCst);
        self.observe
            .map(|observe| observe(attempt))
            .ok_or_else(|| AdapterError::not_supported("fixture locator tree"))
    }
}

impl ActionOps for CountingAdapter {}
impl InputOps for CountingAdapter {}
impl SystemOps for CountingAdapter {}

fn request() -> LocatorResolveRequest {
    request_with_timeout(std::time::Duration::from_secs(5))
}

fn request_with_timeout(timeout: std::time::Duration) -> LocatorResolveRequest {
    LocatorResolveRequest {
        selection: LocatorSelection::Strict,
        deadline: crate::Deadline::from_duration(timeout).unwrap(),
        max_raw_depth: 50,
        materialization: LocatorMaterialization::None,
    }
}

fn observed_tree(complete: bool, stats: LocatorStats) -> ObservedTree {
    let root = super::test_support::node(
        0,
        super::test_support::evidence("window", Some("Fixture")),
        Vec::new(),
        &[],
    );
    let mut tree = super::test_support::tree(vec![root], vec![0], complete);
    tree.stats = stats;
    tree
}

fn churn_then_stable(attempt: usize) -> ObservedTree {
    let mut stats = LocatorStats::default();
    if attempt == 0 {
        stats.traversal.limits.child_count_changes = 1;
        observed_tree(false, stats)
    } else {
        observed_tree(true, stats)
    }
}

fn always_cannot_complete(_: usize) -> ObservedTree {
    let mut stats = LocatorStats::default();
    stats.reads.cannot_complete = 1;
    observed_tree(false, stats)
}

fn node_limit(_: usize) -> ObservedTree {
    let mut stats = LocatorStats::default();
    stats.traversal.limits.node_hits = 1;
    observed_tree(false, stats)
}

fn window() -> WindowInfo {
    WindowInfo {
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
    }
}

#[test]
fn resolve_query_validates_before_calling_the_adapter() {
    let adapter = CountingAdapter {
        builds: AtomicUsize::new(0),
        observe: None,
    };
    let query = LocatorQuery {
        states: vec![StatePredicate {
            token: "imaginary".into(),
            expected: None,
        }],
        ..LocatorQuery::default()
    };
    let error = resolve_query(
        &adapter,
        &query,
        ObservationRoot::Window(&window()),
        &request(),
    )
    .err()
    .unwrap();
    assert_eq!(error.code(), ErrorCode::InvalidArgs.as_str());
    assert_eq!(adapter.builds.load(Ordering::SeqCst), 0);
}

#[test]
fn resolve_query_delegates_valid_queries_once() {
    let adapter = CountingAdapter {
        builds: AtomicUsize::new(0),
        observe: None,
    };
    let error = resolve_query(
        &adapter,
        &LocatorQuery::default(),
        ObservationRoot::Window(&window()),
        &request(),
    )
    .err()
    .unwrap();
    assert_eq!(error.code(), ErrorCode::PlatformNotSupported.as_str());
    assert_eq!(adapter.builds.load(Ordering::SeqCst), 1);
}

#[test]
fn child_count_churn_retries_until_the_observation_is_stable() {
    let adapter = CountingAdapter {
        builds: AtomicUsize::new(0),
        observe: Some(churn_then_stable),
    };

    let resolution = resolve_query(
        &adapter,
        &LocatorQuery::default(),
        ObservationRoot::Window(&window()),
        &request(),
    )
    .unwrap();

    assert!(resolution.meta.complete);
    assert_eq!(adapter.builds.load(Ordering::SeqCst), 2);
    assert_eq!(resolution.stats.reads.observation_attempts, 2);
    assert_eq!(resolution.stats.traversal.limits.child_count_changes, 1);
}

#[test]
fn persistent_cannot_complete_exhausts_one_deadline_with_last_evidence() {
    let adapter = CountingAdapter {
        builds: AtomicUsize::new(0),
        observe: Some(always_cannot_complete),
    };

    let error = resolve_query(
        &adapter,
        &LocatorQuery::default(),
        ObservationRoot::Window(&window()),
        &request_with_timeout(std::time::Duration::from_millis(35)),
    )
    .err()
    .expect("persistent incompleteness must time out");

    let AppError::Adapter(error) = error else {
        panic!("expected adapter error");
    };
    assert_eq!(error.code, ErrorCode::Timeout);
    let details = error.details.unwrap();
    assert_eq!(details["kind"], "locator_transient_incomplete");
    assert!(details["observation_attempts"].as_u64().unwrap() >= 2);
    assert_eq!(details["last_incomplete"]["cannot_complete"], 1);
    assert_eq!(
        details["query_stats"]["reads"]["cannot_complete"],
        details["observation_attempts"]
    );
}

#[test]
fn deterministic_observation_limit_fails_without_retrying() {
    let adapter = CountingAdapter {
        builds: AtomicUsize::new(0),
        observe: Some(node_limit),
    };

    let error = resolve_query(
        &adapter,
        &LocatorQuery::default(),
        ObservationRoot::Window(&window()),
        &request(),
    )
    .err()
    .expect("deterministic limits must reject the resolution");

    let AppError::Adapter(error) = error else {
        panic!("expected adapter error");
    };
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.details.unwrap()["kind"], "locator_budget_limit");
    assert_eq!(adapter.builds.load(Ordering::SeqCst), 1);
}

#[test]
fn strict_unknown_incomplete_result_fails_closed() {
    let adapter = CountingAdapter {
        builds: AtomicUsize::new(0),
        observe: Some(|_| observed_tree(false, LocatorStats::default())),
    };

    let error = match resolve_query(
        &adapter,
        &LocatorQuery::default(),
        ObservationRoot::Window(&window()),
        &request(),
    ) {
        Ok(_) => panic!("strict incomplete result unexpectedly succeeded"),
        Err(error) => error,
    };

    let AppError::Adapter(error) = error else {
        panic!("expected adapter error");
    };
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.details.unwrap()["kind"], "locator_incomplete");
    assert_eq!(adapter.builds.load(Ordering::SeqCst), 1);
}

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
        deadline: crate::Deadline::after(1_000).unwrap(),
        max_raw_depth: 10,
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
    assert_eq!(resolution.stats.reads.observation_attempts, 3);
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
