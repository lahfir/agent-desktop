use super::element::{copy_ax_array, copy_string_attr, element_for_pid, AXElement};
use agent_desktop_core::node::SurfaceInfo;

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use accessibility_sys::{kAXErrorSuccess, AXUIElementCopyAttributeValue, AXUIElementRef};
    use core_foundation::{
        base::{CFType, CFTypeRef, TCFType},
        boolean::CFBoolean,
        string::CFString,
    };

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

    fn copy_bool_attr(el: &AXElement, attr: &str) -> Option<bool> {
        let cf_attr = CFString::new(attr);
        let mut value: CFTypeRef = std::ptr::null_mut();
        let err = unsafe {
            AXUIElementCopyAttributeValue(el.0, cf_attr.as_concrete_TypeRef(), &mut value)
        };
        if err != kAXErrorSuccess || value.is_null() {
            return None;
        }
        let cf_type = unsafe { CFType::wrap_under_create_rule(value) };
        cf_type.downcast::<CFBoolean>().map(|b| b.into())
    }

    fn focused_window_element(pid: i32) -> Option<AXElement> {
        let app = element_for_pid(pid);
        copy_element_attr(&app, "AXFocusedWindow")
    }

    fn open_menubar_menu(pid: i32) -> Option<AXElement> {
        let app = element_for_pid(pid);
        let app_children = copy_ax_array(&app, "AXChildren")?;
        let menubar = app_children
            .into_iter()
            .find(|ch| copy_string_attr(ch, "AXRole").as_deref() == Some("AXMenuBar"))?;
        let items = copy_ax_array(&menubar, "AXChildren")?;
        for item in &items {
            if copy_string_attr(item, "AXRole").as_deref() != Some("AXMenuBarItem") {
                continue;
            }
            if !copy_bool_attr(item, "AXSelected").unwrap_or(false) {
                continue;
            }
            if let Some(children) = copy_ax_array(item, "AXChildren") {
                return children
                    .into_iter()
                    .find(|ch| copy_string_attr(ch, "AXRole").as_deref() == Some("AXMenu"));
            }
        }
        None
    }

    fn context_menu_from_app(pid: i32) -> Option<AXElement> {
        let app = element_for_pid(pid);
        if let Some(focused) = copy_element_attr(&app, "AXFocusedUIElement") {
            if let Some(children) = copy_ax_array(&focused, "AXChildren") {
                if let Some(menu) = children
                    .into_iter()
                    .find(|ch| copy_string_attr(ch, "AXRole").as_deref() == Some("AXMenu"))
                {
                    return Some(menu);
                }
            }
        }
        let children = copy_ax_array(&app, "AXChildren")?;
        children
            .into_iter()
            .find(|ch| copy_string_attr(ch, "AXRole").as_deref() == Some("AXMenu"))
    }

    pub fn menu_element_for_pid(pid: i32) -> Option<AXElement> {
        open_menubar_menu(pid).or_else(|| context_menu_from_app(pid))
    }

    pub fn menubar_for_pid(pid: i32) -> Option<AXElement> {
        let app = element_for_pid(pid);
        let app_children = copy_ax_array(&app, "AXChildren")?;
        app_children
            .into_iter()
            .find(|ch| copy_string_attr(ch, "AXRole").as_deref() == Some("AXMenuBar"))
    }

    pub fn focused_surface_for_pid(pid: i32) -> Option<AXElement> {
        focused_window_element(pid)
    }

    fn first_child_with_subrole(pid: i32, subrole: &str) -> Option<AXElement> {
        let win = focused_window_element(pid)?;
        let children = copy_ax_array(&win, "AXChildren")?;
        children
            .into_iter()
            .find(|child| copy_string_attr(child, "AXSubrole").as_deref() == Some(subrole))
    }

    pub fn sheet_for_pid(pid: i32) -> Option<AXElement> {
        first_child_with_subrole(pid, "AXSheet")
    }

    pub fn popover_for_pid(pid: i32) -> Option<AXElement> {
        first_child_with_subrole(pid, "AXPopover")
    }

    pub fn alert_for_pid(pid: i32) -> Option<AXElement> {
        if let Some(win) = focused_window_element(pid) {
            let children = copy_ax_array(&win, "AXChildren").unwrap_or_default();
            if let Some(found) = children.into_iter().find(|child| {
                let subrole = copy_string_attr(child, "AXSubrole");
                matches!(
                    subrole.as_deref(),
                    Some("AXDialog") | Some("AXAlert") | Some("AXSheet")
                )
            }) {
                return Some(found);
            }
        }

        let app = element_for_pid(pid);
        let windows = copy_ax_array(&app, "AXWindows")?;
        for win in &windows {
            let role = copy_string_attr(win, "AXRole");
            let subrole = copy_string_attr(win, "AXSubrole");
            if matches!(
                subrole.as_deref(),
                Some("AXDialog") | Some("AXAlert") | Some("AXSheet")
            ) || matches!(role.as_deref(), Some("AXSheet"))
            {
                return Some(win.clone());
            }
            let children = copy_ax_array(win, "AXChildren").unwrap_or_default();
            if let Some(found) = children.into_iter().find(|child| {
                let sr = copy_string_attr(child, "AXSubrole");
                matches!(
                    sr.as_deref(),
                    Some("AXDialog") | Some("AXAlert") | Some("AXSheet")
                )
            }) {
                return Some(found);
            }
        }
        None
    }

    pub fn is_menu_open(pid: i32) -> bool {
        open_menubar_menu(pid).is_some() || context_menu_from_app(pid).is_some()
    }

    pub fn list_surfaces_for_pid(pid: i32) -> Vec<SurfaceInfo> {
        let mut surfaces = Vec::new();
        let app = element_for_pid(pid);

        if let Some(app_children) = copy_ax_array(&app, "AXChildren") {
            for ch in &app_children {
                match copy_string_attr(ch, "AXRole").as_deref() {
                    Some("AXMenuBar") => {
                        if let Some(items) = copy_ax_array(ch, "AXChildren") {
                            for item in &items {
                                if copy_string_attr(item, "AXRole").as_deref()
                                    != Some("AXMenuBarItem")
                                {
                                    continue;
                                }
                                if !copy_bool_attr(item, "AXSelected").unwrap_or(false) {
                                    continue;
                                }
                                let title = copy_string_attr(item, "AXTitle");
                                if let Some(menu_children) = copy_ax_array(item, "AXChildren") {
                                    for menu in &menu_children {
                                        if copy_string_attr(menu, "AXRole").as_deref()
                                            == Some("AXMenu")
                                        {
                                            let item_count =
                                                copy_ax_array(menu, "AXChildren").map(|v| v.len());
                                            surfaces.push(SurfaceInfo {
                                                kind: "menu".into(),
                                                title: title.clone(),
                                                item_count,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Some("AXMenu") => {
                        let title = copy_string_attr(ch, "AXTitle")
                            .or_else(|| copy_string_attr(ch, "AXDescription"));
                        let item_count = copy_ax_array(ch, "AXChildren").map(|v| v.len());
                        surfaces.push(SurfaceInfo {
                            kind: "context_menu".into(),
                            title,
                            item_count,
                        });
                    }
                    _ => {}
                }
            }
        }

        if let Some(focused) = copy_element_attr(&app, "AXFocusedUIElement") {
            if let Some(children) = copy_ax_array(&focused, "AXChildren") {
                for ch in &children {
                    if copy_string_attr(ch, "AXRole").as_deref() == Some("AXMenu") {
                        let title = copy_string_attr(ch, "AXTitle")
                            .or_else(|| copy_string_attr(ch, "AXDescription"));
                        let item_count = copy_ax_array(ch, "AXChildren").map(|v| v.len());
                        surfaces.push(SurfaceInfo {
                            kind: "context_menu".into(),
                            title,
                            item_count,
                        });
                    }
                }
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
                    surfaces.push(SurfaceInfo {
                        kind: kind.into(),
                        title,
                        item_count: None,
                    });
                }
            }
        }

        surfaces
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;

    pub fn menu_element_for_pid(_pid: i32) -> Option<AXElement> {
        None
    }
    pub fn menubar_for_pid(_pid: i32) -> Option<AXElement> {
        None
    }
    pub fn focused_surface_for_pid(_pid: i32) -> Option<AXElement> {
        None
    }
    pub fn sheet_for_pid(_pid: i32) -> Option<AXElement> {
        None
    }
    pub fn popover_for_pid(_pid: i32) -> Option<AXElement> {
        None
    }
    pub fn alert_for_pid(_pid: i32) -> Option<AXElement> {
        None
    }
    pub fn is_menu_open(_pid: i32) -> bool {
        false
    }
    pub fn list_surfaces_for_pid(_pid: i32) -> Vec<SurfaceInfo> {
        Vec::new()
    }
}

pub use imp::{
    alert_for_pid, focused_surface_for_pid, is_menu_open, list_surfaces_for_pid,
    menu_element_for_pid, menubar_for_pid, popover_for_pid, sheet_for_pid,
};
