//! The immersive family's resolver: the half of the shell-surface seam the
//! Win32 top-level walk cannot reach (A26-1). The resolver walks the UIA
//! root's children and matches the first child whose class, hosting shell
//! process, landmark and cloak state name the kind's surface; the landmark
//! search's failure classification lives beside it, because "search failed"
//! must never read as "surface closed".
#![allow(dead_code)]

use agent_desktop_core::{AdapterError, ProcessId, WindowInfo};

use super::window_enum::WindowHandle;

/// Walks the UIA root's children - never the Win32 top-level walk, which does
/// not yield the immersive surfaces (A26-1) - and matches the first child
/// whose class, hosting shell process, landmark and cloak state name this
/// kind's surface. The landmark is what keeps two kinds hosted by the same
/// shell process from resolving each other's surface: on this build the Start
/// overlay and the Action Center are both `ShellExperienceHost` children of
/// the root, so neither the class nor the host image alone is an identity.
/// A child whose own attributes fail to read is skipped: the root's
/// population changes under the read, and one raced child must not fail the
/// walk for every other caller. A landmark search that could not run is the
/// exception and surfaces instead: resolving "no surface" from a search
/// fault is the answer a close path would skip a dismissal on.
pub(super) fn resolve_immersive(
    expected_class: &str,
    host_images: &[&str],
    landmarks: &[&str],
) -> Result<Option<WindowInfo>, AdapterError> {
    use uiautomation::types::TreeScope;

    let narrow = super::listing_retry::narrow_to_permitted_codes;
    let client = crate::tree::automation::automation_client().map_err(narrow)?;
    let root = client.get_root_element().map_err(|error| {
        narrow(crate::tree::automation::uia_error(
            &error,
            "read the UIA desktop root",
        ))
    })?;
    let condition = client.create_true_condition().map_err(|error| {
        narrow(crate::tree::automation::uia_error(
            &error,
            "build the desktop children condition",
        ))
    })?;
    let children = root
        .find_all(TreeScope::Children, &condition)
        .map_err(|error| {
            narrow(crate::tree::automation::uia_error(
                &error,
                "read the UIA desktop root's children",
            ))
        })?;
    for child in children {
        if let Some(info) =
            immersive_candidate(&client, &child, expected_class, host_images, landmarks)?
        {
            return Ok(Some(info));
        }
    }
    Ok(None)
}

fn immersive_candidate(
    client: &uiautomation::UIAutomation,
    child: &uiautomation::UIElement,
    expected_class: &str,
    host_images: &[&str],
    landmarks: &[&str],
) -> Result<Option<WindowInfo>, AdapterError> {
    let Some(classname) = child.get_classname().ok() else {
        return Ok(None);
    };
    if classname.ne(expected_class) {
        return Ok(None);
    }
    let handle: isize = match child.get_native_window_handle().ok() {
        Some(handle) => handle.into(),
        None => return Ok(None),
    };
    if handle == 0 || super::window_enum::is_cloaked(handle as WindowHandle) {
        return Ok(None);
    }
    let pid = match child.get_process_id() {
        Ok(pid) => ProcessId::from(pid),
        Err(_) => match super::window_identity::live_window_owner(handle as WindowHandle) {
            Some(pid) => pid,
            None => return Ok(None),
        },
    };
    let Some(image) = super::process_identity::process_image_name(pid) else {
        return Ok(None);
    };
    let image_stem = image.strip_suffix(".exe").unwrap_or(&image);
    if !host_images
        .iter()
        .any(|host| host.eq_ignore_ascii_case(image_stem))
    {
        return Ok(None);
    }
    if !carries_landmark(client, child, landmarks)? {
        return Ok(None);
    }
    Ok(Some(window_info_from_surface(child, handle, pid, image)))
}

/// Whether the candidate's subtree carries one of the kind's landmark
/// `AutomationId`s. A landmark that reads as absent is absence, not failure -
/// `find_first` reports not-found by error, and a subtree that raced away
/// between the root scan and this read simply does not match. A search that
/// could not run - a transport timeout, a denied read - is a fault that
/// surfaces: reading it as absence is what lets a presented surface be
/// reported closed, and a close on that answer is skipped without ever
/// dismissing.
fn carries_landmark(
    client: &uiautomation::UIAutomation,
    candidate: &uiautomation::UIElement,
    landmarks: &[&str],
) -> Result<bool, AdapterError> {
    use uiautomation::types::{TreeScope, UIProperty};
    use uiautomation::variants::Variant;

    let context = "search an immersive surface candidate for its landmark";
    for landmark in landmarks {
        let condition = client
            .create_property_condition(UIProperty::AutomationId, Variant::from(*landmark), None)
            .map_err(|error| {
                super::listing_retry::narrow_to_permitted_codes(crate::tree::automation::uia_error(
                    &error, context,
                ))
            })?;
        if landmark_search_answer(
            candidate.find_first(TreeScope::Descendants, &condition),
            context,
        )? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The one decision the landmark search shares: a found element is present,
/// an absence-family failure is absence, and any other failure surfaces
/// narrowed onto the closed set this crate's read paths report.
fn landmark_search_answer(
    found: Result<uiautomation::UIElement, uiautomation::Error>,
    context: &'static str,
) -> Result<bool, AdapterError> {
    match found {
        Ok(_) => Ok(true),
        Err(error) => {
            if crate::tree::automation::failure_of(&error).is_absence() {
                return Ok(false);
            }
            Err(super::listing_retry::narrow_to_permitted_codes(
                crate::tree::automation::uia_error(&error, context),
            ))
        }
    }
}

/// Builds the surface identity with the same field shapes the window listing
/// emits: `w-<hwnd>` id, the owning process's image name and pid, live rect
/// and window state read off the same handle.
fn window_info_from_surface(
    element: &uiautomation::UIElement,
    handle: isize,
    pid: ProcessId,
    image: String,
) -> WindowInfo {
    use windows_sys::Win32::UI::WindowsAndMessaging::{IsIconic, IsWindowVisible};

    let hwnd = handle as WindowHandle;
    let token = super::process_identity::token_for_pid(pid).ok().flatten();
    WindowInfo {
        id: format!("w-{}", handle),
        title: element.get_name().unwrap_or_default(),
        app: image,
        pid,
        process_instance: token,
        bounds: Some(super::window_enum::window_rect(hwnd)),
        state: agent_desktop_core::WindowState {
            is_focused: super::window_ops::is_foreground_window(hwnd),
            minimized: Some(unsafe { IsIconic(hwnd) } != 0),
            visible: Some(unsafe { IsWindowVisible(hwnd) } != 0),
        },
    }
}

#[cfg(all(test, target_os = "windows"))]
#[path = "shell_surface_landmark_tests.rs"]
mod landmark_tests;
