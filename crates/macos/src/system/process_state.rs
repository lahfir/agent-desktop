use agent_desktop_core::error::AdapterError;
use agent_desktop_core::process_state::ProcessState;

#[cfg(target_os = "macos")]
pub fn process_state_impl(pid: i32) -> Result<ProcessState, AdapterError> {
    use crate::tree::{copy_string_attr, element_for_pid};
    use accessibility_sys::kAXRoleAttribute;

    if pid <= 0 {
        return Ok(ProcessState::Unknown);
    }
    let app = element_for_pid(pid);
    if app.0.is_null() {
        return Ok(ProcessState::Unknown);
    }
    if copy_string_attr(&app, kAXRoleAttribute).is_some() {
        Ok(ProcessState::Responsive)
    } else {
        Ok(ProcessState::Unresponsive)
    }
}

#[cfg(not(target_os = "macos"))]
pub fn process_state_impl(_pid: i32) -> Result<ProcessState, AdapterError> {
    Err(AdapterError::not_supported("process_state"))
}
