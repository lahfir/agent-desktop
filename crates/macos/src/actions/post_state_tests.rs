use super::*;
use crate::tree::node_attrs::{NodeAttrStates, NodeAttrs};

fn attrs_with_bounds(bounds: Rect) -> NodeAttrs {
    NodeAttrs {
        role: Some("AXButton".into()),
        title: Some("Target".into()),
        description: None,
        value: None,
        native_id: None,
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
        bounds: Some(bounds),
        has_scrollbars: false,
    }
}

/// Proves the `window_bounds` parameter this fix threads into
/// `element_state_from_attrs` actually reaches the offscreen computation:
/// with a window smaller than the element's position, the resulting state
/// must carry `offscreen`. Before this fix the call sites hardcoded
/// `window_bounds: None`, so this would never fire regardless of the
/// element's real position relative to its window.
#[test]
fn element_state_from_attrs_includes_offscreen_when_window_bounds_supplied() {
    let el = crate::tree::AXElement(std::ptr::null_mut());
    let attrs = attrs_with_bounds(Rect {
        x: 1000.0,
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

    let state = element_state_from_attrs(&el, attrs, "button".into(), Some(window));

    assert!(
        state
            .states
            .contains(&agent_desktop_core::state::OFFSCREEN.to_string())
    );
}

#[test]
fn element_state_from_attrs_omits_offscreen_without_window_bounds() {
    let el = crate::tree::AXElement(std::ptr::null_mut());
    let attrs = attrs_with_bounds(Rect {
        x: 1000.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    });

    let state = element_state_from_attrs(&el, attrs, "button".into(), None);

    assert!(
        !state
            .states
            .contains(&agent_desktop_core::state::OFFSCREEN.to_string())
    );
}

/// `owning_window_bounds` must degrade to `None` rather than panic when the
/// element has no reachable `AXWindow` (e.g. a detached/null probe), so a
/// missing window never turns into a hard failure for post-action state
/// reads.
#[test]
fn owning_window_bounds_is_none_for_detached_element() {
    let el = crate::tree::AXElement(std::ptr::null_mut());
    assert_eq!(owning_window_bounds(&el), None);
}
