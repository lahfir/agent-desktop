use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{
    adapter::NativeHandle,
    capability,
    error::{AdapterError, ErrorCode},
    hit_test::HitTestResult,
    node::Rect,
    refs::{RefEntry, RefMap},
    refs_store::RefStore,
    refs_test_support::HomeGuard,
};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

struct HoverCaptureAdapter {
    moved_to: Mutex<Option<MouseEvent>>,
    focused_pids: Mutex<Vec<i32>>,
}

impl HoverCaptureAdapter {
    fn new() -> Self {
        Self {
            moved_to: Mutex::new(None),
            focused_pids: Mutex::new(Vec::new()),
        }
    }
}

impl ObservationOps for HoverCaptureAdapter {
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
}

impl ActionOps for HoverCaptureAdapter {}

impl InputOps for HoverCaptureAdapter {
    fn mouse_event(&self, event: MouseEvent) -> Result<(), AdapterError> {
        *self.moved_to.lock().unwrap() = Some(event);
        Ok(())
    }
}

impl SystemOps for HoverCaptureAdapter {
    fn focus_app(&self, pid: i32) -> Result<(), AdapterError> {
        self.focused_pids.lock().unwrap().push(pid);
        Ok(())
    }
}

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

fn ref_args(snapshot_id: String) -> HoverArgs {
    HoverArgs {
        ref_id: Some("@e1".into()),
        snapshot_id: Some(snapshot_id),
        xy: None,
        duration_ms: None,
        timeout_ms: None,
    }
}

#[test]
fn headless_ref_hover_is_policy_denied_before_cursor_move() {
    let _guard = HomeGuard::new();
    let snapshot_id = ref_snapshot(42);
    let adapter = HoverCaptureAdapter::new();

    let err = execute(ref_args(snapshot_id), &adapter, &CommandContext::default()).unwrap_err();

    assert_eq!(err.code(), "POLICY_DENIED");
    assert!(adapter.focused_pids.lock().unwrap().is_empty());
    assert!(adapter.moved_to.lock().unwrap().is_none());
}

#[test]
fn headed_ref_hover_focuses_target_app_once() {
    let _guard = HomeGuard::new();
    let snapshot_id = ref_snapshot(42);
    let adapter = HoverCaptureAdapter::new();

    let value = execute(
        ref_args(snapshot_id),
        &adapter,
        &CommandContext::default().with_headed(true),
    )
    .unwrap();

    assert_eq!(*adapter.focused_pids.lock().unwrap(), vec![42]);
    assert_eq!(value["focused"], true);
    assert_eq!(value["x"], 110.0);
    assert_eq!(value["y"], 205.0);
}

#[test]
fn headed_xy_hover_never_steals_focus() {
    let adapter = HoverCaptureAdapter::new();

    let value = execute(
        HoverArgs {
            ref_id: None,
            snapshot_id: None,
            xy: Some((5.0, 6.0)),
            duration_ms: None,
            timeout_ms: None,
        },
        &adapter,
        &CommandContext::default().with_headed(true),
    )
    .unwrap();

    assert!(adapter.focused_pids.lock().unwrap().is_empty());
    assert!(value.get("focused").is_none());
}

struct StaleThenOkAdapter {
    resolve_calls: AtomicU32,
    fail_until: u32,
}

impl StaleThenOkAdapter {
    fn new(fail_until: u32) -> Self {
        Self {
            resolve_calls: AtomicU32::new(0),
            fail_until,
        }
    }
}

impl ObservationOps for StaleThenOkAdapter {
    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        let n = self.resolve_calls.fetch_add(1, Ordering::SeqCst) + 1;
        if n <= self.fail_until {
            return Err(AdapterError::new(ErrorCode::StaleRef, "not yet resolvable"));
        }
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
}

impl ActionOps for StaleThenOkAdapter {}

impl InputOps for StaleThenOkAdapter {
    fn mouse_event(&self, _event: MouseEvent) -> Result<(), AdapterError> {
        Ok(())
    }
}

impl SystemOps for StaleThenOkAdapter {}

/// Regression for the F2 fix: `hover --ref` previously had no `--timeout-ms`
/// at all, so a transient `STALE_REF` on the resolved ref failed the command
/// outright. This proves the wired budget retries through the real
/// `hover::execute` path.
#[test]
fn transient_stale_ref_retries_then_succeeds_when_timeout_wired() {
    let _guard = HomeGuard::new();
    let snapshot_id = ref_snapshot(42);
    let adapter = StaleThenOkAdapter::new(2);

    let value = execute(
        HoverArgs {
            ref_id: Some("@e1".into()),
            snapshot_id: Some(snapshot_id),
            xy: None,
            duration_ms: None,
            timeout_ms: Some(5_000),
        },
        &adapter,
        &CommandContext::default().with_headed(true),
    )
    .unwrap();

    assert_eq!(value["hovered"], true);
    assert!(adapter.resolve_calls.load(Ordering::SeqCst) >= 3);
}

struct OccludedTargetAdapter {
    moved_to: Mutex<Option<MouseEvent>>,
}

impl ObservationOps for OccludedTargetAdapter {
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
        _point: crate::action::Point,
    ) -> Result<HitTestResult, AdapterError> {
        Ok(HitTestResult::InterceptedBy {
            role: Some("AXSheet".into()),
            name: Some("Save changes?".into()),
            bounds: None,
        })
    }
}

impl ActionOps for OccludedTargetAdapter {}

impl InputOps for OccludedTargetAdapter {
    fn mouse_event(&self, event: MouseEvent) -> Result<(), AdapterError> {
        *self.moved_to.lock().unwrap() = Some(event);
        Ok(())
    }
}

impl SystemOps for OccludedTargetAdapter {}

/// F27 regression: `hover --ref` previously resolved the ref's bounds to a
/// point and dispatched the mouse move without ever consulting `hit_test`,
/// so an occluded target (e.g. a modal sheet over it) was hovered blind.
/// This proves the preflight now fails before any mouse event is sent.
#[test]
fn hover_on_occluded_ref_fails_preflight_before_dispatch() {
    let _guard = HomeGuard::new();
    let snapshot_id = ref_snapshot(42);
    let adapter = OccludedTargetAdapter {
        moved_to: Mutex::new(None),
    };

    let err = execute(
        ref_args(snapshot_id),
        &adapter,
        &CommandContext::default().with_headed(true),
    )
    .unwrap_err();

    assert_eq!(err.code(), "ACTION_FAILED");
    assert!(err.to_string().contains("AXSheet"));
    assert!(adapter.moved_to.lock().unwrap().is_none());
}

#[test]
fn timeout_none_makes_exactly_one_resolve_attempt() {
    let _guard = HomeGuard::new();
    let snapshot_id = ref_snapshot(42);
    let adapter = StaleThenOkAdapter::new(1);

    let err = execute(
        HoverArgs {
            ref_id: Some("@e1".into()),
            snapshot_id: Some(snapshot_id),
            xy: None,
            duration_ms: None,
            timeout_ms: None,
        },
        &adapter,
        &CommandContext::default().with_headed(true),
    )
    .unwrap_err();

    assert_eq!(err.code(), "STALE_REF");
    assert_eq!(adapter.resolve_calls.load(Ordering::SeqCst), 1);
}
