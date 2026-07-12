use super::{AXElement, element::element_for_pid, surface_read};
use agent_desktop_core::{AdapterError, ErrorCode};
use std::time::Instant;

const MAX_SURFACE_NODES: usize = 2_048;

#[cfg(target_os = "macos")]
pub(crate) fn focused_surface_for_pid(
    pid: i32,
    deadline: Instant,
) -> Result<Option<AXElement>, AdapterError> {
    let app = element_for_pid(pid);
    surface_read::element(&app, "AXFocusedWindow", deadline)
}

#[cfg(target_os = "macos")]
pub(crate) fn menubar_for_pid(
    pid: i32,
    deadline: Instant,
) -> Result<Option<AXElement>, AdapterError> {
    let app = element_for_pid(pid);
    for child in surface_read::elements(&app, "AXChildren", deadline)? {
        if has_role(&child, "AXMenuBar", deadline)? {
            return Ok(Some(child));
        }
    }
    Ok(None)
}

#[cfg(target_os = "macos")]
pub(crate) fn menu_element_for_pid(
    pid: i32,
    deadline: Instant,
) -> Result<Option<AXElement>, AdapterError> {
    if let Some(menu) = open_menubar_menu(pid, deadline)? {
        return Ok(Some(menu));
    }
    context_menu_from_app(pid, deadline)
}

#[cfg(target_os = "macos")]
fn open_menubar_menu(pid: i32, deadline: Instant) -> Result<Option<AXElement>, AdapterError> {
    let Some(menubar) = menubar_for_pid(pid, deadline)? else {
        return Ok(None);
    };
    for item in surface_read::elements(&menubar, "AXChildren", deadline)? {
        if !has_role(&item, "AXMenuBarItem", deadline)?
            || surface_read::boolean(&item, "AXSelected", deadline)? != Some(true)
        {
            continue;
        }
        for child in surface_read::elements(&item, "AXChildren", deadline)? {
            if has_role(&child, "AXMenu", deadline)? {
                return Ok(Some(child));
            }
        }
    }
    Ok(None)
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
    let mut stack = vec![(root, 0_u8)];
    let mut visited = 0_usize;
    while let Some((element, depth)) = stack.pop() {
        surface_read::ensure_before_deadline(deadline)?;
        visited += 1;
        if visited > MAX_SURFACE_NODES {
            return Err(surface_limit_error());
        }
        if depth > 8 {
            continue;
        }
        let role = surface_read::string(&element, "AXRole", deadline)?;
        if role.as_deref() == Some("AXMenuBar") {
            continue;
        }
        if role.as_deref() == Some("AXMenu")
            && surface_read::boolean(&element, "AXVisible", deadline)? != Some(false)
        {
            return Ok(Some(element));
        }
        stack.extend(
            surface_read::elements(&element, "AXChildren", deadline)?
                .into_iter()
                .rev()
                .map(|child| (child, depth.saturating_add(1))),
        );
    }
    Ok(None)
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

#[cfg(target_os = "macos")]
fn first_child_with_role_or_subrole(
    pid: i32,
    target: &str,
    deadline: Instant,
) -> Result<Option<AXElement>, AdapterError> {
    let Some(window) = focused_surface_for_pid(pid, deadline)? else {
        return Ok(None);
    };
    if role_or_subrole_matches(&window, target, deadline)? {
        return Ok(Some(window));
    }
    for child in surface_read::elements(&window, "AXChildren", deadline)? {
        if role_or_subrole_matches(&child, target, deadline)? {
            return Ok(Some(child));
        }
    }
    Ok(None)
}

#[cfg(target_os = "macos")]
fn role_or_subrole_matches(
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
fn is_alert(element: &AXElement, deadline: Instant) -> Result<bool, AdapterError> {
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
unsupported_surface!(menubar_for_pid);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_node_limit_is_explicitly_incomplete() {
        let error = surface_limit_error();

        assert_eq!(error.code, ErrorCode::AppUnresponsive);
        assert_eq!(error.details.expect("limit details")["complete"], false);
    }
}
