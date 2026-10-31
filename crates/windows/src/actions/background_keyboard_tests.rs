use super::{BackgroundTypePlan, type_text_by_messages};
use crate::input::keyboard::keyboard_message::{
    WindowMessage, activation_messages, click_messages, text_messages,
};
use crate::input::keyboard::keyboard_message_send::{
    send_window_messages, window_message_fake_sink as sink,
};
use crate::system::fixture::test_time::deadline;
use agent_desktop_core::{
    ActionStepOutcome, AdapterError, DeliveryDisposition, ErrorCode, StepMechanism,
};

const ROOT: isize = 0x1234;

fn plan(was_foreground: bool) -> BackgroundTypePlan<'static> {
    BackgroundTypePlan {
        root: ROOT,
        was_foreground,
        text: "hi",
    }
}

fn sent() -> Vec<WindowMessage> {
    sink::recorded()
        .into_iter()
        .map(|(hwnd, message)| {
            assert_eq!(hwnd, ROOT);
            message
        })
        .collect()
}

fn expected(parts: &[&[WindowMessage]]) -> Vec<WindowMessage> {
    parts.concat()
}

#[test]
fn background_window_is_activated_typed_and_deactivated_in_order() {
    sink::reset();
    let click = click_messages(10, 20);

    let step = type_text_by_messages(
        plan(false),
        deadline(5_000),
        |deadline| {
            send_window_messages(ROOT, &click, deadline);
            Ok(())
        },
        || true,
    )
    .expect("typed");

    assert_eq!(
        sent(),
        expected(&[
            &activation_messages(true),
            &click,
            &text_messages("hi"),
            &activation_messages(false),
        ])
    );
    assert_eq!(step.label(), "WindowMessage.type_text");
    assert_eq!(step.mechanism(), Some(StepMechanism::PhysicalSynthetic));
    assert_eq!(step.verified(), Some(false));
    assert!(matches!(step.outcome, ActionStepOutcome::Succeeded));
}

#[test]
fn foreground_window_is_typed_without_activation_messages() {
    sink::reset();

    type_text_by_messages(plan(true), deadline(5_000), |_| Ok(()), || true).expect("typed");

    assert_eq!(sent(), text_messages("hi"));
}

#[test]
fn window_that_became_foreground_is_not_deactivated() {
    sink::reset();

    type_text_by_messages(plan(false), deadline(5_000), |_| Ok(()), || false).expect("typed");

    assert_eq!(
        sent(),
        expected(&[&activation_messages(true), &text_messages("hi")])
    );
}

#[test]
fn failed_focus_types_nothing_and_still_deactivates() {
    sink::reset();

    let error = type_text_by_messages(
        plan(false),
        deadline(5_000),
        |_| {
            Err(AdapterError::new(
                ErrorCode::ActionFailed,
                "focus did not move",
            ))
        },
        || true,
    )
    .expect_err("focus failed");

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        sent(),
        expected(&[&activation_messages(true), &activation_messages(false)])
    );
    assert!(!sent().iter().any(WindowMessage::carries_text));
}

#[test]
fn expired_deadline_sends_nothing_and_reports_not_delivered() {
    sink::reset();

    let error = type_text_by_messages(plan(false), deadline(0), |_| Ok(()), || true)
        .expect_err("no budget");

    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
    assert!(sent().is_empty());
}
