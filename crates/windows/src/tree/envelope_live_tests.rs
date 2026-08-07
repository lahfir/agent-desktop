use crate::adapter::WindowsAdapter;
use crate::tree::automation::automation_client;
use crate::tree::element::UIAElement;
use crate::tree::fixture::{LocalFixture, ensure_test_apartment};
use crate::tree::fixture_window;
use crate::tree::walker_fake::deadline;
use agent_desktop_core::{AdapterError, NativeHandle, ObservationOps, state::VisibilityEvidence};
use std::time::{Duration, Instant};
use uiautomation::types::Handle;
use uiautomation::{UIElement, UITreeWalker};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow;

/// Selects the lanes that stage the on-screen ScratchWpf host.
const LIVE_WPF_VARIABLE: &str = "AGENT_DESKTOP_LIVE_WPF";

/// How long the host has to appear in the tree. Generous because it covers
/// PowerShell start-up plus WPF assembly loading on a cold runner, and the
/// poll below returns the moment the button is there.
const WPF_STAGE_BUDGET: Duration = Duration::from_secs(20);

const WPF_STAGE_POLL: Duration = Duration::from_millis(250);

/// The host's self-close watchdog, its only protection against outliving a
/// panicking test. It has to outlast the staging budget plus the reads.
const WPF_HOST_WATCHDOG_SECONDS: &str = "45";

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

/// A18-8: WPF is where zero-area evidence has to come from - it exposes
/// `btnZeroSize` to the walk with a real rectangle of no area, which neither
/// the Win32 nor the WinForms fixture reproduces.
///
/// Staging is opt-in because the host is an on-screen window, and a developer
/// running the suite beside the other on-screen fixture legs should not get a
/// second one unasked. The Windows CI lane opts in, so these assertions run
/// unattended on every pull request; `the_windows_lane_stages_the_live_wpf_host`
/// is what keeps that true.
///
/// Where the variable is set, a host that will not stage fails instead of
/// skipping. A lane that claims to run this and then quietly runs nothing is
/// indistinguishable from a lane that ran it and passed, which is how coverage
/// disappears with nothing to announce it.
#[test]
fn live_wpf_zero_bounds_is_visible_false_when_stageable() {
    ensure_test_apartment();
    if std::env::var_os(LIVE_WPF_VARIABLE).is_none() {
        eprintln!(
            "skip live WPF zero-bounds: {LIVE_WPF_VARIABLE} is unset here, so no ScratchWpf host was staged; the Test (Windows) CI lane sets it and owns executing this"
        );
        return;
    }
    let Some((mut child, handle)) = stage_wpf_zero_button() else {
        panic!(
            "{LIVE_WPF_VARIABLE} is set, so this lane owns staging ScratchWpf, but its btnZeroSize did not reach the tree within {WPF_STAGE_BUDGET:?}"
        );
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

/// The lane that owns executing the live WPF leg, pinned at the assignment
/// that gives it that ownership.
///
/// Pinned on the step rather than on the file: the variable has to sit on the
/// step that runs the library tests, because a step's `env` reaches only that
/// step. Set anywhere else it reads as coverage while staging nothing.
#[test]
fn the_windows_lane_stages_the_live_wpf_host() {
    let workflow = include_str!("../../../../.github/workflows/ci.yml").replace("\r\n", "\n");
    let assignment = format!("{LIVE_WPF_VARIABLE}: \"1\"");
    let step = workflow
        .split("- name: ")
        .find(|step| step.starts_with("Core and Windows unit tests"))
        .expect("the Windows lane runs a library-test step");
    assert!(
        step.contains(&assignment),
        "the library-test step no longer stages the ScratchWpf host, so the zero-bounds body would skip on every run"
    );
    assert!(
        step.contains("cargo test --locked -p agent-desktop-core -p agent-desktop-windows --lib"),
        "the staged step is no longer the one that runs the library tests"
    );
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
    let mut child = spawn_wpf_host(&script)?;
    match await_zero_button(child.id()) {
        Some(handle) => Some((child, handle)),
        None => {
            let _ = child.kill();
            let _ = child.wait();
            None
        }
    }
}

fn spawn_wpf_host(script: &std::path::Path) -> Option<std::process::Child> {
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
    std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            script.to_str()?,
            "-Tag",
            "envelope-zero-bounds",
            "-Left",
            &left.to_string(),
            "-Top",
            &top.to_string(),
            "-TimeoutSeconds",
            WPF_HOST_WATCHDOG_SECONDS,
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()
}

/// Polls for the button instead of sleeping a fixed span. Start-up is seconds
/// slower on a cold CI runner than on a warm desktop, and a fixed sleep turns
/// that difference into an unstaged host - which now fails the lane rather
/// than passing it quietly.
fn await_zero_button(pid: u32) -> Option<NativeHandle> {
    let client = automation_client().ok()?;
    let root = client.get_root_element().ok()?;
    let walker = client.get_control_view_walker().ok()?;
    let expiry = Instant::now() + WPF_STAGE_BUDGET;
    loop {
        if let Some(handle) = zero_button_handle(&walker, &root, pid) {
            return Some(handle);
        }
        if Instant::now() >= expiry {
            return None;
        }
        std::thread::sleep(WPF_STAGE_POLL);
    }
}

fn zero_button_handle(walker: &UITreeWalker, root: &UIElement, pid: u32) -> Option<NativeHandle> {
    let window = window_of_process(walker, root, pid)?;
    let element = find_automation_id(walker, &window, "btnZeroSize", 0)?;
    Some(UIAElement::from(element).into_native_handle())
}

/// The host's own top-level window, matched on process rather than searched
/// for by automation id from the desktop root: a ScratchWpf window left behind
/// by an earlier run carries the same ids and would answer for this one.
fn window_of_process(walker: &UITreeWalker, root: &UIElement, pid: u32) -> Option<UIElement> {
    let mut window = walker.get_first_child(root).ok()?;
    loop {
        if window.get_process_id().ok() == Some(pid) {
            return Some(window);
        }
        window = walker.get_next_sibling(&window).ok()?;
    }
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
