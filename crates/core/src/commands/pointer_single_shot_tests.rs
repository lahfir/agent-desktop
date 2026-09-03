use crate::adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps};
use crate::commands::{drag, hover};
use crate::hit_test::HitTestResult;
use crate::refs::{RefEntry, RefMap};
use crate::refs_store::RefStore;
use crate::refs_test_support::HomeGuard;
use crate::{
    AdapterError, CommandContext, DragParams, InteractionLease, MouseEvent, Point, ProcessId, Rect,
};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

struct SingleShotScrollAdapter {
    scrolled: AtomicBool,
    scrolls: AtomicU32,
    resolves: AtomicU32,
    leases: AtomicU32,
    mouse_moves: AtomicU32,
    drags: AtomicU32,
    mouse_point: Mutex<Option<Point>>,
    drag_params: Mutex<Option<DragParams>>,
}

impl SingleShotScrollAdapter {
    fn new() -> Self {
        Self {
            scrolled: AtomicBool::new(false),
            scrolls: AtomicU32::new(0),
            resolves: AtomicU32::new(0),
            leases: AtomicU32::new(0),
            mouse_moves: AtomicU32::new(0),
            drags: AtomicU32::new(0),
            mouse_point: Mutex::new(None),
            drag_params: Mutex::new(None),
        }
    }
}

impl ObservationOps for SingleShotScrollAdapter {
    fn resolve_element_strict(
        &self,
        entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        self.resolves.fetch_add(1, Ordering::SeqCst);
        Ok(NativeHandle::new(entry.process.pid.get()))
    }

    fn get_element_bounds(
        &self,
        handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<Option<Rect>, AdapterError> {
        let pid = *handle.downcast_ref::<u32>().unwrap();
        if pid == 1 && !self.scrolled.load(Ordering::SeqCst) {
            return Ok(None);
        }
        Ok(Some(Rect {
            x: f64::from(pid) * 100.0,
            y: f64::from(pid) * 200.0,
            width: 20.0,
            height: 10.0,
        }))
    }

    fn hit_test(
        &self,
        _handle: &NativeHandle,
        _point: Point,
        _deadline: crate::Deadline,
    ) -> Result<HitTestResult, AdapterError> {
        Ok(HitTestResult::ReachesTarget)
    }
}

impl ActionOps for SingleShotScrollAdapter {
    fn scroll_into_view(
        &self,
        handle: &NativeHandle,
        _lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        assert_eq!(*handle.downcast_ref::<u32>().unwrap(), 1);
        self.scrolls.fetch_add(1, Ordering::SeqCst);
        self.scrolled.store(true, Ordering::SeqCst);
        Ok(())
    }
}

impl InputOps for SingleShotScrollAdapter {
    fn mouse_event(
        &self,
        event: MouseEvent,
        _lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        self.mouse_moves.fetch_add(1, Ordering::SeqCst);
        *self.mouse_point.lock().unwrap() = Some(event.point);
        Ok(())
    }

    fn drag(&self, params: DragParams, _lease: &InteractionLease) -> Result<(), AdapterError> {
        self.drags.fetch_add(1, Ordering::SeqCst);
        *self.drag_params.lock().unwrap() = Some(params);
        Ok(())
    }
}

impl SystemOps for SingleShotScrollAdapter {
    fn acquire_interaction_lease(
        &self,
        deadline: crate::Deadline,
    ) -> Result<InteractionLease, AdapterError> {
        self.leases.fetch_add(1, Ordering::SeqCst);
        InteractionLease::guarded(deadline, ())
    }

    fn resolve_window_strict(
        &self,
        window: &crate::WindowInfo,
        _deadline: crate::Deadline,
    ) -> Result<crate::WindowInfo, AdapterError> {
        Ok(window.clone())
    }

    fn focus_window(
        &self,
        _window: &crate::WindowInfo,
        _lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        Ok(())
    }
}

fn ref_entry(pid: u32) -> RefEntry {
    RefEntry {
        process: crate::RefProcess {
            pid: ProcessId::new(pid),
            process_instance: Some("test-instance".into()),
        },
        identity: crate::RefEntryIdentity {
            role: "button".into(),
            name: Some(format!("Target {pid}")),
            value: None,
            description: None,
            native_id: None,
        },
        geometry: crate::RefGeometry {
            bounds: None,
            bounds_hash: None,
        },
        capabilities: crate::RefCapabilities {
            states: Vec::new(),
            available_actions: vec![crate::capability::CLICK.into()],
        },
        source: crate::RefSource {
            source_app: Some(format!("App {pid}")),
            source_window_id: Some(format!("w-{pid}")),
            source_window_title: Some(format!("Window {pid}")),
            source_window_bounds_hash: None,
            source_surface: crate::adapter::SnapshotSurface::Window,
        },
        scope: crate::RefScope {
            root_ref: None,
            path_is_absolute: false,
            path: smallvec::SmallVec::new(),
        },
    }
}

fn snapshot(refs: u32) -> String {
    let store = RefStore::new().unwrap();
    let mut refmap = RefMap::new();
    for pid in 1..=refs {
        refmap.allocate(ref_entry(pid));
    }
    store.save_new_snapshot(&refmap).unwrap()
}

#[test]
fn hover_none_scrolls_once_then_dispatches_stable_point_once() {
    let _guard = HomeGuard::new();
    let adapter = SingleShotScrollAdapter::new();
    let value = hover::execute(
        hover::HoverArgs {
            ref_id: Some("@e1".into()),
            snapshot_id: Some(snapshot(1)),
            xy: None,
            duration_ms: None,
            timeout_ms: None,
        },
        &adapter,
        &CommandContext::default().with_headed(true),
    )
    .unwrap();

    assert_eq!(adapter.scrolls.load(Ordering::SeqCst), 1);
    assert_eq!(adapter.leases.load(Ordering::SeqCst), 1);
    assert_eq!(adapter.resolves.load(Ordering::SeqCst), 3);
    assert_eq!(adapter.mouse_moves.load(Ordering::SeqCst), 1);
    assert_eq!(value["x"], 110.0);
    assert_eq!(value["y"], 205.0);
}

#[test]
fn drag_none_scrolls_once_then_dispatches_revalidated_endpoints_once() {
    let _guard = HomeGuard::new();
    let adapter = SingleShotScrollAdapter::new();
    let value = drag::execute(
        drag::DragArgs {
            from: drag::DragEndpoint {
                ref_id: Some("@e1".into()),
                xy: None,
            },
            to: drag::DragEndpoint {
                ref_id: Some("@e2".into()),
                xy: None,
            },
            snapshot_id: Some(snapshot(2)),
            duration_ms: None,
            drop_delay_ms: None,
            timeout_ms: None,
            wait_for_scope: drag::WaitForScope::default(),
        },
        &adapter,
        &CommandContext::default().with_headed(true),
    )
    .unwrap();

    assert_eq!(adapter.scrolls.load(Ordering::SeqCst), 1);
    assert_eq!(adapter.leases.load(Ordering::SeqCst), 1);
    assert_eq!(adapter.resolves.load(Ordering::SeqCst), 5);
    assert_eq!(adapter.drags.load(Ordering::SeqCst), 1);
    assert_eq!(value["from"], serde_json::json!({ "x": 110.0, "y": 205.0 }));
    assert_eq!(value["to"], serde_json::json!({ "x": 210.0, "y": 405.0 }));
}
