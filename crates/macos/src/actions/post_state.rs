use agent_desktop_core::{action::Action, adapter::LiveElement, element_state::ElementState};

#[cfg(target_os = "macos")]
pub(crate) fn read_post_state(
    el: &crate::tree::AXElement,
    action: &Action,
) -> Option<ElementState> {
    let delay_ms = match action {
        Action::Click | Action::TypeText(_) => 50,
        Action::Toggle
        | Action::Check
        | Action::Uncheck
        | Action::SetValue(_)
        | Action::Clear
        | Action::Expand
        | Action::Collapse => 0,
        Action::DoubleClick
        | Action::RightClick
        | Action::TripleClick
        | Action::SetFocus
        | Action::Select(_)
        | Action::Scroll(_, _)
        | Action::ScrollTo
        | Action::PressKey(_)
        | Action::KeyDown(_)
        | Action::KeyUp(_)
        | Action::Hover
        | Action::Drag(_) => return None,
    };
    if delay_ms > 0 {
        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
    }
    Some(read_element_state(el))
}

pub(crate) fn read_element_state(el: &crate::tree::AXElement) -> ElementState {
    let attrs = crate::tree::element::fetch_node_attrs(el);
    let role = normalized_role(attrs.role.as_deref());
    element_state_from_attrs(attrs, role)
}

pub(crate) fn read_live_element(el: &crate::tree::AXElement) -> LiveElement {
    let attrs = crate::tree::element::fetch_node_attrs(el);
    let role = normalized_role(attrs.role.as_deref());
    let bounds = attrs.bounds;
    let has_scrollbars = attrs.has_scrollbars;
    let state = element_state_from_attrs(attrs, role.clone());
    LiveElement {
        state: Some(state),
        bounds,
        available_actions: Some(crate::tree::action_list::platform_available_actions(
            el,
            &role,
            has_scrollbars,
        )),
    }
}

pub(crate) fn read_live_actions(el: &crate::tree::AXElement) -> Vec<String> {
    let attrs = crate::tree::element::fetch_node_attrs(el);
    let role = normalized_role(attrs.role.as_deref());
    crate::tree::action_list::platform_available_actions(el, &role, attrs.has_scrollbars)
}

fn element_state_from_attrs(attrs: crate::tree::NodeAttrs, role: String) -> ElementState {
    let value = attrs.value;
    let focused = attrs.states.focused.unwrap_or(false);
    let expanded = attrs
        .states
        .expanded
        .or(attrs.states.disclosing)
        .unwrap_or(false);
    let mut states = Vec::new();
    if focused {
        states.push("focused".into());
    }
    if !attrs.states.enabled {
        states.push("disabled".into());
    }
    if expanded {
        states.push("expanded".into());
    }
    if crate::tree::roles::is_toggleable_role(&role) && value_is_checked(value.as_deref()) {
        states.push("checked".into());
    }
    ElementState {
        role,
        states,
        value,
    }
}

fn normalized_role(ax_role: Option<&str>) -> String {
    ax_role
        .map(crate::tree::roles::ax_role_to_str)
        .unwrap_or("unknown")
        .to_string()
}

fn value_is_checked(value: Option<&str>) -> bool {
    matches!(value, Some("1" | "true"))
}
