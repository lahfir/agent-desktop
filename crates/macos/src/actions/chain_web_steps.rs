#[cfg(target_os = "macos")]
mod imp {
    use crate::actions::ax_helpers;
    use crate::tree::AXElement;

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

    pub(crate) fn is_in_webarea(el: &AXElement) -> bool {
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

    pub(crate) fn activate_web_element(el: &AXElement) -> bool {
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

        tracing::debug!("activate_web: all AX methods had no effect");
        false
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
}

#[cfg(not(target_os = "macos"))]
mod imp {}

#[cfg(target_os = "macos")]
pub(crate) use imp::{activate_web_element, is_in_webarea};
