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

/// Dispatches a mouse event (move / down / up / click) at the given
/// screen point. Click count is only consulted when `event.kind` is
/// `CLICK` (e.g., `click_count == 2` for a double-click).
///
/// # Safety
/// `adapter` must be a non-null pointer returned by `ad_adapter_create`.
/// `event` must be a non-null pointer to a valid `AdMouseEvent`.
#[no_mangle]
pub unsafe extern "C" fn ad_mouse_event(
    adapter: *const AdAdapter,
    event: *const AdMouseEvent,
) -> AdResult {
    trap_panic(|| unsafe {
        if let Err(rc) = crate::main_thread::require_main_thread() {
            return rc;
        }
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        crate::pointer_guard::guard_non_null!(event, c"event is null");
        let adapter = &*adapter;
        let ev = &*event;
        let validated_button = match AdMouseButton::from_c(ev.button) {
            Some(b) => b,
            None => {
                error::set_last_error(&agent_desktop_core::error::AdapterError::new(
                    agent_desktop_core::error::ErrorCode::InvalidArgs,
                    "invalid mouse button discriminant",
                ));
                return AdResult::ErrInvalidArgs;
            }
        };
        let validated_kind = match AdMouseEventKind::from_c(ev.kind) {
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
    fn valid_discriminants_convert_to_typed_enums() {
        let ev = AdMouseEvent {
            kind: AdMouseEventKind::Click as i32,
            point: AdPoint { x: 10.0, y: 20.0 },
            button: AdMouseButton::Left as i32,
            click_count: 2,
        };
        assert!(matches!(
            AdMouseButton::from_c(ev.button),
            Some(AdMouseButton::Left)
        ));
        assert!(matches!(
            AdMouseEventKind::from_c(ev.kind),
            Some(AdMouseEventKind::Click)
        ));
    }

    #[test]
    fn invalid_discriminants_reject_without_ub() {
        let ev = AdMouseEvent {
            kind: 999,
            point: AdPoint { x: 0.0, y: 0.0 },
            button: -5,
            click_count: 0,
        };
        assert!(AdMouseButton::from_c(ev.button).is_none());
        assert!(AdMouseEventKind::from_c(ev.kind).is_none());
    }
}
