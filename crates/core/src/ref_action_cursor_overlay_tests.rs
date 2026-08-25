use super::*;
use crate::adapter::{
    ActionOps, InputOps, NativeHandle, ObservationOps, SnapshotSurface, SystemOps,
};
use crate::{Action, ActionResult, AdapterError, CursorOverlayControl, capability};
use std::sync::Mutex;

struct CursorAdapter {
    presented: Mutex<Vec<CursorOverlayControl>>,
    fail_presentation: bool,
}

impl CursorAdapter {
    fn new(fail_presentation: bool) -> Self {
        Self {
            presented: Mutex::new(Vec::new()),
            fail_presentation,
        }
    }
}

impl ObservationOps for CursorAdapter {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    crate::adapter::complete_live_observation!("button", "Run", [capability::CLICK]);
}

impl ActionOps for CursorAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
        _lease: &crate::InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        Ok(ActionResult::delivered_unverified("click"))
    }
}

impl InputOps for CursorAdapter {}

impl SystemOps for CursorAdapter {
    crate::adapter::guarded_interaction_lease!();
    crate::adapter::exact_window_focus!();

    fn update_cursor_overlay(&self, control: &CursorOverlayControl) -> Result<(), AdapterError> {
        if self.fail_presentation {
            return Err(AdapterError::internal("renderer unavailable"));
        }
        self.presented.lock().unwrap().push(control.clone());
        Ok(())
    }
}

fn entry() -> RefEntry {
    let bounds = crate::Rect {
        x: 1.0,
        y: 1.0,
        width: 20.0,
        height: 20.0,
    };
    RefEntry {
        process: crate::RefProcess {
            pid: crate::ProcessId::new(1),
            process_instance: Some("test-instance".into()),
        },
        identity: crate::RefEntryIdentity {
            role: "button".into(),
            name: Some("Run".into()),
            value: None,
            description: None,
            native_id: None,
        },
        geometry: crate::RefGeometry {
            bounds: Some(bounds),
            bounds_hash: bounds.bounds_hash(),
        },
        capabilities: crate::RefCapabilities {
            states: vec![],
            available_actions: vec![capability::CLICK.into()],
        },
        source: crate::RefSource {
            source_app: Some("Fixture".into()),
            source_window_id: Some("w-42".into()),
            source_window_title: Some("Fixture".into()),
            source_window_bounds_hash: None,
            source_surface: SnapshotSurface::Window,
        },
        scope: crate::RefScope {
            root_ref: None,
            path_is_absolute: false,
            path: smallvec::SmallVec::new(),
        },
    }
}

fn enabled_context() -> CommandContext {
    let config =
        crate::CursorOverlayConfig::enabled(Some("Opening menu".into()), 6).expect("valid config");
    CommandContext::default().with_cursor_overlay_session("test-session", config)
}

#[test]
fn enabled_cursor_moves_before_dispatch_then_clicks_after_it() {
    let adapter = CursorAdapter::new(false);

    execute_entry_with_context(
        &adapter,
        &entry(),
        ActionRequest::headless(Action::Click),
        &enabled_context(),
    )
    .expect("click succeeds");

    let presented = adapter.presented.lock().unwrap();
    let center = crate::Point { x: 11.0, y: 11.0 };
    assert_eq!(presented.len(), 2);
    let travel = presented[0].instruction().expect("travel instruction");
    let click = presented[1].instruction().expect("click instruction");

    assert_eq!(travel.destination(), &center);
    assert!(
        !travel.is_click(),
        "the cursor sets off before the action runs"
    );
    assert!(travel.target().is_none());
    assert_eq!(click.destination(), &center);
    assert!(
        click.is_click(),
        "the click effect lands after dispatch confirms"
    );
    assert_eq!(click.target().map(|rect| rect.width), Some(20.0));
    assert_eq!(presented[0].label(), Some("Opening menu"));
}

#[test]
fn disabled_and_headed_contexts_do_not_present() {
    let adapter = CursorAdapter::new(false);

    execute_entry(&adapter, &entry(), ActionRequest::headless(Action::Click))
        .expect("click succeeds");
    let headed = enabled_context().with_headed(true);
    execute_entry_with_context(
        &adapter,
        &entry(),
        ActionRequest::headed(Action::Click),
        &headed,
    )
    .expect("click succeeds");

    assert!(adapter.presented.lock().unwrap().is_empty());
}

#[test]
fn renderer_failure_does_not_change_successful_action() {
    let adapter = CursorAdapter::new(true);

    let result = execute_entry_with_context(
        &adapter,
        &entry(),
        ActionRequest::headless(Action::Click),
        &enabled_context(),
    )
    .expect("presentation failure stays fail-soft");

    assert_eq!(result.action, "click");
}
