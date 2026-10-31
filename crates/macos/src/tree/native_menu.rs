use agent_desktop_core::{AdapterError, Deadline, ErrorCode};

use crate::tree::{AXElement, surface_read};

pub(crate) fn is_native_menu(
    element: &AXElement,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let end = crate::tree::locator_deadline::from_operation(deadline)?;
    classify(
        element.clone(),
        deadline,
        |element| surface_read::string(element, "AXRole", end),
        |element| surface_read::element(element, "AXParent", end),
        |application, menubar| {
            Ok(surface_read::element(application, "AXMenuBar", end)?
                .is_some_and(|owned| crate::tree::same_element(&owned, menubar)))
        },
    )
}

fn classify<T: Clone>(
    mut element: T,
    deadline: Deadline,
    mut role: impl FnMut(&T) -> Result<Option<String>, AdapterError>,
    mut parent: impl FnMut(&T) -> Result<Option<T>, AdapterError>,
    mut owns_menubar: impl FnMut(&T, &T) -> Result<bool, AdapterError>,
) -> Result<bool, AdapterError> {
    let mut menubar = None;
    for depth in 0..64 {
        deadline.ensure_remaining()?;
        let role = role(&element)?;
        match role.as_deref() {
            Some("AXMenuItem" | "AXMenuBarItem") => {}
            Some("AXMenu") if depth > 0 => {}
            Some("AXMenuBar") if depth > 0 => menubar = Some(element.clone()),
            Some("AXApplication") => {
                return menubar
                    .as_ref()
                    .map_or(Ok(false), |menubar| owns_menubar(&element, menubar));
            }
            _ => return Ok(false),
        }
        let Some(next) = parent(&element)? else {
            return Ok(false);
        };
        element = next;
    }
    Err(AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Native menu ancestry exceeded its observation budget",
    )
    .with_details(serde_json::json!({ "complete": false, "kind": "menu_ancestry_limit" })))
}

#[cfg(test)]
#[path = "native_menu_tests.rs"]
mod tests;
