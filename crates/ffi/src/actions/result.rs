use crate::convert::string::{free_c_string, opt_string_to_c, string_to_c_lossy};
use crate::types::{AdActionResult, AdElementState, action_step::AdActionStep};
use agent_desktop_core::action_result::ActionResult as CoreActionResult;
use agent_desktop_core::action_step_outcome::ActionStepOutcome;
use std::ptr;

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
        steps: action_steps_to_c(r),
        step_count: r.steps.len() as u32,
    }
}

/// # Safety
///
/// `result` must be null or a pointer to an `AdActionResult` previously written
/// by `ad_execute_action`, `ad_execute_action_with_policy`,
/// `ad_execute_ref_action_with_policy`, or `ad_notification_action`. This frees
/// `post_state`, `steps`, and all nested strings. After this call all pointers
/// inside the struct are invalid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_free_action_result(result: *mut AdActionResult) {
    crate::ffi_try::trap_panic_void(|| unsafe {
        if result.is_null() {
            return;
        }
        let r = &mut *result;
        free_c_string(r.action as *mut _);
        free_c_string(r.ref_id as *mut _);
        if !r.steps.is_null() {
            free_step_array(r.steps);
        }
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
        r.steps = ptr::null_mut();
        r.step_count = 0;
    })
}

fn action_steps_to_c(r: &CoreActionResult) -> *mut AdActionStep {
    if r.steps.is_empty() {
        return ptr::null_mut();
    }
    let mut steps = r
        .steps
        .iter()
        .map(|step| AdActionStep {
            label: string_to_c_lossy(step.label()),
            outcome: string_to_c_lossy(step_outcome_name(&step.outcome)),
        })
        .collect::<Vec<_>>();
    steps.push(step_sentinel());
    let mut boxed = steps.into_boxed_slice();
    let raw = boxed.as_mut_ptr();
    std::mem::forget(boxed);
    raw
}

fn step_outcome_name(outcome: &ActionStepOutcome) -> &'static str {
    match outcome {
        ActionStepOutcome::Attempted => "attempted",
        ActionStepOutcome::Skipped => "skipped",
        ActionStepOutcome::Succeeded => "succeeded",
    }
}

fn step_sentinel() -> AdActionStep {
    AdActionStep {
        label: ptr::null(),
        outcome: ptr::null(),
    }
}

unsafe fn free_state_array(states: *mut *mut std::os::raw::c_char) {
    unsafe {
        let mut len = 0;
        while !(*states.add(len)).is_null() {
            len += 1;
        }
        for index in 0..len {
            free_c_string(*states.add(index));
        }
        drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
            states,
            len + 1,
        )));
    }
}

unsafe fn free_step_array(steps: *mut AdActionStep) {
    unsafe {
        let mut len = 0;
        while !step_is_sentinel(&*steps.add(len)) {
            len += 1;
        }
        for index in 0..len {
            let step = &mut *steps.add(index);
            free_c_string(step.label as *mut _);
            free_c_string(step.outcome as *mut _);
        }
        drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
            steps,
            len + 1,
        )));
    }
}

fn step_is_sentinel(step: &AdActionStep) -> bool {
    step.label.is_null() && step.outcome.is_null()
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
            assert!(c_result.steps.is_null());
            assert_eq!(c_result.step_count, 0);
            let state = &*c_result.post_state;
            assert_eq!(c_to_string(state.role).as_deref(), Some("button"));
            assert_eq!(c_to_string(state.value).as_deref(), Some("OK"));
            assert_eq!(state.state_count, 2);
        }
        let mut c_result = c_result;
        unsafe { ad_free_action_result(&mut c_result) };
    }

    #[test]
    fn free_action_result_ignores_mutated_counts() {
        let post_state = Box::new(AdElementState {
            role: crate::convert::string::string_to_c_lossy("button"),
            states: state_array(&["focused"]),
            state_count: u32::MAX,
            value: ptr::null(),
        });
        let mut steps = vec![
            AdActionStep {
                label: crate::convert::string::string_to_c_lossy("AXPress"),
                outcome: crate::convert::string::string_to_c_lossy("succeeded"),
            },
            step_sentinel(),
        ]
        .into_boxed_slice();
        let mut c_result = AdActionResult {
            action: crate::convert::string::string_to_c_lossy("click"),
            ref_id: ptr::null(),
            post_state: Box::into_raw(post_state),
            steps: steps.as_mut_ptr(),
            step_count: u32::MAX,
        };
        std::mem::forget(steps);
        unsafe { ad_free_action_result(&mut c_result) };

        assert!(c_result.post_state.is_null());
        assert!(c_result.steps.is_null());
        assert_eq!(c_result.step_count, 0);
    }

    #[test]
    fn action_result_to_c_preserves_steps() {
        let core_result = CoreActionResult::new("click").with_steps(vec![
            agent_desktop_core::action_step::ActionStep::attempted("AXScrollToVisible"),
            agent_desktop_core::action_step::ActionStep::succeeded("AXPress"),
        ]);

        let mut c_result = action_result_to_c(&core_result);

        unsafe {
            assert_eq!(c_result.step_count, 2);
            assert!(!c_result.steps.is_null());
            assert_eq!(
                c_to_string((*c_result.steps.add(0)).label).as_deref(),
                Some("AXScrollToVisible")
            );
            assert_eq!(
                c_to_string((*c_result.steps.add(0)).outcome).as_deref(),
                Some("attempted")
            );
            assert_eq!(
                c_to_string((*c_result.steps.add(1)).label).as_deref(),
                Some("AXPress")
            );
            assert_eq!(
                c_to_string((*c_result.steps.add(1)).outcome).as_deref(),
                Some("succeeded")
            );
        }

        unsafe { ad_free_action_result(&mut c_result) };
        assert!(c_result.steps.is_null());
        assert_eq!(c_result.step_count, 0);
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
