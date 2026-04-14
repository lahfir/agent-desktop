#[cfg(target_os = "macos")]
mod imp {
    use crate::actions::{ax_helpers, discovery::ElementCaps};
    use crate::tree::AXElement;

    pub fn do_verified_press(el: &AXElement, caps: &ElementCaps) -> bool {
        dispatch_verified_press(el, caps, is_in_webarea(el))
    }

    fn dispatch_verified_press(el: &AXElement, _caps: &ElementCaps, in_web: bool) -> bool {
        if !in_web {
            return verified_press_native(el);
        }
        tracing::debug!("verified_press: web element detected");
        activate_web_element(el)
    }

    /// Native (non-web) elements: AXPress with selection verification
    /// for elements inside selection containers.
    fn verified_press_native(el: &AXElement) -> bool {
        use accessibility_sys::kAXRoleAttribute;
        let parent = crate::tree::copy_element_attr(el, "AXParent");
        let in_container = parent.as_ref().is_some_and(|p| {
            matches!(
                crate::tree::copy_string_attr(p, kAXRoleAttribute).as_deref(),
                Some("AXOutline" | "AXList" | "AXTable")
            )
        });
        if !in_container {
            return ax_helpers::try_ax_action_retried(el, "AXPress");
        }
        tracing::debug!("verified_press: native element in container, using AXPress");
        let selected_before = crate::tree::element::copy_bool_attr(el, "AXSelected");
        if !ax_helpers::try_ax_action_retried(el, "AXPress") {
            return false;
        }
        if selected_before == Some(true) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        let selected_after = crate::tree::element::copy_bool_attr(el, "AXSelected");
        if selected_after == Some(true) {
            return true;
        }
        if crate::tree::copy_string_attr(el, kAXRoleAttribute).is_none() {
            return true;
        }
        tracing::debug!("verified_press: AXPress ok but no state change");
        false
    }

    fn is_in_webarea(el: &AXElement) -> bool {
        use accessibility_sys::kAXRoleAttribute;
        let mut current = crate::tree::copy_element_attr(el, "AXParent");
        for _ in 0..20 {
            let Some(ref parent) = current else {
                return false;
            };
            if crate::tree::copy_string_attr(parent, kAXRoleAttribute).as_deref()
                == Some("AXWebArea")
            {
                return true;
            }
            current = crate::tree::copy_element_attr(parent, "AXParent");
        }
        false
    }

    /// Activate any Electron/web element with verified effect detection.
    ///
    /// Chromium's AX implementation lies: AXPress/AXConfirm return
    /// kAXErrorSuccess but only toggle ARIA state — DOM click handlers
    /// never fire. We verify by checking if the focused UI element
    /// changed after AXPress. If nothing changed, CGClick is the
    /// genuine last resort.
    ///
    /// This handles the FULL escalation for web elements so the chain
    /// engine never reaches false-positive AX steps (AXConfirm, SetBool
    /// AXSelected, etc. all lie on Chromium).
    struct PreActionState {
        focused: Option<AXElement>,
        value: Option<String>,
        selected: Option<bool>,
    }

    impl PreActionState {
        fn capture(app: &AXElement, el: &AXElement) -> Self {
            Self {
                focused: crate::tree::copy_element_attr(app, "AXFocusedUIElement"),
                value: crate::tree::copy_string_attr(el, "AXValue"),
                selected: crate::tree::copy_bool_attr(el, "AXSelected"),
            }
        }
    }

    fn activate_web_element(el: &AXElement) -> bool {
        let Some(pid) = crate::system::app_ops::pid_from_element(el) else {
            return false;
        };

        let app = crate::tree::element_for_pid(pid);
        let before = PreActionState::capture(&app, el);

        if ax_helpers::try_ax_action_retried(el, "AXPress") {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if web_action_had_effect(&app, el, &before) {
                tracing::debug!("activate_web: AXPress had real effect");
                return true;
            }
            tracing::debug!("activate_web: AXPress returned success but no DOM effect");
        }

        let actions = ax_helpers::list_ax_actions(el);
        for action in &["AXConfirm", "AXOpen"] {
            if actions.iter().any(|a| a == action) && ax_helpers::try_ax_action(el, action) {
                std::thread::sleep(std::time::Duration::from_millis(100));
                if web_action_had_effect(&app, el, &before) {
                    tracing::debug!("activate_web: {action} had real effect");
                    return true;
                }
            }
        }

        if ax_helpers::try_each_child(
            el,
            |child| {
                let child_actions = ax_helpers::list_ax_actions(child);
                ax_helpers::try_action_from_list(
                    child,
                    &child_actions,
                    &["AXPress", "AXConfirm", "AXOpen"],
                )
            },
            5,
        ) {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if web_action_had_effect(&app, el, &before) {
                tracing::debug!("activate_web: child action had real effect");
                return true;
            }
        }

        tracing::debug!("activate_web: all AX methods had no effect, CGClick");
        let _ = crate::system::app_ops::ensure_app_focused(pid);
        crate::actions::dispatch::click_via_bounds(
            el,
            agent_desktop_core::action::MouseButton::Left,
            1,
        )
        .is_ok()
    }

