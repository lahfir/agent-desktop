use crate::tree::{copy_ax_array, copy_string_attr, element_for_pid, AXElement};
use agent_desktop_core::node::SurfaceInfo;

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use accessibility_sys::{kAXErrorSuccess, AXUIElementCopyAttributeValue, AXUIElementRef};
    use core_foundation::{base::{CFTypeRef, TCFType}, string::CFString};

    fn copy_element_attr(el: &AXElement, attr: &str) -> Option<AXElement> {
        let cf_attr = CFString::new(attr);
        let mut value: CFTypeRef = std::ptr::null_mut();
        let err = unsafe {
            AXUIElementCopyAttributeValue(el.0, cf_attr.as_concrete_TypeRef(), &mut value)
        };
        if err != kAXErrorSuccess || value.is_null() {
            return None;
        }
        Some(AXElement(value as AXUIElementRef))
    }

    fn focused_window_element(pid: i32) -> Option<AXElement> {
        let app = element_for_pid(pid);
        copy_element_attr(&app, "AXFocusedWindow")
    }

    pub fn menu_element_for_pid(pid: i32) -> Option<AXElement> {
        let app = element_for_pid(pid);
        copy_ax_array(&app, "AXMenus")?.into_iter().next()
    }

    pub fn focused_surface_for_pid(pid: i32) -> Option<AXElement> {
        focused_window_element(pid)
    }

    fn first_child_with_subrole(pid: i32, subrole: &str) -> Option<AXElement> {
        let win = focused_window_element(pid)?;
        let children = copy_ax_array(&win, "AXChildren")?;
        children.into_iter().find(|child| {
            copy_string_attr(child, "AXSubrole").as_deref() == Some(subrole)
        })
    }

    pub fn sheet_for_pid(pid: i32) -> Option<AXElement> {
        first_child_with_subrole(pid, "AXSheet")
    }

    pub fn popover_for_pid(pid: i32) -> Option<AXElement> {
        first_child_with_subrole(pid, "AXPopover")
    }

    pub fn alert_for_pid(pid: i32) -> Option<AXElement> {
        let win = focused_window_element(pid)?;
        let children = copy_ax_array(&win, "AXChildren")?;
        children.into_iter().find(|child| {
            let subrole = copy_string_attr(child, "AXSubrole");
            matches!(subrole.as_deref(), Some("AXDialog") | Some("AXAlert"))
        })
    }

    pub fn is_menu_open(pid: i32) -> bool {
        let app = element_for_pid(pid);
        copy_ax_array(&app, "AXMenus").map(|v| !v.is_empty()).unwrap_or(false)
    }

    pub fn list_surfaces_for_pid(pid: i32) -> Vec<SurfaceInfo> {
        let mut surfaces = Vec::new();
        let app = element_for_pid(pid);

        if let Some(menus) = copy_ax_array(&app, "AXMenus") {
            for menu in &menus {
                let title = copy_string_attr(menu, "AXTitle")
                    .or_else(|| copy_string_attr(menu, "AXDescription"));
                let item_count = copy_ax_array(menu, "AXChildren").map(|v| v.len());
                surfaces.push(SurfaceInfo { kind: "menu".into(), title, item_count });
            }
        }

        if let Some(win) = focused_window_element(pid) {
            if let Some(children) = copy_ax_array(&win, "AXChildren") {
                for child in &children {
                    let subrole = copy_string_attr(child, "AXSubrole");
                    let kind = match subrole.as_deref() {
                        Some("AXSheet") => "sheet",
                        Some("AXPopover") => "popover",
                        Some("AXDialog") | Some("AXAlert") => "alert",
                        _ => continue,
                    };
                    let title = copy_string_attr(child, "AXTitle")
                        .or_else(|| copy_string_attr(child, "AXDescription"));
                    surfaces.push(SurfaceInfo { kind: kind.into(), title, item_count: None });
                }
            }
        }

        surfaces
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;

    pub fn menu_element_for_pid(_pid: i32) -> Option<AXElement> { None }
    pub fn focused_surface_for_pid(_pid: i32) -> Option<AXElement> { None }
    pub fn sheet_for_pid(_pid: i32) -> Option<AXElement> { None }
    pub fn popover_for_pid(_pid: i32) -> Option<AXElement> { None }
    pub fn alert_for_pid(_pid: i32) -> Option<AXElement> { None }
    pub fn is_menu_open(_pid: i32) -> bool { false }
    pub fn list_surfaces_for_pid(_pid: i32) -> Vec<SurfaceInfo> { Vec::new() }
}

pub use imp::{
    alert_for_pid, focused_surface_for_pid, is_menu_open, list_surfaces_for_pid,
    menu_element_for_pid, popover_for_pid, sheet_for_pid,
};
