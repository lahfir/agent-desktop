use agent_desktop_core::{
    AdapterError, AppInfo, Deadline, DeliverySemantics, ErrorCode, ProcessId,
};
use std::time::Duration;

use super::app_ops::is_protected_process;
use super::permissions::ensure_budget;
use super::process_identity;
use super::process_state::hresult_from_win32;

const STILL_ACTIVE: u32 = 259;
const EXIT_POLL: Duration = Duration::from_millis(25);

pub(crate) fn close_app_impl(
    app: &AppInfo,
    force: bool,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    ensure_not_protected(&app.name)?;
    if let Some(bundle_id) = &app.bundle_id {
        ensure_not_protected(bundle_id)?;
    }
    let instance = app.process_instance.as_deref().ok_or_else(|| {
        before_termination(AdapterError::new(
            ErrorCode::InvalidArgs,
            "Exact close requires a process instance token",
        ))
    })?;
    close_verified(app, instance, force, deadline)
}

#[cfg(target_os = "windows")]
fn close_verified(
    app: &AppInfo,
    instance: &str,
    force: bool,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    ensure_budget(deadline).map_err(before_termination)?;
    if process_observed_gone(app.pid, instance).map_err(before_termination)? {
        return Ok(());
    }
    if force {
        force_terminate(app.pid, instance, deadline)?;
    } else {
        graceful_close(app.pid, instance, deadline)?;
    }
    wait_for_exit(&app.name, app.pid, instance, force, deadline)
}

#[cfg(not(target_os = "windows"))]
fn close_verified(
    _app: &AppInfo,
    _instance: &str,
    _force: bool,
    _deadline: Deadline,
) -> Result<(), AdapterError> {
    Err(AdapterError::not_supported("close_app"))
}

fn ensure_not_protected(id: &str) -> Result<(), AdapterError> {
    if is_protected_process(id) {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            format!("'{id}' is a protected system process and cannot be closed"),
        )
        .with_suggestion(
            "Target a regular application; session-critical processes are never closed.",
        ));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn graceful_close(pid: ProcessId, instance: &str, deadline: Deadline) -> Result<(), AdapterError> {
    ensure_budget(deadline).map_err(before_termination)?;
    let windows = top_level_windows_for_pid(pid).map_err(before_termination)?;
    if windows.is_empty() {
        return windowless_graceful_fallback(pid, instance, deadline);
    }
    broadcast_close(&windows, deadline, |hwnd| {
        post_wm_close_if_still_owned(hwnd, pid, instance)
    })
}

/// Posts the close request to every owned window before reporting a failure.
///
/// A multi-window app only closes when the request reaches the window that
/// owns its shutdown, so one window's refusal must not cancel delivery to its
/// siblings - and a window destroyed between enumeration and post is the
/// expected shape once an app starts tearing itself down in response to an
/// earlier post in this very loop. The fan-out therefore keeps going and only
/// surfaces a failure when **no** window accepted the request; when at least
/// one did, `wait_for_exit`'s independent process-gone re-read is the source
/// of truth, not the count of successful posts.
fn broadcast_close(
    windows: &[isize],
    deadline: Deadline,
    mut post: impl FnMut(isize) -> Result<bool, AdapterError>,
) -> Result<(), AdapterError> {
    let mut delivered = false;
    let mut first_failure = None;
    for hwnd in windows {
        ensure_budget(deadline).map_err(before_termination)?;
        match post(*hwnd) {
            Ok(true) => delivered = true,
            Ok(false) => {}
            Err(error) => {
                if first_failure.is_none() {
                    first_failure = Some(error);
                }
            }
        }
    }
    match first_failure {
        Some(error) if !delivered => Err(before_termination(error)),
        _ => Ok(()),
    }
}

#[cfg(target_os = "windows")]
fn windowless_graceful_fallback(
    pid: ProcessId,
    instance: &str,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    ensure_budget(deadline).map_err(before_termination)?;
    if process_observed_gone(pid, instance).map_err(before_termination)? {
        return Ok(());
    }
    Err(AdapterError::new(
        ErrorCode::ActionFailed,
        format!("Process {pid} has no top-level windows to receive WM_CLOSE"),
    )
    .with_details(serde_json::json!({ "pid": u32::from(pid) }))
    .with_suggestion(format!(
        "Retry with --force to terminate pid {pid} without WM_CLOSE"
    ))
    .with_disposition(DeliverySemantics::not_delivered()))
}

