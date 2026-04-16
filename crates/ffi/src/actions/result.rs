use crate::convert::string::{free_c_string, opt_string_to_c, string_to_c_lossy};
use crate::types::{AdActionResult, AdElementState};
use agent_desktop_core::action::ActionResult as CoreActionResult;
use std::ptr;

pub(crate) fn action_result_to_c(r: &CoreActionResult) -> AdActionResult {
    let action = string_to_c_lossy(&r.action);
    let ref_id = opt_string_to_c(r.ref_id.as_deref());
    let post_state = match &r.post_state {
        None => ptr::null_mut(),
        Some(state) => {
            let role = string_to_c_lossy(&state.role);
            let value = opt_string_to_c(state.value.as_deref());
            let state_count = state.states.len() as u32;
            let states = if state.states.is_empty() {
                ptr::null_mut()
            } else {
                let ptrs: Vec<*mut std::os::raw::c_char> =
                    state.states.iter().map(|s| string_to_c_lossy(s)).collect();
                let mut boxed = ptrs.into_boxed_slice();
                let raw = boxed.as_mut_ptr();
                std::mem::forget(boxed);
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
/// `result` must be a pointer to an `AdActionResult` previously written by `ad_execute_action`,
/// or null. After this call all pointers inside the struct are invalid.
#[no_mangle]
pub unsafe extern "C" fn ad_free_action_result(result: *mut AdActionResult) {
    crate::ffi_try::trap_panic_void(|| unsafe {
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
                let slice =
                    std::slice::from_raw_parts_mut(state.states, state.state_count as usize);
                for ptr in slice.iter() {
                    free_c_string(*ptr);
                }
                drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
                    state.states,
                    state.state_count as usize,
                )));
            }
            drop(Box::from_raw(r.post_state));
            r.post_state = ptr::null_mut();
        }
        r.action = ptr::null();
        r.ref_id = ptr::null();
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::convert::string::c_to_string;
    use agent_desktop_core::action::ElementState;

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
            assert_eq!(c_to_string(c_result.action).as_deref(), Some("click"));
            assert_eq!(c_to_string(c_result.ref_id).as_deref(), Some("@e3"));
            assert!(!c_result.post_state.is_null());
            let state = &*c_result.post_state;
            assert_eq!(c_to_string(state.role).as_deref(), Some("button"));
            assert_eq!(c_to_string(state.value).as_deref(), Some("OK"));
            assert_eq!(state.state_count, 2);
        }
        let mut c_result = c_result;
        unsafe { ad_free_action_result(&mut c_result) };
    }

    #[test]
    fn test_free_null_action_result() {
        unsafe { ad_free_action_result(ptr::null_mut()) };
    }
}
