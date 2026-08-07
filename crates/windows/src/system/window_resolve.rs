use agent_desktop_core::{
    AdapterError, Deadline, DeliverySemantics, ErrorCode, InteractionLease, ProcessId, WindowInfo,
    WindowState,
};

use super::window_enum::enumerate_top_level;
use super::window_identity::WindowIdentityEvidence;
use super::window_ops::{parse_handle, passes_filter};

/// Resolves a live window by `WindowInfo.id`, corroborating pid and process
/// generation against the handle's current owner (stored-evidence rule).
pub(crate) fn resolve_window_strict(
    expected: &WindowInfo,
    _deadline: Deadline,
) -> Result<WindowInfo, AdapterError> {
    let handle = parse_handle(&expected.id);
    if handle.is_null() {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "window id is not a Windows HWND-shaped id",
        ));
    }
    if !window_exists(handle) {
        return Err(AdapterError::new(
            ErrorCode::WindowNotFound,
            "window handle is no longer a live top-level window",
        ));
    }
    let Some(evidence) = WindowIdentityEvidence::from_info(handle, expected) else {
        return Err(AdapterError::new(
            ErrorCode::StaleRef,
            "headed focus requires a process-instance token on the stored window",
        ));
    };
    evidence.verify_stored()?;
    live_window_info(handle, expected)
}

/// Raises and focuses the exact window after `resolve_window_strict`.
///
/// Ownership is revalidated at the point of use: the stored pid and process
/// generation are verified against the handle's current owner before any
/// window-state write, and the owning pid is re-read immediately before
/// `SetForegroundWindow`, so a handle destroyed and recycled to another
/// process after selection is refused rather than restored or foregrounded.
/// Confirms foreground membership before returning. Foreground-lock bypass
/// uses the thread-attach pattern; a failure to become foreground is
/// `ACTION_FAILED` with `physical_delivery_started: false`.
pub(crate) fn focus_window(win: &WindowInfo, lease: &InteractionLease) -> Result<(), AdapterError> {
    let handle = parse_handle(&win.id);
    if handle.is_null() {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "window id is not a Windows HWND-shaped id",
        ));
    }
    let Some(evidence) = WindowIdentityEvidence::from_info(handle, win) else {
        return Err(AdapterError::new(
            ErrorCode::StaleRef,
            "headed focus requires a process-instance token on the stored window",
        ));
    };
    evidence.verify_stored()?;
    let _ = lease;
    restore_if_iconic(handle);
    if is_foreground(handle) {
        return Ok(());
    }
    bring_to_foreground(handle, win.pid)?;
    if is_foreground(handle) {
        return Ok(());
    }
    Err(AdapterError::new(
        ErrorCode::ActionFailed,
        "Target window did not become foreground before headed delivery",
    )
    .with_details(serde_json::json!({
        "physical_delivery_started": false,
    }))
    .with_suggestion("Retry after ensuring the target window can accept focus")
    .with_disposition(DeliverySemantics::not_delivered()))
}

fn live_window_info(
    handle: super::window_enum::WindowHandle,
    expected: &WindowInfo,
) -> Result<WindowInfo, AdapterError> {
    let mut found = None;
    enumerate_top_level(|window| {
        if window.handle != handle {
            return true;
        }
        found = Some(window);
        false
    })?;
    let window = found.ok_or_else(|| {
        AdapterError::new(
            ErrorCode::WindowNotFound,
            "window handle is no longer present in the top-level inventory",
        )
    })?;
    if !passes_filter(&window) && !window.iconic {
        return Err(AdapterError::new(
            ErrorCode::WindowNotFound,
            "window is no longer an agent-visible top-level window",
        ));
    }
    let focused = is_foreground(handle);
    Ok(WindowInfo {
        id: expected.id.clone(),
        title: live_title(handle).unwrap_or_else(|| expected.title.clone()),
        app: expected.app.clone(),
        pid: expected.pid,
        process_instance: expected.process_instance.clone(),
        bounds: Some(window.rect),
        state: WindowState {
            is_focused: focused,
            minimized: Some(window.iconic),
            visible: Some(window.visible),
        },
    })
}

#[cfg(target_os = "windows")]
fn window_exists(handle: super::window_enum::WindowHandle) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::IsWindow;
    unsafe { IsWindow(handle) != 0 }
}

#[cfg(not(target_os = "windows"))]
fn window_exists(_handle: super::window_enum::WindowHandle) -> bool {
    false
}

#[cfg(target_os = "windows")]
fn is_foreground(handle: super::window_enum::WindowHandle) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    unsafe { GetForegroundWindow() == handle }
}

#[cfg(not(target_os = "windows"))]
fn is_foreground(_handle: super::window_enum::WindowHandle) -> bool {
    false
}

