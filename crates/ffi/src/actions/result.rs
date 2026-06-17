use crate::convert::string::{free_c_string, opt_string_to_c, string_to_c_lossy};
use crate::types::{AdActionResult, AdElementState};
use agent_desktop_core::action_result::ActionResult as CoreActionResult;
use std::ptr;

const MAX_STATE_STRINGS_TO_FREE: usize = 1024;

pub(crate) fn action_result_to_c(r: &CoreActionResult) -> AdActionResult {
    let action = string_to_c_lossy(&r.action);
    let post_state = match &r.post_state {
        None => ptr::null_mut(),
        Some(state) => {
            let role = string_to_c_lossy(&state.role);
            let value = opt_string_to_c(state.value.as_deref());
            let state_count = state.states.len() as u32;
            let states = if state.states.is_empty() {
                ptr::null_mut()
            } else {
                let mut ptrs: Vec<*mut std::os::raw::c_char> =
                    state.states.iter().map(|s| string_to_c_lossy(s)).collect();
                ptrs.push(ptr::null_mut());
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
        ref_id: ptr::null(),
        post_state,
    }
}

/// # Safety
///
/// `result` must be a pointer to an `AdActionResult` previously written by `ad_execute_action`,
/// or null. After this call all pointers inside the struct are invalid.
#[unsafe(no_mangle)]
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
            if !state.states.is_null() {
                free_state_array(state.states);
            }
            drop(Box::from_raw(r.post_state));
            r.post_state = ptr::null_mut();
        }
        r.action = ptr::null();
        r.ref_id = ptr::null();
    })
}

unsafe fn free_state_array(states: *mut *mut std::os::raw::c_char) {
    unsafe {
        let mut len = 0;
        while len < MAX_STATE_STRINGS_TO_FREE && !(*states.add(len)).is_null() {
            free_c_string(*states.add(len));
            len += 1;
        }
        drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
            states,
            len + 1,
        )));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::convert::string::c_to_string;
    use agent_desktop_core::element_state::ElementState;

    #[test]
    fn test_action_result_to_c_with_state() {
        let core_result = CoreActionResult {
            action: "click".to_owned(),
            post_state: Some(ElementState {
                role: "button".to_owned(),
                states: vec!["focused".to_owned(), "enabled".to_owned()],
                value: Some("OK".to_owned()),
            }),
            steps: Vec::new(),
        };
        let c_result = action_result_to_c(&core_result);
        unsafe {
            assert_eq!(c_to_string(c_result.action).as_deref(), Some("click"));
            assert!(c_result.ref_id.is_null());
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
    fn free_action_result_ignores_mutated_state_count() {
        let post_state = Box::new(AdElementState {
            role: crate::convert::string::string_to_c_lossy("button"),
            states: state_array(&["focused"]),
            state_count: u32::MAX,
            value: ptr::null(),
        });
        let mut c_result = AdActionResult {
            action: crate::convert::string::string_to_c_lossy("click"),
            ref_id: ptr::null(),
            post_state: Box::into_raw(post_state),
        };
        unsafe { ad_free_action_result(&mut c_result) };

        assert!(c_result.post_state.is_null());
    }

    fn state_array(states: &[&str]) -> *mut *mut std::os::raw::c_char {
        let mut ptrs: Vec<*mut std::os::raw::c_char> = states
            .iter()
            .map(|state| crate::convert::string::string_to_c_lossy(state))
            .collect();
        ptrs.push(ptr::null_mut());
        let mut boxed = ptrs.into_boxed_slice();
        let raw = boxed.as_mut_ptr();
        std::mem::forget(boxed);
        raw
    }

    #[test]
    fn test_free_null_action_result() {
        unsafe { ad_free_action_result(ptr::null_mut()) };
    }
}
