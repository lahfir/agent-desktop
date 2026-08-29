//! The independent raise-response oracle behind the shell-surface skip
//! convention.
//!
//! `shell_declined_the_surface` classifies the product's own TIMEOUT
//! envelopes, so on its own it cannot tell a declining desktop from a product
//! whose resolver or raise machinery regressed - the same envelope comes back
//! for both, and every immersive round-trip test would skip green. This
//! module observes whether the shell answered a raise at all, reading raw
//! Win32 and UI Automation state directly: `GetForegroundWindow`,
//! `automation_client` + `get_root_element` + `find_all(Children)` over the
//! desktop root, and a raw `EnumWindows` pass. The product's resolver and
//! raise path are never consulted - that is the independence the skip
//! decision was missing.
//!
//! Residual limits, stated where they belong:
//!
//! - A raise whose chord was dropped before the OS is indistinguishable from
//!   a shell-less desktop by observation alone: nothing on the desktop
//!   changes, the oracle answers "did not respond", and the leg skips - the
//!   regression that broke the raise itself hides. The dev box, where the
//!   shell answers every real accelerator, is what keeps that assertion
//!   honest.
//! - A desktop change the raise did not cause - a sibling test's window
//!   appearing inside the poll window - reads as a response. Observation
//!   alone cannot attribute a change; the crate's lock families keep
//!   foreground-sensitive tests from staging under each other.

#![cfg(all(test, target_os = "windows"))]

use std::time::{Duration, Instant};

/// How long a response verdict polls before answering "did not respond".
const RESPONSE_WINDOW: Duration = Duration::from_secs(3);
const RESPONSE_POLL: Duration = Duration::from_millis(250);

/// The desktop state a raise-response verdict is measured against, captured
/// before the raise attempt.
pub(crate) struct RaiseWitness {
    foreground: isize,
    root_children: Vec<isize>,
    top_levels: Vec<isize>,
}

impl RaiseWitness {
    fn moved_since(&self, witness: &RaiseWitness) -> bool {
        self.foreground != witness.foreground
            || self.root_children != witness.root_children
            || self.top_levels != witness.top_levels
    }
}

/// Captures the pre-raise state [`shell_responded_to_raise`] compares
/// against: the foreground window, the UIA root's children, and the
/// top-level window set. One `find_all` over the root plus one `EnumWindows`
/// pass - cheap enough to sit in front of every raise attempt a live test
/// makes.
///
/// `None` when the desktop's state cannot be read at all. A witness the
/// harness cannot take is a desktop the oracle cannot speak about, so the
/// caller's skip convention stands unchanged rather than gaining a verdict
/// invented from missing evidence.
pub(crate) fn witness_desktop() -> Option<RaiseWitness> {
    read_desktop()
}

/// Whether the desktop moved away from the witness since the raise attempt:
/// the foreground window changed, the UIA root's children changed (count or
/// membership - the immersive surfaces present as new root children,
/// A26-1), or a new top-level window appeared. Polled across
/// [`RESPONSE_WINDOW`] so a shell that answers a beat late is still seen; a
/// desktop that never responds exhausts the window and answers `false`.
pub(crate) fn shell_responded_to_raise(witness: &RaiseWitness) -> bool {
    let deadline = Instant::now() + RESPONSE_WINDOW;
    loop {
        if let Some(current) = read_desktop() {
            if current.moved_since(witness) {
                return true;
            }
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(RESPONSE_POLL);
    }
}

/// The closure shape `or_skip_shell` consumes. A witness that could not be
/// captured keeps the skip convention: no evidence, no verdict.
pub(crate) fn responded_since(witness: &Option<RaiseWitness>) -> bool {
    witness.as_ref().is_some_and(shell_responded_to_raise)
}

fn read_desktop() -> Option<RaiseWitness> {
    Some(RaiseWitness {
        foreground: foreground_hwnd(),
        root_children: uia_root_child_handles()?,
        top_levels: top_level_handles()?,
    })
}

fn foreground_hwnd() -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    (unsafe { GetForegroundWindow() }) as isize
}

/// The desktop root's children by native handle, read straight off UI
/// Automation - the same enumeration the immersive resolver walks, but the
/// bare child list of it, with no class, host, landmark, or cloak matching.
fn uia_root_child_handles() -> Option<Vec<isize>> {
    use uiautomation::types::TreeScope;

    let client = crate::tree::automation::automation_client().ok()?;
    let root = client.get_root_element().ok()?;
    let condition = client.create_true_condition().ok()?;
    let children = root.find_all(TreeScope::Children, &condition).ok()?;
    let mut handles: Vec<isize> = children
        .iter()
        .filter_map(|child| {
            let handle: isize = child.get_native_window_handle().ok()?.into();
            Some(handle).filter(|handle| *handle != 0)
        })
        .collect();
    handles.sort_unstable();
    Some(handles)
}

/// Every top-level window by raw `EnumWindows`, the same walk `window_enum`
/// bridges for the product - collected here directly so the oracle's answer
/// never routes through crate code a shell test could be asserting on. The
/// callback only pushes to the caller's vector: no panic may unwind through
/// the `extern "system"` frame, and nothing re-enters `EnumWindows`.
fn top_level_handles() -> Option<Vec<isize>> {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::EnumWindows;

    unsafe extern "system" fn collect(window: HWND, lparam: isize) -> i32 {
        let handles = unsafe { &mut *(lparam as *mut Vec<isize>) };
        handles.push(window as isize);
        1
    }

    let mut handles: Vec<isize> = Vec::new();
    let succeeded =
        unsafe { EnumWindows(Some(collect), (&mut handles as *mut Vec<isize>) as isize) };
    if succeeded == 0 {
        return None;
    }
    handles.sort_unstable();
    Some(handles)
}
