use super::*;
use crate::tree::{node_attr_states::NodeAttrStates, node_attrs::NodeAttrs};

fn attrs_with_bounds(bounds: Rect) -> NodeAttrs {
    NodeAttrs {
        role: Some("AXButton".into()),
        subrole: None,
        value: None,
        name_evidence: agent_desktop_core::NameEvidence {
            native_title: Some("Target".into()),
            ..agent_desktop_core::NameEvidence::default()
        },
        states: NodeAttrStates::default(),
        bounds: Some(bounds),
        has_scrollbars: false,
    }
}

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

    let state = element_state_from_attrs(&el, attrs, "button".into(), Some(window)).unwrap();

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

    let state = element_state_from_attrs(&el, attrs, "button".into(), None).unwrap();

    assert!(
        !state
            .states
            .contains(&agent_desktop_core::state::OFFSCREEN.to_string())
    );
}

#[test]
fn post_delay_is_skipped_when_it_would_exhaust_the_budget() {
    let deadline = Deadline::after(1).unwrap();

    assert!(!pause_if_budget_allows(
        deadline,
        std::time::Duration::from_millis(50)
    ));
}

#[test]
fn click_does_not_post_read_a_target_that_navigation_may_detach() {
    let element = crate::tree::AXElement(std::ptr::null_mut());
    let state = read_post_state(&element, &Action::Click, Deadline::after(1).unwrap()).unwrap();

    assert!(state.is_none());
}

#[test]
fn post_state_uses_the_same_subrole_mapping_as_snapshot_observation() {
    assert_eq!(
        normalized_role(Some("AXRow"), Some("AXOutlineRow")),
        "treeitem"
    );
}

#[test]
fn secure_subrole_never_exposes_its_value() {
    let el = crate::tree::AXElement(std::ptr::null_mut());
    let mut attrs = attrs_with_bounds(Rect {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    });
    attrs.role = Some("AXTextField".into());
    attrs.subrole = Some("AXSecureTextField".into());
    attrs.value = Some("secret".into());

    let state = element_state_from_attrs(&el, attrs, "textfield".into(), None).unwrap();

    assert_eq!(state.value, None);
}

#[test]
fn element_visibility_preserves_live_hidden_evidence_for_every_role() {
    assert_eq!(hidden_state(None), None);
    assert_eq!(hidden_state(Some(false)), Some(false));
    assert_eq!(hidden_state(Some(true)), Some(true));
}

#[test]
fn top_level_container_is_used_only_when_window_is_authoritatively_absent() {
    let mut attributes = Vec::new();
    let container = first_owning_container(|attribute| {
        attributes.push(attribute);
        Ok((attribute == "AXTopLevelUIElement")
            .then(|| crate::tree::AXElement(std::ptr::null_mut())))
    })
    .unwrap();

    assert!(container.is_some());
    assert_eq!(attributes, ["AXWindow", "AXTopLevelUIElement"]);
}

#[test]
fn incomplete_window_read_never_falls_through_to_a_weaker_container() {
    for error in [
        accessibility_sys::kAXErrorCannotComplete,
        accessibility_sys::kAXErrorInvalidUIElement,
    ] {
        let calls = std::cell::Cell::new(0);
        let result = first_owning_container(|_| {
            calls.set(calls.get() + 1);
            Err(error)
        });

        let failure = match result {
            Err(failure) => failure,
            Ok(_) => panic!("incomplete AXWindow read must fail"),
        };
        assert_eq!(failure, ("AXWindow", error));
        assert_eq!(calls.get(), 1);
    }
}

#[test]
fn optional_identity_gaps_do_not_poison_complete_actionability_evidence() {
    let mut evidence = agent_desktop_core::LocatorEvidence {
        role: LocatorField::Known("scrollarea".into()),
        name: LocatorField::Unknown,
        description: LocatorField::Unknown,
        value: LocatorField::Absent,
        identifiers: agent_desktop_core::IdentifierEvidence::unknown(),
        states: LocatorField::Known(Vec::new()),
        ref_evidence: agent_desktop_core::LocatorRefEvidence {
            bounds: LocatorField::Known(Rect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            }),
            available_actions: LocatorField::Known(vec!["Scroll".into()]),
        },
    };

    assert!(essential_live_evidence_complete(&evidence));
    evidence.states = LocatorField::Unknown;
    assert!(!essential_live_evidence_complete(&evidence));
}
