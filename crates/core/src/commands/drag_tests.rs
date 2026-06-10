use super::*;
use crate::{
    action::DragParams,
    adapter::{NativeHandle, PlatformAdapter},
    error::AdapterError,
    node::Rect,
    refs::{RefEntry, RefMap},
    refs_store::RefStore,
    refs_test_support::HomeGuard,
};
use std::sync::Mutex;

struct DragCaptureAdapter {
    captured: Mutex<Option<DragParams>>,
}

impl DragCaptureAdapter {
    fn new() -> Self {
        Self {
            captured: Mutex::new(None),
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

    let value = execute(xy_args(Some(750)), &adapter, &CommandContext::default()).unwrap();

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

    let value = execute(xy_args(None), &adapter, &CommandContext::default()).unwrap();

    assert!(value.get("drop_delay_ms").is_none());
    let captured = adapter.captured.lock().unwrap().clone().unwrap();
    assert_eq!(captured.drop_delay_ms, None);
}

#[test]
fn drag_resolves_ref_bounds_to_center_point() {
    let _guard = HomeGuard::new();
    let store = RefStore::new().unwrap();
    let mut refmap = RefMap::new();
    refmap.allocate(RefEntry {
        pid: 1,
        role: "button".into(),
        name: Some("Item".into()),
        value: None,
        description: None,
        states: vec![],
        bounds: None,
        bounds_hash: None,
        available_actions: vec!["Click".into()],
        source_app: None,
        source_window_id: None,
        source_window_title: None,
        source_surface: crate::adapter::SnapshotSurface::Window,
        root_ref: None,
        path_is_absolute: false,
        path: smallvec::SmallVec::new(),
    });
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
    execute(args, &adapter, &CommandContext::default()).unwrap();

    let captured = adapter.captured.lock().unwrap().clone().unwrap();
    assert_eq!((captured.from.x, captured.from.y), (30.0, 50.0));
}
