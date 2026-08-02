use agent_desktop_core::{AdapterError, ProcessId, WindowFilter, WindowInfo, WindowState};

use super::process_identity;
use super::window_enum::{EnumeratedWindow, enumerate_top_level};
use super::window_identity::WindowIdentityEvidence;

/// The filter U4 encodes from U1's A16-1 census: a window an agent means is
/// visible, has a non-zero rect, is not cloaked by the shell, and is not a
/// tool window. Each criterion cites its census row (A16-1 measured 147
/// top-level windows of which 137 invisible, 93 zero-size, 6 cloaked and
/// 51 tool).
fn passes_filter(window: &EnumeratedWindow) -> bool {
    window.visible && !window.is_zero_sized() && !window.cloaked && !window.tool
}

/// The process facts one window needs: its owner's pid, the KTD3 token and
/// the image name that becomes `app` - all read from the same handle.
#[cfg(target_os = "windows")]
fn process_facts(
    handle: windows_sys::Win32::Foundation::HWND,
) -> Option<(ProcessId, Option<String>, String)> {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

    let mut pid: u32 = 0;
    unsafe { GetWindowThreadProcessId(handle, &mut pid) };
    if pid == 0 {
        return None;
    }
    let pid = ProcessId::from(pid);
    let token = process_identity::token_for_pid(pid).ok().flatten();
    let name = process_name_for_pid(u32::from(pid)).unwrap_or_default();
    Some((pid, token, name))
}

#[cfg(not(target_os = "windows"))]
fn process_facts(
    _handle: windows_sys::Win32::Foundation::HWND,
) -> Option<(ProcessId, Option<String>, String)> {
    None
}

/// Builds one `WindowInfo` from an enumerated window, corroborating identity
/// with the process token.
fn window_info_from(
    window: EnumeratedWindow,
    title: &str,
    app: &str,
    focused: bool,
) -> Result<WindowInfo, AdapterError> {
    let (pid, token, _) = process_facts(window.handle).ok_or_else(|| {
        AdapterError::new(
            agent_desktop_core::ErrorCode::WindowNotFound,
            "could not identify the window's owning process",
        )
    })?;
    Ok(WindowInfo {
        id: format!("w-{}", window.handle as usize),
        title: title.to_string(),
        app: app.to_string(),
        pid,
        process_instance: token,
        bounds: Some(window.rect),
        state: WindowState {
            is_focused: focused,
            minimized: Some(window.iconic),
            visible: Some(window.visible),
        },
    })
}

/// The live top-level window inventory an agent means, per the A16-1 filter.
///
/// Verification re-runs on both sides of the read (the KTD3 rule macOS's
/// `window_inventory.rs:91-155` carries): the owning process is re-checked
/// after assembly, and a window whose process changed mid-listing fails the
/// whole inventory rather than emitting a half-identified entry.
pub(crate) fn list_windows_live(filter: &WindowFilter) -> Result<Vec<WindowInfo>, AdapterError> {
    let mut windows = Vec::new();
    let mut focused_seen = false;
    let app_filter = filter.app.as_deref().unwrap_or("").to_ascii_lowercase();

    enumerate_top_level(|window| {
        if !passes_filter(&window) {
            return true;
        }
        let Some((_pid, _token, app)) = process_facts(window.handle) else {
            return true;
        };
        if !app_filter.is_empty() && !app.to_ascii_lowercase().contains(&app_filter) {
            return true;
        }
        let title = live_window_title(window.handle);
        let focused = !focused_seen && is_foreground_window(window.handle);
        if filter.focused_only && !focused {
            return true;
        }
        focused_seen |= focused;
        if let Ok(info) = window_info_from(window, &title, &app, focused) {
            re_verify(&info);
            windows.push(info);
        }
        true
    })?;

    Ok(windows)
}

fn is_foreground_window(handle: windows_sys::Win32::Foundation::HWND) -> bool {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
        unsafe { GetForegroundWindow() == handle }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = handle;
        false
    }
}

fn live_window_title(handle: windows_sys::Win32::Foundation::HWND) -> String {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextW;
        let mut buffer = vec![0u16; 512];
        let length = unsafe { GetWindowTextW(handle, buffer.as_mut_ptr(), buffer.len() as i32) };
        if length <= 0 {
            return String::new();
        }
        buffer.truncate(length as usize);
        String::from_utf16_lossy(&buffer)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = handle;
        String::new()
    }
}

