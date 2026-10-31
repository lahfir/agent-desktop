use super::{AXElement, properties::element::element_for_pid, surface_read};
use agent_desktop_core::{AdapterError, ErrorCode, SnapshotSurface};
use std::time::Instant;

const MAX_SURFACE_NODES: usize = 2_048;

pub(crate) fn observe<T>(
    identity: crate::system::process::process_identity::ProcessIdentity,
    deadline: Instant,
    mut operation: impl FnMut() -> Result<T, AdapterError>,
) -> Result<T, AdapterError> {
    surface_read::ensure_before_deadline(deadline)?;
    reacquire_once(deadline, || {
        let result = operation();
        require_generation(identity)?;
        result
    })
}

fn require_generation(
    identity: crate::system::process::process_identity::ProcessIdentity,
) -> Result<(), AdapterError> {
    if identity.still_matches()? {
        Ok(())
    } else {
        Err(AdapterError::stale_ref_because(
            "Menu application changed process instance during observation",
        ))
    }
}

fn reacquire_once<T>(
    deadline: Instant,
    mut operation: impl FnMut() -> Result<T, AdapterError>,
) -> Result<T, AdapterError> {
    surface_read::ensure_before_deadline(deadline)?;
    let first = operation();
    if !first.as_ref().is_err_and(invalidated) {
        return first;
    }
    surface_read::ensure_before_deadline(deadline)?;
    operation().map_err(|mut error| {
        if invalidated(&error) {
            error.code = ErrorCode::AppUnresponsive;
            let mut details = error
                .details
                .take()
                .unwrap_or_else(|| serde_json::json!({}));
            if let Some(fields) = details.as_object_mut() {
                fields.insert("complete".into(), false.into());
                fields.insert("retryable".into(), true.into());
                fields.insert("menu_root_reacquired".into(), true.into());
            }
            error = error.with_details(details);
        }
        error
    })
}

fn invalidated(error: &AdapterError) -> bool {
    error
        .details
        .as_ref()
        .and_then(|details| details["ax_error"].as_i64())
        == Some(i64::from(accessibility_sys::kAXErrorInvalidUIElement))
}

pub(crate) fn root_for_pid(
    pid: i32,
    surface: SnapshotSurface,
    deadline: Instant,
) -> Result<Option<AXElement>, AdapterError> {
    match surface {
        SnapshotSurface::Menubar => menubar_for_pid(pid, deadline),
        SnapshotSurface::Menu => menu_element_for_pid(pid, deadline),
        _ => Err(AdapterError::not_supported("native menu surface")),
    }
}

#[cfg(target_os = "macos")]
fn menubar_for_pid(pid: i32, deadline: Instant) -> Result<Option<AXElement>, AdapterError> {
    let app = element_for_pid(pid);
    let Some(menubar) = surface_read::element(&app, "AXMenuBar", deadline)? else {
        return Ok(None);
    };
    if !has_role(&menubar, "AXMenuBar", deadline)? {
        return Err(AdapterError::new(ErrorCode::AppUnresponsive, "Application returned an invalid menu bar root")
            .with_details(serde_json::json!({ "kind": "surface_root_invalid", "surface": "menubar", "complete": false })));
    }
    Ok(Some(menubar))
}

#[cfg(target_os = "macos")]
fn menu_element_for_pid(pid: i32, deadline: Instant) -> Result<Option<AXElement>, AdapterError> {
    Ok(open_menus_for_pid(pid, deadline)?
        .into_iter()
        .next()
        .map(|(_, menu)| menu))
}