#[cfg(target_os = "windows")]
fn restore_if_iconic(handle: super::window_enum::WindowHandle) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{IsIconic, SW_RESTORE, ShowWindow};
    unsafe {
        if IsIconic(handle) != 0 {
            ShowWindow(handle, SW_RESTORE);
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn restore_if_iconic(_handle: super::window_enum::WindowHandle) {}

#[cfg(target_os = "windows")]
fn bring_to_foreground(
    handle: super::window_enum::WindowHandle,
    expected: ProcessId,
) -> Result<(), AdapterError> {
    use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId, SW_SHOW, SetForegroundWindow, ShowWindow,
    };

    unsafe {
        let mut target_pid = 0u32;
        let target_tid = GetWindowThreadProcessId(handle, &mut target_pid);
        if target_tid == 0 || ProcessId::from(target_pid) != expected {
            return Err(recycled_before_foreground());
        }
        ShowWindow(handle, SW_SHOW);
        let foreground = GetForegroundWindow();
        let mut fore_pid = 0u32;
        let fore_tid = if foreground.is_null() {
            0
        } else {
            GetWindowThreadProcessId(foreground, &mut fore_pid)
        };
        let current_tid = GetCurrentThreadId();
        let attached_fore = fore_tid != 0
            && fore_tid != current_tid
            && AttachThreadInput(current_tid, fore_tid, 1) != 0;
        let attached_target = target_tid != 0
            && target_tid != current_tid
            && AttachThreadInput(current_tid, target_tid, 1) != 0;
        let _ = SetForegroundWindow(handle);
        if attached_target {
            AttachThreadInput(current_tid, target_tid, 0);
        }
        if attached_fore {
            AttachThreadInput(current_tid, fore_tid, 0);
        }
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn bring_to_foreground(
    _handle: super::window_enum::WindowHandle,
    _expected: ProcessId,
) -> Result<(), AdapterError> {
    Err(AdapterError::not_supported("focus_window"))
}

/// The handle's owner changed between selection and the foreground write; the
/// write is refused so a recycled HWND never foregrounds a foreign window.
#[cfg(target_os = "windows")]
fn recycled_before_foreground() -> AdapterError {
    AdapterError::new(
        ErrorCode::WindowNotFound,
        "Target window handle changed ownership before foreground delivery",
    )
    .with_details(serde_json::json!({ "physical_delivery_started": false }))
    .with_disposition(DeliverySemantics::not_delivered())
}

#[cfg(target_os = "windows")]
fn live_title(handle: super::window_enum::WindowHandle) -> Option<String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextW;
    let mut buffer = vec![0u16; 512];
    let length = unsafe { GetWindowTextW(handle, buffer.as_mut_ptr(), buffer.len() as i32) };
    if length <= 0 {
        return None;
    }
    buffer.truncate(length as usize);
    Some(String::from_utf16_lossy(&buffer))
}

#[cfg(not(target_os = "windows"))]
fn live_title(_handle: super::window_enum::WindowHandle) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::window_ops::list_windows_live;
    use agent_desktop_core::WindowFilter;

    #[cfg(target_os = "windows")]
    #[test]
    fn resolve_window_strict_reconfirms_a_listed_fixture_window() {
        crate::tree::fixture::ensure_test_apartment();
        let fixture = crate::tree::fixture::HostedFixture::spawn().expect("fixture host starts");
        let windows = list_windows_live(&WindowFilter::default()).expect("list");
        let expected = windows
            .into_iter()
            .find(|window| window.pid == agent_desktop_core::ProcessId::from(fixture.process_id()))
            .expect("fixture window listed");
        let resolved =
            resolve_window_strict(&expected, Deadline::after(5_000).unwrap()).expect("resolve");
        assert_eq!(resolved.id, expected.id);
        assert_eq!(resolved.pid, expected.pid);
        assert_eq!(resolved.process_instance, expected.process_instance);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn resolve_window_strict_rejects_a_destroyed_handle() {
        let win = WindowInfo {
            id: "w-1".into(),
            title: "gone".into(),
            app: "none.exe".into(),
            pid: agent_desktop_core::ProcessId::from(1u32),
            process_instance: Some("windows-proc-v1:0:0".into()),
            bounds: None,
            state: WindowState::default(),
        };
        let err = resolve_window_strict(&win, Deadline::after(1_000).unwrap()).unwrap_err();
        assert_eq!(err.code, ErrorCode::WindowNotFound);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn focus_window_refuses_a_destroyed_handle_before_any_window_write() {
        let win = WindowInfo {
            id: "w-1".into(),
            title: "gone".into(),
            app: "none.exe".into(),
            pid: agent_desktop_core::ProcessId::from(1u32),
            process_instance: Some("windows-proc-v1:0:0".into()),
            bounds: None,
            state: WindowState::default(),
        };
        let lease = InteractionLease::guarded(Deadline::after(1_000).unwrap(), ()).expect("lease");
        let err = focus_window(&win, &lease).unwrap_err();
        assert_eq!(err.code, ErrorCode::WindowNotFound);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn focus_window_requires_a_process_instance_token() {
        let win = WindowInfo {
            id: "w-1".into(),
            title: "gone".into(),
            app: "none.exe".into(),
            pid: agent_desktop_core::ProcessId::from(1u32),
            process_instance: None,
            bounds: None,
            state: WindowState::default(),
        };
        let lease = InteractionLease::guarded(Deadline::after(1_000).unwrap(), ()).expect("lease");
        let err = focus_window(&win, &lease).unwrap_err();
        assert_eq!(err.code, ErrorCode::StaleRef);
    }
}
