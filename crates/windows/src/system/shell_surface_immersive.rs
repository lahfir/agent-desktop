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
/// the root, so neither the class nor the host image alone is an identity,
/// and a sibling kind's live surface must simply not match - a read may run
/// while the sibling is up, so absence is the correct answer for it. A child
/// whose own attributes fail to read is skipped: the root's population
/// changes under the read, and one raced child must not fail the walk for
/// every other caller. A landmark search that could not run is the exception
/// and surfaces instead: resolving "no surface" from a search fault is the
/// answer a close path would skip a dismissal on.
///
/// The open path needs one more distinction the read cannot make - whether a
/// raise presented a surface whose tree matches no landmark at all - and it
/// carries its own pre-raise witness to attribute the child to its raise
/// ([`super::shell_surface_open::poll_until_observed`] and
/// [`foreign_shape_error`]).
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

/// Whether a presented surface of this kind's class and host, raised by the
/// caller and absent from the pre-raise witness, matches none of the kind's
/// landmarks - the raise presented a shape this build's measurements do not
/// cover, and the caller resolves that as a named refusal rather than as
/// absence, so a caller is told the shape did not match instead of that no
/// surface exists. A child already in the witness is a sibling kind's live
/// surface or a dismissed one's lingering window: it belongs to no raise and
/// never answers this question.
pub(super) fn raise_presented_foreign_shape(
    client: &uiautomation::UIAutomation,
    pre_raise_children: &[isize],
    landmarks: &[&str],
) -> Result<bool, AdapterError> {
    use uiautomation::types::TreeScope;

    let narrow = super::listing_retry::narrow_to_permitted_codes;
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
        let Some(classname) = child.get_classname().ok() else {
            continue;
        };
        if classname.ne(SHELL_CORE_WINDOW_CLASS) {
            continue;
        }
        let handle: isize = match child.get_native_window_handle().ok() {
            Some(handle) => handle.into(),
            None => continue,
        };
        if handle == 0 || pre_raise_children.contains(&handle) {
            continue;
        }
        if super::window_enum::is_cloaked(handle as WindowHandle) {
            continue;
        }
        let pid = match child.get_process_id() {
            Ok(pid) => pid,
            Err(_) => continue,
        };
        let Some(image) = super::process_identity::process_image_name(pid.into()) else {
            continue;
        };
        let image_stem = image.strip_suffix(".exe").unwrap_or(&image);
        if !SHELL_DIAGNOSTIC_HOST_IMAGES
            .iter()
            .any(|host| host.eq_ignore_ascii_case(image_stem))
        {
            continue;
        }
        if !carries_landmark(client, &child, landmarks)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The handles of the root children a raise could present through - the
/// CoreWindow class owned by any shell host image and not cloaked - captured
/// before the raise so the poll can attribute a new foreign-shape child to
/// it rather than to a sibling kind's live surface. The host set is the
/// broad diagnostic one: newer shells route these surfaces through host
/// images this build's rows never named.
pub(super) fn witness_immersive_children() -> Result<Vec<isize>, AdapterError> {
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
        .find_all(uiautomation::types::TreeScope::Children, &condition)
        .map_err(|error| {
            narrow(crate::tree::automation::uia_error(
                &error,
                "read the UIA desktop root's children",
            ))
        })?;
    let mut handles = Vec::new();
    for child in children {
        let Some(classname) = child.get_classname().ok() else {
            continue;
        };
        if classname.ne(SHELL_CORE_WINDOW_CLASS) {
            continue;
        }
        let handle: isize = match child.get_native_window_handle().ok() {
            Some(handle) => handle.into(),
            None => continue,
        };
        if handle == 0 || super::window_enum::is_cloaked(handle as WindowHandle) {
            continue;
        }
        let Some(pid) = child.get_process_id().ok() else {
            continue;
        };
        let Some(image) = super::process_identity::process_image_name(pid.into()) else {
            continue;
        };
        let image_stem = image.strip_suffix(".exe").unwrap_or(&image);
        if SHELL_DIAGNOSTIC_HOST_IMAGES
            .iter()
            .any(|host| host.eq_ignore_ascii_case(image_stem))
        {
            handles.push(handle);
        }
    }
    Ok(handles)
}

/// The shell host images the corpus measured presenting CoreWindow surfaces,
/// plus the newer hosts the foreign-shape diagnosis must also see: a
/// diagnosis that only knew `shellexperiencehost` would miss a surface the
/// newer shell routed through a different host image entirely.
const SHELL_DIAGNOSTIC_HOST_IMAGES: &[&str] = &[
    "shellexperiencehost",
    "searchhost",
    "searchui",
    "searchapp",
    "startmenuexperiencehost",
    "shellhost",
];
const SHELL_CORE_WINDOW_CLASS: &str = "Windows.UI.Core.CoreWindow";

/// The named refusal for a raise that presented a surface whose tree matches
/// none of the kind's landmarks: the shell answered, the shape is not the
/// measured one, and "no surface found" would send a caller to open a
/// surface that is already on screen. The detail names the build and the
/// landmarks the shape was matched against, per the error table's
/// opened-but-foreign-shape row.
pub(super) fn foreign_shape_error(landmarks: &[&str]) -> AdapterError {
    use agent_desktop_core::{DeliverySemantics, ErrorCode};

    AdapterError::new(
        ErrorCode::PlatformNotSupported,
        "the shell presented a surface of this kind whose tree does not match \
         the landmarks this build's adapter measures",
    )
    .with_platform_detail(format!(
        "this Windows build presents a different surface shape; the landmarks \
         matched were: {}",
        landmarks.join(", ")
    ))
    .with_details(serde_json::json!({ "kind": "shell_surface_foreign_shape" }))
    .with_disposition(DeliverySemantics::not_delivered())
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
            accessible: true,
            minimized: Some(unsafe { IsIconic(hwnd) } != 0),
            visible: Some(unsafe { IsWindowVisible(hwnd) } != 0),
        },
    }
}

#[cfg(all(test, target_os = "windows"))]
#[path = "shell_surface_landmark_tests.rs"]
mod landmark_tests;
