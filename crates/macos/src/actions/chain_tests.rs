use super::{ChainStep, build_step, step_mechanism, step_verifies_effect};
use agent_desktop_core::action::MouseButton;
use agent_desktop_core::action_step_outcome::ActionStepOutcome;
use agent_desktop_core::step_mechanism::StepMechanism;

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
fn step_verifies_effect_matches_verified_chain_steps() {
    assert!(step_verifies_effect(&ChainStep::SetBool {
        attr: "AXSelected",
        value: true,
    }));
    assert!(step_verifies_effect(&ChainStep::Custom {
        label: "verified_press",
        func: |_| Ok(false),
    }));
    assert!(step_verifies_effect(&ChainStep::CustomWithDeadline {
        label: "expand_verified",
        func: |_, _| Ok(false),
    }));
    assert!(!step_verifies_effect(&ChainStep::Action("AXPress")));
}

#[test]
fn build_step_tags_mechanism_and_verified_on_success() {
    let step = ChainStep::SetBool {
        attr: "AXSelected",
        value: true,
    };
    let built = build_step(&step, ActionStepOutcome::Succeeded);
    assert_eq!(built.mechanism(), Some(StepMechanism::SemanticApi));
    assert_eq!(built.verified(), Some(true));
}

#[test]
fn build_step_skipped_does_not_tag_verified() {
    let step = ChainStep::SetBool {
        attr: "AXSelected",
        value: true,
    };
    let built = build_step(&step, ActionStepOutcome::Skipped);
    assert_eq!(built.mechanism(), Some(StepMechanism::SemanticApi));
    assert!(built.verified().is_none());
}
