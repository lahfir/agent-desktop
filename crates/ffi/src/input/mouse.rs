use crate::enum_validation::enum_raw_i32;
use crate::error::{self, AdResult};
use crate::ffi_try::trap_panic;
use crate::types::{AdMouseButton, AdMouseEvent, AdMouseEventKind};
use crate::AdAdapter;
use agent_desktop_core::action::{
    MouseButton as CoreMouseButton, MouseEvent as CoreMouseEvent,
    MouseEventKind as CoreMouseEventKind, Point as CorePoint,
};

pub(crate) fn mouse_button_from_c(b: AdMouseButton) -> CoreMouseButton {
    match b {
        AdMouseButton::Left => CoreMouseButton::Left,
        AdMouseButton::Right => CoreMouseButton::Right,
        AdMouseButton::Middle => CoreMouseButton::Middle,
    }
}

/// # Safety
///
/// `adapter` must be a non-null pointer returned by `ad_adapter_create`.
/// `event` must be a non-null pointer to a valid `AdMouseEvent`.
#[no_mangle]
pub unsafe extern "C" fn ad_mouse_event(
    adapter: *const AdAdapter,
    event: *const AdMouseEvent,
) -> AdResult {
    trap_panic(|| unsafe {
        let adapter = &*adapter;
        let ev = &*event;
        let validated_button = match AdMouseButton::from_c(enum_raw_i32(&ev.button)) {
            Some(b) => b,
            None => {
                error::set_last_error(&agent_desktop_core::error::AdapterError::new(
                    agent_desktop_core::error::ErrorCode::InvalidArgs,
                    "invalid mouse button discriminant",
                ));
                return AdResult::ErrInvalidArgs;
            }
        };
        let validated_kind = match AdMouseEventKind::from_c(enum_raw_i32(&ev.kind)) {
            Some(k) => k,
            None => {
                error::set_last_error(&agent_desktop_core::error::AdapterError::new(
                    agent_desktop_core::error::ErrorCode::InvalidArgs,
                    "invalid mouse event kind discriminant",
                ));
                return AdResult::ErrInvalidArgs;
            }
        };
        let point = CorePoint {
            x: ev.point.x,
            y: ev.point.y,
        };
        let button = mouse_button_from_c(validated_button);
        let kind = match validated_kind {
            AdMouseEventKind::Move => CoreMouseEventKind::Move,
            AdMouseEventKind::Down => CoreMouseEventKind::Down,
            AdMouseEventKind::Up => CoreMouseEventKind::Up,
            AdMouseEventKind::Click => CoreMouseEventKind::Click {
                count: ev.click_count,
            },
        };
        let core_event = CoreMouseEvent {
            kind,
            point,
            button,
        };
        match adapter.inner.mouse_event(core_event) {
            Ok(()) => AdResult::Ok,
            Err(e) => {
                error::set_last_error(&e);
                error::last_error_code()
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AdPoint;

    #[test]
    fn test_mouse_button_mapping() {
        assert!(matches!(
            mouse_button_from_c(AdMouseButton::Left),
            CoreMouseButton::Left
        ));
        assert!(matches!(
            mouse_button_from_c(AdMouseButton::Right),
            CoreMouseButton::Right
        ));
        assert!(matches!(
            mouse_button_from_c(AdMouseButton::Middle),
            CoreMouseButton::Middle
        ));
    }

    #[test]
    fn test_mouse_event_kind_click_count() {
        let ev = AdMouseEvent {
            kind: AdMouseEventKind::Click,
            point: AdPoint { x: 10.0, y: 20.0 },
            button: AdMouseButton::Left,
            click_count: 2,
        };
        let point = CorePoint {
            x: ev.point.x,
            y: ev.point.y,
        };
        let button = mouse_button_from_c(ev.button);
        let kind = match ev.kind {
            AdMouseEventKind::Move => CoreMouseEventKind::Move,
            AdMouseEventKind::Down => CoreMouseEventKind::Down,
            AdMouseEventKind::Up => CoreMouseEventKind::Up,
            AdMouseEventKind::Click => CoreMouseEventKind::Click {
                count: ev.click_count,
            },
        };
        let core_event = CoreMouseEvent {
            kind,
            point,
            button,
        };
        assert!(matches!(
            core_event.kind,
            CoreMouseEventKind::Click { count: 2 }
        ));
        assert_eq!(core_event.point.x, 10.0);
        assert_eq!(core_event.point.y, 20.0);
    }
}
