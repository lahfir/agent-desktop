use super::*;
use crate::{
    action::DragParams,
    adapter::{NativeHandle, PlatformAdapter},
    capability,
    error::AdapterError,
    node::Rect,
    refs::{RefEntry, RefMap},
    refs_store::RefStore,
    refs_test_support::HomeGuard,
};
use std::sync::Mutex;

struct DragCaptureAdapter {
    captured: Mutex<Option<DragParams>>,
    focused_pids: Mutex<Vec<i32>>,
}

impl DragCaptureAdapter {
    fn new() -> Self {
        Self {
            captured: Mutex::new(None),
            focused_pids: Mutex::new(Vec::new()),
        }
    }
}

impl PlatformAdapter for DragCaptureAdapter {
    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn get_element_bounds(&self, _handle: &NativeHandle) -> Result<Option<Rect>, AdapterError> {
        Ok(Some(Rect {
            x: 10.0,
            y: 20.0,
            width: 40.0,
            height: 60.0,
        }))
    }

    fn focus_app(&self, pid: i32) -> Result<(), AdapterError> {
        self.focused_pids.lock().unwrap().push(pid);
        Ok(())
    }

    fn drag(&self, params: DragParams) -> Result<(), AdapterError> {
        *self.captured.lock().unwrap() = Some(params);
        Ok(())
    }
}

fn xy_args(drop_delay_ms: Option<u64>) -> DragArgs {
    DragArgs {
        from_ref: None,
        from_xy: Some((1.0, 2.0)),
        to_ref: None,
        to_xy: Some((3.0, 4.0)),
        snapshot_id: None,
        duration_ms: None,
        drop_delay_ms,
    }
}

#[test]
fn drop_delay_is_threaded_into_drag_params_and_response() {
    let adapter = DragCaptureAdapter::new();

    let value = execute(
        xy_args(Some(750)),
        &adapter,
        &CommandContext::default().with_headed(true),
    )
    .unwrap();

    assert_eq!(value["dragged"], true);
    assert_eq!(value["drop_delay_ms"], 750);
    let captured = adapter.captured.lock().unwrap().clone().unwrap();
    assert_eq!(captured.drop_delay_ms, Some(750));
    assert_eq!((captured.from.x, captured.from.y), (1.0, 2.0));
    assert_eq!((captured.to.x, captured.to.y), (3.0, 4.0));
}

#[test]
fn drop_delay_omitted_uses_adapter_default_and_no_response_field() {
    let adapter = DragCaptureAdapter::new();

    let value = execute(
        xy_args(None),
        &adapter,
        &CommandContext::default().with_headed(true),
    )
    .unwrap();

    assert!(value.get("drop_delay_ms").is_none());
    let captured = adapter.captured.lock().unwrap().clone().unwrap();
    assert_eq!(captured.drop_delay_ms, None);
}

fn ref_entry(pid: i32) -> RefEntry {
    RefEntry {
        pid,
        role: "button".into(),
        name: Some("Item".into()),
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
    }
}

fn cross_app_snapshot() -> String {
    let store = RefStore::new().unwrap();
    let mut refmap = RefMap::new();
    refmap.allocate(ref_entry(1));
    refmap.allocate(ref_entry(2));
    store.save_new_snapshot(&refmap).unwrap()
}

fn cross_app_args(snapshot_id: String) -> DragArgs {
    DragArgs {
        from_ref: Some("@e1".into()),
        from_xy: None,
        to_ref: Some("@e2".into()),
        to_xy: None,
        snapshot_id: Some(snapshot_id),
        duration_ms: None,
        drop_delay_ms: None,
    }
}

#[test]
fn drag_resolves_ref_bounds_to_center_point() {
    let _guard = HomeGuard::new();
    let store = RefStore::new().unwrap();
    let mut refmap = RefMap::new();
    refmap.allocate(ref_entry(1));
    let snapshot_id = store.save_new_snapshot(&refmap).unwrap();
    let adapter = DragCaptureAdapter::new();

    let args = DragArgs {
        from_ref: Some("@e1".into()),
        from_xy: None,
        to_ref: None,
        to_xy: Some((100.0, 200.0)),
        snapshot_id: Some(snapshot_id),
        duration_ms: None,
        drop_delay_ms: Some(300),
    };
    execute(args, &adapter, &CommandContext::default().with_headed(true)).unwrap();

    let captured = adapter.captured.lock().unwrap().clone().unwrap();
    assert_eq!((captured.from.x, captured.from.y), (30.0, 50.0));
}

#[test]
fn headless_ref_drag_is_policy_denied_before_cursor_move() {
    let _guard = HomeGuard::new();
    let snapshot_id = cross_app_snapshot();
    let adapter = DragCaptureAdapter::new();

    let err = execute(
        cross_app_args(snapshot_id),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "POLICY_DENIED");
    assert!(adapter.focused_pids.lock().unwrap().is_empty());
    assert!(adapter.captured.lock().unwrap().is_none());
}

#[test]
fn headed_ref_drag_focuses_only_the_from_app_once() {
    let _guard = HomeGuard::new();
    let snapshot_id = cross_app_snapshot();
    let adapter = DragCaptureAdapter::new();

    let value = execute(
        cross_app_args(snapshot_id),
        &adapter,
        &CommandContext::default().with_headed(true),
    )
    .unwrap();

    assert_eq!(*adapter.focused_pids.lock().unwrap(), vec![1]);
    assert_eq!(value["focused"], true);
}

#[test]
fn headed_xy_drag_never_steals_focus() {
    let adapter = DragCaptureAdapter::new();

    let value = execute(
        xy_args(None),
        &adapter,
        &CommandContext::default().with_headed(true),
    )
    .unwrap();

    assert!(adapter.focused_pids.lock().unwrap().is_empty());
    assert!(value.get("focused").is_none());
}
