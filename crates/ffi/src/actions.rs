use crate::convert::{c_to_str, free_c_string, opt_string_to_c, string_to_c};
use crate::error::{self, AdResult};
use crate::types::{
    AdAction, AdActionKind, AdActionResult, AdDirection, AdElementState, AdKeyCombo, AdModifier,
    AdNativeHandle, AdRefEntry,
};
use crate::AdAdapter;
use agent_desktop_core::action::{
    Action, ActionResult as CoreActionResult, Direction, DragParams as CoreDragParams,
    KeyCombo as CoreKeyCombo, Modifier, Point as CorePoint,
};
use agent_desktop_core::adapter::NativeHandle;
use agent_desktop_core::refs::RefEntry as CoreRefEntry;
use std::ptr;

pub(crate) fn direction_from_c(d: AdDirection) -> Direction {
    match d {
        AdDirection::Up => Direction::Up,
        AdDirection::Down => Direction::Down,
        AdDirection::Left => Direction::Left,
        AdDirection::Right => Direction::Right,
    }
}

pub(crate) unsafe fn key_combo_from_c(k: &AdKeyCombo) -> Result<CoreKeyCombo, &'static str> {
    let key = c_to_str(k.key)
        .ok_or("key is null or invalid UTF-8")?
        .to_owned();
    let mut modifiers = Vec::new();
    if !k.modifiers.is_null() && k.modifier_count > 0 {
        let slice = std::slice::from_raw_parts(k.modifiers, k.modifier_count as usize);
        for m in slice {
            let modifier = match m {
                AdModifier::Cmd => Modifier::Cmd,
                AdModifier::Ctrl => Modifier::Ctrl,
                AdModifier::Alt => Modifier::Alt,
                AdModifier::Shift => Modifier::Shift,
            };
            modifiers.push(modifier);
        }
    }
    Ok(CoreKeyCombo { key, modifiers })
}

pub(crate) unsafe fn action_from_c(action: &AdAction) -> Result<Action, &'static str> {
    match action.kind {
        AdActionKind::Click => Ok(Action::Click),
        AdActionKind::DoubleClick => Ok(Action::DoubleClick),
        AdActionKind::RightClick => Ok(Action::RightClick),
        AdActionKind::TripleClick => Ok(Action::TripleClick),
        AdActionKind::SetFocus => Ok(Action::SetFocus),
        AdActionKind::Expand => Ok(Action::Expand),
        AdActionKind::Collapse => Ok(Action::Collapse),
        AdActionKind::Toggle => Ok(Action::Toggle),
        AdActionKind::Check => Ok(Action::Check),
        AdActionKind::Uncheck => Ok(Action::Uncheck),
        AdActionKind::ScrollTo => Ok(Action::ScrollTo),
        AdActionKind::Clear => Ok(Action::Clear),
        AdActionKind::Hover => Ok(Action::Hover),
        AdActionKind::SetValue => {
            let text = c_to_str(action.text).ok_or("text is null or invalid UTF-8")?;
            Ok(Action::SetValue(text.to_owned()))
        }
        AdActionKind::Select => {
            let text = c_to_str(action.text).ok_or("text is null or invalid UTF-8")?;
            Ok(Action::Select(text.to_owned()))
        }
        AdActionKind::TypeText => {
            let text = c_to_str(action.text).ok_or("text is null or invalid UTF-8")?;
            Ok(Action::TypeText(text.to_owned()))
        }
        AdActionKind::Scroll => {
            let dir = direction_from_c(action.scroll.direction);
            Ok(Action::Scroll(dir, action.scroll.amount))
        }
        AdActionKind::PressKey => {
            let combo = key_combo_from_c(&action.key)?;
            Ok(Action::PressKey(combo))
        }
        AdActionKind::KeyDown => {
            let combo = key_combo_from_c(&action.key)?;
            Ok(Action::KeyDown(combo))
        }
        AdActionKind::KeyUp => {
            let combo = key_combo_from_c(&action.key)?;
            Ok(Action::KeyUp(combo))
        }
        AdActionKind::Drag => {
            let params = CoreDragParams {
                from: CorePoint {
                    x: action.drag.from.x,
                    y: action.drag.from.y,
                },
                to: CorePoint {
                    x: action.drag.to.x,
                    y: action.drag.to.y,
                },
                duration_ms: if action.drag.duration_ms == 0 {
                    None
                } else {
                    Some(action.drag.duration_ms)
                },
            };
            Ok(Action::Drag(params))
        }
    }
}

