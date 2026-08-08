use agent_desktop_core::{
    AdapterError, Deadline, DeliverySemantics, ErrorCode, InteractionLease, WindowInfo, WindowState,
};

use super::window_enum::enumerate_top_level;
use super::window_identity::{WindowIdentityEvidence, live_window_title};
use super::window_ops::{is_foreground_window, parse_handle, passes_filter};

/// Resolves a live window by `WindowInfo.id`, corroborating pid and process
/// generation against the handle's current owner (stored-evidence rule).
pub(crate) fn resolve_window_strict(
    expected: &WindowInfo,
    deadline: Deadline,
) -> Result<WindowInfo, AdapterError> {
    super::permissions::ensure_budget(deadline)?;
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
/// A handle destroyed and recycled to another process after selection can
/// never be restored, foregrounded, or reported as focused. Check-then-act
/// on an HWND cannot be made atomic, so ownership is enforced on both sides
/// of every write rather than once at entry: the stored pid and process
/// generation are verified here, each native write re-reads the owning pid
/// immediately before issuing it, and success itself is ownership-qualified:
/// `is_owned_foreground` requires the foreground window to be this handle
/// **and** still owned by the expected process. A recycle that wins the
/// residual race therefore fails closed as not-delivered instead of
/// reporting focus over a foreign window and licensing headed input into it.
/// Foreground-lock bypass uses the thread-attach pattern; a failure to
/// become foreground is `ACTION_FAILED` with `physical_delivery_started`
/// false.
///
/// The budget is consulted before anything else, as `resolve_window_strict`
/// does. A call with no time left has no business spending three cross-process
/// identity reads to find that out, and those reads answer for the state of
/// the desktop rather than for the budget: an exhausted lease must report
/// `TIMEOUT` and not whatever a window whose owner was mid-teardown happened to
/// say. Identity is still verified in full before any native write.
pub(crate) fn focus_window(win: &WindowInfo, lease: &InteractionLease) -> Result<(), AdapterError> {
    super::permissions::ensure_budget(lease.deadline())?;
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
    restore_if_iconic(handle, &evidence)?;
    if is_owned_foreground(handle, &evidence) {
        return Ok(());
    }
    bring_to_foreground(handle, &evidence)?;
    if is_owned_foreground(handle, &evidence) {
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
    let focused = is_foreground_window(handle);
    Ok(WindowInfo {
        id: expected.id.clone(),
        title: live_window_title(handle).unwrap_or_else(|| expected.title.clone()),
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

/// Whether the handle is the foreground window **and** is still owned by the
/// stored process instance.
///
/// Handle equality alone would accept a recycled HWND; pid equality alone
/// would accept a replacement process that inherited a recycled pid. The
/// success predicate therefore asks the same full-identity question the
/// admission check asks, and an unreadable answer is not a success — a
/// generation read that fails leaves the window unproven, so this reports
/// false and the caller returns not-delivered.
#[cfg(target_os = "windows")]
fn is_owned_foreground(
    handle: super::window_enum::WindowHandle,
    evidence: &WindowIdentityEvidence<'_>,
) -> bool {
    is_foreground_window(handle) && evidence.owns_handle_now().unwrap_or(false)
}

#[cfg(not(target_os = "windows"))]
fn is_owned_foreground(
    _handle: super::window_enum::WindowHandle,
    _evidence: &WindowIdentityEvidence<'_>,
) -> bool {
    false
}

#[cfg(target_os = "windows")]
fn restore_if_iconic(
    handle: super::window_enum::WindowHandle,
    evidence: &WindowIdentityEvidence<'_>,
) -> Result<(), AdapterError> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{IsIconic, SW_RESTORE, ShowWindow};
    unsafe {
        if IsIconic(handle) == 0 {
            return Ok(());
        }
        if !evidence.owns_handle_now()? {
            return Err(recycled_before_foreground());
        }
        ShowWindow(handle, SW_RESTORE);
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn restore_if_iconic(
    _handle: super::window_enum::WindowHandle,
    _evidence: &WindowIdentityEvidence<'_>,
) -> Result<(), AdapterError> {
    Ok(())
}

#[cfg(target_os = "windows")]
fn bring_to_foreground(
    handle: super::window_enum::WindowHandle,
    evidence: &WindowIdentityEvidence<'_>,
) -> Result<(), AdapterError> {
    use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId, IsWindowVisible, SW_SHOW,
        SetForegroundWindow, ShowWindow,
    };

    unsafe {
        if !evidence.owns_handle_now()? {
            return Err(recycled_before_foreground());
        }
        if IsWindowVisible(handle) == 0 {
            ShowWindow(handle, SW_SHOW);
        }
        let mut target_pid = 0u32;
        let target_tid = GetWindowThreadProcessId(handle, &mut target_pid);
        if target_tid == 0 || !evidence.owns_handle_now()? {
            return Err(recycled_before_foreground());
        }
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
        let still_owned = evidence.owns_handle_now().unwrap_or(false);
        if still_owned {
            let _ = SetForegroundWindow(handle);
        }
        if attached_target {
            AttachThreadInput(current_tid, target_tid, 0);
        }
        if attached_fore {
            AttachThreadInput(current_tid, fore_tid, 0);
        }
        if !still_owned {
            return Err(recycled_before_foreground());
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
///
/// What this cannot promise, stated so it is a known limit rather than a
/// missed case: Win32 offers no atomic "act on this window only while that
/// process still owns it", so an ownership read and the native call it guards
/// are always two instructions. A recycle landing between them mutates the
/// replacement window, and nothing can undo that. Three things bound it: the
/// read sits immediately before each write, the only unconditional write left
/// is the foreground call itself (the visibility write is skipped when the
/// window is already visible, which it is on every path that reaches here),
/// and success is ownership-qualified — so the outcome of losing the race is
/// a foreign window raised once and an honest not-delivered refusal, never
/// input delivered to it. Closing it entirely needs a kernel primitive that
/// does not exist.
#[cfg(target_os = "windows")]
fn recycled_before_foreground() -> AdapterError {
    AdapterError::new(
        ErrorCode::WindowNotFound,
        "Target window handle changed ownership before foreground delivery",
    )
    .with_details(serde_json::json!({ "physical_delivery_started": false }))
    .with_disposition(DeliverySemantics::not_delivered())
}

#[cfg(test)]
#[path = "window_resolve_tests.rs"]
mod tests;
