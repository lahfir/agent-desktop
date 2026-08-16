#![allow(dead_code)]

use agent_desktop_core::{AdapterError, Deadline, ErrorCode, ProcessId};

use super::permissions::ensure_budget;
use super::process_state::hresult_from_win32;
use super::window_enum::{enumerate_top_level, window_is_responsive};

#[cfg(target_os = "windows")]
use super::listing_retry::narrow_to_permitted_codes;

/// Whether a menu is open right now for `pid`, composed from the two sources
/// A23-1 and A23-11 measured as covering every stack the corpus could stage:
/// classic `GetGUIThreadInfo` menu-mode flags, read per thread of the target
/// pid rather than the foreground thread, and a root-level UI Automation
/// child of the pid that is a tool window with a Menu/MenuBar/MenuItem
/// element reachable at or under it. Bare Menu-family reachability alone is
/// constant-true at idle on both stacks (A23-1, A23-2) and is never used by
/// itself - both sources here are compound predicates that round-trip false
/// across a real open and close.
pub(crate) fn menu_is_open(pid: ProcessId, deadline: Deadline) -> Result<bool, AdapterError> {
    ensure_budget(deadline)?;
    ensure_process_exists(pid)?;
    ensure_budget(deadline)?;
    if classic_menu_mode_active(pid, deadline)? {
        return Ok(true);
    }
    ensure_budget(deadline)?;
    uia_menu_reachable(pid, deadline)
}

#[path = "menu_state_multi.rs"]
mod multi;
pub(crate) use multi::menus_open_for;

#[cfg(target_os = "windows")]
fn process_not_found_error(pid: ProcessId) -> AdapterError {
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        "The target process could not be found",
    )
    .with_details(serde_json::json!({
        "kind": "process_not_found",
        "pid": u32::from(pid),
    }))
}

#[cfg(target_os = "windows")]
fn unresponsive_menu_probe_error() -> AdapterError {
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        "A candidate menu window is not processing messages, so a UI Automation read would block",
    )
    .with_details(serde_json::json!({ "kind": "menu_probe_window_not_pumping" }))
}

/// A pid that does not exist must not read as a false "no menu open" -
/// neither source below fails on a nonexistent pid (an empty thread set and
/// an empty candidate list are both indistinguishable from "closed"), so
/// existence is checked explicitly. `ERROR_ACCESS_DENIED` means the process
/// is alive but its handle could not be opened; neither source needs that
/// handle, so this proceeds rather than misreporting a protected-but-live
/// target as gone.
#[cfg(target_os = "windows")]
fn ensure_process_exists(pid: ProcessId) -> Result<(), AdapterError> {
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_INVALID_PARAMETER, GetLastError};
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, u32::from(pid)) };
    if !handle.is_null() {
        unsafe { CloseHandle(handle) };
        return Ok(());
    }
    if unsafe { GetLastError() } == ERROR_INVALID_PARAMETER {
        return Err(process_not_found_error(pid));
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn ensure_process_exists(_pid: ProcessId) -> Result<(), AdapterError> {
    Ok(())
}

/// Source A (A23-1): a `TH32CS_SNAPTHREAD` snapshot filtered to `pid`, with
/// `GetGUIThreadInfo` called per thread. The existing call site in
/// `key_dispatch.rs` passes `0`, which asks about the foreground thread; this
/// predicate is asked about a named pid that may not be foreground, so every
/// one of that pid's threads is read individually instead.
#[cfg(target_os = "windows")]
fn classic_menu_mode_active(pid: ProcessId, deadline: Deadline) -> Result<bool, AdapterError> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
    };

    #[cfg(test)]
    thread_snapshot_calls::record();

    let target = u32::from(pid);
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if snapshot.is_null() {
        return Err(narrow_to_permitted_codes(win32_last_error(
            "CreateToolhelp32Snapshot failed for classic menu-mode detection",
        )));
    }
    let mut entry = THREADENTRY32 {
        dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };
    let mut found = false;
    let mut first = true;
    let mut ok = unsafe { Thread32First(snapshot, &mut entry) };
    while ok != 0 {
        if !first {
            if let Err(error) = ensure_budget(deadline) {
                unsafe { CloseHandle(snapshot) };
                #[cfg(test)]
                thread_snapshot_closes::record();
                return Err(error);
            }
        }
        first = false;
        if entry.th32OwnerProcessID == target && thread_reports_menu_mode(entry.th32ThreadID) {
            found = true;
            break;
        }
        ok = unsafe { Thread32Next(snapshot, &mut entry) };
    }
    unsafe { CloseHandle(snapshot) };
    #[cfg(test)]
    thread_snapshot_closes::record();
    Ok(found)
}

