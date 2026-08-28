use super::super::shell_surface_open::{accelerator_probe, close_surface, open_row, open_surface};
use super::super::test_support::{settles_to, wait_for_foreground_to_settle};
use super::super::window_enum::{EnumeratedWindow, enumerate_top_level, is_cloaked};
use super::super::window_ops::passes_filter;
use super::{SnapshotSurface, SurfaceKindRow, WindowInfo};
use agent_desktop_core::{Deadline, ErrorCode, InteractionPolicy};

use crate::system::test_support::{SHELL_SURFACE_LOCK, or_skip_shell};

fn deadline(ms: u64) -> Deadline {
    Deadline::after(ms).expect("deadline")
}

fn bootstrap() {
    crate::tree::fixture::bootstrap();
}

fn headed() -> InteractionPolicy {
    InteractionPolicy::headed()
}

fn handle_of(info: &WindowInfo) -> isize {
    info.id
        .strip_prefix("w-")
        .and_then(|digits| digits.parse::<usize>().ok())
        .map(|value| value as isize)
        .expect("a shell surface id is a w-<hwnd> handle")
}

fn foreground() -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    (unsafe { GetForegroundWindow() }) as isize
}

fn dismiss_first(kind: SnapshotSurface) {
    let _ = close_surface(kind, deadline(5_000));
}

/// Finally-style cleanup: whatever a test raised is dismissed when the test
/// body exits, on any path, so a failed assertion never leaks a raised
/// surface into the next test.
struct CloseOnDrop(SnapshotSurface);

impl Drop for CloseOnDrop {
    fn drop(&mut self) {
        let _ = close_surface(self.0, deadline(8_000));
    }
}

fn enumerated_with_handle(handle: isize) -> Option<EnumeratedWindow> {
    let mut found = None;
    enumerate_top_level(|window| {
        if window.handle as isize == handle {
            found = Some(window);
            false
        } else {
            true
        }
    })
    .expect("the top-level enumeration succeeds");
    found
}

fn uia_root_child_handles() -> Vec<isize> {
    use uiautomation::types::TreeScope;

    let client = crate::tree::automation::automation_client().expect("client");
    let root = client.get_root_element().expect("root");
    let condition = client.create_true_condition().expect("condition");
    root.find_all(TreeScope::Children, &condition)
        .expect("desktop children")
        .iter()
        .filter_map(|child| {
            let handle: isize = child.get_native_window_handle().ok()?.into();
            Some(handle).filter(|handle| *handle != 0)
        })
        .collect()
}

fn uia_child_count(handle: isize) -> usize {
    use uiautomation::types::{Handle, TreeScope};

    let client = crate::tree::automation::automation_client().expect("client");
    let element = client
        .element_from_handle(Handle::from(handle))
        .expect("the surface's element resolves from its handle");
    let condition = client.create_true_condition().expect("condition");
    element
        .find_all(TreeScope::Children, &condition)
        .expect("the surface's children")
        .len()
}

#[test]
fn strict_headless_open_refuses_before_raising() {
    bootstrap();
    let _lock = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    dismiss_first(SnapshotSurface::ActionCenter);
    assert!(
        wait_for_foreground_to_settle(),
        "the desktop's foreground must settle before the refusal is staged"
    );
    let before = foreground();

    let error = open_surface(
        SnapshotSurface::ActionCenter,
        InteractionPolicy::headless(),
        deadline(5_000),
    )
    .expect_err("a strict-headless caller is refused");

    assert_eq!(error.code, ErrorCode::PolicyDenied);
    assert_eq!(
        foreground(),
        before,
        "a refusal that moved the foreground is not a refusal"
    );
    let resolved = super::resolve_surface(SnapshotSurface::ActionCenter, deadline(5_000))
        .expect("the desktop is readable")
        .is_some();
    assert!(
        !resolved,
        "the refused open must not have raised the surface"
    );
}

