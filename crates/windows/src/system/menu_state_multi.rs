use std::collections::HashMap;

use agent_desktop_core::{AdapterError, Deadline, ProcessId};

use super::ensure_budget;

/// [`super::menu_is_open`] for every pid in `pids`, sharing one classic-source
/// `TH32CS_SNAPTHREAD` snapshot across all of them instead of the one
/// snapshot per pid that calling [`super::menu_is_open`] in a loop would
/// take. The snapshot enumerates every thread on the desktop regardless of
/// which pid is asked about, so a multi-pid caller pays for that walk once.
/// Each pid still gets its own [`super::uia_menu_reachable`] probe when the
/// shared classic read did not already resolve it true, and every per-pid
/// result is identical to what calling [`super::menu_is_open`] on that pid
/// alone would have produced.
#[cfg(target_os = "windows")]
pub(crate) fn menus_open_for(
    pids: &[ProcessId],
    deadline: Deadline,
) -> Result<HashMap<ProcessId, bool>, AdapterError> {
    ensure_budget(deadline)?;
    for &pid in pids {
        super::ensure_process_exists(pid)?;
    }
    ensure_budget(deadline)?;
    let mut results = classic_menu_mode_for_many(pids, deadline)?;
    for &pid in pids {
        if results.get(&pid).copied().unwrap_or(false) {
            continue;
        }
        ensure_budget(deadline)?;
        let reachable = super::uia_menu_reachable(pid, deadline)?;
        results.insert(pid, reachable);
    }
    Ok(results)
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn menus_open_for(
    pids: &[ProcessId],
    deadline: Deadline,
) -> Result<HashMap<ProcessId, bool>, AdapterError> {
    ensure_budget(deadline)?;
    Ok(pids.iter().map(|&pid| (pid, false)).collect())
}

/// The multi-pid form of the classic source: one `TH32CS_SNAPTHREAD`
/// snapshot, walked once, with each thread's owner tested against every pid
/// in `pids` rather than one snapshot walk per pid. `GetGUIThreadInfo` is
/// still read per thread through the same
/// [`super::thread_reports_menu_mode`] the single-pid source uses; only the
/// snapshot enumeration is shared.
#[cfg(target_os = "windows")]
fn classic_menu_mode_for_many(
    pids: &[ProcessId],
    deadline: Deadline,
) -> Result<HashMap<ProcessId, bool>, AdapterError> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
    };

    #[cfg(test)]
    super::thread_snapshot_calls::record();

    let mut found: HashMap<ProcessId, bool> = pids.iter().map(|&pid| (pid, false)).collect();
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if snapshot.is_null() {
        return Err(super::narrow_to_permitted_codes(super::win32_last_error(
            "CreateToolhelp32Snapshot failed for classic menu-mode detection",
        )));
    }
    let mut entry = THREADENTRY32 {
        dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };
    let mut first = true;
    let mut ok = unsafe { Thread32First(snapshot, &mut entry) };
    while ok != 0 {
        if !first {
            if let Err(error) = ensure_budget(deadline) {
                unsafe { CloseHandle(snapshot) };
                return Err(error);
            }
        }
        first = false;
        if let Some(slot) = found.get_mut(&ProcessId::from(entry.th32OwnerProcessID)) {
            if !*slot && super::thread_reports_menu_mode(entry.th32ThreadID) {
                *slot = true;
            }
        }
        ok = unsafe { Thread32Next(snapshot, &mut entry) };
    }
    unsafe { CloseHandle(snapshot) };
    Ok(found)
}