    fn web_action_had_effect(app: &AXElement, el: &AXElement, before: &PreActionState) -> bool {
        use core_foundation::base::{CFEqual, CFTypeRef};

        let value_after = crate::tree::copy_string_attr(el, "AXValue");
        if before.value != value_after {
            return true;
        }

        let selected_after = crate::tree::copy_bool_attr(el, "AXSelected");
        if before.selected != selected_after {
            return true;
        }

        let focused_after = crate::tree::copy_element_attr(app, "AXFocusedUIElement");
        match (&before.focused, &focused_after) {
            (Some(before_f), Some(after_f)) => unsafe {
                CFEqual(before_f.0 as CFTypeRef, after_f.0 as CFTypeRef) == 0
            },
            (None, Some(_)) => true,
            _ => false,
        }
    }

    pub fn try_focus_then_verified_confirm_or_press(el: &AXElement, caps: &ElementCaps) -> bool {
        if !ax_helpers::ax_focus(el) {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        let in_web = is_in_webarea(el);
        if !in_web && ax_helpers::try_ax_action_retried(el, "AXConfirm") {
            return true;
        }
        dispatch_verified_press(el, caps, in_web)
    }

    pub fn try_value_relay(el: &AXElement, _caps: &ElementCaps) -> bool {
        if !ax_helpers::list_ax_actions(el).is_empty() {
            return false;
        }
        let win = crate::tree::copy_element_attr(el, "AXWindow");
        let is_dialog = win.as_ref().is_some_and(|w| {
            crate::tree::copy_string_attr(w, "AXSubrole").as_deref() == Some("AXDialog")
        });
        if !is_dialog {
            return false;
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
            return false;
        };
        let Some(pid) = crate::system::app_ops::pid_from_element(el) else {
            return false;
        };
        let app = crate::tree::element_for_pid(pid);
        let Some(owner) = crate::tree::copy_element_attr(&app, "AXFocusedUIElement") else {
            return false;
        };
        if !ax_helpers::is_attr_settable(&owner, "AXValue") {
            return false;
        }
        let orig = crate::tree::copy_string_attr(&owner, "AXValue");
        if ax_helpers::set_ax_string_or_err(&owner, "AXValue", &label).is_err() {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(150));
        if !ax_helpers::try_ax_action(&owner, "AXConfirm") {
            if let Some(o) = &orig {
                let _ = ax_helpers::set_ax_string_or_err(&owner, "AXValue", o);
            }
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(150));
        let final_val = crate::tree::copy_string_attr(&owner, "AXValue");
        if final_val.as_deref() != Some(label.as_str()) {
            tracing::debug!("value_relay: reverted to {final_val:?}, expected {label:?}");
        }
        final_val.as_deref() == Some(label.as_str())
    }

    pub fn select_all_then_delete(el: &AXElement, _caps: &ElementCaps) -> bool {
        use accessibility_sys::AXUIElementPostKeyboardEvent as PostKey;
        if !ax_helpers::ax_focus(el) {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        let Some(pid) = crate::system::app_ops::pid_from_element(el) else {
            return false;
        };
        let a = crate::tree::element_for_pid(pid);
        unsafe {
            PostKey(a.0, 0, 55, true);
            PostKey(a.0, 0, 0, true);
            PostKey(a.0, 0, 0, false);
            PostKey(a.0, 0, 55, false);
        }
        std::thread::sleep(std::time::Duration::from_millis(30));
        unsafe {
            PostKey(a.0, 0, 51, true);
            PostKey(a.0, 0, 51, false);
        }
        true
    }

    pub fn walk_parents_and_scroll(el: &AXElement, _caps: &ElementCaps) -> bool {
        use accessibility_sys::kAXRoleAttribute;
        let Some(bounds) = crate::tree::read_bounds(el) else {
            return false;
        };
        let mut current = crate::tree::copy_element_attr(el, "AXParent");
        for _ in 0..8 {
            let Some(parent) = &current else { return false };
            if crate::tree::copy_string_attr(parent, kAXRoleAttribute).as_deref()
                == Some("AXScrollArea")
            {
                let Some(pb) = crate::tree::read_bounds(parent) else {
                    return false;
                };
                let ty = bounds.y + bounds.height / 2.0;
                if ty < pb.y || ty > pb.y + pb.height {
                    let dy = if ty > pb.y + pb.height / 2.0 { -5 } else { 5 };
                    let (cx, cy) = (pb.x + pb.width / 2.0, pb.y + pb.height / 2.0);
                    for _ in 0..20 {
                        let _ = crate::input::mouse::synthesize_scroll_at(cx, cy, dy, 0);
                        std::thread::sleep(std::time::Duration::from_millis(16));
                    }
                }
                return true;
            }
            current = crate::tree::copy_element_attr(parent, "AXParent");
        }
        false
    }

    pub fn try_show_alternate_ui(el: &AXElement, _caps: &ElementCaps) -> bool {
        if !ax_helpers::has_ax_action(el, "AXShowAlternateUI") {
            return false;
        }
        ax_helpers::try_ax_action(el, "AXShowAlternateUI");
        std::thread::sleep(std::time::Duration::from_millis(100));
        ax_helpers::try_each_child(
            el,
            |child| {
                let ca = ax_helpers::list_ax_actions(child);
                ax_helpers::try_action_from_list(child, &ca, &["AXPress"])
            },
            5,
        )
    }

    pub fn try_parent_row_select(el: &AXElement, _caps: &ElementCaps) -> bool {
        use accessibility_sys::kAXRoleAttribute;
        let Some(parent) = crate::tree::copy_element_attr(el, "AXParent") else {
            return false;
        };
        let role = crate::tree::copy_string_attr(&parent, kAXRoleAttribute).unwrap_or_default();
        if !matches!(role.as_str(), "AXRow" | "AXOutlineRow") {
            return false;
        }
        if !ax_helpers::is_attr_settable(&parent, "AXSelected") {
            return false;
        }
        ax_helpers::set_ax_bool(&parent, "AXSelected", true)
    }

    pub fn try_select_via_parent(el: &AXElement, _caps: &ElementCaps) -> bool {
        use accessibility_sys::{kAXErrorSuccess, kAXRoleAttribute, AXUIElementSetAttributeValue};
        use core_foundation::{
            array::CFArray,
            base::{CFRetain, CFType, CFTypeRef, TCFType},
            string::CFString,
        };
        let Some(parent) = crate::tree::copy_element_attr(el, "AXParent") else {
            return false;
        };
        let Some(role) = crate::tree::copy_string_attr(&parent, kAXRoleAttribute) else {
            return false;
        };
        if !matches!(role.as_str(), "AXTable" | "AXOutline" | "AXList") {
            return false;
        }
        if !ax_helpers::is_attr_settable(&parent, "AXSelectedRows") {
            return false;
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
        err == kAXErrorSuccess
    }

    pub fn try_custom_actions(el: &AXElement, _caps: &ElementCaps) -> bool {
        let has = !crate::tree::copy_ax_array(el, "AXCustomActions")
            .unwrap_or_default()
            .is_empty();
        has && ax_helpers::try_ax_action(el, "AXPerformCustomAction")
    }

    pub fn try_keyboard_activate(el: &AXElement, _caps: &ElementCaps) -> bool {
        use accessibility_sys::AXUIElementPostKeyboardEvent as PostKey;
        if !ax_helpers::ax_focus(el) {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        let Some(pid) = crate::system::app_ops::pid_from_element(el) else {
            return false;
        };
        let a = crate::tree::element_for_pid(pid);
        unsafe {
            PostKey(a.0, 0, 49, true);
            PostKey(a.0, 0, 49, false);
        }
        true
    }

    pub fn focus_app_then_show_menu(el: &AXElement, _caps: &ElementCaps) -> bool {
        let Some(pid) = crate::system::app_ops::pid_from_element(el) else {
            return false;
        };
        let _ = crate::system::app_ops::ensure_app_focused(pid);
        std::thread::sleep(std::time::Duration::from_millis(50));
        ax_helpers::try_ax_action(el, "AXShowMenu")
    }

    pub fn select_then_show_menu(el: &AXElement, _caps: &ElementCaps) -> bool {
        if !ax_helpers::is_attr_settable(el, "AXSelected")
            || !ax_helpers::set_ax_bool(el, "AXSelected", true)
        {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        ax_helpers::try_ax_action(el, "AXShowMenu")
    }
}

#[cfg(target_os = "macos")]
pub(crate) use imp::*;
