use super::{AXElement, properties::element::element_for_pid, surface_read};
use agent_desktop_core::{AdapterError, SnapshotSurface};
use std::time::Instant;

#[cfg(target_os = "macos")]
pub(crate) fn focused_surface_for_pid(
    pid: i32,
    deadline: Instant,
) -> Result<Option<AXElement>, AdapterError> {
    let app = element_for_pid(pid);
    surface_read::element(&app, "AXFocusedWindow", deadline)
}

#[cfg(target_os = "macos")]
pub(crate) fn menu_element_for_pid(
    pid: i32,
    deadline: Instant,
) -> Result<Option<AXElement>, AdapterError> {
    let identity = crate::system::process::process_identity::ProcessIdentity::capture(pid)?
        .ok_or_else(|| AdapterError::stale_ref_because("Menu application exited"))?;
    super::app_menu::observe(identity, deadline, || {
        super::app_menu::root_for_pid(pid, SnapshotSurface::Menu, deadline)
    })
}

#[cfg(target_os = "macos")]
fn first_child_with_role_or_subrole(
    pid: i32,
    target: &str,
    deadline: Instant,
) -> Result<Option<AXElement>, AdapterError> {
    let Some(window) = focused_surface_for_pid(pid, deadline)? else {
        return Ok(None);
    };
    first_in_window(&window, deadline, |element| {
        role_or_subrole_matches(element, target, deadline)
    })
}

/// Finds a sheet, popover or alert on this window or its direct children only.
#[cfg(target_os = "macos")]
pub(crate) fn surface_in_window(
    window: &AXElement,
    surface: SnapshotSurface,
    deadline: Instant,
) -> Result<Option<AXElement>, AdapterError> {
    let target = match surface {
        SnapshotSurface::Sheet => "AXSheet",
        SnapshotSurface::Popover => "AXPopover",
        SnapshotSurface::Alert => {
            return first_in_window(window, deadline, |element| is_alert(element, deadline));
        }
        _ => return Err(AdapterError::not_supported("window-owned surface")),
    };
    first_in_window(window, deadline, |element| {
        role_or_subrole_matches(element, target, deadline)
    })
}

#[cfg(target_os = "macos")]
fn first_in_window(
    window: &AXElement,
    deadline: Instant,
    matches: impl Fn(&AXElement) -> Result<bool, AdapterError>,
) -> Result<Option<AXElement>, AdapterError> {
    if matches(window)? {
        return Ok(Some(window.clone()));
    }
    for child in surface_read::elements(window, "AXChildren", deadline)? {
        if matches(&child)? {
            return Ok(Some(child));
        }
    }
    Ok(None)
}

#[cfg(target_os = "macos")]
pub(super) fn role_or_subrole_matches(
    element: &AXElement,
    target: &str,
    deadline: Instant,
) -> Result<bool, AdapterError> {
    if surface_read::string(element, "AXRole", deadline)?.as_deref() == Some(target) {
        return Ok(true);
    }
    Ok(surface_read::string(element, "AXSubrole", deadline)?.as_deref() == Some(target))
}

#[cfg(target_os = "macos")]
pub(crate) fn sheet_for_pid(
    pid: i32,
    deadline: Instant,
) -> Result<Option<AXElement>, AdapterError> {
    first_child_with_role_or_subrole(pid, "AXSheet", deadline)
}

#[cfg(target_os = "macos")]
pub(crate) fn popover_for_pid(
    pid: i32,
    deadline: Instant,
) -> Result<Option<AXElement>, AdapterError> {
    first_child_with_role_or_subrole(pid, "AXPopover", deadline)
}

#[cfg(target_os = "macos")]
pub(crate) fn alert_for_pid(
    pid: i32,
    deadline: Instant,
) -> Result<Option<AXElement>, AdapterError> {
    let app = element_for_pid(pid);
    let mut windows = surface_read::elements(&app, "AXWindows", deadline)?;
    if let Some(focused) = focused_surface_for_pid(pid, deadline)? {
        windows.insert(0, focused);
    }
    for window in windows {
        if is_alert(&window, deadline)? {
            return Ok(Some(window));
        }
        for child in surface_read::elements(&window, "AXChildren", deadline)? {
            if is_alert(&child, deadline)? {
                return Ok(Some(child));
            }
        }
    }
    Ok(None)
}

#[cfg(target_os = "macos")]
pub(super) fn is_alert(element: &AXElement, deadline: Instant) -> Result<bool, AdapterError> {
    let role = surface_read::string(element, "AXRole", deadline)?;
    let subrole = surface_read::string(element, "AXSubrole", deadline)?;
    Ok(matches!(role.as_deref(), Some("AXSheet"))
        || matches!(
            subrole.as_deref(),
            Some("AXDialog") | Some("AXAlert") | Some("AXSheet")
        ))
}

#[cfg(target_os = "macos")]
pub(crate) fn is_menu_open(pid: i32, deadline: Instant) -> Result<bool, AdapterError> {
    Ok(menu_element_for_pid(pid, deadline)?.is_some())
}

#[cfg(not(target_os = "macos"))]
macro_rules! unsupported_surface {
    ($name:ident) => {
        pub(crate) fn $name(
            _pid: i32,
            _deadline: Instant,
        ) -> Result<Option<AXElement>, AdapterError> {
            Ok(None)
        }
    };
}

#[cfg(not(target_os = "macos"))]
unsupported_surface!(focused_surface_for_pid);
#[cfg(not(target_os = "macos"))]
unsupported_surface!(menu_element_for_pid);
#[cfg(not(target_os = "macos"))]
unsupported_surface!(sheet_for_pid);
#[cfg(not(target_os = "macos"))]
unsupported_surface!(popover_for_pid);
#[cfg(not(target_os = "macos"))]
unsupported_surface!(alert_for_pid);

#[cfg(not(target_os = "macos"))]
pub(crate) fn is_menu_open(_pid: i32, _deadline: Instant) -> Result<bool, AdapterError> {
    Ok(false)
}
