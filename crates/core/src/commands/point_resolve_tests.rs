use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{
    adapter::NativeHandle,
    capability,
    error::ErrorCode,
    hit_test::HitTestResult,
    node::Rect,
    refs::{RefEntry, RefMap},
    refs_store::RefStore,
    refs_test_support::HomeGuard,
};

#[test]
fn physical_input_requires_headed_context() {
    let err = require_cursor_policy(&CommandContext::default(), "mouse-move").unwrap_err();

    assert_eq!(err.code(), "POLICY_DENIED");
}

#[test]
fn headed_context_allows_physical_input() {
    require_cursor_policy(&CommandContext::default().with_headed(true), "mouse-move").unwrap();
}

/// An adapter whose `hit_test` outcome is fixed per test, so each occlusion
/// scenario (F27) exercises the real `resolve_point_from_ref_or_xy_with_context`
/// path rather than a mock that always echoes success.
struct HitTestOutcomeAdapter {
    outcome: Result<HitTestResult, AdapterError>,
}

impl ObservationOps for HitTestOutcomeAdapter {
    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn get_element_bounds(&self, _handle: &NativeHandle) -> Result<Option<Rect>, AdapterError> {
        Ok(Some(Rect {
            x: 100.0,
            y: 200.0,
            width: 20.0,
            height: 10.0,
        }))
    }

    fn hit_test(
        &self,
        _handle: &NativeHandle,
        _point: Point,
    ) -> Result<HitTestResult, AdapterError> {
        self.outcome.clone()
    }
}

impl ActionOps for HitTestOutcomeAdapter {}
impl InputOps for HitTestOutcomeAdapter {}
impl SystemOps for HitTestOutcomeAdapter {}

fn ref_snapshot(pid: i32) -> String {
    let store = RefStore::new().unwrap();
    let mut refmap = RefMap::new();
    refmap.allocate(RefEntry {
        pid,
        role: "button".into(),
        name: Some("Target".into()),
        value: None,
        description: None,
        native_id: None,
        states: vec![],
        bounds: None,
        bounds_hash: None,
        available_actions: vec![capability::CLICK.into()],
        source_app: None,
        source_window_id: None,
        source_window_title: None,
        source_surface: crate::adapter::SnapshotSurface::Window,
        root_ref: None,
        path_is_absolute: false,
        path: smallvec::SmallVec::new(),
    });
    store.save_new_snapshot(&refmap).unwrap()
}

fn ref_args(snapshot_id: &str) -> PointResolveArgs<'_> {
    PointResolveArgs {
        ref_id: Some("@e1"),
        xy: None,
        snapshot_id: Some(snapshot_id),
        missing_input_message: "Provide a ref (@e1) or --xy x,y",
    }
}

/// F27 regression: previously the ref-targeted path never called
/// `adapter.hit_test` at all, so an occluded target resolved to a point and
/// dispatch proceeded blind. This proves `InterceptedBy` now blocks
/// resolution and names the occluder.
#[test]
fn intercepted_by_blocks_ref_targeted_point_resolution() {
    let _guard = HomeGuard::new();
    let snapshot_id = ref_snapshot(42);
    let adapter = HitTestOutcomeAdapter {
        outcome: Ok(HitTestResult::InterceptedBy {
            role: Some("AXSheet".into()),
            name: Some("Save changes?".into()),
            bounds: None,
        }),
    };

    let result = resolve_point_from_ref_or_xy_with_context(
        ref_args(&snapshot_id),
        &adapter,
        &CommandContext::default(),
    );
    let err = match result {
        Ok(_) => panic!("occluded ref target must not resolve to a point"),
        Err(err) => err,
    };

    assert_eq!(err.code(), ErrorCode::ActionFailed.as_str());
    let message = err.to_string();
    assert!(message.contains("AXSheet"));
}

#[test]
fn reaches_target_allows_ref_targeted_point_resolution() {
    let _guard = HomeGuard::new();
    let snapshot_id = ref_snapshot(42);
    let adapter = HitTestOutcomeAdapter {
        outcome: Ok(HitTestResult::ReachesTarget),
    };

    let resolved = resolve_point_from_ref_or_xy_with_context(
        ref_args(&snapshot_id),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(resolved.point.x, 110.0);
    assert_eq!(resolved.point.y, 205.0);
}

#[test]
fn unknown_hit_test_result_does_not_block_ref_targeted_resolution() {
    let _guard = HomeGuard::new();
    let snapshot_id = ref_snapshot(42);
    let adapter = HitTestOutcomeAdapter {
        outcome: Ok(HitTestResult::Unknown),
    };

    resolve_point_from_ref_or_xy_with_context(
        ref_args(&snapshot_id),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();
}

/// A hit-test probe error must never be treated as occlusion (the reliability
/// learning's evidence rule) — resolution proceeds exactly as if hit-testing
/// were unavailable.
#[test]
fn hit_test_probe_error_does_not_block_ref_targeted_resolution() {
    let _guard = HomeGuard::new();
    let snapshot_id = ref_snapshot(42);
    let adapter = HitTestOutcomeAdapter {
        outcome: Err(AdapterError::internal(
            "AXUIElementCopyElementAtPosition failed",
        )),
    };

    resolve_point_from_ref_or_xy_with_context(
        ref_args(&snapshot_id),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();
}

/// Raw `--xy` input stays raw by design (KTD4): no ref means no occlusion
/// check, even against an adapter that would otherwise report occlusion.
#[test]
fn raw_xy_input_never_calls_hit_test() {
    let adapter = HitTestOutcomeAdapter {
        outcome: Ok(HitTestResult::InterceptedBy {
            role: Some("AXSheet".into()),
            name: None,
            bounds: None,
        }),
    };

    let resolved = resolve_point_from_ref_or_xy_with_context(
        PointResolveArgs {
            ref_id: None,
            xy: Some((5.0, 6.0)),
            snapshot_id: None,
            missing_input_message: "Provide a ref (@e1) or --xy x,y",
        },
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!((resolved.point.x, resolved.point.y), (5.0, 6.0));
}
