use agent_desktop_core::{action::MouseButton, error::AdapterError};

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use crate::tree::AXElement;
    use accessibility_sys::{
        kAXErrorSuccess, kAXFocusedAttribute, kAXRoleAttribute, AXUIElementCopyActionNames,
        AXUIElementIsAttributeSettable, AXUIElementPerformAction, AXUIElementSetAttributeValue,
    };
    use core_foundation::{
        array::CFArray,
        base::{CFRetain, CFType, CFTypeRef, TCFType},
        boolean::CFBoolean,
        string::CFString,
    };
    use std::os::raw::c_uchar;

    pub fn smart_activate(el: &AXElement) -> Result<(), AdapterError> {
        let scroll_action = CFString::new("AXScrollToVisible");
        unsafe { AXUIElementPerformAction(el.0, scroll_action.as_concrete_TypeRef()) };

        let actions = list_ax_actions(el);

        if try_action_from_list(el, &actions, &["AXPress"]) {
            return Ok(());
        }
        if try_action_from_list(el, &actions, &["AXConfirm"]) {
            return Ok(());
        }
        if try_action_from_list(el, &actions, &["AXOpen"]) {
            return Ok(());
        }
        if try_action_from_list(el, &actions, &["AXPick"]) {
            return Ok(());
        }
        if try_show_alternate_ui(el) {
            return Ok(());
        }

        if try_child_activation(el) {
            return Ok(());
        }
        if try_set_selected(el) {
            return Ok(());
        }
        if try_select_via_parent(el) {
            return Ok(());
        }
        if try_custom_actions(el) {
            return Ok(());
        }
        if try_focus_then_activate(el) {
            return Ok(());
        }
        if try_keyboard_activate(el) {
            return Ok(());
        }
        if try_parent_activation(el) {
            return Ok(());
        }

        crate::actions::dispatch::click_via_bounds(el, MouseButton::Left, 1)
    }

    /// AXOpen first, then two smart_activate calls with a gap, then CGEvent double-click.
    pub fn smart_double_activate(el: &AXElement) -> Result<(), AdapterError> {
        let actions = list_ax_actions(el);
        if try_action_from_list(el, &actions, &["AXOpen"]) {
            return Ok(());
        }

        let _ = smart_activate(el);
        std::thread::sleep(std::time::Duration::from_millis(50));
        let _ = smart_activate(el);

        crate::actions::dispatch::click_via_bounds(el, MouseButton::Left, 2)
    }

    pub fn smart_right_activate(el: &AXElement) -> Result<(), AdapterError> {
        if ax_show_menu(el) {
            return Ok(());
        }

        if let Some(pid) = crate::system::app_ops::pid_from_element(el) {
            let _ = crate::system::app_ops::ensure_app_focused(pid);
            std::thread::sleep(std::time::Duration::from_millis(50));
            if ax_show_menu(el) {
                return Ok(());
            }
        }

        if try_select_then_show_menu(el) {
            return Ok(());
        }

        if try_focus_then_show_menu(el) {
            return Ok(());
        }

        if try_parent_show_menu(el) {
            return Ok(());
        }

        if try_child_show_menu(el) {
            return Ok(());
        }
        crate::actions::dispatch::click_via_bounds(el, MouseButton::Right, 1)
    }

    fn ax_show_menu(el: &AXElement) -> bool {
        let show = CFString::new("AXShowMenu");
        let err = unsafe { AXUIElementPerformAction(el.0, show.as_concrete_TypeRef()) };
        err == kAXErrorSuccess
    }

    fn try_select_then_show_menu(el: &AXElement) -> bool {
        if !is_attr_settable(el, "AXSelected") {
            return false;
        }
        let cf_attr = CFString::new("AXSelected");
        let err = unsafe {
            AXUIElementSetAttributeValue(
                el.0,
                cf_attr.as_concrete_TypeRef(),
                CFBoolean::true_value().as_CFTypeRef(),
            )
        };
        if err != kAXErrorSuccess {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        ax_show_menu(el)
    }

    fn try_focus_then_show_menu(el: &AXElement) -> bool {
        let cf_attr = CFString::new(kAXFocusedAttribute);
        let err = unsafe {
            AXUIElementSetAttributeValue(
                el.0,
                cf_attr.as_concrete_TypeRef(),
                CFBoolean::true_value().as_CFTypeRef(),
            )
        };
        if err != kAXErrorSuccess {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        ax_show_menu(el)
    }

    fn try_parent_show_menu(el: &AXElement) -> bool {
        let mut current = crate::tree::copy_element_attr(el, "AXParent");
        for _ in 0..3 {
            let ancestor = match &current {
                Some(a) => a,
                None => return false,
            };
            if ax_show_menu(ancestor) {
                return true;
            }
            current = crate::tree::copy_element_attr(ancestor, "AXParent");
        }
        false
    }

    fn try_child_show_menu(el: &AXElement) -> bool {
        let children = crate::tree::copy_ax_array(el, "AXChildren").unwrap_or_default();
        for child in children.iter().take(5) {
            if ax_show_menu(child) {
                return true;
            }
        }
        false
    }

    /// Three smart_activate calls with gaps, then CGEvent triple-click.
    pub fn smart_triple_activate(el: &AXElement) -> Result<(), AdapterError> {
        for _ in 0..3 {
            let _ = smart_activate(el);
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        crate::actions::dispatch::click_via_bounds(el, MouseButton::Left, 3)
    }

    fn list_ax_actions(el: &AXElement) -> Vec<String> {
        let mut actions_ref: core_foundation_sys::array::CFArrayRef = std::ptr::null();
        let err = unsafe { AXUIElementCopyActionNames(el.0, &mut actions_ref) };
        if err != kAXErrorSuccess || actions_ref.is_null() {
            return Vec::new();
        }
        let actions: CFArray<CFType> = unsafe { TCFType::wrap_under_create_rule(actions_ref) };
        let mut result = Vec::new();
        for i in 0..actions.len() {
            if let Some(name) = actions.get(i).and_then(|v| v.downcast::<CFString>()) {
                result.push(name.to_string());
            }
        }
        result
    }

    pub fn is_attr_settable(el: &AXElement, attr: &str) -> bool {
        let cf_attr = CFString::new(attr);
        let mut settable: c_uchar = 0;
        let err = unsafe {
            AXUIElementIsAttributeSettable(el.0, cf_attr.as_concrete_TypeRef(), &mut settable)
        };
        err == kAXErrorSuccess && settable != 0
    }

    fn try_action_from_list(el: &AXElement, actions: &[String], targets: &[&str]) -> bool {
        for target in targets {
            if actions.iter().any(|a| a == target) {
                let action = CFString::new(target);
                let err = unsafe { AXUIElementPerformAction(el.0, action.as_concrete_TypeRef()) };
                if err == kAXErrorSuccess {
                    return true;
                }
            }
        }
        false
    }

    fn try_set_selected(el: &AXElement) -> bool {
        if !is_attr_settable(el, "AXSelected") {
            return false;
        }
        let cf_attr = CFString::new("AXSelected");
        let err = unsafe {
            AXUIElementSetAttributeValue(
                el.0,
                cf_attr.as_concrete_TypeRef(),
                CFBoolean::true_value().as_CFTypeRef(),
            )
        };
        err == kAXErrorSuccess
    }

    fn try_select_via_parent(el: &AXElement) -> bool {
        let parent = match crate::tree::copy_element_attr(el, "AXParent") {
            Some(p) => p,
            None => return false,
        };
        let parent_role = match crate::tree::copy_string_attr(&parent, kAXRoleAttribute) {
            Some(r) => r,
            None => return false,
        };
        if !matches!(parent_role.as_str(), "AXTable" | "AXOutline" | "AXList") {
            return false;
        }
        if !is_attr_settable(&parent, "AXSelectedRows") {
            return false;
        }
        unsafe { CFRetain(el.0 as CFTypeRef) };
        let el_as_cftype = unsafe { CFType::wrap_under_create_rule(el.0 as CFTypeRef) };
        let arr = CFArray::from_CFTypes(&[el_as_cftype]);
        let cf_attr = CFString::new("AXSelectedRows");
        let err = unsafe {
            AXUIElementSetAttributeValue(
                parent.0,
                cf_attr.as_concrete_TypeRef(),
                arr.as_CFTypeRef(),
            )
        };
        err == kAXErrorSuccess
    }

    fn try_focus_then_activate(el: &AXElement) -> bool {
        let cf_attr = CFString::new(kAXFocusedAttribute);
        let err = unsafe {
            AXUIElementSetAttributeValue(
                el.0,
                cf_attr.as_concrete_TypeRef(),
                CFBoolean::true_value().as_CFTypeRef(),
            )
        };
        if err != kAXErrorSuccess {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        let confirm = CFString::new("AXConfirm");
        if unsafe { AXUIElementPerformAction(el.0, confirm.as_concrete_TypeRef()) }
            == kAXErrorSuccess
        {
            return true;
        }
        let press = CFString::new("AXPress");
        let err = unsafe { AXUIElementPerformAction(el.0, press.as_concrete_TypeRef()) };
        err == kAXErrorSuccess
    }

    fn try_child_activation(el: &AXElement) -> bool {
        let children = crate::tree::copy_ax_array(el, "AXChildren").unwrap_or_default();
        for child in children.iter().take(3) {
            let child_actions = list_ax_actions(child);
            if try_action_from_list(child, &child_actions, &["AXPress", "AXConfirm", "AXOpen"]) {
                return true;
            }
        }
        false
    }

    fn try_parent_activation(el: &AXElement) -> bool {
        let mut current = crate::tree::copy_element_attr(el, "AXParent");
        for _ in 0..2 {
            let ancestor = match &current {
                Some(a) => a,
                None => return false,
            };
            let actions = list_ax_actions(ancestor);
            if try_action_from_list(ancestor, &actions, &["AXPress", "AXConfirm"]) {
                return true;
            }
            current = crate::tree::copy_element_attr(ancestor, "AXParent");
        }
        false
    }

    fn try_show_alternate_ui(el: &AXElement) -> bool {
        let actions = list_ax_actions(el);
        if !actions.iter().any(|a| a == "AXShowAlternateUI") {
            return false;
        }
        let action = CFString::new("AXShowAlternateUI");
        unsafe { AXUIElementPerformAction(el.0, action.as_concrete_TypeRef()) };
        std::thread::sleep(std::time::Duration::from_millis(100));
        let children = crate::tree::copy_ax_array(el, "AXChildren").unwrap_or_default();
        for child in children.iter().take(5) {
            let child_actions = list_ax_actions(child);
            if try_action_from_list(child, &child_actions, &["AXPress"]) {
                return true;
            }
        }
        false
    }

    fn try_custom_actions(el: &AXElement) -> bool {
        let custom = crate::tree::copy_ax_array(el, "AXCustomActions").unwrap_or_default();
        if custom.is_empty() {
            return false;
        }
        let action = CFString::new("AXPerformCustomAction");
        let err = unsafe { AXUIElementPerformAction(el.0, action.as_concrete_TypeRef()) };
        err == kAXErrorSuccess
    }

    fn try_keyboard_activate(el: &AXElement) -> bool {
        use accessibility_sys::AXUIElementPostKeyboardEvent;
        let cf_focused = CFString::new(kAXFocusedAttribute);
        let err = unsafe {
            AXUIElementSetAttributeValue(
                el.0,
                cf_focused.as_concrete_TypeRef(),
                CFBoolean::true_value().as_CFTypeRef(),
            )
        };
        if err != kAXErrorSuccess {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        let pid = match crate::system::app_ops::pid_from_element(el) {
            Some(p) => p,
            None => return false,
        };
        let app = crate::tree::element_for_pid(pid);
        unsafe {
            AXUIElementPostKeyboardEvent(app.0, 0, 49, true);
            AXUIElementPostKeyboardEvent(app.0, 0, 49, false);
        };
        true
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;
    use crate::tree::AXElement;

    pub fn smart_activate(_el: &AXElement) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("smart_activate"))
    }
    pub fn smart_double_activate(_el: &AXElement) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("smart_double_activate"))
    }
    pub fn smart_right_activate(_el: &AXElement) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("smart_right_activate"))
    }
    pub fn smart_triple_activate(_el: &AXElement) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("smart_triple_activate"))
    }
    pub fn is_attr_settable(_el: &AXElement, _attr: &str) -> bool {
        false
    }
}

pub(crate) use imp::{
    is_attr_settable, smart_activate, smart_double_activate, smart_right_activate,
    smart_triple_activate,
};
