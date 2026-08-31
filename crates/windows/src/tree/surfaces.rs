use agent_desktop_core::{AdapterError, Deadline, ErrorCode, ObservationRoot, SnapshotSurface};
use serde_json::json;

use super::automation::root_from_hwnd;
use super::chromium;
use super::element::UIAElement;
#[cfg(target_os = "windows")]
use super::properties::read_one;
#[cfg(target_os = "windows")]
use super::property_ids::TreeProperty;
use crate::system::window_enum::enumerate_top_level;
use crate::system::window_ops::passes_filter;

/// Resolves the element an observation root names, per-surface (`Window`
/// resolves the named window directly; `Focused` composes the focused-only
/// inventory).
///
/// `Surface` semantics live here: only the surfaces this adapter advertises
/// are handled (core validates the requested surface before the adapter is
/// called, so an unsupported surface never reaches this match).
pub(crate) fn surface_root(
    root: ObservationRoot<'_>,
    surface: SnapshotSurface,
    deadline: Deadline,
) -> Result<UIAElement, AdapterError> {
    match (root, surface) {
        (ObservationRoot::Window(window), SnapshotSurface::Window) => {
            let handle = window_hwnd(&window.id)?;
            root_from_hwnd(handle, deadline)
        }
        (ObservationRoot::Window(window), SnapshotSurface::Focused) => {
            let handle = focused_hwnd_of(&window.id)?;
            root_from_hwnd(handle, deadline)
        }
        (ObservationRoot::Window(window), SnapshotSurface::Sheet) => {
            let handle = focused_hwnd_of(&window.id)?;
            let element = root_from_hwnd(handle, deadline)?;
            if window_is_modal_sheet(&element, chromium::is_chromium_root(&element)) {
                Ok(element)
            } else {
                Err(AdapterError::new(
                    ErrorCode::WindowNotFound,
                    "The focused window is not a modal sheet",
                ))
            }
        }
        (ObservationRoot::Window(window), SnapshotSurface::Menu) => {
            let location = crate::system::menu_state::locate_menu(window.pid, deadline)?
                .ok_or_else(|| {
                    AdapterError::new(
                        ErrorCode::WindowNotFound,
                        "No open menu was found for this application",
                    )
                })?;
            root_from_hwnd(location.root_handle(), deadline)
        }
        (ObservationRoot::Window(window), SnapshotSurface::StartMenu)
        | (ObservationRoot::Window(window), SnapshotSurface::Taskbar)
        | (ObservationRoot::Window(window), SnapshotSurface::ActionCenter) => {
            let handle = window_hwnd(&window.id)?;
            root_from_hwnd(handle, deadline)
        }
        (ObservationRoot::Window(window), SnapshotSurface::SystemTray)
        | (ObservationRoot::Window(window), SnapshotSurface::SystemTrayOverflow) => {
            let handle = window_hwnd(&window.id)?;
            tray_surface_root(handle, deadline)
        }
        (ObservationRoot::Element { handle, .. }, _) => {
            super::element::uia_element(handle).cloned().map_err(|_| {
                AdapterError::new(
                    ErrorCode::StaleRef,
                    "Element root handle is invalid for this platform",
                )
                .with_details(json!({ "kind": "element_root_wrong_payload" }))
            })
        }
        (ObservationRoot::Window(_), _) => Err(AdapterError::not_supported("surface")),
    }
}

fn window_hwnd(id: &str) -> Result<isize, AdapterError> {
    id.strip_prefix("w-")
        .and_then(|number| number.parse::<isize>().ok())
        .filter(|handle| *handle > 0)
        .ok_or_else(|| AdapterError::new(ErrorCode::InvalidArgs, "Malformed window identifier"))
}

/// The focused window's HWND, found through the focused-only window inventory
/// the same way `focused_window` composes it.
fn focused_hwnd_of(expected: &str) -> Result<isize, AdapterError> {
    let mut focused = None;
    enumerate_top_level(|window| {
        if !passes_filter(&window) {
            return true;
        }
        let is_focused = {
            #[cfg(target_os = "windows")]
            {
                use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
                unsafe { GetForegroundWindow() == window.handle }
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = &window;
                false
            }
        };
        if is_focused {
            focused = Some(window.handle as isize);
            false
        } else {
            true
        }
    })?;
    let handle = focused.ok_or_else(|| {
        AdapterError::new(
            ErrorCode::WindowNotFound,
            "There is no focused window on this desktop",
        )
    })?;
    if format!("w-{handle}") != expected {
        return Err(AdapterError::new(
            ErrorCode::WindowNotFound,
            "The requested focused window is no longer the focused window",
        ));
    }
    Ok(handle)
}

/// Whether a window is a modal sheet surface, tested from the window's own
/// `WindowIsModal` property **before** any child is consulted (the macOS
/// window-is-surface pattern from `crates/macos/src/tree/surfaces.rs`).
///
/// This classifies a Chromium modal as a `Sheet` surface, making it reachable
/// via the sheet surface.
#[cfg(target_os = "windows")]
pub(crate) fn window_is_modal_sheet(root: &UIAElement, _chromium: bool) -> bool {
    matches!(
        read_one(root, TreeProperty::WindowIsModal).flag(),
        Some(true)
    )
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn window_is_modal_sheet(_root: &UIAElement, _chromium: bool) -> bool {
    false
}

#[cfg(target_os = "windows")]
fn tray_surface_root(handle: isize, deadline: Deadline) -> Result<UIAElement, AdapterError> {
    if let Some(parent) = parent_window_handle(handle) {
        if let Ok(parent_root) = root_from_hwnd(parent, deadline) {
            if let Some(element) = find_descendant_by_hwnd(&parent_root, handle) {
                return Ok(element);
            }
        }
    }
    root_from_hwnd(handle, deadline)
}

#[cfg(not(target_os = "windows"))]
fn tray_surface_root(handle: isize, deadline: Deadline) -> Result<UIAElement, AdapterError> {
    root_from_hwnd(handle, deadline)
}

#[cfg(target_os = "windows")]
fn parent_window_handle(hwnd: isize) -> Option<isize> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GA_ROOT, GetAncestor};

    if hwnd == 0 {
        return None;
    }
    let root = unsafe { GetAncestor(hwnd as *mut std::ffi::c_void, GA_ROOT) };
    if root.is_null() {
        None
    } else {
        let parent = root as isize;
        (parent != hwnd).then_some(parent)
    }
}

#[cfg(target_os = "windows")]
fn find_descendant_by_hwnd(parent: &UIAElement, target: isize) -> Option<UIAElement> {
    use uiautomation::types::TreeScope;

    let client = crate::tree::automation::automation_client().ok()?;
    let condition = client.create_true_condition().ok()?;
    let descendants = parent.0.find_all(TreeScope::Descendants, &condition).ok()?;
    for child in descendants {
        let handle: isize = match child.get_native_window_handle().ok() {
            Some(handle) => handle.into(),
            None => continue,
        };
        if handle == target {
            return Some(UIAElement::from(child));
        }
    }
    None
}

#[cfg(test)]
#[path = "surfaces_tests.rs"]
mod tests;

#[cfg(all(test, target_os = "windows"))]
#[path = "surfaces_advertising_tests.rs"]
mod advertising_tests;
