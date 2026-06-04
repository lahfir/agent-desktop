use agent_desktop_core::error::AdapterError;

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use crate::actions::{
        ax_helpers,
        chain::{ChainDef, ChainStep},
        chain_menu_steps, chain_steps,
        discovery::ElementCaps,
    };
    use crate::tree::AXElement;
    use agent_desktop_core::{action::MouseButton, action_request::InteractionPolicy};

    pub(crate) static CLICK_CHAIN: ChainDef = ChainDef {
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
            ChainStep::Custom {
                label: "containing_item_select",
                func: chain_steps::try_select_containing_item,
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
                label: "value_relay",
                func: chain_steps::try_value_relay,
            },
            ChainStep::Custom {
                label: "select_via_parent",
                func: chain_steps::try_select_via_parent,
            },
            ChainStep::ChildActions {
                actions: &["AXPress", "AXConfirm", "AXOpen"],
                limit: 3,
            },
            ChainStep::Custom {
                label: "custom_actions",
                func: chain_steps::try_custom_actions,
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

    pub(crate) static RIGHT_CLICK_CHAIN: ChainDef = ChainDef {
        pre_scroll: false,
        steps: &[
            ChainStep::Custom {
                label: "show_menu",
                func: chain_menu_steps::show_menu,
            },
            ChainStep::Custom {
                label: "select_then_show_menu",
                func: chain_menu_steps::select_then_show_menu,
            },
            ChainStep::Custom {
                label: "selected_items_menu",
                func: chain_menu_steps::select_then_selected_items_menu,
            },
            ChainStep::Custom {
                label: "child_show_menu",
                func: chain_menu_steps::show_menu_on_children,
            },
            ChainStep::Custom {
                label: "ancestor_show_menu",
                func: chain_menu_steps::show_menu_on_ancestors,
            },
            ChainStep::CGClick {
                button: MouseButton::Right,
                count: 1,
            },
        ],
        suggestion: "Try 'mouse-click --button right --xy X,Y'.",
    };

    pub(crate) static EXPAND_CHAIN: ChainDef = ChainDef {
        pre_scroll: false,
        steps: &[
            ChainStep::Action("AXExpand"),
            ChainStep::SetBool {
                attr: "AXExpanded",
                value: true,
            },
            ChainStep::SetBool {
                attr: "AXDisclosing",
                value: true,
            },
        ],
        suggestion: "Try 'click' to open it instead.",
    };

    pub(crate) static COLLAPSE_CHAIN: ChainDef = ChainDef {
        pre_scroll: false,
        steps: &[
            ChainStep::Action("AXCollapse"),
            ChainStep::SetBool {
                attr: "AXExpanded",
                value: false,
            },
            ChainStep::SetBool {
                attr: "AXDisclosing",
                value: false,
            },
        ],
        suggestion: "Try 'click' to close it instead.",
    };

    const VALUE_STEPS: &[ChainStep] = &[
        ChainStep::SetDynamic { attr: "AXValue" },
        ChainStep::FocusThenSetDynamic { attr: "AXValue" },
    ];

    pub(crate) static SET_VALUE_CHAIN: ChainDef = ChainDef {
        pre_scroll: false,
        steps: VALUE_STEPS,
        suggestion: "Try 'clear' then 'type', or check element is a text field.",
    };

    pub(crate) static CLEAR_CHAIN: ChainDef = ChainDef {
        pre_scroll: false,
        steps: &[
            ChainStep::SetDynamic { attr: "AXValue" },
            ChainStep::FocusThenSetDynamic { attr: "AXValue" },
            ChainStep::FocusThenClearByKeyboard,
        ],
        suggestion: "Try 'press cmd+a' then 'press delete'.",
    };

    pub(crate) static FOCUS_CHAIN: ChainDef = ChainDef {
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

    pub(crate) static SCROLL_TO_CHAIN: ChainDef = ChainDef {
        pre_scroll: false,
        steps: &[
            ChainStep::Action("AXScrollToVisible"),
            ChainStep::Custom {
                label: "visible_in_scroll_context",
                func: chain_steps::element_is_visible_in_scroll_context,
            },
        ],
        suggestion: "Element may not be in a scrollable container.",
    };

    pub(crate) fn double_click(
        el: &AXElement,
        _caps: &ElementCaps,
        policy: InteractionPolicy,
    ) -> Result<(), AdapterError> {
        if ax_helpers::try_ax_action(el, "AXOpen") {
            return Ok(());
        }
        crate::actions::dispatch::click_via_bounds(el, MouseButton::Left, 2, policy)
    }

    pub(crate) fn triple_click(
        el: &AXElement,
        _caps: &ElementCaps,
        policy: InteractionPolicy,
    ) -> Result<(), AdapterError> {
        crate::actions::dispatch::click_via_bounds(el, MouseButton::Left, 3, policy)
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {}

#[cfg(target_os = "macos")]
pub(crate) use imp::{
    CLEAR_CHAIN, CLICK_CHAIN, COLLAPSE_CHAIN, EXPAND_CHAIN, FOCUS_CHAIN, RIGHT_CLICK_CHAIN,
    SCROLL_TO_CHAIN, SET_VALUE_CHAIN, double_click, triple_click,
};
