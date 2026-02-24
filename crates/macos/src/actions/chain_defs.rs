use agent_desktop_core::error::AdapterError;

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use crate::actions::{
        ax_helpers,
        chain::{execute_chain, ChainContext, ChainDef, ChainStep},
        chain_steps,
        discovery::ElementCaps,
    };
    use crate::tree::AXElement;
    use agent_desktop_core::action::MouseButton;

    pub static CLICK_CHAIN: ChainDef = ChainDef {
        pre_scroll: true,
        steps: &[
            ChainStep::Custom {
                label: "verified_press",
                func: chain_steps::do_verified_press,
            },
            ChainStep::Action("AXConfirm"),
            ChainStep::Action("AXOpen"),
            ChainStep::Action("AXPick"),
            ChainStep::Custom {
                label: "show_alternate_ui",
                func: chain_steps::try_show_alternate_ui,
            },
            ChainStep::ChildActions {
                actions: &["AXPress", "AXConfirm", "AXOpen"],
                limit: 3,
            },
            ChainStep::Custom {
                label: "value_relay",
                func: chain_steps::try_value_relay,
            },
            ChainStep::SetBool {
                attr: "AXSelected",
                value: true,
            },
            ChainStep::Custom {
                label: "parent_row_select",
                func: chain_steps::try_parent_row_select,
            },
            ChainStep::Custom {
                label: "select_via_parent",
                func: chain_steps::try_select_via_parent,
            },
            ChainStep::Custom {
                label: "custom_actions",
                func: chain_steps::try_custom_actions,
            },
            ChainStep::Custom {
                label: "focus_verified_confirm_or_press",
                func: chain_steps::try_focus_then_verified_confirm_or_press,
            },
            ChainStep::Custom {
                label: "keyboard_activate",
                func: chain_steps::try_keyboard_activate,
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
                func: chain_steps::focus_app_then_show_menu,
            },
            ChainStep::Custom {
                label: "select_then_show_menu",
                func: chain_steps::select_then_show_menu,
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
                func: chain_steps::select_all_then_delete,
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
                func: chain_steps::walk_parents_and_scroll,
            },
        ],
        suggestion: "Element may not be in a scrollable container.",
    };

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
