use super::*;
use crate::AdapterError;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use std::sync::Mutex;

struct ModifierCaptureAdapter {
    captured: Mutex<Option<MouseEvent>>,
    presented: Mutex<Option<crate::CursorOverlayControl>>,
}

impl ModifierCaptureAdapter {
    fn new() -> Self {
        Self {
            captured: Mutex::new(None),
            presented: Mutex::new(None),
        }
    }
}

impl ObservationOps for ModifierCaptureAdapter {}
impl ActionOps for ModifierCaptureAdapter {}
impl SystemOps for ModifierCaptureAdapter {
    crate::adapter::guarded_interaction_lease!();

    fn update_cursor_overlay(
        &self,
        control: &crate::CursorOverlayControl,
    ) -> Result<(), AdapterError> {
        *self.presented.lock().unwrap() = Some(control.clone());
        Ok(())
    }
}

impl InputOps for ModifierCaptureAdapter {
    fn mouse_event(
        &self,
        event: MouseEvent,
        _lease: &crate::InteractionLease,
    ) -> Result<(), AdapterError> {
        *self.captured.lock().unwrap() = Some(event);
        Ok(())
    }
}

/// F10 regression: `mouse-click` previously hardcoded `modifiers: Vec::new()`
/// in this command, so `--modifiers cmd,shift` was accepted (once parsed)
/// but silently discarded before reaching the adapter. This proves the
/// requested chord survives unchanged into the dispatched `MouseEvent`.
#[test]
fn requested_modifiers_reach_the_dispatched_mouse_event() {
    let adapter = ModifierCaptureAdapter::new();

    execute(
        MouseClickArgs {
            x: 10.0,
            y: 20.0,
            button: MouseButton::Left,
            count: 1,
            modifiers: vec![Modifier::Meta, Modifier::Shift],
        },
        &adapter,
        &CommandContext::default().with_headed(true),
    )
    .unwrap();

    let captured = adapter.captured.lock().unwrap();
    let event = captured
        .as_ref()
        .expect("mouse_event must have been called");
    assert_eq!(event.modifiers, vec![Modifier::Meta, Modifier::Shift]);
}

#[test]
fn no_modifiers_requested_dispatches_empty_modifiers() {
    let adapter = ModifierCaptureAdapter::new();

    execute(
        MouseClickArgs {
            x: 10.0,
            y: 20.0,
            button: MouseButton::Left,
            count: 1,
            modifiers: Vec::new(),
        },
        &adapter,
        &CommandContext::default().with_headed(true),
    )
    .unwrap();

    let captured = adapter.captured.lock().unwrap();
    assert!(captured.as_ref().unwrap().modifiers.is_empty());
}

#[test]
fn headed_mouse_click_suppresses_cursor_overlay() {
    let adapter = ModifierCaptureAdapter::new();
    let config = crate::CursorOverlayConfig::enabled(None, 6).expect("valid config");

    execute(
        MouseClickArgs {
            x: 10.0,
            y: 20.0,
            button: MouseButton::Left,
            count: 1,
            modifiers: Vec::new(),
        },
        &adapter,
        &CommandContext::default()
            .with_headed(true)
            .with_cursor_overlay(config),
    )
    .expect("mouse click succeeds");

    assert!(adapter.presented.lock().unwrap().is_none());
}

#[test]
fn zero_clicks_fail_before_dispatch() {
    let adapter = ModifierCaptureAdapter::new();

    let err = execute(
        MouseClickArgs {
            x: 10.0,
            y: 20.0,
            button: MouseButton::Left,
            count: 0,
            modifiers: Vec::new(),
        },
        &adapter,
        &CommandContext::default().with_headed(true),
    )
    .unwrap_err();

    assert_eq!(err.code(), "INVALID_ARGS");
    assert!(adapter.captured.lock().unwrap().is_none());
}

#[test]
fn excessive_click_count_fails_before_dispatch() {
    let adapter = ModifierCaptureAdapter::new();

    let err = execute(
        MouseClickArgs {
            x: 10.0,
            y: 20.0,
            button: MouseButton::Left,
            count: crate::MAX_MOUSE_CLICK_COUNT + 1,
            modifiers: Vec::new(),
        },
        &adapter,
        &CommandContext::default().with_headed(true),
    )
    .unwrap_err();

    assert_eq!(err.code(), "INVALID_ARGS");
    assert!(adapter.captured.lock().unwrap().is_none());
}