#[cfg(not(target_os = "windows"))]
fn classic_menu_mode_active(_pid: ProcessId, deadline: Deadline) -> Result<bool, AdapterError> {
    ensure_budget(deadline)?;
    Ok(false)
}

/// A thread that exited between the snapshot and this read is skipped, not
/// an error: `GetGUIThreadInfo` returning failure is the same honest
/// absence-of-menu-mode signal for a gone thread and for a thread with no
/// GUI state at all (A23-10 measured a bare console process failing every
/// thread's read outright, never a zeroed success).
#[cfg(target_os = "windows")]
fn thread_reports_menu_mode(thread_id: u32) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GUI_INMENUMODE, GUI_POPUPMENUMODE, GUI_SYSTEMMENUMODE, GUITHREADINFO, GetGUIThreadInfo,
    };

    let mut info = GUITHREADINFO {
        cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };
    if unsafe { GetGUIThreadInfo(thread_id, &mut info) } == 0 {
        return false;
    }
    info.flags & (GUI_INMENUMODE | GUI_POPUPMENUMODE | GUI_SYSTEMMENUMODE) != 0
}

/// Source B (A23-11): every root-level top-level window owned by `pid` that
/// carries `WS_EX_TOOLWINDOW` (the shipped agent-facing filter already
/// excludes these, so this predicate cannot fabricate a `window-opened`
/// event) is probed for a reachable `Menu`/`MenuBar`/`MenuItem` descendant.
/// This is the compound predicate A23-11 measured round-tripping
/// false-to-true-to-false on both classic Win32 and WPF context menus; a WPF
/// menu bar dropdown is also covered, a classic menu-bar dropdown is not
/// (A23-1 covers that stack fully on its own).
#[cfg(target_os = "windows")]
fn uia_menu_reachable(pid: ProcessId, deadline: Deadline) -> Result<bool, AdapterError> {
    ensure_budget(deadline)?;
    let candidates = tool_window_candidates(pid)?;
    if candidates.is_empty() {
        return Ok(false);
    }
    let client = crate::tree::automation::automation_client().map_err(narrow_to_permitted_codes)?;
    let condition = menu_family_condition(&client)?;
    for (index, handle) in candidates.into_iter().enumerate() {
        if index > 0 {
            ensure_budget(deadline)?;
        }
        if probe_candidate(&client, &condition, handle)? {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(not(target_os = "windows"))]
fn uia_menu_reachable(_pid: ProcessId, deadline: Deadline) -> Result<bool, AdapterError> {
    ensure_budget(deadline)?;
    Ok(false)
}

#[cfg(target_os = "windows")]
fn tool_window_candidates(pid: ProcessId) -> Result<Vec<isize>, AdapterError> {
    let mut handles = Vec::new();
    enumerate_top_level(|window| {
        if super::window_identity::live_window_owner(window.handle) == Some(pid) && window.tool {
            handles.push(window.handle as isize);
        }
        true
    })
    .map_err(narrow_to_permitted_codes)?;
    Ok(handles)
}

#[cfg(target_os = "windows")]
fn menu_family_condition(
    client: &uiautomation::UIAutomation,
) -> Result<uiautomation::core::UICondition, AdapterError> {
    use uiautomation::types::{ControlType, UIProperty};
    use uiautomation::variants::Variant;

    let control_type_condition = |control: ControlType| {
        client
            .create_property_condition(UIProperty::ControlType, Variant::from(control as i32), None)
            .map_err(|error| {
                narrow_to_permitted_codes(crate::tree::automation::uia_error(
                    &error,
                    "build the menu-family search condition",
                ))
            })
    };
    let menu = control_type_condition(ControlType::Menu)?;
    let menu_bar = control_type_condition(ControlType::MenuBar)?;
    let menu_item = control_type_condition(ControlType::MenuItem)?;
    let menu_or_bar = client
        .create_or_condition(menu, menu_bar)
        .map_err(|error| {
            narrow_to_permitted_codes(crate::tree::automation::uia_error(
                &error,
                "combine the menu-family search condition",
            ))
        })?;
    client
        .create_or_condition(menu_or_bar, menu_item)
        .map_err(|error| {
            narrow_to_permitted_codes(crate::tree::automation::uia_error(
                &error,
                "combine the menu-family search condition",
            ))
        })
}

/// Gated on [`window_is_responsive`] before any cross-process UI Automation
/// call: a target that stopped dispatching would otherwise block a read this
/// predicate's callers cannot rescue - `wait_for_menu` makes one call with no
/// retry of its own, so an in-flight block has no backstop at any layer.
#[cfg(target_os = "windows")]
fn probe_candidate(
    client: &uiautomation::UIAutomation,
    condition: &uiautomation::core::UICondition,
    handle: isize,
) -> Result<bool, AdapterError> {
    use uiautomation::types::{Handle, TreeScope};

    if !crate::tree::automation::window_exists(handle) {
        return Ok(false);
    }
    if !window_is_responsive(handle as *mut core::ffi::c_void) {
        return Err(unresponsive_menu_probe_error());
    }
    let element = match client.element_from_handle(Handle::from(handle)) {
        Ok(element) => element,
        Err(error) => return probe_step(&error, "resolve a candidate menu window"),
    };
    match element.find_first(TreeScope::Subtree, condition) {
        Ok(_) => Ok(true),
        Err(error) => probe_step(
            &error,
            "search a candidate window for a reachable menu element",
        ),
    }
}

/// A vanished candidate - the window closed between discovery and this read,
/// the ordinary close race at a 50ms poll interval - is skipped rather than
/// reported, mirroring source A's own exited-thread skip. `is_exhaustion`
/// covers `find_first` finding nothing; the element-not-available family
/// covers the window itself having gone away mid-read.
#[cfg(target_os = "windows")]
fn probe_step(error: &uiautomation::Error, context: &str) -> Result<bool, AdapterError> {
    let failure = crate::tree::automation::failure_of(error);
    if failure.is_exhaustion() || is_vanished(failure) {
        return Ok(false);
    }
    Err(narrow_to_permitted_codes(
        crate::tree::automation::uia_failure_error(failure, context),
    ))
}

#[cfg(target_os = "windows")]
fn is_vanished(failure: crate::tree::automation::UiaFailure) -> bool {
    use crate::tree::automation::{ERR_NOTFOUND, UiaFailure};

    matches!(
        failure,
        UiaFailure::Hresult(super::hresult::UIA_E_ELEMENTNOTAVAILABLE)
            | UiaFailure::Sentinel(ERR_NOTFOUND)
    )
}

/// Win32 `GetLastError` -> `HRESULT_FROM_WIN32` into the shared HRESULT
/// table, the convention `launch.rs`/`process_state.rs`/`window_op.rs`
/// already use for their own native failures.
#[cfg(target_os = "windows")]
fn win32_last_error(message: &str) -> AdapterError {
    let error = unsafe { windows_sys::Win32::Foundation::GetLastError() };
    adapter_error_from_win32(error, message)
}

#[cfg(target_os = "windows")]
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

/// Counts snapshot handle closes so a test can prove every opened ToolHelp
/// snapshot is closed on every exit path. The deadline check inside the walk
/// returns early, and an early return that skips the close leaks a handle per
/// expired capture on a loop that polls every 200ms.
#[cfg(test)]
pub(super) mod thread_snapshot_closes {
    use std::cell::Cell;

    thread_local! {
        static COUNT: Cell<usize> = const { Cell::new(0) };
    }

    pub(super) fn record() {
        COUNT.with(|cell| cell.set(cell.get() + 1));
    }

    pub(super) fn take() -> usize {
        COUNT.with(|cell| {
            let value = cell.get();
            cell.set(0);
            value
        })
    }
}

#[cfg(test)]
pub(super) mod thread_snapshot_calls {
    use std::cell::Cell;

    thread_local! {
        static COUNT: Cell<usize> = const { Cell::new(0) };
    }

    pub(super) fn record() {
        COUNT.with(|cell| cell.set(cell.get() + 1));
    }

    pub(super) fn take() -> usize {
        COUNT.with(|cell| {
            let value = cell.get();
            cell.set(0);
            value
        })
    }
}

#[cfg(test)]
#[path = "menu_state_tests.rs"]
mod tests;