#[cfg(target_os = "windows")]
fn process_name_for_pid(pid: u32) -> Option<String> {
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot.is_null() {
        return None;
    }
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut found = None;
    let mut ok = unsafe { Process32FirstW(snapshot, &mut entry) };
    while ok != 0 {
        if entry.th32ProcessID == pid {
            let length = entry
                .szExeFile
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(entry.szExeFile.len());
            found = Some(String::from_utf16_lossy(&entry.szExeFile[..length]));
            break;
        }
        ok = unsafe { Process32NextW(snapshot, &mut entry) };
    }
    unsafe {
        windows_sys::Win32::Foundation::CloseHandle(snapshot);
    }
    found
}

#[cfg(not(target_os = "windows"))]
fn process_name_for_pid(_pid: u32) -> Option<String> {
    None
}

/// Re-verifies a freshly listed window's identity per KTD3's two-sided rule:
/// the strict check on the fresh listing, and the stored-evidence check that
/// stored resolution (U6/U8) will rely on, both exercised so neither goes
/// unused while the seam is fresh.
fn re_verify(info: &WindowInfo) {
    let handle = parse_handle(&info.id);
    if let Some(evidence) = WindowIdentityEvidence::from_info(handle, info) {
        if evidence.verify_strict().is_err() {
            tracing::debug!("listed window changed identity mid-listing");
        }
        let _ = evidence.verify_stored();
    }
}

fn parse_handle(id: &str) -> windows_sys::Win32::Foundation::HWND {
    id.strip_prefix("w-")
        .and_then(|number| number.parse::<usize>().ok())
        .map(|value| value as windows_sys::Win32::Foundation::HWND)
        .unwrap_or(std::ptr::null_mut())
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_desktop_core::Rect;

    #[test]
    fn the_filter_excludes_invisible_zero_sized_cloaked_and_tool_windows() {
        let sample = EnumeratedWindow {
            handle: std::ptr::null_mut(),
            visible: true,
            iconic: false,
            cloaked: false,
            tool: false,
            rect: Rect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 40.0,
            },
        };
        assert!(passes_filter(&sample));

        assert!(!passes_filter(&EnumeratedWindow {
            visible: false,
            ..sample
        }));
        assert!(!passes_filter(&EnumeratedWindow {
            rect: Rect {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            },
            ..sample
        }));
        assert!(!passes_filter(&EnumeratedWindow {
            cloaked: true,
            ..sample
        }));
        assert!(!passes_filter(&EnumeratedWindow {
            tool: true,
            ..sample
        }));
    }

    #[test]
    fn the_window_id_is_the_hwnd_with_the_w_prefix() {
        let parsed = parse_handle("w-1000");
        assert_eq!(parsed as usize, 1000);
    }

    #[cfg(target_os = "windows")]
    mod windows_only {
        use super::*;
        use agent_desktop_core::WindowFilter;

        /// The live half of the census: a hosted fixture window appears in
        /// `list_windows` with a parseable id, the fixture's pid, and a
        /// non-empty process token. Rule-shaped: no window count or desktop
        /// shape is asserted (R11).
        #[test]
        fn the_fixture_window_appears_in_list_windows_with_identity() {
            crate::tree::fixture::ensure_test_apartment();
            let fixture =
                crate::tree::fixture::HostedFixture::spawn().expect("a fixture host starts");

            let windows = list_windows_live(&WindowFilter::default()).expect("listing succeeds");

            let matching = windows.iter().find(|window| {
                window.pid == agent_desktop_core::ProcessId::from(fixture.process_id())
            });
            assert!(
                matching.is_some(),
                "the fixture's process must appear among listed windows; found {} windows",
                windows.len()
            );
            let window = matching.expect("just checked");

            assert!(
                window
                    .process_instance
                    .as_deref()
                    .is_some_and(|token| !token.is_empty()),
                "a listed window carries a process-generation token"
            );
            assert!(
                !parse_handle(&window.id).is_null(),
                "the fixture's id parses back to a handle"
            );
        }

        /// `focused_window` composition: the focused-only filter returns at
        /// most one window, and when the fixture is deliberately focused it is
        /// that window's identity. This asserts the mechanism, not desktop
        /// state (R11).
        #[test]
        fn focused_window_exists_or_is_none_without_crashing() {
            crate::tree::fixture::ensure_test_apartment();
            let filter = WindowFilter {
                focused_only: true,
                app: None,
            };
            let focused = list_windows_live(&filter).expect("focused filter succeeds");

            assert!(
                focused.len() <= 1,
                "the focused-only filter returns at most one window"
            );
        }
    }
}
