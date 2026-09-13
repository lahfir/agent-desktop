use super::*;
use crate::{
    CommandContext, MouseButton, MouseEvent, MouseEventKind, Point,
    adapter::{ActionOps, InputOps, ObservationOps, SystemOps},
};
use std::sync::Mutex;

struct RoutingCaptureAdapter {
    presented: Mutex<Vec<CursorOverlayControl>>,
}

impl RoutingCaptureAdapter {
    fn new() -> Self {
        Self {
            presented: Mutex::new(Vec::new()),
        }
    }
}

impl ObservationOps for RoutingCaptureAdapter {}
impl ActionOps for RoutingCaptureAdapter {}

impl InputOps for RoutingCaptureAdapter {
    fn mouse_event(
        &self,
        _event: MouseEvent,
        _lease: &crate::InteractionLease,
    ) -> Result<(), crate::AdapterError> {
        Ok(())
    }
}

impl SystemOps for RoutingCaptureAdapter {
    crate::adapter::guarded_interaction_lease!();

    fn update_cursor_overlay(
        &self,
        control: &CursorOverlayControl,
    ) -> Result<(), crate::AdapterError> {
        self.presented.lock().unwrap().push(control.clone());
        Ok(())
    }
}

fn context(multi_agent: bool, agent: &str) -> CommandContext {
    let config = CursorOverlayConfig::enabled(None, 6)
        .expect("valid config")
        .with_multi_agent(multi_agent);
    CommandContext::default()
        .with_agent_id(Some(agent.into()))
        .expect("valid agent id")
        .with_cursor_overlay_session("test-session", config)
}

fn click_event() -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Move,
        point: Point { x: 10.0, y: 20.0 },
        button: MouseButton::Left,
        modifiers: Vec::new(),
    }
}

fn lease() -> crate::InteractionLease {
    let deadline = crate::Deadline::detached_after(5_000).expect("valid deadline");
    crate::InteractionLease::guarded(deadline, ()).expect("valid lease")
}

/// Ordinary (non-multi-agent) sessions must route `--agent-id` desktop actions
/// to the default renderer so the existing default socket handles the
/// presentation instead of spawning a second per-agent renderer.
#[test]
fn ordinary_session_routes_agent_actions_to_the_default_socket() {
    let adapter = RoutingCaptureAdapter::new();
    dispatch_mouse_event_with_cursor(
        &adapter,
        &context(false, "agent-a"),
        click_event(),
        true,
        &lease(),
    )
    .expect("dispatch succeeds");
    assert!(
        adapter
            .presented
            .lock()
            .unwrap()
            .iter()
            .all(|control| control.agent_id().is_none()),
        "ordinary sessions must not stamp agent ids on present controls"
    );
}

/// Multi-agent sessions must route `--agent-id` desktop actions to the
/// per-agent renderer, preserving the per-cursor contract.
#[test]
fn multi_agent_session_routes_agent_actions_to_the_per_agent_socket() {
    let adapter = RoutingCaptureAdapter::new();
    dispatch_mouse_event_with_cursor(
        &adapter,
        &context(true, "agent-a"),
        click_event(),
        true,
        &lease(),
    )
    .expect("dispatch succeeds");
    assert!(
        adapter
            .presented
            .lock()
            .unwrap()
            .iter()
            .all(|control| control.agent_id() == Some("agent-a")),
        "multi-agent sessions must stamp the agent id on present controls"
    );
}

/// `cancel_drag` in an ordinary session must hide the default renderer; the
/// unconditional stamp previously dropped the `Hide` when the per-agent
/// socket did not exist, leaving the default cursor visible.
#[test]
fn ordinary_session_cancel_drag_routes_to_the_default_socket() {
    let adapter = RoutingCaptureAdapter::new();
    crate::cursor_overlay::cancel_drag(&adapter, &context(false, "agent-a"));
    let presented = adapter.presented.lock().unwrap();
    let cancel = presented.last().expect("cancel control");
    assert!(cancel.is_hide());
    assert_eq!(cancel.agent_id(), None);
}

/// `cancel_drag` in a multi-agent session must hide the per-agent renderer,
/// preserving the per-agent visibility scope.
#[test]
fn multi_agent_session_cancel_drag_routes_to_the_per_agent_socket() {
    let adapter = RoutingCaptureAdapter::new();
    crate::cursor_overlay::cancel_drag(&adapter, &context(true, "agent-a"));
    let presented = adapter.presented.lock().unwrap();
    let cancel = presented.last().expect("cancel control");
    assert!(cancel.is_hide());
    assert_eq!(cancel.agent_id(), Some("agent-a"));
}
