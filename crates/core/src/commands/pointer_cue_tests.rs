//! Producer-side contract for physical-pointer cues: a ref-resolved pointer
//! target binds its cue to the exact live window, and coordinate-only input
//! still presents an unbound cue instead of being dropped.

use crate::adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps};
use crate::commands::drag::{self, DragArgs, DragEndpoint, WaitForScope};
use crate::commands::hover::{self, HoverArgs};
use crate::{
    AdapterError, CommandContext, CursorOverlayControl, CursorPhase, DragParams, MouseEvent,
    ProcessId, Rect, capability,
    hit_test::HitTestResult,
    refs::{RefEntry, RefMap},
    refs_store::RefStore,
    refs_test_support::HomeGuard,
};
use std::sync::Mutex;

const SESSION: &str = "test-session";

/// Reports each ref's snapshot source window as its live presentation window.
#[derive(Default)]
struct CueAdapter {
    presented: Mutex<Vec<CursorOverlayControl>>,
}

fn live_entry(handle: &NativeHandle) -> Result<&RefEntry, AdapterError> {
    handle
        .downcast_ref::<RefEntry>()
        .ok_or_else(|| AdapterError::internal("test handle carries no ref entry"))
}

impl ObservationOps for CueAdapter {
    fn resolve_element_strict(
        &self,
        entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::new(entry.clone()))
    }

    fn get_element_bounds(
        &self,
        handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<Option<Rect>, AdapterError> {
        let offset = if live_entry(handle)?.identity.name.as_deref() == Some("Source") {
            0.0
        } else {
            200.0
        };
        Ok(Some(Rect {
            x: 10.0 + offset,
            y: 20.0,
            width: 40.0,
            height: 60.0,
        }))
    }

    fn get_presentation_window_id(
        &self,
        handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<Option<String>, AdapterError> {
        Ok(live_entry(handle)?.source.source_window_id.clone())
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

impl ActionOps for CueAdapter {}

impl InputOps for CueAdapter {
    fn mouse_event(
        &self,
        _event: MouseEvent,
        _lease: &crate::InteractionLease,
    ) -> Result<(), AdapterError> {
        Ok(())
    }

    fn drag(
        &self,
        _params: DragParams,
        _lease: &crate::InteractionLease,
    ) -> Result<(), AdapterError> {
        Ok(())
    }
}

impl SystemOps for CueAdapter {
    crate::adapter::guarded_interaction_lease!();

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
        _lease: &crate::InteractionLease,
    ) -> Result<(), AdapterError> {
        Ok(())
    }

    fn update_cursor_overlay(&self, control: &CursorOverlayControl) -> Result<(), AdapterError> {
        self.presented.lock().unwrap().push(control.clone());
        Ok(())
    }
}

impl CueAdapter {
    fn cues(&self) -> Vec<(CursorPhase, Option<(ProcessId, String)>)> {
        self.presented
            .lock()
            .unwrap()
            .iter()
            .map(|control| {
                let instruction = control.instruction().expect("present control");
                (instruction.phase(), instruction.window().cloned())
            })
            .collect()
    }
}

fn entry(pid: u32, name: &str, window: &str) -> RefEntry {
    RefEntry {
        process: crate::RefProcess {
            pid: ProcessId::new(pid),
            process_instance: Some("test-instance".into()),
        },
        identity: crate::RefEntryIdentity {
            retained_object: None,
            role: "button".into(),
            name: Some(name.into()),
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
            source_app: Some(format!("App {pid}")),
            source_window_id: Some(window.into()),
            source_window_title: Some(format!("Window {window}")),
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

fn snapshot(entries: [RefEntry; 2]) -> String {
    let mut refmap = RefMap::new();
    for entry in entries {
        refmap.allocate(entry);
    }
    RefStore::for_session(Some(SESSION))
        .unwrap()
        .save_new_snapshot(&refmap)
        .unwrap()
}

fn headed_overlay() -> CommandContext {
    let config = crate::CursorOverlayConfig::enabled(None, 6).expect("valid config");
    CommandContext::default()
        .with_headed(true)
        .with_cursor_overlay_session(SESSION, config)
}

fn window(pid: u32, id: &str) -> Option<(ProcessId, String)> {
    Some((ProcessId::new(pid), id.into()))
}

fn hover_args(ref_id: Option<&str>, snapshot_id: Option<String>) -> HoverArgs {
    HoverArgs {
        ref_id: ref_id.map(str::to_owned),
        snapshot_id,
        xy: ref_id.is_none().then_some((5.0, 6.0)),
        duration_ms: None,
        timeout_ms: None,
    }
}

fn ref_drag(snapshot_id: String) -> DragArgs {
    DragArgs {
        from: DragEndpoint {
            ref_id: Some("@e1".into()),
            xy: None,
        },
        to: DragEndpoint {
            ref_id: Some("@e2".into()),
            xy: None,
        },
        snapshot_id: Some(snapshot_id),
        duration_ms: None,
        drop_delay_ms: None,
        timeout_ms: None,
        wait_for_scope: WaitForScope::default(),
    }
}

#[test]
fn ref_hover_binds_its_cues_to_the_live_target_window() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot([entry(7, "Source", "w-70"), entry(7, "Drop", "w-70")]);
    let adapter = CueAdapter::default();

    hover::execute(
        hover_args(Some("@e1"), Some(snapshot_id)),
        &adapter,
        &headed_overlay(),
    )
    .expect("hover succeeds");

    assert_eq!(
        adapter.cues(),
        [
            (CursorPhase::Travel, window(7, "w-70")),
            (CursorPhase::Effect, window(7, "w-70")),
        ]
    );
}

#[test]
fn coordinate_hover_still_presents_an_unbound_cue() {
    let adapter = CueAdapter::default();

    hover::execute(hover_args(None, None), &adapter, &headed_overlay()).expect("hover succeeds");

    assert_eq!(
        adapter.cues(),
        [(CursorPhase::Travel, None), (CursorPhase::Effect, None)]
    );
}

#[test]
fn ref_drag_within_one_window_binds_every_phase_to_it() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot([entry(7, "Source", "w-70"), entry(7, "Drop", "w-70")]);
    let adapter = CueAdapter::default();

    drag::execute(ref_drag(snapshot_id), &adapter, &headed_overlay()).expect("drag succeeds");

    assert_eq!(
        adapter.cues(),
        [
            (CursorPhase::Travel, window(7, "w-70")),
            (CursorPhase::Drag, window(7, "w-70")),
            (CursorPhase::Effect, window(7, "w-70")),
        ]
    );
}

#[test]
fn cross_window_ref_drag_binds_only_the_pickup_travel() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot([entry(7, "Source", "w-70"), entry(8, "Drop", "w-80")]);
    let adapter = CueAdapter::default();

    drag::execute(ref_drag(snapshot_id), &adapter, &headed_overlay()).expect("drag succeeds");

    assert_eq!(
        adapter.cues(),
        [
            (CursorPhase::Travel, window(7, "w-70")),
            (CursorPhase::Drag, None),
            (CursorPhase::Effect, None),
        ]
    );
}
