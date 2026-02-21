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

    /// Attempts a 10-step AX-first activation chain before falling back to CGEvent.
    pub fn smart_activate(el: &AXElement) -> Result<(), AdapterError> {
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

        if try_set_selected(el) {
            return Ok(());
        }
        if try_select_via_parent(el) {
            return Ok(());
        }
        if try_focus_then_activate(el) {
            return Ok(());
        }
        if try_child_activation(el) {
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

    /// AXShowMenu first, then CGEvent right-click.
    pub fn smart_right_activate(el: &AXElement) -> Result<(), AdapterError> {
        let actions = list_ax_actions(el);
        if try_action_from_list(el, &actions, &["AXShowMenu"]) {
            return Ok(());
        }
        crate::actions::dispatch::click_via_bounds(el, MouseButton::Right, 1)
    }

    /// Three smart_activate calls with gaps, then CGEvent triple-click.
    pub fn smart_triple_activate(el: &AXElement) -> Result<(), AdapterError> {
        for _ in 0..3 {
            let _ = smart_activate(el);
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        crate::actions::dispatch::click_via_bounds(el, MouseButton::Left, 3)
    }

    pub fn is_attr_settable_pub(el: &AXElement, attr: &str) -> bool {
        is_attr_settable(el, attr)
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

    fn is_attr_settable(el: &AXElement, attr: &str) -> bool {
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
            let press = CFString::new("AXPress");
            if unsafe { AXUIElementPerformAction(child.0, press.as_concrete_TypeRef()) }
                == kAXErrorSuccess
            {
                return true;
            }
            let confirm = CFString::new("AXConfirm");
            if unsafe { AXUIElementPerformAction(child.0, confirm.as_concrete_TypeRef()) }
                == kAXErrorSuccess
            {
                return true;
            }
        }
        false
    }

    fn try_parent_activation(el: &AXElement) -> bool {
        if let Some(parent) = crate::tree::copy_element_attr(el, "AXParent") {
            let press = CFString::new("AXPress");
            if unsafe { AXUIElementPerformAction(parent.0, press.as_concrete_TypeRef()) }
                == kAXErrorSuccess
            {
                return true;
            }
            let confirm = CFString::new("AXConfirm");
            if unsafe { AXUIElementPerformAction(parent.0, confirm.as_concrete_TypeRef()) }
                == kAXErrorSuccess
            {
                return true;
            }
            if let Some(grandparent) = crate::tree::copy_element_attr(&parent, "AXParent") {
                let press = CFString::new("AXPress");
                if unsafe { AXUIElementPerformAction(grandparent.0, press.as_concrete_TypeRef()) }
                    == kAXErrorSuccess
                {
                    return true;
                }
                let confirm = CFString::new("AXConfirm");
                if unsafe { AXUIElementPerformAction(grandparent.0, confirm.as_concrete_TypeRef()) }
                    == kAXErrorSuccess
                {
                    return true;
                }
            }
        }
        false
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
    pub fn is_attr_settable_pub(_el: &AXElement, _attr: &str) -> bool {
        false
    }
}

pub(crate) use imp::{
    is_attr_settable_pub, smart_activate, smart_double_activate, smart_right_activate,
    smart_triple_activate,
};
