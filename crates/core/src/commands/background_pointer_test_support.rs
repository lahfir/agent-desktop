use super::*;
use crate::adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps, WindowFilter};
use crate::{
    AdapterError, ProcessId, Rect, WindowState, capability,
    refs::{RefEntry, RefMap},
    refs_store::RefStore,
};
use std::sync::Mutex;

pub(super) const PID: u32 = 4242;
pub(super) const WINDOW_ID: &str = "w-9555";

pub(super) fn window_bounds() -> Rect {
    Rect {
        x: -3000.0,
        y: 100.0,
        width: 800.0,
        height: 600.0,
    }
}

pub(super) fn live_window(pid: u32) -> WindowInfo {
    WindowInfo {
        id: WINDOW_ID.into(),
        title: "live title".into(),
        app: "Code".into(),
        pid: ProcessId::new(pid),
        process_instance: Some("test-instance".into()),
        bounds: Some(window_bounds()),
        state: WindowState::default(),
    }
}

pub(super) struct BackgroundCaptureAdapter {
    pub(super) live_pid: u32,
    pub(super) element_bounds: Rect,
    pub(super) report: BackgroundDeliveryReport,
    pub(super) expected_windows: Mutex<Vec<WindowInfo>>,
    pub(super) delivered: Mutex<Vec<(WindowInfo, MouseEvent)>>,
    pub(super) real_mouse_events: Mutex<u32>,
}

impl BackgroundCaptureAdapter {
    pub(super) fn new() -> Self {
        Self {
            live_pid: PID,
            element_bounds: Rect {
                x: -2900.0,
                y: 150.0,
                width: 100.0,
                height: 20.0,
            },
            report: BackgroundDeliveryReport {
                frontmost_pid_before: Some(ProcessId::new(7)),
                frontmost_pid_after: Some(ProcessId::new(7)),
                ..BackgroundDeliveryReport::default()
            },
            expected_windows: Mutex::new(Vec::new()),
            delivered: Mutex::new(Vec::new()),
            real_mouse_events: Mutex::new(0),
        }
    }

    pub(super) fn delivered(&self) -> Vec<(WindowInfo, MouseEvent)> {
        self.delivered.lock().unwrap().clone()
    }
}

impl ObservationOps for BackgroundCaptureAdapter {
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
        Ok(Some(self.element_bounds))
    }

    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(vec![live_window(PID)])
    }
}

impl ActionOps for BackgroundCaptureAdapter {}

impl InputOps for BackgroundCaptureAdapter {
    fn mouse_event(
        &self,
        _event: MouseEvent,
        _lease: &crate::InteractionLease,
    ) -> Result<(), AdapterError> {
        *self.real_mouse_events.lock().unwrap() += 1;
        Ok(())
    }

    fn background_mouse_event(
        &self,
        window: &WindowInfo,
        event: MouseEvent,
        _lease: &crate::InteractionLease,
    ) -> Result<BackgroundDeliveryReport, AdapterError> {
        self.delivered.lock().unwrap().push((window.clone(), event));
        Ok(self.report.clone())
    }
}

impl SystemOps for BackgroundCaptureAdapter {
    crate::adapter::guarded_interaction_lease!();

    fn resolve_window_strict(
        &self,
        window: &WindowInfo,
        _deadline: crate::Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        self.expected_windows.lock().unwrap().push(window.clone());
        Ok(live_window(self.live_pid))
    }
}

pub(super) fn ref_snapshot(source_window_id: Option<&str>) -> String {
    let store = RefStore::new().unwrap();
    let mut refmap = RefMap::new();
    refmap.allocate(RefEntry {
        process: crate::RefProcess {
            pid: ProcessId::new(PID),
            process_instance: Some("test-instance".into()),
        },
        identity: crate::RefEntryIdentity {
            retained_object: None,
            role: "button".into(),
            name: Some("Explorer Section: project".into()),
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
            source_app: Some("Code".into()),
            source_window_id: source_window_id.map(str::to_string),
            source_window_title: Some("stale title".into()),
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

pub(super) fn ref_args(
    action: BackgroundPointerAction,
    snapshot_id: String,
) -> BackgroundPointerArgs {
    BackgroundPointerArgs {
        action,
        target: BackgroundPointerTarget::Ref {
            ref_id: "@e1".into(),
            snapshot_id: Some(snapshot_id),
        },
        timeout_ms: None,
    }
}

pub(super) fn point_args(action: BackgroundPointerAction, x: f64, y: f64) -> BackgroundPointerArgs {
    BackgroundPointerArgs {
        action,
        target: BackgroundPointerTarget::Point {
            x,
            y,
            window_id: WINDOW_ID.into(),
        },
        timeout_ms: None,
    }
}

pub(super) fn left_click(count: u32) -> BackgroundPointerAction {
    BackgroundPointerAction::Click {
        button: MouseButton::Left,
        count,
        modifiers: Vec::new(),
    }
}
