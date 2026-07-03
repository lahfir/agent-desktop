use agent_desktop_core::error::AdapterError;
use agent_desktop_core::process_state::ProcessState;

/// Result of one AX responsiveness read, decoupled from the raw AXError so
/// `classify` (below) is testable without a live accessibility tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AxProbeResult {
    Responsive,
    CannotComplete,
}

/// Pure classification: alive/dead + a probe closure in, `ProcessState` out.
/// Kept free of any platform API so the retry threshold (one transient
/// `CannotComplete` must not classify `Unresponsive`; two consecutive must)
/// is unit-testable on every host, not just macOS with a live AX tree.
pub(crate) fn classify(pid_alive: bool, mut probe: impl FnMut() -> AxProbeResult) -> ProcessState {
    if !pid_alive {
        return ProcessState::Exited { code: None };
    }
    match probe() {
        AxProbeResult::Responsive => ProcessState::Running,
        AxProbeResult::CannotComplete => match probe() {
            AxProbeResult::Responsive => ProcessState::Running,
            AxProbeResult::CannotComplete => ProcessState::Unresponsive,
        },
    }
}

#[cfg(target_os = "macos")]
pub fn process_state_impl(pid: i32) -> Result<ProcessState, AdapterError> {
    use crate::tree::element_for_pid;

    Ok(classify(pid_is_alive(pid), || {
        ax_probe(&element_for_pid(pid))
    }))
}

/// `kill(pid, 0)`-style liveness check: signal 0 sends no actual signal, the
/// kernel only validates the target exists and is reachable. Mirrors the
/// convention already used by `system::force_close::signal_result`.
#[cfg(target_os = "macos")]
fn pid_is_alive(pid: i32) -> bool {
    const POSIX_ESRCH: i32 = 3;

    if pid <= 0 {
        return false;
    }
    unsafe extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    if unsafe { kill(pid, 0) } == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() != Some(POSIX_ESRCH)
}

#[cfg(target_os = "macos")]
fn ax_probe(app: &crate::tree::AXElement) -> AxProbeResult {
    use accessibility_sys::{
        AXUIElementCopyAttributeValue, kAXErrorCannotComplete, kAXErrorSuccess, kAXRoleAttribute,
    };
    use core_foundation::{
        base::{CFType, CFTypeRef, TCFType},
        string::CFString,
    };

    if app.0.is_null() {
        return AxProbeResult::Responsive;
    }
    let cf_attr = CFString::new(kAXRoleAttribute);
    let mut value: CFTypeRef = std::ptr::null_mut();
    let err =
        unsafe { AXUIElementCopyAttributeValue(app.0, cf_attr.as_concrete_TypeRef(), &mut value) };
    if err == kAXErrorCannotComplete {
        return AxProbeResult::CannotComplete;
    }
    if err == kAXErrorSuccess && !value.is_null() {
        unsafe { CFType::wrap_under_create_rule(value) };
    }
    AxProbeResult::Responsive
}

#[cfg(not(target_os = "macos"))]
pub fn process_state_impl(_pid: i32) -> Result<ProcessState, AdapterError> {
    Err(AdapterError::not_supported("process_state"))
}

#[cfg(test)]
#[path = "process_state_tests.rs"]
mod tests;