pub(crate) fn action_result_to_c(r: &CoreActionResult) -> AdActionResult {
    let action = string_to_c(&r.action);
    let ref_id = opt_string_to_c(r.ref_id.as_deref());
    let post_state = match &r.post_state {
        None => ptr::null_mut(),
        Some(state) => {
            let role = string_to_c(&state.role);
            let value = opt_string_to_c(state.value.as_deref());
            let state_count = state.states.len() as u32;
            let states = if state.states.is_empty() {
                ptr::null_mut()
            } else {
                let mut ptrs: Vec<*mut std::os::raw::c_char> =
                    state.states.iter().map(|s| string_to_c(s)).collect();
                ptrs.shrink_to_fit();
                let raw = ptrs.as_mut_ptr();
                std::mem::forget(ptrs);
                raw
            };
            let elem = Box::new(AdElementState {
                role,
                states,
                state_count,
                value,
            });
            Box::into_raw(elem)
        }
    };
    AdActionResult {
        action,
        ref_id,
        post_state,
    }
}

/// # Safety
///
/// `adapter` must be a non-null pointer returned by `ad_adapter_create`.
/// `entry` must be a non-null pointer to a valid `AdRefEntry`.
/// `out` must be a non-null pointer to an `AdNativeHandle` to write the result into.
#[no_mangle]
pub unsafe extern "C" fn ad_resolve_element(
    adapter: *const AdAdapter,
    entry: *const AdRefEntry,
    out: *mut AdNativeHandle,
) -> AdResult {
    let adapter = &*adapter;
    let entry = &*entry;
    let role = match c_to_str(entry.role) {
        Some(s) => s.to_owned(),
        None => {
            error::set_last_error(&agent_desktop_core::error::AdapterError::new(
                agent_desktop_core::error::ErrorCode::InvalidArgs,
                "role is null or invalid UTF-8",
            ));
            return AdResult::ErrInvalidArgs;
        }
    };
    let name = c_to_str(entry.name).map(|s| s.to_owned());
    let bounds_hash = if entry.has_bounds_hash {
        Some(entry.bounds_hash)
    } else {
        None
    };
    let core_entry = CoreRefEntry {
        pid: entry.pid,
        role,
        name,
        value: None,
        states: vec![],
        bounds: None,
        bounds_hash,
        available_actions: vec![],
        source_app: None,
    };
    match adapter.inner.resolve_element(&core_entry) {
        Ok(handle) => {
            (*out).ptr = handle.as_raw();
            error::clear_last_error();
            AdResult::Ok
        }
        Err(e) => {
            error::set_last_error(&e);
            AdResult::ErrElementNotFound
        }
    }
}

