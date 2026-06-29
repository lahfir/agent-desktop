use agent_desktop_core::error::{AdapterError, ErrorCode};

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use crate::tree::AXElement;
    use accessibility_sys::{
        AXUIElementCopyAttributeValue, AXUIElementIsAttributeSettable, AXUIElementPerformAction,
        AXUIElementSetAttributeValue, AXUIElementSetMessagingTimeout, kAXErrorAPIDisabled,
        kAXErrorCannotComplete, kAXErrorSuccess, kAXFocusedAttribute, kAXValueAttribute,
    };
    use core_foundation::{
        base::{CFType, CFTypeRef, TCFType},
        boolean::CFBoolean,
        number::CFNumber,
        string::CFString,
    };
    use std::os::raw::c_uchar;

    pub(crate) fn try_ax_action(el: &AXElement, name: &str) -> bool {
        let action = CFString::new(name);
        let err = unsafe { AXUIElementPerformAction(el.0, action.as_concrete_TypeRef()) };
        err == kAXErrorSuccess
    }

    pub(crate) fn try_ax_action_retried(el: &AXElement, name: &str) -> bool {
        try_ax_action_retried_or_err(el, name).unwrap_or(false)
    }

    pub(crate) fn try_ax_action_retried_or_err(
        el: &AXElement,
        name: &str,
    ) -> Result<bool, AdapterError> {
        let action = CFString::new(name);
        let err = unsafe { AXUIElementPerformAction(el.0, action.as_concrete_TypeRef()) };
        if err == kAXErrorSuccess {
            return Ok(true);
        }
        if err == kAXErrorCannotComplete {
            std::thread::sleep(std::time::Duration::from_millis(100));
            let retry = unsafe { AXUIElementPerformAction(el.0, action.as_concrete_TypeRef()) };
            if retry == kAXErrorSuccess {
                return Ok(true);
            }
            ax_error_result(name, retry)?;
            return Ok(false);
        }
        ax_error_result(name, err)?;
        Ok(false)
    }

    pub(crate) fn set_ax_bool(el: &AXElement, attr: &str, value: bool) -> bool {
        set_ax_bool_or_err(el, attr, value).unwrap_or(false)
    }

    pub(crate) fn set_ax_bool_or_err(
        el: &AXElement,
        attr: &str,
        value: bool,
    ) -> Result<bool, AdapterError> {
        let cf_attr = CFString::new(attr);
        let cf_val = if value {
            CFBoolean::true_value()
        } else {
            CFBoolean::false_value()
        };
        let err = unsafe {
            AXUIElementSetAttributeValue(el.0, cf_attr.as_concrete_TypeRef(), cf_val.as_CFTypeRef())
        };
        if err == kAXErrorSuccess {
            return Ok(true);
        }
        ax_error_result(attr, err)?;
        Ok(false)
    }

    pub(crate) fn set_ax_string_or_err(
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
            ax_error_result(attr, err)?;
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                format!("AXSetAttributeValue({attr}) failed (err={err})"),
            )
            .with_suggestion("Attribute may be read-only. Try 'click' or 'type' instead."));
        }
        Ok(())
    }

    pub(crate) fn is_attr_settable(el: &AXElement, attr: &str) -> bool {
        let cf_attr = CFString::new(attr);
        let mut settable: c_uchar = 0;
        let err = unsafe {
            AXUIElementIsAttributeSettable(el.0, cf_attr.as_concrete_TypeRef(), &mut settable)
        };
        err == kAXErrorSuccess && settable != 0
    }

    pub(crate) fn list_ax_actions(el: &AXElement) -> Vec<String> {
        crate::tree::capabilities::copy_action_names(el)
    }

    pub(crate) fn has_ax_action(el: &AXElement, target: &str) -> bool {
        list_ax_actions(el).iter().any(|a| a == target)
    }

    pub(crate) fn try_action_from_list(
        el: &AXElement,
        actions: &[String],
        targets: &[&str],
    ) -> bool {
        for target in targets {
            if actions.iter().any(|a| a == target) && try_ax_action(el, target) {
                return true;
            }
        }
        false
    }

    pub(crate) fn try_each_child(
        el: &AXElement,
        f: impl Fn(&AXElement) -> bool,
        limit: usize,
    ) -> bool {
        let children = crate::tree::copy_ax_array(el, "AXChildren").unwrap_or_default();
        for child in children.iter().take(limit) {
            if f(child) {
                return true;
            }
        }
        false
    }

    pub(crate) fn try_each_ancestor(
        el: &AXElement,
        f: impl Fn(&AXElement) -> bool,
        limit: usize,
    ) -> bool {
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

    pub(crate) fn ensure_visible(el: &AXElement) {
        let action = CFString::new("AXScrollToVisible");
        unsafe { AXUIElementPerformAction(el.0, action.as_concrete_TypeRef()) };
    }

    pub(crate) fn set_messaging_timeout(el: &AXElement, seconds: f32) {
        let err = unsafe { AXUIElementSetMessagingTimeout(el.0, seconds) };
        if err != kAXErrorSuccess {
            tracing::warn!(
                err,
                "AXUIElementSetMessagingTimeout failed; AX calls may use the default timeout"
            );
        }
    }

    pub(crate) fn ax_focus_or_err(el: &AXElement) -> Result<bool, AdapterError> {
        set_ax_bool_or_err(el, kAXFocusedAttribute, true)
    }

    pub(crate) fn ax_set_value(el: &AXElement, val: &str) -> Result<(), AdapterError> {
        set_ax_string_or_err(el, kAXValueAttribute, val)
    }

    /// Sets `AXValue` with a CoreFoundation type matching the element's
    /// current value: numeric controls (sliders, steppers, progress) hold a
    /// `CFNumber` and reject a `CFString`, so a typed write is required. Falls
    /// back to a string write when the current value is a string or absent.
    pub(crate) fn set_ax_value_coerced(el: &AXElement, value: &str) -> Result<(), AdapterError> {
        let cf_attr = CFString::new(kAXValueAttribute);
        let mut current: CFTypeRef = std::ptr::null_mut();
        let read = unsafe {
            AXUIElementCopyAttributeValue(el.0, cf_attr.as_concrete_TypeRef(), &mut current)
        };
        let coerced: Option<CFType> = if read == kAXErrorSuccess && !current.is_null() {
            let cur = unsafe { CFType::wrap_under_create_rule(current) };
            if cur.downcast::<CFNumber>().is_some() {
                Some(number_cf_from_str(value)?)
            } else if cur.downcast::<CFBoolean>().is_some() {
                let truthy = matches!(value, "1" | "true" | "True" | "on" | "yes");
                Some(CFBoolean::from(truthy).as_CFType())
            } else {
                None
            }
        } else {
            None
        };

        match coerced {
            Some(cf_value) => {
                let err = unsafe {
                    AXUIElementSetAttributeValue(
                        el.0,
                        cf_attr.as_concrete_TypeRef(),
                        cf_value.as_CFTypeRef(),
                    )
                };
                if err != kAXErrorSuccess {
                    ax_error_result(kAXValueAttribute, err)?;
                    return Err(AdapterError::new(
                        ErrorCode::ActionFailed,
                        format!("AXSetAttributeValue(AXValue) failed (err={err})"),
                    )
                    .with_suggestion(
                        "Value may be read-only or out of range. Try 'click' to focus then arrow keys.",
                    ));
                }
                Ok(())
            }
            None => set_ax_string_or_err(el, kAXValueAttribute, value),
        }
    }

    fn number_cf_from_str(value: &str) -> Result<CFType, AdapterError> {
        if let Ok(i) = value.parse::<i64>() {
            return Ok(CFNumber::from(i).as_CFType());
        }
        if let Ok(f) = value.parse::<f64>() {
            return Ok(CFNumber::from(f).as_CFType());
        }
        Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            format!("'{value}' is not a number; this control holds a numeric value"),
        )
        .with_suggestion("Pass a numeric value, e.g. set-value @e1 50"))
    }

    pub(crate) fn ax_press(el: &AXElement) -> bool {
        try_ax_action(el, "AXPress")
    }

    pub(crate) fn element_role(el: &AXElement) -> Option<String> {
        use accessibility_sys::kAXRoleAttribute;
        crate::tree::copy_string_attr(el, kAXRoleAttribute)
            .map(|r| crate::tree::roles::ax_role_to_str(&r).to_string())
    }

    /// Soft-error gate for AX API return codes.
    ///
    /// Only `kAXErrorAPIDisabled` is promoted to a hard error (permission denied).
    /// All other codes — including `kAXErrorInvalidUIElement` — intentionally
    /// return `Ok(())` so that action-chain steps fall through to the next
    /// strategy or physical fallback instead of aborting the whole chain.
    /// Genuine staleness is caught earlier at resolve time (`STALE_REF`);
    /// re-escalating other codes here requires a chain-level stale detector first.
    fn ax_error_result(operation: &str, err: i32) -> Result<(), AdapterError> {
        if err == kAXErrorAPIDisabled {
            return Err(AdapterError::permission_denied()
                .with_platform_detail(format!("{operation} failed with kAXErrorAPIDisabled")));
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use accessibility_sys::{
            kAXErrorAPIDisabled, kAXErrorCannotComplete, kAXErrorInvalidUIElement, kAXErrorSuccess,
        };
        use agent_desktop_core::error::ErrorCode;

        use super::ax_error_result;

        #[test]
        fn success_code_is_ok() {
            ax_error_result("op", kAXErrorSuccess).unwrap();
        }

        #[test]
        fn api_disabled_yields_perm_denied() {
            let err = ax_error_result("press", kAXErrorAPIDisabled).unwrap_err();
            assert_eq!(err.code, ErrorCode::PermDenied);
            assert!(
                err.platform_detail
                    .as_deref()
                    .unwrap()
                    .contains("kAXErrorAPIDisabled")
            );
        }

        #[test]
        fn invalid_ui_element_is_soft_ok_so_chains_fall_through() {
            ax_error_result("press", kAXErrorInvalidUIElement).unwrap();
        }

        #[test]
        fn unrecognised_ax_code_is_soft_ok() {
            ax_error_result("op", kAXErrorCannotComplete).unwrap();
        }
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
    pub fn try_ax_action_retried_or_err(
        _el: &AXElement,
        _name: &str,
    ) -> Result<bool, AdapterError> {
        Ok(false)
    }
    pub fn set_ax_bool(_el: &AXElement, _attr: &str, _value: bool) -> bool {
        false
    }
    pub fn set_ax_bool_or_err(
        _el: &AXElement,
        _attr: &str,
        _value: bool,
    ) -> Result<bool, AdapterError> {
        Ok(false)
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
    pub fn ax_focus_or_err(_el: &AXElement) -> Result<bool, AdapterError> {
        Ok(false)
    }
    pub fn ax_set_value(_el: &AXElement, _val: &str) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("ax_set_value"))
    }
    pub fn set_ax_value_coerced(_el: &AXElement, _value: &str) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("set_ax_value_coerced"))
    }
    pub fn ax_press(_el: &AXElement) -> bool {
        false
    }
    pub fn element_role(_el: &AXElement) -> Option<String> {
        None
    }
}

pub(crate) use imp::{
    ax_focus_or_err, ax_press, ax_set_value, element_role, ensure_visible, has_ax_action,
    is_attr_settable, list_ax_actions, set_ax_bool, set_ax_bool_or_err, set_ax_string_or_err,
    set_ax_value_coerced, set_messaging_timeout, try_action_from_list, try_ax_action,
    try_ax_action_retried, try_ax_action_retried_or_err, try_each_ancestor, try_each_child,
};
