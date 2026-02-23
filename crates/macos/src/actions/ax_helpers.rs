use agent_desktop_core::error::AdapterError;

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use crate::tree::AXElement;
    use accessibility_sys::{
        kAXErrorCannotComplete, kAXErrorSuccess, kAXFocusedAttribute, kAXValueAttribute,
        AXUIElementCopyActionNames, AXUIElementIsAttributeSettable, AXUIElementPerformAction,
        AXUIElementSetAttributeValue, AXUIElementSetMessagingTimeout,
    };
    use core_foundation::{
        array::CFArray,
        base::{CFType, TCFType},
        boolean::CFBoolean,
        string::CFString,
    };
    use std::os::raw::c_uchar;

    pub fn try_ax_action(el: &AXElement, name: &str) -> bool {
        let action = CFString::new(name);
        let err = unsafe { AXUIElementPerformAction(el.0, action.as_concrete_TypeRef()) };
        err == kAXErrorSuccess
    }

    pub fn try_ax_action_retried(el: &AXElement, name: &str) -> bool {
        let action = CFString::new(name);
        let err = unsafe { AXUIElementPerformAction(el.0, action.as_concrete_TypeRef()) };
        if err == kAXErrorSuccess {
            return true;
        }
        if err == kAXErrorCannotComplete {
            std::thread::sleep(std::time::Duration::from_millis(100));
            let retry = unsafe { AXUIElementPerformAction(el.0, action.as_concrete_TypeRef()) };
            return retry == kAXErrorSuccess;
        }
        false
    }

    pub fn set_ax_bool(el: &AXElement, attr: &str, value: bool) -> bool {
        let cf_attr = CFString::new(attr);
        let cf_val = if value {
            CFBoolean::true_value()
        } else {
            CFBoolean::false_value()
        };
        let err = unsafe {
            AXUIElementSetAttributeValue(el.0, cf_attr.as_concrete_TypeRef(), cf_val.as_CFTypeRef())
        };
        err == kAXErrorSuccess
    }

    pub fn set_ax_string_or_err(
        el: &AXElement,
        attr: &str,
        value: &str,
    ) -> Result<(), AdapterError> {
        let cf_attr = CFString::new(attr);
        let cf_val = CFString::new(value);
        let err = unsafe {
            AXUIElementSetAttributeValue(el.0, cf_attr.as_concrete_TypeRef(), cf_val.as_CFTypeRef())
        };
        if err != kAXErrorSuccess {
            return Err(AdapterError::new(
                agent_desktop_core::error::ErrorCode::ActionFailed,
                format!("AXSetAttributeValue({attr}) failed (err={err})"),
            )
            .with_suggestion("Attribute may be read-only. Try 'click' or 'type' instead."));
        }
        Ok(())
    }

    pub fn is_attr_settable(el: &AXElement, attr: &str) -> bool {
        let cf_attr = CFString::new(attr);
        let mut settable: c_uchar = 0;
        let err = unsafe {
            AXUIElementIsAttributeSettable(el.0, cf_attr.as_concrete_TypeRef(), &mut settable)
        };
        err == kAXErrorSuccess && settable != 0
    }

    pub fn list_ax_actions(el: &AXElement) -> Vec<String> {
        let mut actions_ref: core_foundation_sys::array::CFArrayRef = std::ptr::null();
        let err = unsafe { AXUIElementCopyActionNames(el.0, &mut actions_ref) };
        if err != kAXErrorSuccess || actions_ref.is_null() {
            return Vec::new();
        }
        let actions: CFArray<CFType> = unsafe { TCFType::wrap_under_create_rule(actions_ref) };
        let mut result = Vec::with_capacity(actions.len() as usize);
        for i in 0..actions.len() {
            if let Some(name) = actions.get(i).and_then(|v| v.downcast::<CFString>()) {
                result.push(name.to_string());
            }
        }
        result
    }

    pub fn has_ax_action(el: &AXElement, target: &str) -> bool {
        list_ax_actions(el).iter().any(|a| a == target)
    }

    pub fn try_action_from_list(el: &AXElement, actions: &[String], targets: &[&str]) -> bool {
        for target in targets {
            if actions.iter().any(|a| a == target) && try_ax_action(el, target) {
                return true;
            }
        }
        false
    }

    pub fn try_each_child(el: &AXElement, f: impl Fn(&AXElement) -> bool, limit: usize) -> bool {
        let children = crate::tree::copy_ax_array(el, "AXChildren").unwrap_or_default();
        for child in children.iter().take(limit) {
            if f(child) {
                return true;
            }
        }
        false
    }

    pub fn try_each_ancestor(el: &AXElement, f: impl Fn(&AXElement) -> bool, limit: usize) -> bool {
        let mut current = crate::tree::copy_element_attr(el, "AXParent");
        for _ in 0..limit {
            let ancestor = match &current {
                Some(a) => a,
                None => return false,
            };
            if f(ancestor) {
                return true;
            }
            current = crate::tree::copy_element_attr(ancestor, "AXParent");
        }
        false
    }

    pub fn ensure_visible(el: &AXElement) {
        let action = CFString::new("AXScrollToVisible");
        unsafe { AXUIElementPerformAction(el.0, action.as_concrete_TypeRef()) };
    }

    pub fn set_messaging_timeout(el: &AXElement, seconds: f32) {
        unsafe { AXUIElementSetMessagingTimeout(el.0, seconds) };
    }

    pub fn ax_focus(el: &AXElement) -> bool {
        set_ax_bool(el, kAXFocusedAttribute, true)
    }

    pub fn ax_set_value(el: &AXElement, val: &str) -> Result<(), AdapterError> {
        set_ax_string_or_err(el, kAXValueAttribute, val)
    }

    pub fn ax_press(el: &AXElement) -> bool {
        try_ax_action(el, "AXPress")
    }

    pub fn element_role(el: &AXElement) -> Option<String> {
        use accessibility_sys::kAXRoleAttribute;
        crate::tree::copy_string_attr(el, kAXRoleAttribute)
            .map(|r| crate::tree::roles::ax_role_to_str(&r).to_string())
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;
    use crate::tree::AXElement;

    pub fn try_ax_action(_el: &AXElement, _name: &str) -> bool {
        false
    }
    pub fn try_ax_action_retried(_el: &AXElement, _name: &str) -> bool {
        false
    }
    pub fn set_ax_bool(_el: &AXElement, _attr: &str, _value: bool) -> bool {
        false
    }
    pub fn set_ax_string_or_err(
        _el: &AXElement,
        _attr: &str,
        _value: &str,
    ) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("set_ax_string_or_err"))
    }
    pub fn is_attr_settable(_el: &AXElement, _attr: &str) -> bool {
        false
    }
    pub fn list_ax_actions(_el: &AXElement) -> Vec<String> {
        Vec::new()
    }
    pub fn has_ax_action(_el: &AXElement, _target: &str) -> bool {
        false
    }
    pub fn try_action_from_list(_el: &AXElement, _actions: &[String], _targets: &[&str]) -> bool {
        false
    }
    pub fn try_each_child(_el: &AXElement, _f: impl Fn(&AXElement) -> bool, _limit: usize) -> bool {
        false
    }
    pub fn try_each_ancestor(
        _el: &AXElement,
        _f: impl Fn(&AXElement) -> bool,
        _limit: usize,
    ) -> bool {
        false
    }
    pub fn ensure_visible(_el: &AXElement) {}
    pub fn set_messaging_timeout(_el: &AXElement, _seconds: f32) {}
    pub fn ax_focus(_el: &AXElement) -> bool {
        false
    }
    pub fn ax_set_value(_el: &AXElement, _val: &str) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("ax_set_value"))
    }
    pub fn ax_press(_el: &AXElement) -> bool {
        false
    }
    pub fn element_role(_el: &AXElement) -> Option<String> {
        None
    }
}

pub(crate) use imp::{
    ax_focus, ax_press, ax_set_value, element_role, ensure_visible, has_ax_action,
    is_attr_settable, list_ax_actions, set_ax_bool, set_ax_string_or_err, set_messaging_timeout,
    try_action_from_list, try_ax_action, try_ax_action_retried, try_each_ancestor, try_each_child,
};
