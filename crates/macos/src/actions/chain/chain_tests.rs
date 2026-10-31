use super::{ChainStep, exhaustion_disposition, record_step_outcome, step_allowed, step_mechanism};
use crate::actions::chain::chain_delivery::DeliveryOutcome;
use agent_desktop_core::MouseButton;
use agent_desktop_core::actions::step_mechanism::StepMechanism;

#[test]
fn right_click_prefers_physical_input_before_semantic_fallbacks() {
    let labels: Vec<&str> = crate::actions::chain::chain_defs::RIGHT_CLICK_CHAIN
        .steps
        .iter()
        .map(|step| match step {
            ChainStep::CustomWithDeadline { label, .. } => *label,
            ChainStep::CGClick { .. } => "CGClick",
            ChainStep::Action(name) => name,
            _ => "other",
        })
        .collect();

    assert_eq!(
        labels,
        [
            "CGClick",
            "show_menu",
            "select_then_show_menu",
            "selected_items_menu",
            "child_show_menu",
            "ancestor_show_menu",
        ]
    );
}

#[test]
fn click_and_clear_prefer_physical_delivery_when_policy_allows_it() {
    assert!(matches!(
        crate::actions::chain::chain_defs::click_chain(false)
            .steps
            .first(),
        Some(ChainStep::CGClick {
            button: MouseButton::Left,
            count: 1
        })
    ));
    assert!(matches!(
        crate::actions::chain::chain_defs::CLEAR_CHAIN.steps.first(),
        Some(ChainStep::FocusThenClearByKeyboard)
    ));
}

#[test]
fn step_mechanism_tags_physical_for_cgclick_and_keyboard_clear() {
    assert_eq!(
        step_mechanism(&ChainStep::CGClick {
            button: MouseButton::Left,
            count: 1,
        }),
        StepMechanism::PhysicalSynthetic
    );
    assert_eq!(
        step_mechanism(&ChainStep::FocusThenClearByKeyboard),
        StepMechanism::PhysicalSynthetic
    );
    assert_eq!(
        step_mechanism(&ChainStep::Action("AXPress")),
        StepMechanism::SemanticApi
    );
}

#[test]
fn headless_chains_omit_physical_steps_before_reporting() {
    let headless = agent_desktop_core::InteractionPolicy::headless();
    let headed = agent_desktop_core::InteractionPolicy::headed();
    let physical = ChainStep::CGClick {
        button: MouseButton::Left,
        count: 1,
    };

    assert!(!step_allowed(&physical, headless));
    assert!(step_allowed(&physical, headed));
    assert!(step_allowed(&ChainStep::Action("AXPress"), headless));
}

#[test]
fn headed_menubar_click_uses_only_semantic_press_and_keeps_other_clicks_physical() {
    let menu = crate::actions::chain::chain_defs::click_chain(true);
    assert!(matches!(menu.steps, [ChainStep::Action("AXPress")]));
    assert!(!menu.continue_after_unverified_delivery);
    let mut steps = Vec::new();
    assert!(record_step_outcome(
        &mut steps,
        &menu.steps[0],
        DeliveryOutcome::DeliveredUnverified,
        false
    ));
    assert_eq!(steps[0].mechanism(), Some(StepMechanism::SemanticApi));
    assert_eq!(steps[0].verified(), Some(false));
    let mut unsupported = Vec::new();
    assert!(!record_step_outcome(
        &mut unsupported,
        &menu.steps[0],
        DeliveryOutcome::NotDelivered,
        false,
    ));
    assert_eq!(
        exhaustion_disposition(&unsupported),
        agent_desktop_core::DeliverySemantics::not_delivered()
    );
    assert!(matches!(
        crate::actions::chain::chain_defs::click_chain(false)
            .steps
            .first(),
        Some(ChainStep::CGClick { .. })
    ));
}

#[test]
fn targets_without_native_menu_provenance_keep_the_default_chain() {
    let fallback = crate::actions::chain::chain_defs::click_chain(false);
    assert!(matches!(
        fallback.steps.first(),
        Some(ChainStep::CGClick {
            button: MouseButton::Left,
            count: 1
        })
    ));
    assert!(!std::ptr::eq(
        fallback,
        crate::actions::chain::chain_defs::click_chain(true)
    ));
}

#[test]
fn the_click_chain_verifies_its_press_before_writing_a_selection() {
    let labels: Vec<&str> = crate::actions::chain::chain_defs::click_chain(false)
        .steps
        .iter()
        .map(|step| match step {
            ChainStep::CustomWithDeadline { label, .. } => *label,
            ChainStep::CGClick { .. } => "CGClick",
            ChainStep::Action(name) => *name,
            _ => "other",
        })
        .collect();

    assert_eq!(
        labels,
        [
            "CGClick",
            "AXPress",
            "AXOpen",
            "activate_descendant",
            "select_within_container",
            "AXConfirm",
        ]
    );
}
