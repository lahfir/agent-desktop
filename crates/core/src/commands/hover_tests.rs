use super::*;
use crate::{
    adapter::NativeHandle,
    capability,
    error::AdapterError,
    node::Rect,
    refs::{RefEntry, RefMap},
    refs_store::RefStore,
    refs_test_support::HomeGuard,
};
use std::sync::Mutex;

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

impl PlatformAdapter for HoverCaptureAdapter {
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

    fn focus_app(&self, pid: i32) -> Result<(), AdapterError> {
        self.focused_pids.lock().unwrap().push(pid);
        Ok(())
    }

    fn mouse_event(&self, event: MouseEvent) -> Result<(), AdapterError> {
        *self.moved_to.lock().unwrap() = Some(event);
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
        },
        &adapter,
        &CommandContext::default().with_headed(true),
    )
    .unwrap();

    assert!(adapter.focused_pids.lock().unwrap().is_empty());
    assert!(value.get("focused").is_none());
}
