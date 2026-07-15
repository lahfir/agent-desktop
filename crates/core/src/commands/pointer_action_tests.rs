use super::*;
use crate::adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps};
use crate::refs::{RefEntry, RefMap};
use crate::refs_store::RefStore;
use crate::refs_test_support::HomeGuard;
use crate::{AdapterError, ErrorCode, Rect, capability, hit_test::HitTestResult};
use std::sync::atomic::{AtomicU32, Ordering};

#[test]
fn unstable_bounds_are_retryable() {
    let err: AppError = AdapterError::new(ErrorCode::ActionFailed, "bounds changed")
        .with_details(json!({ "check": "stable", "observed_bounds_hash": 42 }))
        .into();

    assert!(is_retryable_point_error(&err));
}

#[test]
fn occlusion_failure_is_retryable() {
    let err: AppError = AdapterError::new(ErrorCode::ActionFailed, "target is occluded")
        .with_details(json!({ "check": "receives_events" }))
        .into();

    assert!(is_retryable_point_error(&err));
}

#[test]
fn hidden_target_failure_is_retryable() {
    let err: AppError = AdapterError::new(ErrorCode::ActionFailed, "target is hidden")
        .with_details(json!({ "check": "visible" }))
        .into();

    assert!(is_retryable_point_error(&err));
}

#[test]
fn app_unresponsive_requires_explicit_retry_evidence() {
    let err: AppError = AdapterError::app_unresponsive("Fixture").into();

    assert!(!is_retryable_point_error(&err));
}

fn point_ref_snapshot() -> String {
    let store = RefStore::new().unwrap();
    let mut refmap = RefMap::new();
    refmap.allocate(RefEntry {
        process: crate::RefProcess {
            pid: crate::ProcessId::new(42),
            process_instance: Some("test-instance".into()),
        },
        identity: crate::RefEntryIdentity {
            role: "button".into(),
            name: Some("Target".into()),
            value: None,
            description: None,
            native_id: None,
        },
        geometry: crate::RefGeometry {
            bounds: None,
            bounds_hash: None,
        },
        capabilities: crate::RefCapabilities {
            states: vec![],
            available_actions: vec![capability::CLICK.into()],
        },
        source: crate::RefSource {
            source_app: None,
            source_window_id: None,
            source_window_title: None,
            source_window_bounds_hash: None,
            source_surface: crate::adapter::SnapshotSurface::Window,
        },
        scope: crate::RefScope {
            root_ref: None,
            path_is_absolute: false,
            path: smallvec::SmallVec::new(),
        },
    });
    store.save_new_snapshot(&refmap).unwrap()
}

fn point_args(snapshot_id: &str) -> crate::commands::point_resolve::PointResolveArgs<'_> {
    crate::commands::point_resolve::PointResolveArgs {
        ref_id: Some("@e1"),
        xy: None,
        snapshot_id: Some(snapshot_id),
        missing_input_message: "target required",
        headed_requirement: crate::HeadedRequirement::None,
    }
}

struct TerminalAfterRetryAdapter {
    bounds_reads: AtomicU32,
}

impl ObservationOps for TerminalAfterRetryAdapter {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn get_element_bounds(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<Option<Rect>, AdapterError> {
        if self.bounds_reads.fetch_add(1, Ordering::SeqCst) == 0 {
            return Ok(Some(Rect {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0,
            }));
        }
        std::thread::sleep(std::time::Duration::from_millis(120));
        Err(AdapterError::stale_ref("terminal target"))
    }
}

impl ActionOps for TerminalAfterRetryAdapter {}
impl InputOps for TerminalAfterRetryAdapter {}
impl SystemOps for TerminalAfterRetryAdapter {}

#[test]
fn terminal_stale_ref_is_preserved_after_retry_budget_expires() {
    let _guard = HomeGuard::new();
    let snapshot_id = point_ref_snapshot();
    let adapter = TerminalAfterRetryAdapter {
        bounds_reads: AtomicU32::new(0),
    };
    let deadline = crate::Deadline::after(100).unwrap();

    let err = match wait_for_point_with_deadline(
        point_args(&snapshot_id),
        deadline,
        &adapter,
        &crate::CommandContext::default(),
    ) {
        Ok(_) => panic!("terminal stale ref must not resolve"),
        Err(err) => err,
    };

    assert_eq!(err.code(), "STALE_REF");
}

struct UnresponsiveThenReadyAdapter {
    resolve_calls: AtomicU32,
}

impl ObservationOps for UnresponsiveThenReadyAdapter {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        if self.resolve_calls.fetch_add(1, Ordering::SeqCst) == 0 {
            return Err(AdapterError::app_unresponsive("Fixture")
                .with_details(json!({ "retryable": true })));
        }
        Ok(NativeHandle::null())
    }

    fn get_element_bounds(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<Option<Rect>, AdapterError> {
        Ok(Some(Rect {
            x: 10.0,
            y: 20.0,
            width: 40.0,
            height: 60.0,
        }))
    }

    fn hit_test(
        &self,
        _handle: &NativeHandle,
        _point: crate::Point,
        _deadline: crate::Deadline,
    ) -> Result<HitTestResult, AdapterError> {
        Ok(HitTestResult::ReachesTarget)
    }
}

impl ActionOps for UnresponsiveThenReadyAdapter {}
impl InputOps for UnresponsiveThenReadyAdapter {}
impl SystemOps for UnresponsiveThenReadyAdapter {}

#[test]
fn explicitly_retryable_app_unresponsive_recovers_within_shared_deadline() {
    let _guard = HomeGuard::new();
    let snapshot_id = point_ref_snapshot();
    let adapter = UnresponsiveThenReadyAdapter {
        resolve_calls: AtomicU32::new(0),
    };
    let deadline = crate::Deadline::after(1_000).unwrap();

    let resolved = wait_for_point_with_deadline(
        point_args(&snapshot_id),
        deadline,
        &adapter,
        &crate::CommandContext::default(),
    )
    .unwrap();

    assert_eq!((resolved.point.x, resolved.point.y), (30.0, 50.0));
    assert_eq!(adapter.resolve_calls.load(Ordering::SeqCst), 3);
}
