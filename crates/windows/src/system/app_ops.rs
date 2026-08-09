use agent_desktop_core::{AdapterError, AppInfo, ProcessId};

use super::process_identity;
use super::window_enum::{EnumeratedWindow, enumerate_top_level};
use super::window_ops::passes_filter;

const PROTECTED_PROCESSES: &[&str] = &[
    "csrss.exe",
    "wininit.exe",
    "winlogon.exe",
    "services.exe",
    "lsass.exe",
    "smss.exe",
    "lsaiso.exe",
    "dwm.exe",
    "explorer.exe",
];

/// Exact case-insensitive `.exe` image-name match against the session- and
/// shell-critical set. Near-misses (`iexplore.exe`, `explorer++.exe`, a name
/// merely containing `lsass`) are deliberately not protected.
pub(crate) fn is_protected_process(identifier: &str) -> bool {
    let image = identifier.rsplit(['\\', '/']).next().unwrap_or(identifier);
    PROTECTED_PROCESSES
        .iter()
        .any(|protected| image.eq_ignore_ascii_case(protected))
}

/// A process snapshot row: the image name and the identity token for one pid.
///
/// `list_apps` joins the window inventory's owning processes with this
/// snapshot so the two inventories corroborate each other on
/// `process_instance` (mirroring
/// `crates/macos/src/system/app_inventory.rs:133-146` where two sources must
/// agree or the inventory fails).
#[derive(Debug, Clone)]
pub(crate) struct ProcessRow {
    pub(crate) pid: ProcessId,
    pub(crate) name: String,
}

/// Snapshots every running process's image name from the ToolHelp table.
#[cfg(target_os = "windows")]
pub(crate) fn process_snapshot() -> Result<Vec<ProcessRow>, AdapterError> {
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot.is_null() {
        return Err(AdapterError::internal(
            "CreateToolhelp32Snapshot failed to enumerate processes",
        ));
    }
    let mut rows = Vec::new();
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut ok = unsafe { Process32FirstW(snapshot, &mut entry) };
    while ok != 0 {
        let length = entry
            .szExeFile
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(entry.szExeFile.len());
        rows.push(ProcessRow {
            pid: ProcessId::from(entry.th32ProcessID),
            name: String::from_utf16_lossy(&entry.szExeFile[..length]),
        });
        ok = unsafe { Process32NextW(snapshot, &mut entry) };
    }
    unsafe {
        windows_sys::Win32::Foundation::CloseHandle(snapshot);
    }
    Ok(rows)
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn process_snapshot() -> Result<Vec<ProcessRow>, AdapterError> {
    Ok(Vec::new())
}

/// The owning processes of the agent-facing window set - the "apps with a
/// window" population `list_apps` reports.
#[cfg(target_os = "windows")]
fn owning_processes() -> Result<Vec<EnumeratedWindow>, AdapterError> {
    let mut owners = Vec::new();
    enumerate_top_level(|window| {
        if passes_filter(&window) {
            owners.push(window);
        }
        true
    })?;
    Ok(owners)
}

#[cfg(not(target_os = "windows"))]
fn owning_processes() -> Result<Vec<EnumeratedWindow>, AdapterError> {
    Ok(Vec::new())
}

/// The live `list_apps` inventory.
///
/// Every listed app owns at least one agent-facing window (the A16-1 filter)
/// and corroborates its identity against the process snapshot: a pid present
/// in the window inventory but absent from the process snapshot fails the
/// inventory rather than emitting a half-identified app. The process
/// generation the inventory captured is re-read at assembly time and compared,
/// so a mid-listing generation change also fails the inventory. `bundle_id`
/// has no Windows analogue and is recorded, not faked.
pub(crate) fn list_apps_live() -> Result<Vec<AppInfo>, AdapterError> {
    let owners = owning_processes()?;
    let snapshot = process_snapshot()?;
    let snapshot_by_pid: std::collections::HashMap<u32, &ProcessRow> = snapshot
        .iter()
        .map(|row| (u32::from(row.pid), row))
        .collect();

    let mut seen = std::collections::HashSet::new();
    let mut apps = Vec::new();
    for window in owners {
        let Some((pid, token)) = process_token_of(window.handle) else {
            continue;
        };
        if !seen.insert(pid) {
            continue;
        }
        let Some(row) = snapshot_by_pid.get(&u32::from(pid)) else {
            return Err(AdapterError::internal(
                "a window-owning process is absent from the process snapshot",
            ));
        };
        if let Some(window_token) = token.as_deref() {
            let fresh = process_identity::token_for_pid(pid).ok().flatten();
            if fresh.as_deref() != Some(window_token) {
                return Err(AdapterError::internal(
                    "window and process identities disagree for an app",
                ));
            }
        }
        apps.push(AppInfo {
            name: row.name.clone(),
            pid,
            bundle_id: None,
            process_instance: token,
        });
    }
    apps.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(apps)
}

/// The owner pid + token for a window, reusing the identity path.
fn process_token_of(
    handle: super::window_enum::WindowHandle,
) -> Option<(ProcessId, Option<String>)> {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

        let mut pid: u32 = 0;
        unsafe { GetWindowThreadProcessId(handle, &mut pid) };
        if pid == 0 {
            return None;
        }
        let pid = ProcessId::from(pid);
        let token = process_identity::token_for_pid(pid).ok().flatten();
        Some((pid, token))
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = handle;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "windows")]
    #[test]
    fn process_snapshot_lists_the_current_process() {
        let rows = process_snapshot().expect("the snapshot enumerates");

        assert!(
            rows.iter()
                .any(|row| row.pid == ProcessId::from(std::process::id())),
            "the current process must be in its own snapshot"
        );
        assert!(
            rows.iter()
                .find(|row| row.pid == ProcessId::from(std::process::id()))
                .is_some_and(|row| !row.name.is_empty()),
            "the current process has a non-empty image name"
        );
    }

    #[test]
    fn protected_list_members_match_exactly_case_insensitively() {
        for name in PROTECTED_PROCESSES {
            assert!(is_protected_process(name), "{name} must be protected");
            assert!(
                is_protected_process(&name.to_ascii_uppercase()),
                "{name} must match case-insensitively"
            );
        }
        assert!(is_protected_process("explorer.exe"));
        assert!(is_protected_process(r"C:\Windows\explorer.exe"));
    }

    #[test]
    fn near_miss_image_names_are_not_protected() {
        assert!(!is_protected_process("iexplore.exe"));
        assert!(!is_protected_process("explorer++.exe"));
        assert!(!is_protected_process("notepad.exe"));
        assert!(!is_protected_process("my-lsass-helper.exe"));
        assert!(!is_protected_process("lsass"));
        assert!(!is_protected_process("lsass.exe.bak"));
    }
}
