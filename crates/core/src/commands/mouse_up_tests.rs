use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::error::AdapterError;
use std::sync::Mutex;

struct ModifierCaptureAdapter {
    captured: Mutex<Option<MouseEvent>>,
}

impl ModifierCaptureAdapter {
    fn new() -> Self {
        Self {
            captured: Mutex::new(None),
        }
    }
}

impl ObservationOps for ModifierCaptureAdapter {}
impl ActionOps for ModifierCaptureAdapter {}
impl SystemOps for ModifierCaptureAdapter {}

impl InputOps for ModifierCaptureAdapter {
    fn mouse_event(&self, event: MouseEvent) -> Result<(), AdapterError> {
        *self.captured.lock().unwrap() = Some(event);
        Ok(())
    }
}

/// F10 regression: `mouse-up` previously hardcoded `modifiers: Vec::new()`
/// in this command, silently discarding a requested chord. This proves the
/// requested chord survives unchanged into the dispatched `MouseEvent`.
#[test]
fn requested_modifiers_reach_the_dispatched_mouse_event() {
    let adapter = ModifierCaptureAdapter::new();

    execute(
        MouseUpArgs {
            x: 10.0,
            y: 20.0,
            button: MouseButton::Left,
            modifiers: vec![Modifier::Alt, Modifier::Shift],
        },
        &adapter,
        &CommandContext::default().with_headed(true),
    )
    .unwrap();

    let captured = adapter.captured.lock().unwrap();
    let event = captured
        .as_ref()
        .expect("mouse_event must have been called");
    assert_eq!(event.modifiers, vec![Modifier::Alt, Modifier::Shift]);
}

#[test]
fn no_modifiers_requested_dispatches_empty_modifiers() {
    let adapter = ModifierCaptureAdapter::new();

    execute(
        MouseUpArgs {
            x: 10.0,
            y: 20.0,
            button: MouseButton::Left,
            modifiers: Vec::new(),
        },
        &adapter,
        &CommandContext::default().with_headed(true),
    )
    .unwrap();

    let captured = adapter.captured.lock().unwrap();
    assert!(captured.as_ref().unwrap().modifiers.is_empty());
}
