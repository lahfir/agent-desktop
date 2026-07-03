use agent_desktop_core::{adapter::LiveElement, element_state::ElementState, node::Rect};

pub(crate) fn read_post_state(
    el: &crate::tree::AXElement,
    action: &agent_desktop_core::action::Action,
) -> Option<ElementState> {
    let delay_ms = match action {
        agent_desktop_core::action::Action::Click
        | agent_desktop_core::action::Action::TypeText(_) => 50,
        agent_desktop_core::action::Action::Toggle
        | agent_desktop_core::action::Action::Check
        | agent_desktop_core::action::Action::Uncheck
        | agent_desktop_core::action::Action::SetValue(_)
        | agent_desktop_core::action::Action::Clear
        | agent_desktop_core::action::Action::Expand
        | agent_desktop_core::action::Action::Collapse => 0,
        agent_desktop_core::action::Action::DoubleClick
        | agent_desktop_core::action::Action::RightClick
        | agent_desktop_core::action::Action::TripleClick
        | agent_desktop_core::action::Action::SetFocus
        | agent_desktop_core::action::Action::Select(_)
        | agent_desktop_core::action::Action::Scroll(_, _)
        | agent_desktop_core::action::Action::ScrollTo
        | agent_desktop_core::action::Action::PressKey(_)
        | agent_desktop_core::action::Action::KeyDown(_)
        | agent_desktop_core::action::Action::KeyUp(_)
        | agent_desktop_core::action::Action::Hover
        | agent_desktop_core::action::Action::Drag(_) => return None,
    };
    if delay_ms > 0 {
        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
    }
    Some(read_element_state(el))
}

pub(crate) fn read_element_state(el: &crate::tree::AXElement) -> ElementState {
    let attrs = crate::tree::element::fetch_node_attrs(el);
    let role = normalized_role(attrs.role.as_deref());
    element_state_from_attrs(el, attrs, role, owning_window_bounds(el))
}

pub(crate) fn read_live_element(el: &crate::tree::AXElement) -> LiveElement {
    let attrs = crate::tree::element::fetch_node_attrs(el);
    let role = normalized_role(attrs.role.as_deref());
    let bounds = attrs.bounds;
    let has_scrollbars = attrs.has_scrollbars;
    let window_bounds = owning_window_bounds(el);
    let state = element_state_from_attrs(el, attrs, role.clone(), window_bounds);
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

/// Looks up the AX window that contains `el` via the `AXWindow` attribute
/// (the same single-hop lookup `dispatch.rs`/`chain_steps.rs` use to raise a
/// window) and reads its bounds, so post-action state reads can compute the
/// `offscreen` token instead of always seeing `window_bounds: None`. Returns
/// `None` when the element has no reachable window (e.g. a detached probe
/// or a not-yet-mapped element) — offscreen simply stays unknown, same as
/// today's behavior for those elements.
fn owning_window_bounds(el: &crate::tree::AXElement) -> Option<Rect> {
    let window = crate::tree::copy_element_attr(el, "AXWindow")?;
    crate::tree::read_bounds(&window)
}

pub(crate) fn read_live_actions(el: &crate::tree::AXElement) -> Vec<String> {
    let attrs = crate::tree::element::fetch_node_attrs(el);
    let role = normalized_role(attrs.role.as_deref());
    crate::tree::action_list::platform_available_actions(el, &role, attrs.has_scrollbars)
}

fn element_state_from_attrs(
    el: &crate::tree::AXElement,
    attrs: crate::tree::NodeAttrs,
    role: String,
    window_bounds: Option<Rect>,
) -> ElementState {
    let is_secure = attrs.role.as_deref() == Some("AXSecureTextField");
    let ctx = crate::tree::state_reader::StateReaderContext {
        focused: None,
        window_bounds,
        is_secure_text: is_secure,
    };
    let states = crate::tree::state_reader::states_from_element(el, &attrs, &role, &ctx);
    ElementState {
        role,
        states,
        value: attrs.value,
    }
}

fn normalized_role(ax_role: Option<&str>) -> String {
    ax_role
        .map(crate::tree::roles::ax_role_to_str)
        .unwrap_or("unknown")
        .to_string()
}

#[cfg(test)]
#[path = "post_state_tests.rs"]
mod tests;