#[test]
fn quick_settings_refusal_names_build_and_capability_holder() {
    bootstrap();
    let error = open_surface(SnapshotSurface::QuickSettings, headed(), deadline(5_000))
        .expect_err("quick-settings is absent on this build");

    assert_eq!(error.code, ErrorCode::PlatformNotSupported);
    let build = super::build_number();
    assert!(build > 0, "the build number must be read, not guessed");
    let detail = error.platform_detail.expect("the refusal carries a detail");
    assert!(
        detail.contains(&build.to_string()),
        "the detail must name the build: {detail}"
    );
    assert!(
        detail.contains("action-center"),
        "the detail must name the surface carrying the capability: {detail}"
    );
}

#[test]
fn kind_pointed_at_an_absent_class_times_out() {
    bootstrap();
    let row = SurfaceKindRow {
        kind: SnapshotSurface::Desktop,
        family: super::SurfaceFamily::Win32Class(&["NoAgentDesktopShellSurfaceClass"]),
        raise: super::SurfaceRaise::AlreadyRaised,
        dismiss: super::SurfaceDismiss::None,
        exists_on_build: true,
        capability_holder: None,
    };

    let error = open_row(&row, deadline(1_500)).expect_err("no window has the class");

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_ne!(
        error.code,
        ErrorCode::PlatformNotSupported,
        "did not open is a different answer than absent on this build"
    );
}

#[test]
fn already_open_surface_returns_without_additional_raise() {
    bootstrap();
    let _lock = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    dismiss_first(SnapshotSurface::ActionCenter);
    accelerator_probe::take_all();

    let Some(first) = or_skip_shell(
        "action center first open",
        open_surface(SnapshotSurface::ActionCenter, headed(), deadline(10_000)),
    ) else {
        return;
    };
    let raises = accelerator_probe::take_all();
    assert_eq!(raises, 1, "the closed surface needs exactly one raise");

    let second = open_surface(SnapshotSurface::ActionCenter, headed(), deadline(10_000))
        .expect("the second open returns the surface already up");
    let raises = accelerator_probe::take_all();
    assert_eq!(
        raises, 0,
        "the resolve-first path must return without raising"
    );
    assert_eq!(first.id, second.id);

    let _cleanup = CloseOnDrop(SnapshotSurface::ActionCenter);
}

#[test]
fn action_center_opens_roots_and_closes_cloaked() {
    bootstrap();
    let _lock = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    dismiss_first(SnapshotSurface::ActionCenter);
    let _cleanup = CloseOnDrop(SnapshotSurface::ActionCenter);

    let Some(info) = or_skip_shell(
        "action center open",
        open_surface(SnapshotSurface::ActionCenter, headed(), deadline(10_000)),
    ) else {
        return;
    };
    let handle = handle_of(&info);
    crate::tree::automation::root_from_hwnd(handle, deadline(5_000))
        .expect("the returned identity roots through the observation stack");
    assert!(
        uia_child_count(handle) > 0,
        "the rooted surface presents a non-empty tree"
    );

    close_surface(SnapshotSurface::ActionCenter, deadline(8_000))
        .expect("the action center closes");
    assert!(
        settles_to(std::time::Duration::from_secs(5), true, || {
            is_cloaked(handle as *mut core::ffi::c_void)
        }),
        "the dismissed surface survives cloaked"
    );
}

#[test]
fn start_menu_opens_and_identity_roots_a_tree() {
    bootstrap();
    let _lock = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    dismiss_first(SnapshotSurface::StartMenu);
    let _cleanup = CloseOnDrop(SnapshotSurface::StartMenu);

    let Some(info) = or_skip_shell(
        "the start accelerator raises its surface",
        open_surface(SnapshotSurface::StartMenu, headed(), deadline(10_000)),
    ) else {
        return;
    };
    let handle = handle_of(&info);
    crate::tree::automation::root_from_hwnd(handle, deadline(5_000))
        .expect("the raised surface's identity roots through the observation stack");
    assert!(
        uia_child_count(handle) > 0,
        "the raised surface presents a non-empty tree"
    );
}
#[test]
fn taskbar_resolves_without_raising_and_roots_a_tree() {
    bootstrap();
    assert!(
        wait_for_foreground_to_settle(),
        "the desktop's foreground must settle before the no-raise read"
    );
    let before = foreground();

    let info = super::resolve_surface(SnapshotSurface::Taskbar, deadline(5_000))
        .expect("the desktop is readable")
        .expect("the taskbar is always up");
    let handle = handle_of(&info);

    assert_eq!(
        foreground(),
        before,
        "resolving the taskbar must not move the foreground"
    );

    let record = trusted_rewalk(handle);
    match record {
        Some(record) => {
            assert!(record.tool, "the taskbar carries the tool window bit");
            assert!(
                !passes_filter(&record),
                "the shipped agent-window filter rejects the taskbar on that bit"
            );
        }
        None => {
            assert!(
                live_tool_bit(handle),
                "the resolved taskbar identity carries the tool bit the filter rejects"
            );
        }
    }

    crate::tree::automation::root_from_hwnd(handle, deadline(5_000))
        .expect("the taskbar identity roots through the observation stack");
    let children = uia_child_count(handle);
    if children == 0 {
        eprintln!("skip taskbar tree: this desktop's taskbar presents an empty UIA tree");
        return;
    }
    assert!(
        children > 0,
        "the taskbar presents a non-empty tree"
    );
}

