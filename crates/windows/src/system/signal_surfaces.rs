#![allow(dead_code)]

use std::collections::HashMap;

use agent_desktop_core::{
    AdapterError, AppInfo, Deadline, ErrorCode, ProcessId, SnapshotSurface, SurfaceSignal,
};

use super::menu_state::menu_is_open;
use super::permissions::ensure_budget;
use super::window_enum::{WindowHandle, enumerate_top_level};
use super::window_ops::passes_filter;
use crate::tree::automation::root_from_hwnd;
use crate::tree::surfaces::window_is_modal_sheet;

/// The identity a surface needs to be reported: the pieces of an already
/// filtered [`AppInfo`] that a `SurfaceSignal` actually carries, borrowed
/// rather than cloned until a surface is actually found for it.
struct SurfaceTarget<'a> {
    pid: ProcessId,
    name: &'a str,
    instance: &'a str,
}

/// `SurfaceSignal`s for every process named in `apps`, scoped to exactly
/// those processes.
///
/// Core rejects `surface-appeared` / `surface-dismissed` without `--app`
/// (`wait_mode.rs:168-177`), so this is only ever called with a filtered,
/// non-empty app population — never a desktop-wide walk. Each app in `apps`
/// contributes at most one `Menu` signal (the predicate is a per-process
/// boolean) and zero or more `Sheet` signals (one per modal owned window).
///
/// An app whose `process_instance` is `None` is skipped rather than
/// producing a surface with a fabricated instance: `signal_inventory`
/// already excludes such entries, so this is a defensive backstop, not the
/// primary enforcement of that rule.
pub(crate) fn app_scoped_surfaces(
    apps: &[AppInfo],
    deadline: Deadline,
) -> Result<Vec<SurfaceSignal>, AdapterError> {
    ensure_budget(deadline)?;
    let targets = identified_targets(apps);
    if targets.is_empty() {
        return Ok(Vec::new());
    }
    let mut surfaces = sheet_surfaces(&targets, deadline)?;
    for (index, target) in targets.values().enumerate() {
        if index > 0 {
            ensure_budget(deadline)?;
        }
        if menu_is_open(target.pid, deadline)? {
            surfaces.push(menu_signal(target));
        }
    }
    Ok(surfaces)
}

fn identified_targets(apps: &[AppInfo]) -> HashMap<ProcessId, SurfaceTarget<'_>> {
    apps.iter()
        .filter_map(|app| {
            let instance = app.process_instance.as_deref()?;
            Some((
                app.pid,
                SurfaceTarget {
                    pid: app.pid,
                    name: app.name.as_str(),
                    instance,
                },
            ))
        })
        .collect()
}

fn menu_signal(target: &SurfaceTarget<'_>) -> SurfaceSignal {
    SurfaceSignal {
        kind: SnapshotSurface::Menu,
        app: target.name.to_string(),
        pid: target.pid,
        process_instance: target.instance.to_string(),
        id: format!("menu-{}", u32::from(target.pid)),
        title: None,
    }
}

fn sheet_signal(target: &SurfaceTarget<'_>, handle: WindowHandle) -> SurfaceSignal {
    SurfaceSignal {
        kind: SnapshotSurface::Sheet,
        app: target.name.to_string(),
        pid: target.pid,
        process_instance: target.instance.to_string(),
        id: format!("w-{}", handle as usize),
        title: None,
    }
}

/// One top-level scan, shared across every target pid in `targets`, so a
/// filter naming several processes still costs a single `EnumWindows` walk
/// rather than one per process.
///
/// A candidate window is admitted the same way the ordinary window inventory
/// admits one (`passes_filter`) plus carrying an owner — the "owned window"
/// half of the measured modal signature. `window_is_modal_sheet` supplies the
/// other half, so the signal path and `snapshot --surface sheet` agree about
/// what a sheet is.
#[cfg(target_os = "windows")]
fn sheet_surfaces(
    targets: &HashMap<ProcessId, SurfaceTarget<'_>>,
    deadline: Deadline,
) -> Result<Vec<SurfaceSignal>, AdapterError> {
    ensure_budget(deadline)?;
    let mut candidates = Vec::new();
    enumerate_top_level(|window| {
        if passes_filter(&window) && has_owner(window.handle) {
            candidates.push(window.handle);
        }
        true
    })
    .map_err(narrow_to_closed_set)?;

    let mut surfaces = Vec::new();
    for (index, handle) in candidates.into_iter().enumerate() {
        if index > 0 {
            ensure_budget(deadline)?;
        }
        let Some(pid) = owning_pid(handle) else {
            continue;
        };
        let Some(target) = targets.get(&pid) else {
            continue;
        };
        match root_from_hwnd(handle as isize, deadline) {
            Ok(root) if window_is_modal_sheet(&root, false) => {
                surfaces.push(sheet_signal(target, handle));
            }
            Ok(_) => {}
            Err(error) if error.code == ErrorCode::WindowNotFound => {}
            Err(error) => return Err(narrow_to_closed_set(error)),
        }
    }
    Ok(surfaces)
}

#[cfg(not(target_os = "windows"))]
fn sheet_surfaces(
    _targets: &HashMap<ProcessId, SurfaceTarget<'_>>,
    deadline: Deadline,
) -> Result<Vec<SurfaceSignal>, AdapterError> {
    ensure_budget(deadline)?;
    Ok(Vec::new())
}

#[cfg(target_os = "windows")]
fn owning_pid(handle: WindowHandle) -> Option<ProcessId> {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

    let mut pid: u32 = 0;
    unsafe { GetWindowThreadProcessId(handle, &mut pid) };
    if pid == 0 {
        None
    } else {
        Some(ProcessId::from(pid))
    }
}

#[cfg(target_os = "windows")]
fn has_owner(handle: WindowHandle) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GET_WINDOW_CMD, GW_OWNER, GetWindow};

    let command: GET_WINDOW_CMD = GW_OWNER;
    !unsafe { GetWindow(handle, command) }.is_null()
}

/// Narrows any surface-read failure onto the two codes a filtered capture
/// may return: `TIMEOUT` passes through unchanged, everything else -
/// `EnumWindows` failing outright, or a UI Automation read other than the
/// vanished-window and non-pumping cases already handled by their own
/// callers - becomes `APP_UNRESPONSIVE`, the code the Error and Disposition
/// table assigns to "a surface read was blocked."
fn narrow_to_closed_set(mut error: AdapterError) -> AdapterError {
    if error.code != ErrorCode::Timeout {
        error.code = ErrorCode::AppUnresponsive;
    }
    error
}

#[cfg(test)]
#[path = "signal_surfaces_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "wait_surface_live_tests.rs"]
mod live_surface_tests;
