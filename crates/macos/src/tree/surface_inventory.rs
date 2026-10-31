use crate::tree::{AXElement, properties::element::element_for_pid};
use agent_desktop_core::{AdapterError, SurfaceInfo};
use std::time::Instant;

pub(crate) fn list_surfaces_for_process(
    identity: crate::system::process::process_identity::ProcessIdentity,
    deadline: Instant,
) -> Result<Vec<SurfaceInfo>, AdapterError> {
    super::app_menu::observe(identity, deadline, || {
        list_surfaces_once(identity.pid(), deadline)
    })
}

fn list_surfaces_once(pid: i32, deadline: Instant) -> Result<Vec<SurfaceInfo>, AdapterError> {
    let app = element_for_pid(pid);
    let mut surfaces = Vec::new();
    for (index, (kind, menu)) in super::app_menu::open_menus_for_pid(pid, deadline)?
        .into_iter()
        .enumerate()
    {
        push_menu_surface(
            &menu,
            kind,
            format!("app/{kind}/{index}"),
            deadline,
            &mut surfaces,
        )?;
    }
    if let Some(window) = read_element(&app, "AXFocusedWindow", deadline)? {
        collect_window_surfaces(&window, "focused-window", deadline, &mut surfaces)?;
    }
    ensure_before_deadline(deadline)?;
    Ok(surfaces)
}

fn push_menu_surface(
    menu: &AXElement,
    kind: &str,
    id: String,
    deadline: Instant,
    surfaces: &mut Vec<SurfaceInfo>,
) -> Result<(), AdapterError> {
    let title = read_title(menu, deadline)?;
    let item_count = read_array(menu, "AXChildren", deadline)?.map(|items| items.len());
    surfaces.push(SurfaceInfo {
        id,
        kind: kind.into(),
        title,
        item_count,
        unclassified: Vec::new(),
    });
    Ok(())
}

fn collect_window_surfaces(
    window: &AXElement,
    window_id: &str,
    deadline: Instant,
    surfaces: &mut Vec<SurfaceInfo>,
) -> Result<(), AdapterError> {
    if let Some(kind) = surface_kind(window, deadline)? {
        surfaces.push(SurfaceInfo {
            id: window_id.to_string(),
            kind: kind.into(),
            title: read_title(window, deadline)?,
            item_count: None,
            unclassified: Vec::new(),
        });
    }
    let Some(children) = read_array(window, "AXChildren", deadline)? else {
        return Ok(());
    };
    for (index, child) in children.iter().enumerate() {
        if let Some(kind) = surface_kind(child, deadline)? {
            surfaces.push(SurfaceInfo {
                id: format!("{window_id}/children/{index}"),
                kind: kind.into(),
                title: read_title(child, deadline)?,
                item_count: None,
                unclassified: Vec::new(),
            });
        }
    }
    Ok(())
}

fn surface_kind(
    element: &AXElement,
    deadline: Instant,
) -> Result<Option<&'static str>, AdapterError> {
    let role = read_string(element, "AXRole", deadline)?;
    let subrole = read_string(element, "AXSubrole", deadline)?;
    Ok(match subrole.as_deref() {
        Some("AXSheet") => Some("sheet"),
        Some("AXPopover") => Some("popover"),
        Some("AXDialog") | Some("AXAlert") => Some("alert"),
        _ => match role.as_deref() {
            Some("AXSheet") => Some("sheet"),
            Some("AXPopover") => Some("popover"),
            _ => None,
        },
    })
}

fn read_title(element: &AXElement, deadline: Instant) -> Result<Option<String>, AdapterError> {
    match read_string(element, "AXTitle", deadline)? {
        Some(title) => Ok(Some(title)),
        None => read_string(element, "AXDescription", deadline),
    }
}

fn read_string(
    element: &AXElement,
    attribute: &str,
    deadline: Instant,
) -> Result<Option<String>, AdapterError> {
    crate::tree::surface_read::string(element, attribute, deadline)
}

fn read_array(
    element: &AXElement,
    attribute: &str,
    deadline: Instant,
) -> Result<Option<Vec<AXElement>>, AdapterError> {
    crate::tree::surface_read::elements(element, attribute, deadline).map(Some)
}

fn read_element(
    element: &AXElement,
    attribute: &str,
    deadline: Instant,
) -> Result<Option<AXElement>, AdapterError> {
    crate::tree::surface_read::element(element, attribute, deadline)
}

fn ensure_before_deadline(deadline: Instant) -> Result<(), AdapterError> {
    crate::tree::surface_read::ensure_before_deadline(deadline)
}

#[cfg(test)]
#[path = "surface_inventory_tests.rs"]
mod tests;
