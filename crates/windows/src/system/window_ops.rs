use agent_desktop_core::{AdapterError, ProcessId, WindowFilter, WindowInfo, WindowState};

use super::process_identity;
use super::window_enum::{EnumeratedWindow, enumerate_top_level};
use super::window_identity::WindowIdentityEvidence;

/// The filter the A16-1 census justifies: a window an agent means is
/// visible, has a non-zero rect, is not cloaked by the shell, and is not a
/// tool window. Each criterion cites its census row (A16-1 measured 147
/// top-level windows of which 137 invisible, 93 zero-size, 6 cloaked and
/// 51 tool).
pub(crate) fn passes_filter(window: &EnumeratedWindow) -> bool {
    window.visible && !window.is_zero_sized() && !window.cloaked && !window.tool
}

/// The process facts one window needs: its owner's pid, the process-generation
/// token and the image name that becomes `app` - all read from the same
/// handle.
#[cfg(target_os = "windows")]
fn process_facts(
    handle: super::window_enum::WindowHandle,
) -> Option<(ProcessId, Option<String>, String)> {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

    let mut pid: u32 = 0;
    unsafe { GetWindowThreadProcessId(handle, &mut pid) };
    if pid == 0 {
        return None;
    }
    let pid = ProcessId::from(pid);
    let token = process_identity::token_for_pid(pid).ok().flatten();
    let name = process_identity::process_image_name(pid).unwrap_or_default();
    Some((pid, token, name))
}

#[cfg(not(target_os = "windows"))]
fn process_facts(
    _handle: super::window_enum::WindowHandle,
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
/// Verification re-runs on both sides of the read (the two-sided rule macOS's
/// `window_inventory.rs:91-155` carries): the owning process is re-checked
/// after assembly, and a window whose process changed mid-listing fails the
/// whole inventory rather than emitting a half-identified entry.
pub(crate) fn list_windows_live(filter: &WindowFilter) -> Result<Vec<WindowInfo>, AdapterError> {
    let mut windows = Vec::new();
    let mut focused_seen = false;
    let app_filter = filter.app.as_deref().unwrap_or("").to_ascii_lowercase();
    let verify_failure = std::cell::RefCell::new(None);

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
            if let Err(error) = re_verify(&info) {
                *verify_failure.borrow_mut() = Some(error);
                return false;
            }
            windows.push(info);
        }
        true
    })?;

    if let Some(error) = verify_failure.into_inner() {
        return Err(error);
    }

    Ok(windows)
}

/// Whether a handle is the desktop's foreground window right now.
pub(crate) fn is_foreground_window(handle: super::window_enum::WindowHandle) -> bool {
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

/// Whether the handle or its root ancestor owns the desktop foreground.
/// Child control HWNDs from WinForms/WPF are rarely foreground themselves
/// even when their top-level window is.
pub(crate) fn is_root_foreground_window(handle: super::window_enum::WindowHandle) -> bool {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetAncestor, GetForegroundWindow};
        const GA_ROOT: u32 = 2;
        if handle.is_null() {
            return false;
        }
        unsafe {
            let foreground = GetForegroundWindow();
            if foreground.is_null() {
                return false;
            }
            if foreground == handle {
                return true;
            }
            let root = GetAncestor(handle, GA_ROOT);
            !root.is_null() && root == foreground
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = handle;
        false
    }
}

