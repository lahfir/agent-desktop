use std::time::Duration;

use agent_desktop_core::{
    AdapterError, DeliverySemantics, ErrorCode, InteractionLease, WindowInfo,
};

use crate::input::elevation::{
    activation_elevation_denied, current_process_integrity_rid, process_integrity_rid,
};

use super::permissions::ensure_budget;
use super::window_identity::WindowIdentityEvidence;
use super::window_ops::{is_foreground_window, parse_handle};

/// A21-6: uncontended first attempt always lands; keep a finite retry of one
/// additional attach-and-set-foreground after a short delay so the retry can
/// observe the attach. Never unbounded.
const FOCUS_STEAL_BUDGET: u32 = 2;
const INTER_ATTEMPT_DELAY: Duration = Duration::from_millis(25);
/// How long the target gets to answer the liveness ping before activation is
/// refused as unresponsive.
const PUMP_PROBE_MS: u64 = 500;

/// Raises and focuses the exact window after stored-identity verification.
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
///
/// Restore-versus-raise ordering: restore-then-raise when iconic; raise
/// without restore when visible-but-not-foreground. Cross-integrity targets
/// are attempt-and-verify (A21-7); budget exhaustion on a strictly-higher
/// integrity target is activation-worded `PERM_DENIED`, otherwise
/// `ACTION_FAILED` with `physical_delivery_started: false`.
pub(crate) fn focus_window(win: &WindowInfo, lease: &InteractionLease) -> Result<(), AdapterError> {
    ensure_budget(lease.deadline()).map_err(before_activation)?;
    let handle = parse_handle(&win.id);
    if handle.is_null() {
        return Err(before_activation(AdapterError::new(
            ErrorCode::InvalidArgs,
            "window id is not a Windows HWND-shaped id",
        )));
    }
    let Some(evidence) = WindowIdentityEvidence::from_info(handle, win) else {
        return Err(before_activation(AdapterError::new(
            ErrorCode::StaleRef,
            "headed focus requires a process-instance token on the stored window",
        )));
    };
    evidence.verify_stored().map_err(before_activation)?;
    ensure_window_is_pumping(handle).map_err(before_activation)?;
    restore_if_iconic(handle, &evidence).map_err(before_activation)?;
    if is_owned_foreground(handle, &evidence) {
        return Ok(());
    }
    let strictly_higher = activation_target_is_strictly_higher(win.pid);
    raise_with_budget(handle, &evidence, lease, strictly_higher)
}

fn before_activation(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::not_delivered())
}

/// Refuses activation of a target that is not dispatching messages.
///
/// Every step that follows reaches the owning thread's message queue, so a
/// window whose thread never dispatches blocks the caller inside an OS call -
/// the same shape A14-11 recorded for `ElementFromHandle`. The raise is not
/// the first such step: identity verification reads the live title, and
/// `GetWindowTextW` sends `WM_GETTEXT`, so it blocks before any activation
/// call is reached. The retry budget cannot bound that either, because it is
/// consulted between attempts while the block happens inside one. Every headed
/// command reaches this path, so the ping runs once, ahead of verification.
#[cfg(target_os = "windows")]
fn ensure_window_is_pumping(handle: super::window_enum::WindowHandle) -> Result<(), AdapterError> {
    if crate::tree::automation::window_is_pumping(handle as isize, PUMP_PROBE_MS) {
        return Ok(());
    }
    Err(AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Target window is not processing messages, so activation would block",
    )
    .with_details(serde_json::json!({ "physical_delivery_started": false }))
    .with_suggestion("Wait for the application to recover before retrying headed input"))
}

#[cfg(not(target_os = "windows"))]
fn ensure_window_is_pumping(_handle: super::window_enum::WindowHandle) -> Result<(), AdapterError> {
    Ok(())
}

fn activation_target_is_strictly_higher(pid: agent_desktop_core::ProcessId) -> bool {
    #[cfg(all(test, target_os = "windows"))]
    if force_strictly_higher::is_active() {
        return true;
    }
    integrity_is_strictly_higher(current_process_integrity_rid(), process_integrity_rid(pid))
}

