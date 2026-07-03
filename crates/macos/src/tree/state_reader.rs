use agent_desktop_core::node::Rect;
use agent_desktop_core::state;

use super::attributes::copy_bool_attr;
use super::{AXElement, NodeAttrs};

pub(crate) struct StateReaderContext<'a> {
    pub focused: Option<&'a AXElement>,
    pub window_bounds: Option<Rect>,
    pub is_secure_text: bool,
}

pub(crate) fn states_from_element(
    el: &AXElement,
    attrs: &NodeAttrs,
    role: &str,
    ctx: &StateReaderContext<'_>,
) -> Vec<String> {
    let mut states = Vec::new();
    if ctx
        .focused
        .is_some_and(|focused| super::capabilities::same_element(el, focused))
        || attrs.states.focused == Some(true)
    {
        states.push(state::FOCUSED.into());
    }
    if !attrs.states.enabled {
        states.push(state::DISABLED.into());
    }
    if ctx.is_secure_text {
        states.push(state::SECURE.into());
    }
    if is_expanded(el, attrs) {
        states.push(state::EXPANDED.into());
    }
    if super::roles::is_toggleable_role(role) {
        if value_is_checked(attrs.value.as_deref()) {
            states.push(state::CHECKED.into());
        } else if value_is_indeterminate(attrs.value.as_deref()) {
            states.push(state::INDETERMINATE.into());
        }
    }
    if attrs.states.selected == Some(true) {
        states.push(state::SELECTED.into());
    }
    if attrs.states.hidden == Some(true) {
        states.push(state::HIDDEN.into());
    }
    if attrs.states.busy == Some(true) {
        states.push(state::BUSY.into());
    }
    if attrs.states.modal == Some(true) {
        states.push(state::MODAL.into());
    }
    if attrs.states.required == Some(true) {
        states.push(state::REQUIRED.into());
    }
    if role == "button" && value_is_checked(attrs.value.as_deref()) {
        states.push(state::PRESSED.into());
    }
    if attrs.states.readonly == Some(true) {
        states.push(state::READONLY.into());
    }
    if is_offscreen(attrs.bounds, ctx.window_bounds) {
        states.push(state::OFFSCREEN.into());
    }
    states
}

fn is_expanded(el: &AXElement, attrs: &NodeAttrs) -> bool {
    if attrs
        .states
        .expanded
        .or(attrs.states.disclosing)
        .unwrap_or(false)
    {
        return true;
    }
    if attrs.states.expanded.is_some() || attrs.states.disclosing.is_some() {
        return false;
    }
    copy_bool_attr(el, "AXExpanded")
        .or_else(|| copy_bool_attr(el, "AXDisclosing"))
        .unwrap_or(false)
}

fn value_is_checked(value: Option<&str>) -> bool {
    matches!(value, Some("1" | "true"))
}

fn value_is_indeterminate(value: Option<&str>) -> bool {
    matches!(value, Some("2" | "mixed"))
}

fn is_offscreen(bounds: Option<Rect>, window_bounds: Option<Rect>) -> bool {
    let (Some(el), Some(win)) = (bounds, window_bounds) else {
        return false;
    };
    let el_right = el.x + el.width;
    let el_bottom = el.y + el.height;
    let win_right = win.x + win.width;
    let win_bottom = win.y + win.height;
    el_right <= win.x || el.x >= win_right || el_bottom <= win.y || el.y >= win_bottom
}

#[cfg(test)]
#[path = "state_reader_tests.rs"]
mod tests;
