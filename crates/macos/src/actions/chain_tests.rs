use super::{ChainStep, build_step, record_step_outcome, step_mechanism};
use crate::actions::chain_delivery::DeliveryOutcome;
use agent_desktop_core::MouseButton;
use agent_desktop_core::step_mechanism::StepMechanism;

#[test]
fn right_click_restores_semantic_menu_fallbacks_before_physical_input() {
    let labels: Vec<&str> = crate::actions::chain_defs::RIGHT_CLICK_CHAIN
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
            "show_menu",
            "select_then_show_menu",
            "selected_items_menu",
            "child_show_menu",
            "ancestor_show_menu",
            "CGClick",
        ]
    );
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
        step_mechanism(&ChainStep::CGDisclosureClick { expanded: true }),
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
fn build_step_tags_mechanism_and_verified_on_success() {
    let step = ChainStep::SetBool {
        attr: "AXSelected",
        value: true,
    };
    let built = build_step(&step, DeliveryOutcome::DeliveredVerified);
    assert_eq!(built.mechanism(), Some(StepMechanism::SemanticApi));
    assert_eq!(built.verified(), Some(true));
}

#[test]
fn build_step_skipped_does_not_tag_verified() {
    let step = ChainStep::SetBool {
        attr: "AXSelected",
        value: true,
    };
    let built = build_step(&step, DeliveryOutcome::NotDelivered);
    assert_eq!(built.mechanism(), Some(StepMechanism::SemanticApi));
    assert!(built.verified().is_none());
}

#[test]
fn satisfied_without_delivery_stops_fallback_and_is_skipped_verified() {
    let step = ChainStep::Action("AlreadySatisfied");
    let mut steps = Vec::new();

    assert!(record_step_outcome(
        &mut steps,
        &step,
        DeliveryOutcome::SatisfiedNoDelivery,
        false,
    ));
    assert!(matches!(
        steps[0].outcome,
        agent_desktop_core::ActionStepOutcome::Skipped
    ));
    assert_eq!(steps[0].verified(), Some(true));
}

#[test]
fn build_step_marks_delivered_unverified_explicitly() {
    let built = build_step(
        &ChainStep::Action("AXPress"),
        DeliveryOutcome::DeliveredUnverified,
    );

    assert_eq!(built.mechanism(), Some(StepMechanism::SemanticApi));
    assert_eq!(built.verified(), Some(false));
}

#[test]
fn native_list_press_success_stops_click_chain_before_other_mutations() {
    let rungs = [
        (
            ChainStep::Action("AXPress"),
            DeliveryOutcome::DeliveredUnverified,
        ),
        (
            ChainStep::Action("AXConfirm"),
            DeliveryOutcome::DeliveredVerified,
        ),
        (
            ChainStep::Action("AXOpen"),
            DeliveryOutcome::DeliveredVerified,
        ),
        (
            ChainStep::CGClick {
                button: MouseButton::Left,
                count: 1,
            },
            DeliveryOutcome::DeliveredUnverified,
        ),
    ];
    let mut calls = Vec::new();
    let mut steps = Vec::new();

    for (step, outcome) in &rungs {
        calls.push(match step {
            ChainStep::Action(name) => *name,
            ChainStep::CGClick { .. } => "CGClick",
            _ => "other",
        });
        if record_step_outcome(&mut steps, step, *outcome, false) {
            break;
        }
    }

    assert_eq!(calls, ["AXPress"]);
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].verified(), Some(false));
}

#[test]
fn idempotent_chain_continues_after_unverified_delivery() {
    let step = ChainStep::SetDynamic { attr: "AXValue" };
    let mut steps = Vec::new();

    assert!(!record_step_outcome(
        &mut steps,
        &step,
        DeliveryOutcome::DeliveredUnverified,
        true,
    ));
    assert_eq!(steps[0].verified(), Some(false));
}

#[test]
fn non_idempotent_chain_stops_after_unverified_delivery() {
    let step = ChainStep::Action("AXPress");
    let mut steps = Vec::new();

    assert!(record_step_outcome(
        &mut steps,
        &step,
        DeliveryOutcome::DeliveredUnverified,
        false,
    ));
}