pub(crate) fn open_menus_for_pid(
    pid: i32,
    deadline: Instant,
) -> Result<Vec<(&'static str, AXElement)>, AdapterError> {
    let mut menus = Vec::new();
    if let Some(menubar) = menubar_for_pid(pid, deadline)? {
        for item in surface_read::elements(&menubar, "AXChildren", deadline)? {
            if !has_role(&item, "AXMenuBarItem", deadline)?
                || surface_read::boolean(&item, "AXSelected", deadline)? != Some(true)
            {
                continue;
            }
            for child in surface_read::elements(&item, "AXChildren", deadline)? {
                if has_role(&child, "AXMenu", deadline)? {
                    menus.push(("menu", child));
                }
            }
        }
    }
    if menus.is_empty()
        && let Some(menu) = context_menu_from_app(pid, deadline)?
    {
        menus.push(("context_menu", menu));
    }
    Ok(menus)
}

#[cfg(target_os = "macos")]
fn context_menu_from_app(pid: i32, deadline: Instant) -> Result<Option<AXElement>, AdapterError> {
    let app = element_for_pid(pid);
    for menu in surface_read::elements(&app, "AXMenus", deadline)? {
        if displayed_menu(&menu, deadline)? {
            return Ok(Some(menu));
        }
    }
    if let Some(focused) = surface_read::element(&app, "AXFocusedUIElement", deadline)?
        && let Some(menu) = find_menu_descendant(focused, deadline)?
    {
        return Ok(Some(menu));
    }
    for child in surface_read::elements(&app, "AXChildren", deadline)? {
        if let Some(menu) = find_menu_descendant(child, deadline)? {
            return Ok(Some(menu));
        }
    }
    Ok(None)
}

#[cfg(target_os = "macos")]
fn find_menu_descendant(
    root: AXElement,
    deadline: Instant,
) -> Result<Option<AXElement>, AdapterError> {
    find_menu_descendant_with(root, deadline, |element| {
        let role = surface_read::string(element, "AXRole", deadline)?;
        if role.as_deref() == Some("AXMenuBar") {
            return Ok((false, Vec::new()));
        }
        if role.as_deref() == Some("AXMenu")
            && surface_read::boolean(element, "AXVisible", deadline)? != Some(false)
        {
            return Ok((true, Vec::new()));
        }
        Ok((
            false,
            surface_read::elements(element, "AXChildren", deadline)?,
        ))
    })
}

fn find_menu_descendant_with<T>(
    root: T,
    deadline: Instant,
    mut inspect: impl FnMut(&T) -> Result<(bool, Vec<T>), AdapterError>,
) -> Result<Option<T>, AdapterError> {
    let mut pending = std::collections::VecDeque::from([root]);
    let mut incomplete = None;
    let mut visited = 0_usize;
    while let Some(element) = pending.pop_front() {
        surface_read::ensure_before_deadline(deadline)?;
        visited += 1;
        if visited > MAX_SURFACE_NODES {
            return Err(surface_limit_error());
        }
        let (is_menu, children) = match inspect(&element) {
            Ok(observed) => observed,
            Err(error) if error.code == ErrorCode::AppUnresponsive && !invalidated(&error) => {
                incomplete.get_or_insert(error);
                continue;
            }
            Err(error) => return Err(error),
        };
        if is_menu {
            return Ok(Some(element));
        }
        pending.extend(children);
    }
    incomplete.map_or(Ok(None), Err)
}

#[cfg(target_os = "macos")]
fn displayed_menu(element: &AXElement, deadline: Instant) -> Result<bool, AdapterError> {
    Ok(has_role(element, "AXMenu", deadline)?
        && surface_read::boolean(element, "AXVisible", deadline)? != Some(false))
}

#[cfg(target_os = "macos")]
fn has_role(element: &AXElement, expected: &str, deadline: Instant) -> Result<bool, AdapterError> {
    Ok(surface_read::string(element, "AXRole", deadline)?.as_deref() == Some(expected))
}

fn surface_limit_error() -> AdapterError {
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Accessibility surface search exceeded its node budget",
    )
    .with_details(serde_json::json!({
        "kind": "surface_search_limit",
        "limit": MAX_SURFACE_NODES,
        "complete": false,
    }))
    .with_suggestion("Retry with the target application in a more stable UI state")
}

#[cfg(test)]
#[path = "app_menu_tests.rs"]
mod tests;
