#[cfg(target_os = "macos")]
mod imp {
    use crate::actions::{ax_helpers, discovery::ElementCaps};
    use crate::tree::AXElement;
    use agent_desktop_core::error::AdapterError;

    pub(crate) fn do_verified_press(
        el: &AXElement,
        caps: &ElementCaps,
    ) -> Result<bool, AdapterError> {
        dispatch_verified_press(el, caps, crate::actions::chain_web_steps::is_in_webarea(el))
    }

    fn dispatch_verified_press(
        el: &AXElement,
        _caps: &ElementCaps,
        in_web: bool,
    ) -> Result<bool, AdapterError> {
        if !in_web {
            return verified_press_native(el);
        }
        tracing::debug!("verified_press: web element detected");
        Ok(crate::actions::chain_web_steps::activate_web_element(el))
    }

    fn verified_press_native(el: &AXElement) -> Result<bool, AdapterError> {
        use accessibility_sys::kAXRoleAttribute;
        let parent = crate::tree::copy_element_attr(el, "AXParent");
        let in_container = parent.as_ref().is_some_and(|p| {
            matches!(
                crate::tree::copy_string_attr(p, kAXRoleAttribute).as_deref(),
                Some("AXOutline" | "AXList" | "AXTable")
            )
        });
        if !in_container {
            return ax_helpers::try_ax_action_retried_or_err(el, "AXPress");
        }
        tracing::debug!("verified_press: native element in container, using AXPress");
        let selected_before = crate::tree::copy_bool_attr(el, "AXSelected");
        if !ax_helpers::try_ax_action_retried_or_err(el, "AXPress")? {
            return Ok(false);
        }
        if selected_before == Some(true) {
            return Ok(true);
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        let selected_after = crate::tree::copy_bool_attr(el, "AXSelected");
        if selected_after == Some(true) {
            return Ok(true);
        }
        if crate::tree::copy_string_attr(el, kAXRoleAttribute).is_none() {
            return Ok(true);
        }
        tracing::debug!("verified_press: AXPress ok but no state change");
        Ok(false)
    }

    pub(crate) fn try_value_relay(
        el: &AXElement,
        _caps: &ElementCaps,
    ) -> Result<bool, AdapterError> {
        if !ax_helpers::list_ax_actions(el).is_empty() {
            return Ok(false);
        }
        let win = crate::tree::copy_element_attr(el, "AXWindow");
        let is_dialog = win.as_ref().is_some_and(|w| {
            crate::tree::copy_string_attr(w, "AXSubrole").as_deref() == Some("AXDialog")
        });
        if !is_dialog {
            return Ok(false);
        }
        let label = std::cell::RefCell::new(None::<String>);
        ax_helpers::try_each_child(
            el,
            |child| {
                let d = crate::tree::copy_string_attr(child, "AXDescription").unwrap_or_default();
                if d.is_empty() {
                    return false;
                }
                *label.borrow_mut() = Some(d.split(',').next().unwrap_or(&d).trim().to_owned());
                true
            },
            5,
        );
        let Some(label) = label.into_inner() else {
            return Ok(false);
        };
        let Some(pid) = crate::system::app_ops::pid_from_element(el) else {
            return Ok(false);
        };
        let app = crate::tree::element_for_pid(pid);
        let Some(owner) = crate::tree::copy_element_attr(&app, "AXFocusedUIElement") else {
            return Ok(false);
        };
        if !same_window(&owner, win.as_ref()) {
            return Ok(false);
        }
        if !ax_helpers::is_attr_settable(&owner, "AXValue") {
            return Ok(false);
        }
        let orig = crate::tree::copy_string_attr(&owner, "AXValue");
        ax_helpers::set_ax_string_or_err(&owner, "AXValue", &label)?;
        std::thread::sleep(std::time::Duration::from_millis(150));
        if !ax_helpers::try_ax_action_retried_or_err(&owner, "AXConfirm")? {
            if let Some(o) = &orig {
                let _ = ax_helpers::set_ax_string_or_err(&owner, "AXValue", o);
            }
            return Ok(false);
        }
        std::thread::sleep(std::time::Duration::from_millis(150));
        let final_val = crate::tree::copy_string_attr(&owner, "AXValue");
        if final_val.as_deref() != Some(label.as_str()) {
            tracing::debug!("value_relay: reverted to {final_val:?}, expected {label:?}");
        }
        Ok(final_val.as_deref() == Some(label.as_str()))
    }

    fn same_window(owner: &AXElement, expected_window: Option<&AXElement>) -> bool {
        let Some(expected_window) = expected_window else {
            return false;
        };
        let Some(owner_window) = crate::tree::copy_element_attr(owner, "AXWindow") else {
            return false;
        };
        crate::tree::same_element(&owner_window, expected_window)
    }

    pub(crate) fn element_is_visible_in_scroll_context(
        el: &AXElement,
        _caps: &ElementCaps,
    ) -> Result<bool, AdapterError> {
        use accessibility_sys::kAXRoleAttribute;
        let Some(bounds) = crate::tree::read_bounds(el) else {
            return Ok(false);
        };
        if !rect_has_area(&bounds) {
            return Ok(false);
        }
        let mut current = crate::tree::copy_element_attr(el, "AXParent");
        for _ in 0..8 {
            let Some(parent) = &current else {
                return Ok(true);
            };
            if crate::tree::copy_string_attr(parent, kAXRoleAttribute).as_deref()
                == Some("AXScrollArea")
            {
                let Some(pb) = crate::tree::read_bounds(parent) else {
                    return Ok(false);
                };
                return Ok(rect_has_area(&pb) && center_is_inside(&bounds, &pb));
            }
            current = crate::tree::copy_element_attr(parent, "AXParent");
        }
        Ok(true)
    }

    pub(crate) fn rect_has_area(rect: &agent_desktop_core::node::Rect) -> bool {
        rect.width > 0.0 && rect.height > 0.0
    }

    pub(crate) fn center_is_inside(
        inner: &agent_desktop_core::node::Rect,
        outer: &agent_desktop_core::node::Rect,
    ) -> bool {
        let x = inner.x + inner.width / 2.0;
        let y = inner.y + inner.height / 2.0;
        x >= outer.x && x <= outer.x + outer.width && y >= outer.y && y <= outer.y + outer.height
    }

    pub(crate) fn try_show_alternate_ui(
        el: &AXElement,
        _caps: &ElementCaps,
    ) -> Result<bool, AdapterError> {
        if !ax_helpers::has_ax_action(el, "AXShowAlternateUI") {
            return Ok(false);
        }
        ax_helpers::try_ax_action_retried_or_err(el, "AXShowAlternateUI")?;
        std::thread::sleep(std::time::Duration::from_millis(100));
        Ok(ax_helpers::try_each_child(
            el,
            |child| {
                let ca = ax_helpers::list_ax_actions(child);
                ax_helpers::try_action_from_list(child, &ca, &["AXPress"])
            },
            5,
        ))
    }

    pub(crate) fn try_parent_row_select(
        el: &AXElement,
        _caps: &ElementCaps,
    ) -> Result<bool, AdapterError> {
        use accessibility_sys::kAXRoleAttribute;
        let Some(parent) = crate::tree::copy_element_attr(el, "AXParent") else {
            return Ok(false);
        };
        let role = crate::tree::copy_string_attr(&parent, kAXRoleAttribute).unwrap_or_default();
        if !matches!(role.as_str(), "AXRow" | "AXOutlineRow") {
            return Ok(false);
        }
        if !ax_helpers::is_attr_settable(&parent, "AXSelected") {
            return Ok(false);
        }
        ax_helpers::set_ax_bool_or_err(&parent, "AXSelected", true)
    }

    pub(crate) fn try_select_containing_item(
        el: &AXElement,
        _caps: &ElementCaps,
    ) -> Result<bool, AdapterError> {
        let mut current = Some(el.clone());
        for _ in 0..4 {
            let Some(candidate) = current else {
                return Ok(false);
            };
            if select_candidate(&candidate)? || select_candidate_in_container(&candidate) {
                return Ok(true);
            }
            current = crate::tree::copy_element_attr(&candidate, "AXParent");
        }
        Ok(false)
    }

    fn select_candidate(candidate: &AXElement) -> Result<bool, AdapterError> {
        Ok(ax_helpers::is_attr_settable(candidate, "AXSelected")
            && ax_helpers::set_ax_bool_or_err(candidate, "AXSelected", true)?
            && selected_state_settled(candidate))
    }

    fn selected_state_settled(candidate: &AXElement) -> bool {
        std::thread::sleep(std::time::Duration::from_millis(50));
        crate::tree::copy_bool_attr(candidate, "AXSelected") == Some(true)
    }

    fn select_candidate_in_container(candidate: &AXElement) -> bool {
        for attr in ["AXSelectedChildren", "AXSelectedRows"] {
            if set_container_selection(candidate, attr)
                && container_selection_contains(candidate, attr)
            {
                return true;
            }
        }
        false
    }

    fn set_container_selection(candidate: &AXElement, attr: &str) -> bool {
        use accessibility_sys::{AXUIElementSetAttributeValue, kAXErrorSuccess};
        use core_foundation::{
            array::CFArray,
            base::{CFRetain, CFType, CFTypeRef, TCFType},
            string::CFString,
        };
        let Some(container) = crate::tree::copy_element_attr(candidate, "AXParent") else {
            return false;
        };
        if !ax_helpers::is_attr_settable(&container, attr) {
            return false;
        }
        unsafe { CFRetain(candidate.0 as CFTypeRef) };
        let candidate_cf = unsafe { CFType::wrap_under_create_rule(candidate.0 as CFTypeRef) };
        let selected = CFArray::from_CFTypes(&[candidate_cf]);
        let cf_attr = CFString::new(attr);
        let err = unsafe {
            AXUIElementSetAttributeValue(
                container.0,
                cf_attr.as_concrete_TypeRef(),
                selected.as_CFTypeRef(),
            )
        };
        err == kAXErrorSuccess
    }

    fn container_selection_contains(candidate: &AXElement, attr: &str) -> bool {
        std::thread::sleep(std::time::Duration::from_millis(50));
        let Some(container) = crate::tree::copy_element_attr(candidate, "AXParent") else {
            return false;
        };
        crate::tree::copy_ax_array(&container, attr)
            .unwrap_or_default()
            .iter()
            .any(|selected| crate::tree::same_element(selected, candidate))
    }

    pub(crate) fn try_select_via_parent(
        el: &AXElement,
        _caps: &ElementCaps,
    ) -> Result<bool, AdapterError> {
        use accessibility_sys::{AXUIElementSetAttributeValue, kAXErrorSuccess, kAXRoleAttribute};
        use core_foundation::{
            array::CFArray,
            base::{CFRetain, CFType, CFTypeRef, TCFType},
            string::CFString,
        };
        let Some(parent) = crate::tree::copy_element_attr(el, "AXParent") else {
            return Ok(false);
        };
        let Some(role) = crate::tree::copy_string_attr(&parent, kAXRoleAttribute) else {
            return Ok(false);
        };
        if !matches!(role.as_str(), "AXTable" | "AXOutline" | "AXList") {
            return Ok(false);
        }
        if !ax_helpers::is_attr_settable(&parent, "AXSelectedRows") {
            return Ok(false);
        }
        unsafe { CFRetain(el.0 as CFTypeRef) };
        let el_cf = unsafe { CFType::wrap_under_create_rule(el.0 as CFTypeRef) };
        let arr = CFArray::from_CFTypes(&[el_cf]);
        let cf_attr = CFString::new("AXSelectedRows");
        let err = unsafe {
            AXUIElementSetAttributeValue(
                parent.0,
                cf_attr.as_concrete_TypeRef(),
                arr.as_CFTypeRef(),
            )
        };
        Ok(err == kAXErrorSuccess)
    }

    pub(crate) fn try_custom_actions(
        el: &AXElement,
        _caps: &ElementCaps,
    ) -> Result<bool, AdapterError> {
        let has = !crate::tree::copy_ax_array(el, "AXCustomActions")
            .unwrap_or_default()
            .is_empty();
        Ok(has && ax_helpers::try_ax_action_retried_or_err(el, "AXPerformCustomAction")?)
    }
}

#[cfg(target_os = "macos")]
pub(crate) use imp::*;
