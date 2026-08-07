use crate::adapter::WindowsAdapter;
use crate::tree::automation::automation_client;
use crate::tree::element::UIAElement;
use crate::tree::fixture::{LocalFixture, ensure_test_apartment};
use crate::tree::fixture_window;
use crate::tree::walker_fake::deadline;
use agent_desktop_core::{AdapterError, NativeHandle, ObservationOps, state::VisibilityEvidence};
use uiautomation::types::Handle;
use uiautomation::{UIElement, UITreeWalker};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow;

/// Live evidence for the disabled envelope: `EnableWindow(FALSE)` projects
/// `enabled: Some(false)` through the shipped live reader (A18-8).
#[test]
fn live_disabled_button_projects_enabled_false() {
    ensure_test_apartment();
    let fixture = LocalFixture::create().expect("fixture");
    let button = fixture_window::find_button(fixture.handle());
    assert!(!button.is_null());
    unsafe { EnableWindow(button, 0) };
    std::thread::sleep(std::time::Duration::from_millis(50));
    let handle = control_handle(button).expect("button handle");
    let live = WindowsAdapter::new()
        .get_live_element(&handle, deadline())
        .expect("live read");
    assert_eq!(live.state.enabled, Some(false));
    assert_eq!(live.state.role, "button");
}

/// A18-8: WPF exposes `btnZeroSize` to the walk. Opt-in via
/// `AGENT_DESKTOP_LIVE_WPF=1` so the ScratchWpf host does not collide with
/// parallel on-screen fixture legs; soft-skip otherwise.
#[test]
fn live_wpf_zero_bounds_is_visible_false_when_stageable() {
    ensure_test_apartment();
    if std::env::var_os("AGENT_DESKTOP_LIVE_WPF").is_none() {
        eprintln!(
            "skip live WPF zero-bounds: set AGENT_DESKTOP_LIVE_WPF=1 to stage ScratchWpf on this lane"
        );
        return;
    }
    let Some((mut child, handle)) = stage_wpf_zero_button() else {
        eprintln!("skip live WPF zero-bounds: ScratchWpf host unavailable on this lane");
        return;
    };
    let live = WindowsAdapter::new()
        .get_live_element(&handle, deadline())
        .expect("zero-size live read");
    let _ = child.kill();
    let _ = child.wait();
    let bounds = live.bounds.expect("WPF zero button reports bounds");
    assert!(
        !(bounds.width > 0.0 && bounds.height > 0.0),
        "A18-8: WPF zero button must lack positive area"
    );
    let visibility = VisibilityEvidence {
        bounds: Some(bounds),
        states: live.state.states.clone(),
        bounds_from_live: true,
        states_from_live: true,
    };
    assert!(visibility.applicable());
    assert!(!visibility.result());
}

fn control_handle(button: *mut std::ffi::c_void) -> Result<NativeHandle, AdapterError> {
    let client = automation_client()?;
    let element = client
        .element_from_handle(Handle::from(button as isize))
        .map_err(|error| {
            crate::tree::automation::uia_error(&error, "resolve the fixture button")
        })?;
    Ok(UIAElement::from(element).into_native_handle())
}

fn stage_wpf_zero_button() -> Option<(std::process::Child, NativeHandle)> {
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../probes/windows/scratch/ScratchWpf.ps1");
    if !script.is_file() {
        return None;
    }
    let (vx, vy, vw, vh) = unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
            SM_YVIRTUALSCREEN,
        };
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    };
    let left = (vx + vw - 420).max(vx);
    let top = (vy + vh - 520).max(vy);
    let mut child = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            script.to_str()?,
            "-Tag",
            "u5-envelope",
            "-Left",
            &left.to_string(),
            "-Top",
            &top.to_string(),
            "-TimeoutSeconds",
            "12",
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    std::thread::sleep(std::time::Duration::from_millis(1_500));
    let client = automation_client().ok()?;
    let root = client.get_root_element().ok()?;
    let walker = client.get_control_view_walker().ok()?;
    let found = find_automation_id(&walker, &root, "btnZeroSize", 0);
    let Some(element) = found else {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    };
    Some((child, UIAElement::from(element).into_native_handle()))
}

fn find_automation_id(
    walker: &UITreeWalker,
    element: &UIElement,
    automation_id: &str,
    depth: u32,
) -> Option<UIElement> {
    if depth > 40 {
        return None;
    }
    if element.get_automation_id().ok().as_deref() == Some(automation_id) {
        return Some(element.clone());
    }
    let Ok(mut child) = walker.get_first_child(element) else {
        return None;
    };
    loop {
        if let Some(found) = find_automation_id(walker, &child, automation_id, depth + 1) {
            return Some(found);
        }
        match walker.get_next_sibling(&child) {
            Ok(sibling) => child = sibling,
            Err(_) => return None,
        }
    }
}
