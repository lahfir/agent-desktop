use super::{clear_judged_for, parse_finite_f64, set_value_judged_for};
use crate::actions::chain::DeliveryOutcome;
use agent_desktop_core::{
    Action, ActionStepOutcome, Deadline, DeliveryDisposition, ElementState, ErrorCode,
    InteractionPolicy,
};
use std::cell::Cell;

fn deadline() -> Deadline {
    Deadline::after(5_000).expect("deadline")
}

#[test]
fn set_value_verified_when_readback_equals() {
    let steps = set_value_judged_for(
        deadline(),
        InteractionPolicy::headless(),
        "hello",
        true,
        false,
        || Ok(DeliveryOutcome::DeliveredVerified),
        || Ok(DeliveryOutcome::NotDelivered),
    )
    .expect("set-value");
    assert_eq!(steps.len(), 1);
    assert!(matches!(steps[0].outcome, ActionStepOutcome::Succeeded));
    assert_eq!(steps[0].verified(), Some(true));
    let result = agent_desktop_core::ActionResult::from_execution(
        &Action::SetValue("hello".into()),
        steps,
        None,
    )
    .expect("result");
    assert_eq!(
        result.disposition().delivery(),
        DeliveryDisposition::DeliveredVerified
    );
}

#[test]
fn unequal_readback_continues_to_range_value() {
    let value_calls = Cell::new(0u8);
    let range_calls = Cell::new(0u8);
    let steps = set_value_judged_for(
        deadline(),
        InteractionPolicy::headless(),
        "77",
        true,
        true,
        || {
            value_calls.set(value_calls.get() + 1);
            Ok(DeliveryOutcome::DeliveredUnverified)
        },
        || {
            range_calls.set(range_calls.get() + 1);
            Ok(DeliveryOutcome::DeliveredVerified)
        },
    )
    .expect("continues");
    assert_eq!(value_calls.get(), 1);
    assert_eq!(range_calls.get(), 1);
    assert_eq!(steps[0].verified(), Some(false));
    assert_eq!(steps[1].label(), "RangeValuePattern.SetValue");
    assert_eq!(steps[1].verified(), Some(true));
}

#[test]
fn range_value_numeric_on_value_less_control() {
    let steps = set_value_judged_for(
        deadline(),
        InteractionPolicy::headless(),
        "77",
        false,
        true,
        || Ok(DeliveryOutcome::DeliveredVerified),
        || Ok(DeliveryOutcome::DeliveredVerified),
    )
    .expect("range only");
    assert_eq!(steps[0].label(), "ValuePattern.SetValue");
    assert!(matches!(steps[0].outcome, ActionStepOutcome::Skipped));
    assert_eq!(steps[1].verified(), Some(true));
}

#[test]
fn unparsable_range_value_exhausts_honestly() {
    let range_calls = Cell::new(0u8);
    let error = set_value_judged_for(
        deadline(),
        InteractionPolicy::headless(),
        "not-a-number",
        false,
        true,
        || Ok(DeliveryOutcome::NotDelivered),
        || {
            range_calls.set(range_calls.get() + 1);
            Ok(DeliveryOutcome::DeliveredVerified)
        },
    )
    .expect_err("exhausted");
    assert_eq!(range_calls.get(), 0);
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
}

#[test]
fn parse_finite_rejects_nan_and_non_numeric() {
    assert_eq!(parse_finite_f64("77"), Some(77.0));
    assert!(parse_finite_f64("abc").is_none());
    assert!(parse_finite_f64("NaN").is_none());
    assert!(parse_finite_f64("inf").is_none());
}

#[test]
fn clear_empty_post_state_satisfies_core_postcondition() {
    let steps = clear_judged_for(deadline(), InteractionPolicy::headless(), true, || {
        Ok(DeliveryOutcome::DeliveredVerified)
    })
    .expect("clear");
    let state = ElementState {
        role: "textfield".into(),
        states: vec![],
        value: Some(String::new()),
        enabled: Some(true),
        hidden: None,
        offscreen: Some(false),
    };
    let result =
        agent_desktop_core::ActionResult::from_execution(&Action::Clear, steps, Some(state))
            .expect("clear ok");
    assert!(result.post_state.is_some());
}

#[test]
fn clear_refusing_fake_yields_action_failed_delivered_unverified() {
    let steps = clear_judged_for(deadline(), InteractionPolicy::headless(), true, || {
        Ok(DeliveryOutcome::DeliveredUnverified)
    })
    .expect("delivered");
    let state = ElementState {
        role: "textfield".into(),
        states: vec![],
        value: Some("still-here".into()),
        enabled: Some(true),
        hidden: None,
        offscreen: Some(false),
    };
    let error =
        agent_desktop_core::ActionResult::from_execution(&Action::Clear, steps, Some(state))
            .expect_err("postcondition");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::DeliveredUnverified
    );
}

#[test]
fn error_envelope_carries_value_chars_never_marker_text() {
    const MARKER: &str = "zzsetvaluemarkerzz";
    let error = set_value_judged_for(
        deadline(),
        InteractionPolicy::headless(),
        MARKER,
        false,
        false,
        || Ok(DeliveryOutcome::NotDelivered),
        || Ok(DeliveryOutcome::NotDelivered),
    )
    .expect_err("exhausted");
    assert!(!error.message.contains(MARKER));
    assert!(
        !error
            .platform_detail
            .as_deref()
            .unwrap_or("")
            .contains(MARKER)
    );
    assert_eq!(
        error
            .details
            .as_ref()
            .and_then(|details| details.get("value_chars")),
        Some(&serde_json::json!(MARKER.chars().count()))
    );
}