#[cfg(target_os = "windows")]
fn force_terminate(pid: ProcessId, instance: &str, deadline: Deadline) -> Result<(), AdapterError> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
        TerminateProcess,
    };

    ensure_budget(deadline).map_err(before_termination)?;
    if process_observed_gone(pid, instance).map_err(before_termination)? {
        return Ok(());
    }
    let access = PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE;
    let handle = unsafe { OpenProcess(access, 0, u32::from(pid)) };
    if handle.is_null() {
        if process_observed_gone(pid, instance).map_err(before_termination)? {
            return Ok(());
        }
        return Err(before_termination(win32_last_error(
            "OpenProcess failed before TerminateProcess",
        )));
    }
    let token_matches = handle_matches_instance(handle, pid, instance).map_err(|error| {
        unsafe { CloseHandle(handle) };
        before_termination(error)
    })?;
    if !token_matches {
        unsafe { CloseHandle(handle) };
        return Ok(());
    }
    let ok = unsafe { TerminateProcess(handle, 1) };
    let error = if ok == 0 {
        Some(unsafe { windows_sys::Win32::Foundation::GetLastError() })
    } else {
        None
    };
    unsafe { CloseHandle(handle) };
    if let Some(code) = error {
        if process_observed_gone(pid, instance).map_err(before_termination)? {
            return Ok(());
        }
        return Err(before_termination(adapter_error_from_win32(
            code,
            "TerminateProcess failed",
        )));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn wait_for_exit(
    id: &str,
    pid: ProcessId,
    instance: &str,
    force: bool,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    loop {
        if process_observed_gone(pid, instance).map_err(after_termination)? {
            return Ok(());
        }
        if deadline.is_expired() {
            return Err(deadline
                .timeout_error()
                .with_details(serde_json::json!({
                    "app": id,
                    "pid": u32::from(pid),
                    "force": force,
                }))
                .with_disposition(DeliverySemantics::delivered_unverified()));
        }
        ensure_budget(deadline).map_err(after_termination)?;
        let pause = deadline
            .remaining_slice(EXIT_POLL)
            .map_err(after_termination)?;
        std::thread::sleep(pause.min(EXIT_POLL));
    }
}

/// Wait-gated exit observation (A21-3): signaled handle + non-`STILL_ACTIVE`
/// exit code, or a creation-time token that no longer matches a live pid.
#[cfg(target_os = "windows")]
fn process_observed_gone(pid: ProcessId, instance: &str) -> Result<bool, AdapterError> {
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
        WaitForSingleObject,
    };

    if !process_identity::matches_instance(pid, instance)? {
        return Ok(true);
    }
    let access = PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE;
    let handle = unsafe { OpenProcess(access, 0, u32::from(pid)) };
    if handle.is_null() {
        return Ok(!process_identity::matches_instance(pid, instance)?);
    }
    let wait = unsafe { WaitForSingleObject(handle, 0) };
    let mut code: u32 = 0;
    let read_ok = unsafe { GetExitCodeProcess(handle, &mut code) };
    unsafe { CloseHandle(handle) };
    if wait == WAIT_OBJECT_0 && read_ok != 0 && code != STILL_ACTIVE {
        return Ok(true);
    }
    Ok(!process_identity::matches_instance(pid, instance)?)
}

#[cfg(target_os = "windows")]
fn top_level_windows_for_pid(pid: ProcessId) -> Result<Vec<isize>, AdapterError> {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

    let target = u32::from(pid);
    let mut handles = Vec::new();
    super::window_enum::enumerate_top_level(|window| {
        let mut owner: u32 = 0;
        unsafe { GetWindowThreadProcessId(window.handle, &mut owner) };
        if owner == target {
            handles.push(window.handle as isize);
        }
        true
    })?;
    Ok(handles)
}

#[cfg(target_os = "windows")]
/// Posts `WM_CLOSE` to one window, reporting whether the request was actually
/// delivered so the fan-out can tell a deliberate skip from a send.
fn post_wm_close_if_still_owned(
    hwnd: isize,
    pid: ProcessId,
    instance: &str,
) -> Result<bool, AdapterError> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowThreadProcessId, PostMessageW, WM_CLOSE,
    };

    let mut owner: u32 = 0;
    unsafe { GetWindowThreadProcessId(hwnd as *mut core::ffi::c_void, &mut owner) };
    if owner != u32::from(pid) {
        return Ok(false);
    }
    if !process_identity::matches_instance(pid, instance)? {
        return Ok(false);
    }
    let posted = unsafe { PostMessageW(hwnd as *mut core::ffi::c_void, WM_CLOSE, 0, 0) };
    if posted == 0 {
        return Err(win32_last_error("PostMessageW(WM_CLOSE) failed"));
    }
    Ok(true)
}

/// Verifies the open process handle's creation times still match `instance`
/// before a terminate write, so a recycled pid cannot be killed.
#[cfg(target_os = "windows")]
fn handle_matches_instance(
    handle: windows_sys::Win32::Foundation::HANDLE,
    _pid: ProcessId,
    instance: &str,
) -> Result<bool, AdapterError> {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::GetProcessTimes;

    let mut created = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    let read_ok =
        unsafe { GetProcessTimes(handle, &mut created, &mut exit, &mut kernel, &mut user) };
    if read_ok == 0 {
        return Ok(false);
    }
    let ticks = (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
    if ticks == 0 {
        return Ok(false);
    }
    const TICKS_PER_SECOND: u64 = 10_000_000;
    let from_handle = format!(
        "windows-proc-v1:{}:{}",
        ticks / TICKS_PER_SECOND,
        ticks % TICKS_PER_SECOND
    );
    Ok(from_handle == instance)
}

#[cfg(target_os = "windows")]
fn win32_last_error(message: &str) -> AdapterError {
    let error = unsafe { windows_sys::Win32::Foundation::GetLastError() };
    adapter_error_from_win32(error, message)
}

fn adapter_error_from_win32(error: u32, message: &str) -> AdapterError {
    let hresult = hresult_from_win32(error);
    let record = super::hresult::hresult_record(hresult);
    let mut err = AdapterError::new(record.code, message)
        .with_platform_detail(super::hresult::com_hresult_detail(hresult));
    if let Some(suggestion) = record.suggestion {
        err = err.with_suggestion(suggestion);
    }
    err
}

fn before_termination(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::not_delivered())
}

fn after_termination(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::delivered_unverified())
}

#[cfg(test)]
#[path = "close_tests.rs"]
mod tests;
