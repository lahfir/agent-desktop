use super::*;
use crate::action::Modifier;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::error::AdapterError;
use std::sync::Mutex;

#[derive(Debug, PartialEq)]
struct WheelCall {
    x: f64,
    y: f64,
    dy: i32,
    dx: i32,
    modifiers: Vec<Modifier>,
}

struct WheelCaptureAdapter {
    captured: Mutex<Option<WheelCall>>,
    fail: bool,
}

impl WheelCaptureAdapter {
    fn recording() -> Self {
        Self {
            captured: Mutex::new(None),
            fail: false,
        }
    }

    fn failing() -> Self {
        Self {
            captured: Mutex::new(None),
            fail: true,
        }
    }
}

impl ObservationOps for WheelCaptureAdapter {}
impl ActionOps for WheelCaptureAdapter {}
impl SystemOps for WheelCaptureAdapter {}

impl InputOps for WheelCaptureAdapter {
    fn mouse_wheel(
        &self,
        x: f64,
        y: f64,
        dy: i32,
        dx: i32,
        modifiers: &[Modifier],
    ) -> Result<(), AdapterError> {
        *self.captured.lock().unwrap() = Some(WheelCall {
            x,
            y,
            dy,
            dx,
            modifiers: modifiers.to_vec(),
        });
        if self.fail {
            return Err(AdapterError::not_supported("mouse_wheel"));
        }
        Ok(())
    }
}

#[test]
fn requested_wheel_args_reach_the_adapter_unchanged() {
    let adapter = WheelCaptureAdapter::recording();

    execute(
        MouseWheelArgs {
            x: 10.0,
            y: 20.0,
            dy: -3,
            dx: 5,
            modifiers: vec![Modifier::Shift, Modifier::Alt],
        },
        &adapter,
    )
    .unwrap();

    let captured = adapter.captured.lock().unwrap();
    let call = captured
        .as_ref()
        .expect("mouse_wheel must have been called");
    assert_eq!(
        *call,
        WheelCall {
            x: 10.0,
            y: 20.0,
            dy: -3,
            dx: 5,
            modifiers: vec![Modifier::Shift, Modifier::Alt],
        }
    );
}

#[test]
fn returns_scrolled_envelope_with_requested_deltas() {
    let adapter = WheelCaptureAdapter::recording();

    let value = execute(
        MouseWheelArgs {
            x: 0.0,
            y: 0.0,
            dy: 7,
            dx: -2,
            modifiers: Vec::new(),
        },
        &adapter,
    )
    .unwrap();

    assert_eq!(value, json!({ "scrolled": true, "dy": 7, "dx": -2 }));
}

#[test]
fn adapter_error_propagates_as_err() {
    let adapter = WheelCaptureAdapter::failing();

    let result = execute(
        MouseWheelArgs {
            x: 1.0,
            y: 2.0,
            dy: 1,
            dx: 0,
            modifiers: Vec::new(),
        },
        &adapter,
    );

    assert!(result.is_err());
}
