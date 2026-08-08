use super::{press_key_from_gate, press_key_global, type_text_from_gate};
use crate::actions::physical_target::ensure_keyboard_policy;
use crate::input::keyboard_send_fake_sink as key_sink;
use agent_desktop_core::{
    ActionStepOutcome, Deadline, DeliveryDisposition, ErrorCode, InteractionPolicy, KeyCombo,
    StepMechanism,
};

fn deadline() -> Deadline {
    Deadline::after(5_000).expect("deadline")
}

fn combo() -> KeyCombo {
    KeyCombo {
        key: "a".into(),
        modifiers: vec![],
    }
}

#[test]
fn headless_keyboard_policy_is_denied_before_injection() {
    key_sink::reset();
    let error = ensure_keyboard_policy(InteractionPolicy::headless()).expect_err("headless");
    assert_eq!(error.code, ErrorCode::PolicyDenied);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
    assert!(key_sink::recorded().is_empty());
}

#[test]
fn type_text_lost_focus_fails_not_delivered_with_zero_injection() {
    key_sink::reset();

    let error = type_text_from_gate("hello", false, deadline()).expect_err("lost focus");

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
    assert!(key_sink::recorded().is_empty());
}

#[test]
fn type_text_with_focus_ready_synthesizes() {
    key_sink::reset();

    let step = type_text_from_gate("hi", true, deadline()).expect("type");

    assert_eq!(step.label(), "SendInput.type_text");
    assert_eq!(step.mechanism(), Some(StepMechanism::PhysicalSynthetic));
    assert_eq!(step.verified(), Some(false));
    assert!(matches!(step.outcome, ActionStepOutcome::Succeeded));
    assert!(!key_sink::recorded().is_empty());
}

#[test]
fn press_key_element_gate_synthesizes_when_focus_ready() {
    key_sink::reset();

    let step = press_key_from_gate(&combo(), true, deadline()).expect("press");

    assert_eq!(step.label(), "SendInput.press_key");
    assert_eq!(step.mechanism(), Some(StepMechanism::PhysicalSynthetic));
    assert!(!key_sink::recorded().is_empty());
}

#[test]
fn press_key_global_synthesizes_without_focus_gate() {
    key_sink::reset();

    let result = press_key_global(&combo(), deadline()).expect("global press");

    assert_eq!(result.action, "press");
    assert_eq!(result.steps.len(), 1);
    assert_eq!(result.steps[0].label(), "SendInput.press_key");
    assert_eq!(
        result.steps[0].mechanism(),
        Some(StepMechanism::PhysicalSynthetic)
    );
    assert!(!key_sink::recorded().is_empty());
}

#[test]
fn press_key_lost_focus_fails_not_delivered_with_zero_injection() {
    key_sink::reset();

    let error = press_key_from_gate(&combo(), false, deadline()).expect_err("lost focus");

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
    assert!(key_sink::recorded().is_empty());
}
