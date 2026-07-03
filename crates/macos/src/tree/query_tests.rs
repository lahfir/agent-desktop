use super::*;
use crate::tree::node_attrs::{NodeAttrStates, NodeAttrs};
use agent_desktop_core::locator::StatePredicate;

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

fn offscreen_query() -> LocatorQuery {
    LocatorQuery {
        states: vec![StatePredicate {
            token: "offscreen".into(),
            expected: Some(true),
        }],
        ..Default::default()
    }
}

/// Proves the `window_bounds` this fix threads from `collect_matches` into
/// `element_matches` actually reaches `states_from_element`: a `find`/`query`
/// filtering on `states: [offscreen]` must match an element positioned
/// outside its window once real window bounds are supplied. Before this fix
/// every call site hardcoded `window_bounds: None`, so an `offscreen` state
/// filter could never match anything through `find`/`query`.
#[test]
fn element_matches_detects_offscreen_when_window_bounds_supplied() {
    let el = AXElement(std::ptr::null_mut());
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
    let query = offscreen_query();

    let matched = element_matches(&el, &attrs, "button", &query, Some(window), 0).unwrap();

    assert!(matched);
}

#[test]
fn element_matches_never_flags_offscreen_without_window_bounds() {
    let el = AXElement(std::ptr::null_mut());
    let attrs = attrs_with_bounds(Rect {
        x: 1000.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    });
    let query = offscreen_query();

    let matched = element_matches(&el, &attrs, "button", &query, None, 0).unwrap();

    assert!(!matched);
}

#[test]
fn window_bounds_for_children_captures_window_own_bounds() {
    let mut attrs = attrs_with_bounds(Rect {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
    });
    attrs.role = Some("AXWindow".into());
    let inherited = Some(Rect {
        x: 10.0,
        y: 10.0,
        width: 5.0,
        height: 5.0,
    });

    let result = window_bounds_for_children(&attrs, inherited);

    assert_eq!(result, attrs.bounds);
}

#[test]
fn window_bounds_for_children_passes_through_inherited_for_non_window_roles() {
    let attrs = attrs_with_bounds(Rect {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
    });
    let inherited = Some(Rect {
        x: 10.0,
        y: 10.0,
        width: 5.0,
        height: 5.0,
    });

    let result = window_bounds_for_children(&attrs, inherited);

    assert_eq!(result, inherited);
}