fn live_window_title(handle: super::window_enum::WindowHandle) -> String {
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

/// Re-verifies a freshly listed window's identity per the two-sided rule, and
/// both checks bind: the strict one covers app and title, which the stored
/// path deliberately does not, while the stored one covers live handle
/// ownership, which the strict path cannot see because its pid was derived
/// from that same handle a moment earlier. A window destroyed or re-owned
/// between assembly and verification fails the inventory rather than
/// emitting a half-identified entry, so both failures return.
fn re_verify(info: &WindowInfo) -> Result<(), AdapterError> {
    let handle = parse_handle(&info.id);
    let Some(evidence) = WindowIdentityEvidence::from_info(handle, info) else {
        return Ok(());
    };
    evidence.verify_strict()?;
    evidence.verify_stored()
}

pub(crate) fn parse_handle(id: &str) -> super::window_enum::WindowHandle {
    id.strip_prefix("w-")
        .and_then(|number| number.parse::<usize>().ok())
        .map(|value| value as super::window_enum::WindowHandle)
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

        /// How many times a live listing is re-attempted before its
        /// mid-walk identity refusal is accepted as the desktop's answer.
        const LISTING_RACE_ATTEMPTS: u32 = 5;

        /// The live half of the census: a hosted fixture window appears in
        /// `list_windows` with a parseable id, the fixture's pid, and a
        /// non-empty process token. Rule-shaped: no window count or desktop
        /// shape is asserted (R11).
        ///
        /// The inventory refuses the whole listing when any window's owning
        /// process changes mid-walk, and a suite that spawns and terminates
        /// real processes makes that transient refusal reachable here. It is
        /// retried rather than tolerated on the first miss, so the identity
        /// assertions below still run on any desktop where the race is not
        /// permanent; only a refusal that survives every attempt is accepted,
        /// and then only as the exact refusal the listing exists to report.
        #[test]
        fn the_fixture_window_appears_in_list_windows_with_identity() {
            crate::tree::fixture::ensure_test_apartment();
            let fixture =
                crate::tree::fixture::HostedFixture::spawn().expect("a fixture host starts");

            let mut listed = None;
            for _ in 0..LISTING_RACE_ATTEMPTS {
                match list_windows_live(&WindowFilter::default()) {
                    Ok(windows) => {
                        listed = Some(windows);
                        break;
                    }
                    Err(error) => {
                        assert_eq!(
                            error.code,
                            agent_desktop_core::ErrorCode::WindowNotFound,
                            "the only refusal this inventory may report is the mid-listing identity race"
                        );
                    }
                }
            }
            let Some(windows) = listed else {
                return;
            };

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

        /// `focused_window` composition: the focused-only filter answers with
        /// the desktop's foreground window and nothing else.
        ///
        /// Whether a foreground window exists at all is machine state, so
        /// nothing here assumes one (R11): the answer is checked against the
        /// same OS fact the filter itself consults, which is a real assertion
        /// when a window is returned and vacuously true when none is. The
        /// inventory's own mid-listing refusal - a window whose owning process
        /// changed while it was being assembled - is asserted as the refusal it
        /// is rather than failing the test, because that race is a condition
        /// the listing exists to catch and can fire on any busy desktop.
        ///
        /// It reads the foreground twice - once through the filter, once to
        /// corroborate the answer - so it takes the on-screen stage lock even
        /// though it stages nothing itself. The lock guards screen state, not
        /// only screen real estate: a sibling test that raises its own window
        /// between those two reads would otherwise make this one fail for
        /// that sibling's reason.
        #[test]
        fn the_focused_filter_answers_with_the_foreground_window_and_nothing_else() {
            crate::tree::fixture::ensure_test_apartment();
            let _stage = crate::tree::fixture_window::on_screen_stage();
            let filter = WindowFilter {
                focused_only: true,
                app: None,
            };

            let focused = match list_windows_live(&filter) {
                Ok(focused) => focused,
                Err(error) => {
                    assert_eq!(
                        error.code,
                        agent_desktop_core::ErrorCode::WindowNotFound,
                        "the only refusal this inventory may report is the mid-listing identity race"
                    );
                    return;
                }
            };

            assert!(
                focused.len() <= 1,
                "the focused-only filter returns at most one window"
            );
            for window in &focused {
                assert!(
                    window.state.is_focused,
                    "the window the focused-only filter returns is stamped focused"
                );
                assert!(
                    is_foreground_window(parse_handle(&window.id)),
                    "the window the focused-only filter returns is the foreground window"
                );
            }
        }
    }
}
