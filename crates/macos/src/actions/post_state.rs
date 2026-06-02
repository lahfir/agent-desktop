use agent_desktop_core::{
    action::{Action, ElementState},
    adapter::LiveElement,
};

#[cfg(target_os = "macos")]
pub(crate) fn read_post_state(
    el: &crate::tree::AXElement,
    action: &Action,
) -> Option<ElementState> {
    let delay_ms = match action {
        Action::Click | Action::Toggle | Action::Check | Action::Uncheck | Action::TypeText(_) => {
            50
        }
        Action::SetValue(_) | Action::Clear | Action::Expand | Action::Collapse => 0,
        _ => return None,
    };
    if delay_ms > 0 {
        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
    }
    Some(read_element_state(el))
}

pub(crate) fn read_element_state(el: &crate::tree::AXElement) -> ElementState {
    let attrs = crate::tree::element::fetch_node_attrs(el);
    let role = normalized_role(attrs.role.as_deref());
    element_state_from_attrs(el, attrs, role)
}

pub(crate) fn read_live_element(el: &crate::tree::AXElement) -> LiveElement {
    let attrs = crate::tree::element::fetch_node_attrs(el);
    let role = normalized_role(attrs.role.as_deref());
    let state = element_state_from_attrs(el, attrs, role.clone());
    LiveElement {
        state: Some(state),
        bounds: crate::tree::read_bounds(el),
        available_actions: Some(crate::tree::action_list::platform_available_actions(
            el, &role,
        )),
    }
}

fn element_state_from_attrs(
    el: &crate::tree::AXElement,
    attrs: crate::tree::NodeAttrs,
    role: String,
) -> ElementState {
    let value = attrs.value;
    let focused = crate::tree::element::copy_bool_attr(el, "AXFocused").unwrap_or(false);
    let expanded = crate::tree::element::copy_bool_attr(el, "AXExpanded")
        .or_else(|| crate::tree::element::copy_bool_attr(el, "AXDisclosing"))
        .unwrap_or(false);
    let mut states = Vec::new();
    if focused {
        states.push("focused".into());
    }
    if !attrs.enabled {
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