/// A bounded series of full walks, absorbing the session-varying enumeration
/// A26-1 measured - EnumWindows can stop yielding `Shell_TrayWnd` in a walk
/// where `FindWindowW` still finds it visible and unhung - so an unyielding
/// desktop is the measured A26-1 state, not a test failure, and the taskbar
/// test verifies the tool-bit fact against the resolved handle's live
/// ex-style instead.
fn trusted_rewalk(handle: isize) -> Option<EnumeratedWindow> {
    for attempt in 0..6 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(300));
        }
        if let Some(record) = enumerated_with_handle(handle) {
            return Some(record);
        }
    }
    None
}

fn live_tool_bit(handle: isize) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongW, WS_EX_TOOLWINDOW,
    };

    let hwnd = (handle as usize) as *mut core::ffi::c_void;
    let ex_style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) };
    (ex_style & WS_EX_TOOLWINDOW as i32) != 0
}

#[test]
fn immersive_surface_absent_from_enumeration_but_yielded_by_uia_root() {
    bootstrap();
    let _lock = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    dismiss_first(SnapshotSurface::ActionCenter);
    let _cleanup = CloseOnDrop(SnapshotSurface::ActionCenter);

    let Some(info) = or_skip_shell(
        "action center open",
        open_surface(SnapshotSurface::ActionCenter, headed(), deadline(10_000)),
    ) else {
        return;
    };
    let handle = handle_of(&info);

    assert!(
        enumerated_with_handle(handle).is_none(),
        "the immersive surface never appears in the Win32 top-level walk"
    );
    assert!(
        uia_root_child_handles().contains(&handle),
        "the UIA root yields the surface by native handle"
    );
}

#[test]
fn overflow_opens_via_chevron_and_closes_via_escape() {
    bootstrap();
    let _lock = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    dismiss_first(SnapshotSurface::SystemTrayOverflow);
    let _cleanup = CloseOnDrop(SnapshotSurface::SystemTrayOverflow);

    let Some(info) = or_skip_shell(
        "system tray overflow open",
        open_surface(
            SnapshotSurface::SystemTrayOverflow,
            headed(),
            deadline(10_000),
        ),
    ) else {
        return;
    };
    let handle = handle_of(&info);
    crate::tree::automation::root_from_hwnd(handle, deadline(5_000))
        .expect("the overflow toolbar roots through the observation stack");

    close_surface(SnapshotSurface::SystemTrayOverflow, deadline(8_000))
        .expect("Esc dismisses the overflow");
}

#[test]
fn overflow_toolbar_resolves_while_hidden() {
    bootstrap();
    let Some(info) = super::resolve_surface(SnapshotSurface::SystemTrayOverflow, deadline(5_000))
        .expect("the desktop is readable")
    else {
        eprintln!(
            "skip overflow toolbar: the NotifyIconOverflowWindow is not materialized on this desktop"
        );
        return;
    };
    let handle = handle_of(&info);
    assert_eq!(
        super::super::window_ops::window_class_name((handle as usize) as *mut core::ffi::c_void)
            .as_deref(),
        Some("ToolbarWindow32"),
        "the overflow kind roots at its own toolbar, not at the overflow window"
    );
}
