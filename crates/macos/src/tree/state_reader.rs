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
mod tests {
    use super::*;
    use crate::tree::node_attrs::{NodeAttrStates, NodeAttrs};
    use agent_desktop_core::node::Rect;

    fn sample_attrs() -> NodeAttrs {
        NodeAttrs {
            role: Some("AXCheckBox".into()),
            title: None,
            description: None,
            value: Some("2".into()),
            states: NodeAttrStates {
                enabled: true,
                focused: None,
                expanded: None,
                disclosing: None,
                selected: None,
                hidden: None,
                busy: None,
                modal: None,
                required: None,
                readonly: None,
            },
            bounds: Some(Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            }),
            has_scrollbars: false,
        }
    }

    #[test]
    fn hidden_and_offscreen_tokens_are_vocabulary_members() {
        for token in [state::HIDDEN, state::OFFSCREEN, state::INDETERMINATE] {
            state::assert_states_in_vocabulary(&[token.to_string()]);
        }
    }

    #[test]
    fn mixed_checkbox_emits_indeterminate_not_checked() {
        let attrs = sample_attrs();
        let ctx = StateReaderContext {
            focused: None,
            window_bounds: None,
            is_secure_text: false,
        };
        let el = AXElement(std::ptr::null_mut());
        let states = states_from_element(&el, &attrs, "checkbox", &ctx);
        assert!(states.contains(&state::INDETERMINATE.to_string()));
        assert!(!states.contains(&state::CHECKED.to_string()));
    }

    #[test]
    fn hidden_attr_emits_hidden_token() {
        let mut attrs = sample_attrs();
        attrs.states.hidden = Some(true);
        let ctx = StateReaderContext {
            focused: None,
            window_bounds: None,
            is_secure_text: false,
        };
        let el = AXElement(std::ptr::null_mut());
        let states = states_from_element(&el, &attrs, "button", &ctx);
        assert!(states.contains(&state::HIDDEN.to_string()));
    }

    #[test]
    fn clipped_bounds_emit_offscreen() {
        let mut attrs = sample_attrs();
        attrs.bounds = Some(Rect {
            x: 100.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        });
        let window = Rect {
            x: 0.0,
            y: 0.0,
            width: 50.0,
            height: 50.0,
        };
        let ctx = StateReaderContext {
            focused: None,
            window_bounds: Some(window),
            is_secure_text: false,
        };
        let el = AXElement(std::ptr::null_mut());
        let states = states_from_element(&el, &attrs, "button", &ctx);
        assert!(states.contains(&state::OFFSCREEN.to_string()));
    }
}