/// Pure integrity comparison for activation policy: a readable, strictly
/// higher target RID is the cross-integrity branch (A21-7). Unreadable sides
/// are best-effort, matching the input gate.
pub(crate) fn integrity_is_strictly_higher(
    caller_rid: Option<u32>,
    target_rid: Option<u32>,
) -> bool {
    matches!((caller_rid, target_rid), (Some(caller), Some(target)) if target > caller)
}

fn raise_with_budget(
    handle: super::window_enum::WindowHandle,
    evidence: &WindowIdentityEvidence<'_>,
    lease: &InteractionLease,
    strictly_higher: bool,
) -> Result<(), AdapterError> {
    for attempt in 0..FOCUS_STEAL_BUDGET {
        ensure_budget(lease.deadline())?;
        if attempt > 0 {
            std::thread::sleep(INTER_ATTEMPT_DELAY);
            ensure_budget(lease.deadline())?;
        }
        #[cfg(test)]
        attempt_probe::begin_attempt(attempt);
        bring_to_foreground(handle, evidence)?;
        if is_owned_foreground(handle, evidence) {
            return Ok(());
        }
    }
    Err(budget_exhausted(strictly_higher))
}

pub(crate) fn budget_exhausted(strictly_higher: bool) -> AdapterError {
    if strictly_higher {
        activation_elevation_denied()
    } else {
        AdapterError::new(
            ErrorCode::ActionFailed,
            "Target window did not become foreground before headed delivery",
        )
        .with_details(serde_json::json!({
            "physical_delivery_started": false,
        }))
        .with_suggestion("Retry after ensuring the target window can accept focus")
        .with_disposition(DeliverySemantics::not_delivered())
    }
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
pub(crate) fn is_owned_foreground(
    handle: super::window_enum::WindowHandle,
    evidence: &WindowIdentityEvidence<'_>,
) -> bool {
    #[cfg(all(test, target_os = "windows"))]
    if never_foreground::is_active() {
        return false;
    }
    #[cfg(target_os = "windows")]
    {
        is_foreground_window(handle) && evidence.owns_handle_now().unwrap_or(false)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (handle, evidence);
        false
    }
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
        if !owned_for_write(evidence)? {
            return Err(recycled_before_foreground());
        }
        ShowWindow(handle, SW_RESTORE);
        #[cfg(test)]
        restore_probe::record();
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
        if !owned_for_write(evidence)? {
            return Err(recycled_before_foreground());
        }
        if IsWindowVisible(handle) == 0 {
            ShowWindow(handle, SW_SHOW);
        }
        let mut target_pid = 0u32;
        let target_tid = GetWindowThreadProcessId(handle, &mut target_pid);
        if target_tid == 0 || !owned_for_write(evidence)? {
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
        let still_owned = owned_for_write(evidence).unwrap_or(false);
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
    _evidence: &WindowIdentityEvidence<'_>,
) -> Result<(), AdapterError> {
    Err(AdapterError::not_supported("focus_window"))
}

fn owned_for_write(evidence: &WindowIdentityEvidence<'_>) -> Result<bool, AdapterError> {
    #[cfg(all(test, target_os = "windows"))]
    if force_unowned_from_attempt::blocks(attempt_probe::current()) {
        return Ok(false);
    }
    evidence.owns_handle_now()
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
fn recycled_before_foreground() -> AdapterError {
    AdapterError::new(
        ErrorCode::WindowNotFound,
        "Target window handle changed ownership before foreground delivery",
    )
    .with_details(serde_json::json!({ "physical_delivery_started": false }))
    .with_disposition(DeliverySemantics::not_delivered())
}

#[cfg(all(test, target_os = "windows"))]
#[path = "window_activate_hooks.rs"]
mod hooks;

#[cfg(all(test, target_os = "windows"))]
use hooks::{
    attempt_probe, force_strictly_higher, force_unowned_from_attempt, never_foreground,
    restore_probe,
};

#[cfg(test)]
#[path = "window_activate_policy_tests.rs"]
mod policy_tests;

#[cfg(test)]
#[path = "window_activate_tests.rs"]
mod tests;
