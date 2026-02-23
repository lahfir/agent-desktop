use agent_desktop_core::error::AdapterError;

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use crate::actions::{
        ax_helpers,
        chain::{execute_chain, ChainContext, ChainDef, ChainStep},
        discovery::ElementCaps,
    };
    use crate::tree::AXElement;
    use agent_desktop_core::action::MouseButton;

    pub static CLICK_CHAIN: ChainDef = ChainDef {
        pre_scroll: true,
        steps: &[
            ChainStep::Action("AXPress"),
            ChainStep::Action("AXConfirm"),
            ChainStep::Action("AXOpen"),
            ChainStep::Action("AXPick"),
            ChainStep::Custom {
                label: "show_alternate_ui",
                func: try_show_alternate_ui,
            },
            ChainStep::ChildActions {
                actions: &["AXPress", "AXConfirm", "AXOpen"],
                limit: 3,
            },
            ChainStep::SetBool {
                attr: "AXSelected",
                value: true,
            },
            ChainStep::Custom {
                label: "select_via_parent",
                func: try_select_via_parent,
            },
            ChainStep::Custom {
                label: "custom_actions",
                func: try_custom_actions,
            },
            ChainStep::FocusThenConfirmOrPress,
            ChainStep::Custom {
                label: "keyboard_activate",
                func: try_keyboard_activate,
            },
            ChainStep::AncestorActions {
                actions: &["AXPress", "AXConfirm"],
                limit: 2,
            },
            ChainStep::CGClick {
                button: MouseButton::Left,
                count: 1,
            },
        ],
        suggestion: "Element may not be interactable. Try 'mouse-click --xy X,Y'.",
    };

    pub static RIGHT_CLICK_CHAIN: ChainDef = ChainDef {
        pre_scroll: false,
        steps: &[
            ChainStep::Action("AXShowMenu"),
            ChainStep::Custom {
                label: "focus_app_show_menu",
                func: focus_app_then_show_menu,
            },
            ChainStep::Custom {
                label: "select_then_show_menu",
                func: select_then_show_menu,
            },
            ChainStep::FocusThenAction("AXShowMenu"),
            ChainStep::AncestorActions {
                actions: &["AXShowMenu"],
                limit: 3,
            },
            ChainStep::ChildActions {
                actions: &["AXShowMenu"],
                limit: 5,
            },
            ChainStep::CGClick {
                button: MouseButton::Right,
                count: 1,
            },
        ],
        suggestion: "Try 'mouse-click --button right --xy X,Y'.",
    };

    pub static EXPAND_CHAIN: ChainDef = ChainDef {
        pre_scroll: false,
        steps: &[
            ChainStep::Action("AXExpand"),
            ChainStep::SetBool {
                attr: "AXDisclosing",
                value: true,
            },
        ],
        suggestion: "Try 'click' to open it instead.",
    };

    pub static COLLAPSE_CHAIN: ChainDef = ChainDef {
        pre_scroll: false,
        steps: &[
            ChainStep::Action("AXCollapse"),
            ChainStep::SetBool {
                attr: "AXDisclosing",
                value: false,
            },
        ],
        suggestion: "Try 'click' to close it instead.",
    };

    pub static SET_VALUE_CHAIN: ChainDef = ChainDef {
        pre_scroll: false,
        steps: &[
            ChainStep::SetDynamic { attr: "AXValue" },
            ChainStep::FocusThenSetDynamic { attr: "AXValue" },
        ],
        suggestion: "Try 'clear' then 'type', or check element is a text field.",
    };

    pub static CLEAR_CHAIN: ChainDef = ChainDef {
        pre_scroll: false,
        steps: &[
            ChainStep::SetDynamic { attr: "AXValue" },
            ChainStep::FocusThenSetDynamic { attr: "AXValue" },
            ChainStep::Custom {
                label: "select_all_delete",
                func: select_all_then_delete,
            },
        ],
        suggestion: "Try 'press cmd+a' then 'press delete'.",
    };

    pub static FOCUS_CHAIN: ChainDef = ChainDef {
        pre_scroll: false,
        steps: &[
            ChainStep::SetBool {
                attr: "AXFocused",
                value: true,
            },
            ChainStep::Action("AXRaise"),
            ChainStep::Action("AXPress"),
            ChainStep::SetBool {
                attr: "AXSelected",
                value: true,
            },
            ChainStep::CGClick {
                button: MouseButton::Left,
                count: 1,
            },
        ],
        suggestion: "Try 'click' to focus the element instead.",
    };

    pub static SCROLL_TO_CHAIN: ChainDef = ChainDef {
        pre_scroll: false,
        steps: &[
            ChainStep::Action("AXScrollToVisible"),
            ChainStep::Custom {
                label: "walk_parents_scroll",
                func: walk_parents_and_scroll,
            },
        ],
        suggestion: "Element may not be in a scrollable container.",
    };

    fn select_all_then_delete(el: &AXElement, _caps: &ElementCaps) -> bool {
        use accessibility_sys::AXUIElementPostKeyboardEvent;

        if !ax_helpers::ax_focus(el) {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        let pid = match crate::system::app_ops::pid_from_element(el) {
            Some(p) => p,
            None => return false,
        };
        let app = crate::tree::element_for_pid(pid);
        unsafe {
            AXUIElementPostKeyboardEvent(app.0, 0, 55, true);
            AXUIElementPostKeyboardEvent(app.0, 0, 0, true);
            AXUIElementPostKeyboardEvent(app.0, 0, 0, false);
            AXUIElementPostKeyboardEvent(app.0, 0, 55, false);
        };
        std::thread::sleep(std::time::Duration::from_millis(30));
        unsafe {
            AXUIElementPostKeyboardEvent(app.0, 0, 51, true);
            AXUIElementPostKeyboardEvent(app.0, 0, 51, false);
        };
        true
    }

    fn walk_parents_and_scroll(el: &AXElement, _caps: &ElementCaps) -> bool {
        use accessibility_sys::kAXRoleAttribute;

        let bounds = match crate::tree::read_bounds(el) {
            Some(b) => b,
            None => return false,
        };
        let mut current = crate::tree::copy_element_attr(el, "AXParent");
        for _ in 0..8 {
            let parent = match &current {
                Some(p) => p,
                None => return false,
            };
            let role = crate::tree::copy_string_attr(parent, kAXRoleAttribute);
            if role.as_deref() == Some("AXScrollArea") {
                let parent_bounds = match crate::tree::read_bounds(parent) {
                    Some(b) => b,
                    None => return false,
                };
                let target_y = bounds.y + bounds.height / 2.0;
                let visible_mid = parent_bounds.y + parent_bounds.height / 2.0;
                if target_y < parent_bounds.y || target_y > parent_bounds.y + parent_bounds.height {
                    let dy = if target_y > visible_mid { -5 } else { 5 };
                    let cx = parent_bounds.x + parent_bounds.width / 2.0;
                    let cy = parent_bounds.y + parent_bounds.height / 2.0;
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

    fn try_show_alternate_ui(el: &AXElement, _caps: &ElementCaps) -> bool {
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

    fn try_select_via_parent(el: &AXElement, _caps: &ElementCaps) -> bool {
        use accessibility_sys::{kAXErrorSuccess, kAXRoleAttribute, AXUIElementSetAttributeValue};
        use core_foundation::{
            array::CFArray,
            base::{CFRetain, CFType, CFTypeRef, TCFType},
            string::CFString,
        };

        let parent = match crate::tree::copy_element_attr(el, "AXParent") {
            Some(p) => p,
            None => return false,
        };
        let role = match crate::tree::copy_string_attr(&parent, kAXRoleAttribute) {
            Some(r) => r,
            None => return false,
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

    fn try_custom_actions(el: &AXElement, _caps: &ElementCaps) -> bool {
        let custom = crate::tree::copy_ax_array(el, "AXCustomActions").unwrap_or_default();
        if custom.is_empty() {
            return false;
        }
        ax_helpers::try_ax_action(el, "AXPerformCustomAction")
    }

    fn try_keyboard_activate(el: &AXElement, _caps: &ElementCaps) -> bool {
        use accessibility_sys::AXUIElementPostKeyboardEvent;

        if !ax_helpers::ax_focus(el) {
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

    fn focus_app_then_show_menu(el: &AXElement, _caps: &ElementCaps) -> bool {
        let pid = match crate::system::app_ops::pid_from_element(el) {
            Some(p) => p,
            None => return false,
        };
        let _ = crate::system::app_ops::ensure_app_focused(pid);
        std::thread::sleep(std::time::Duration::from_millis(50));
        ax_helpers::try_ax_action(el, "AXShowMenu")
    }

    fn select_then_show_menu(el: &AXElement, _caps: &ElementCaps) -> bool {
        if !ax_helpers::is_attr_settable(el, "AXSelected") {
            return false;
        }
        if !ax_helpers::set_ax_bool(el, "AXSelected", true) {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        ax_helpers::try_ax_action(el, "AXShowMenu")
    }

    pub fn double_click(el: &AXElement, caps: &ElementCaps) -> Result<(), AdapterError> {
        if ax_helpers::try_ax_action(el, "AXOpen") {
            return Ok(());
        }
        let ctx = ChainContext {
            dynamic_value: None,
        };
        let _ = execute_chain(el, caps, &CLICK_CHAIN, &ctx);
        std::thread::sleep(std::time::Duration::from_millis(50));
        let _ = execute_chain(el, caps, &CLICK_CHAIN, &ctx);
        crate::actions::dispatch::click_via_bounds(el, MouseButton::Left, 2)
    }

    pub fn triple_click(el: &AXElement, caps: &ElementCaps) -> Result<(), AdapterError> {
        let ctx = ChainContext {
            dynamic_value: None,
        };
        for _ in 0..3 {
            let _ = execute_chain(el, caps, &CLICK_CHAIN, &ctx);
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        crate::actions::dispatch::click_via_bounds(el, MouseButton::Left, 3)
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {}

#[cfg(target_os = "macos")]
pub(crate) use imp::{
    double_click, triple_click, CLEAR_CHAIN, CLICK_CHAIN, COLLAPSE_CHAIN, EXPAND_CHAIN,
    FOCUS_CHAIN, RIGHT_CLICK_CHAIN, SCROLL_TO_CHAIN, SET_VALUE_CHAIN,
};