/// # Safety
///
/// `adapter` must be a non-null pointer returned by `ad_adapter_create`.
/// `handle` must be a non-null pointer to a valid `AdNativeHandle`.
/// `action` must be a non-null pointer to a valid `AdAction`.
/// `out` must be a non-null pointer to an `AdActionResult` to write the result into.
#[no_mangle]
pub unsafe extern "C" fn ad_execute_action(
    adapter: *const AdAdapter,
    handle: *const AdNativeHandle,
    action: *const AdAction,
    out: *mut AdActionResult,
) -> AdResult {
    let adapter = &*adapter;
    let handle_ref = &*handle;
    let action_ref = &*action;
    let core_action = match action_from_c(action_ref) {
        Ok(a) => a,
        Err(msg) => {
            error::set_last_error(&agent_desktop_core::error::AdapterError::new(
                agent_desktop_core::error::ErrorCode::InvalidArgs,
                msg,
            ));
            return AdResult::ErrInvalidArgs;
        }
    };
    let native_handle = NativeHandle::from_ptr(handle_ref.ptr);
    match adapter.inner.execute_action(&native_handle, core_action) {
        Ok(result) => {
            *out = action_result_to_c(&result);
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
/// `result` must be a pointer to an `AdActionResult` previously written by `ad_execute_action`,
/// or null. After this call all pointers inside the struct are invalid.
#[no_mangle]
pub unsafe extern "C" fn ad_free_action_result(result: *mut AdActionResult) {
    if result.is_null() {
        return;
    }
    let r = &mut *result;
    free_c_string(r.action as *mut _);
    free_c_string(r.ref_id as *mut _);
    if !r.post_state.is_null() {
        let state = &mut *r.post_state;
        free_c_string(state.role as *mut _);
        free_c_string(state.value as *mut _);
        if !state.states.is_null() && state.state_count > 0 {
            let slice = std::slice::from_raw_parts_mut(state.states, state.state_count as usize);
            for ptr in slice.iter() {
                free_c_string(*ptr);
            }
            drop(Box::from_raw(
                std::slice::from_raw_parts_mut(state.states, state.state_count as usize)
                    .as_mut_ptr(),
            ));
        }
        drop(Box::from_raw(r.post_state));
        r.post_state = ptr::null_mut();
    }
    r.action = ptr::null();
    r.ref_id = ptr::null();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::convert::string_to_c;
    use crate::types::{AdDragParams, AdPoint, AdScrollParams};
    use agent_desktop_core::action::ElementState;

    fn make_scroll_params() -> AdScrollParams {
        AdScrollParams {
            direction: AdDirection::Down,
            amount: 3,
        }
    }

    fn make_key_combo() -> AdKeyCombo {
        AdKeyCombo {
            key: ptr::null(),
            modifiers: ptr::null(),
            modifier_count: 0,
        }
    }

    fn make_drag_params() -> AdDragParams {
        AdDragParams {
            from: AdPoint { x: 0.0, y: 0.0 },
            to: AdPoint { x: 0.0, y: 0.0 },
            duration_ms: 0,
        }
    }

    #[test]
    fn test_simple_action_roundtrip() {
        let action = AdAction {
            kind: AdActionKind::Click,
            text: ptr::null(),
            scroll: make_scroll_params(),
            key: make_key_combo(),
            drag: make_drag_params(),
        };
        let result = unsafe { action_from_c(&action) };
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), Action::Click));
    }

    #[test]
    fn test_action_result_to_c_with_state() {
        let core_result = CoreActionResult {
            action: "click".to_owned(),
            ref_id: Some("@e3".to_owned()),
            post_state: Some(ElementState {
                role: "button".to_owned(),
                states: vec!["focused".to_owned(), "enabled".to_owned()],
                value: Some("OK".to_owned()),
            }),
        };
        let c_result = action_result_to_c(&core_result);
        unsafe {
            assert_eq!(c_to_str(c_result.action), Some("click"));
            assert_eq!(c_to_str(c_result.ref_id), Some("@e3"));
            assert!(!c_result.post_state.is_null());
            let state = &*c_result.post_state;
            assert_eq!(c_to_str(state.role), Some("button"));
            assert_eq!(c_to_str(state.value), Some("OK"));
            assert_eq!(state.state_count, 2);
        }
        let mut c_result = c_result;
        unsafe { ad_free_action_result(&mut c_result) };
    }

    #[test]
    fn test_free_null_action_result() {
        unsafe { ad_free_action_result(ptr::null_mut()) };
    }

    #[test]
    fn test_set_value_action() {
        let text = string_to_c("hello world");
        let action = AdAction {
            kind: AdActionKind::SetValue,
            text,
            scroll: make_scroll_params(),
            key: make_key_combo(),
            drag: make_drag_params(),
        };
        let result = unsafe { action_from_c(&action) };
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), Action::SetValue(ref s) if s == "hello world"));
        unsafe { free_c_string(text as *mut _) };
    }

    #[test]
    fn test_scroll_action() {
        let action = AdAction {
            kind: AdActionKind::Scroll,
            text: ptr::null(),
            scroll: AdScrollParams {
                direction: AdDirection::Up,
                amount: 5,
            },
            key: make_key_combo(),
            drag: make_drag_params(),
        };
        let result = unsafe { action_from_c(&action) };
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), Action::Scroll(Direction::Up, 5)));
    }
}
