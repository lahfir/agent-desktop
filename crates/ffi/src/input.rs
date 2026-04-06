use crate::convert::{c_to_str, free_c_string, string_to_c};
use crate::error::{self, AdResult};
use crate::types::{AdDragParams, AdMouseButton, AdMouseEvent, AdMouseEventKind};
use crate::AdAdapter;
use agent_desktop_core::action::{
    DragParams as CoreDragParams, MouseButton as CoreMouseButton, MouseEvent as CoreMouseEvent,
    MouseEventKind as CoreMouseEventKind, Point as CorePoint,
};
use std::os::raw::c_char;

/// # Safety
///
/// `adapter` must be a non-null pointer returned by `ad_adapter_create`.
/// `out` must be a non-null pointer to a `*mut c_char` to receive the allocated string.
/// Free the result with `ad_free_string`.
#[no_mangle]
pub unsafe extern "C" fn ad_get_clipboard(
    adapter: *const AdAdapter,
    out: *mut *mut c_char,
) -> AdResult {
    let adapter = &*adapter;
    match adapter.inner.get_clipboard() {
        Ok(text) => {
            *out = string_to_c(&text);
            error::clear_last_error();
            AdResult::Ok
        }
        Err(e) => {
            error::set_last_error(&e);
            AdResult::ErrActionFailed
        }
    }
}

/// # Safety
///
/// `adapter` must be a non-null pointer returned by `ad_adapter_create`.
/// `text` must be a non-null, valid UTF-8 C string.
#[no_mangle]
pub unsafe extern "C" fn ad_set_clipboard(
    adapter: *const AdAdapter,
    text: *const c_char,
) -> AdResult {
    let adapter = &*adapter;
    let text = match c_to_str(text) {
        Some(s) => s,
        None => {
            error::set_last_error(&agent_desktop_core::error::AdapterError::new(
                agent_desktop_core::error::ErrorCode::InvalidArgs,
                "text is null or invalid UTF-8",
            ));
            return AdResult::ErrInvalidArgs;
        }
    };
    match adapter.inner.set_clipboard(text) {
        Ok(()) => {
            error::clear_last_error();
            AdResult::Ok
        }
        Err(e) => {
            error::set_last_error(&e);
            AdResult::ErrActionFailed
        }
    }
}

/// # Safety
///
/// `adapter` must be a non-null pointer returned by `ad_adapter_create`.
#[no_mangle]
pub unsafe extern "C" fn ad_clear_clipboard(adapter: *const AdAdapter) -> AdResult {
    let adapter = &*adapter;
    match adapter.inner.clear_clipboard() {
        Ok(()) => {
            error::clear_last_error();
            AdResult::Ok
        }
        Err(e) => {
            error::set_last_error(&e);
            AdResult::ErrActionFailed
        }
    }
}

/// # Safety
///
/// `s` must be a pointer previously returned by `ad_get_clipboard`, or null.
/// After this call the pointer is invalid and must not be used.
#[no_mangle]
pub unsafe extern "C" fn ad_free_string(s: *mut c_char) {
    free_c_string(s);
}

fn mouse_button_from_c(b: AdMouseButton) -> CoreMouseButton {
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
    let adapter = &*adapter;
    let ev = &*event;
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
    match adapter.inner.mouse_event(core_event) {
        Ok(()) => {
            error::clear_last_error();
            AdResult::Ok
        }
        Err(e) => {
            error::set_last_error(&e);
            AdResult::ErrActionFailed
        }
    }
}

/// # Safety
///
/// `adapter` must be a non-null pointer returned by `ad_adapter_create`.
/// `params` must be a non-null pointer to a valid `AdDragParams`.
#[no_mangle]
pub unsafe extern "C" fn ad_drag(
    adapter: *const AdAdapter,
    params: *const AdDragParams,
) -> AdResult {
    let adapter = &*adapter;
    let p = &*params;
    let core_params = CoreDragParams {
        from: CorePoint {
            x: p.from.x,
            y: p.from.y,
        },
        to: CorePoint {
            x: p.to.x,
            y: p.to.y,
        },
        duration_ms: if p.duration_ms == 0 {
            None
        } else {
            Some(p.duration_ms)
        },
    };
    match adapter.inner.drag(core_params) {
        Ok(()) => {
            error::clear_last_error();
            AdResult::Ok
        }
        Err(e) => {
            error::set_last_error(&e);
            AdResult::ErrActionFailed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AdMouseButton, AdMouseEvent, AdMouseEventKind, AdPoint};

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

    #[test]
    fn test_drag_zero_duration_becomes_none() {
        let p = AdDragParams {
            from: AdPoint { x: 0.0, y: 0.0 },
            to: AdPoint { x: 100.0, y: 200.0 },
            duration_ms: 0,
        };
        let core = CoreDragParams {
            from: CorePoint {
                x: p.from.x,
                y: p.from.y,
            },
            to: CorePoint {
                x: p.to.x,
                y: p.to.y,
            },
            duration_ms: if p.duration_ms == 0 {
                None
            } else {
                Some(p.duration_ms)
            },
        };
        assert!(core.duration_ms.is_none());
        assert_eq!(core.to.x, 100.0);
    }

    #[test]
    fn test_drag_nonzero_duration() {
        let p = AdDragParams {
            from: AdPoint { x: 0.0, y: 0.0 },
            to: AdPoint { x: 50.0, y: 50.0 },
            duration_ms: 500,
        };
        let core = CoreDragParams {
            from: CorePoint {
                x: p.from.x,
                y: p.from.y,
            },
            to: CorePoint {
                x: p.to.x,
                y: p.to.y,
            },
            duration_ms: if p.duration_ms == 0 {
                None
            } else {
                Some(p.duration_ms)
            },
        };
        assert_eq!(core.duration_ms, Some(500));
    }
}
