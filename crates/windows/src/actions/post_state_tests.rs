use super::attaches_post_state;
use super::{after_delivery, delivery_occurred, post_state_for_steps};
use crate::tree::automation::automation_client;
use crate::tree::element::UIAElement;
use crate::tree::fixture::{LocalFixture, ensure_test_apartment};
use crate::tree::fixture_window;
use agent_desktop_core::{
    Action, ActionStep, AdapterError, Deadline, DeliveryDisposition, ErrorCode,
};
use uiautomation::types::Handle;

fn deadline() -> Deadline {
    Deadline::after(5_000).expect("deadline")
}

fn with_fixture_button(run: impl FnOnce(&UIAElement)) {
    ensure_test_apartment();
    let fixture = LocalFixture::create().expect("fixture");
    let button = fixture_window::find_button(fixture.handle());
    let client = automation_client().expect("client");
    let element = client
        .element_from_handle(Handle::from(button as isize))
        .expect("button element");
    let element = UIAElement::from(element);
    run(&element);
    drop(fixture);
}

#[test]
fn state_bearing_actions_attach_post_state() {
    for action in [
        Action::SetValue("x".into()),
        Action::Clear,
        Action::Toggle,
        Action::Check,
        Action::Uncheck,
        Action::Expand,
        Action::Collapse,
    ] {
        assert!(
            attaches_post_state(&action),
            "{} should attach post_state",
            action.name()
        );
    }
}

#[test]
fn pointer_scroll_and_focus_attach_none() {
    for action in [
        Action::Click,
        Action::DoubleClick,
        Action::RightClick,
        Action::TripleClick,
        Action::SetFocus,
        Action::Select("x".into()),
        Action::Scroll(agent_desktop_core::Direction::Down, 1),
        Action::ScrollTo,
        Action::Hover,
    ] {
        assert!(
            !attaches_post_state(&action),
            "{} must not attach post_state",
            action.name()
        );
    }
}

#[test]
fn delivery_occurred_requires_a_succeeded_step() {
    assert!(!delivery_occurred(&[ActionStep::skipped(
        "ValuePattern.SetValue"
    )]));
    assert!(delivery_occurred(&[
        ActionStep::skipped("ValuePattern.SetValue"),
        ActionStep::succeeded("RangeValuePattern.SetValue"),
    ]));
}

#[test]
fn after_delivery_forces_delivered_unverified() {
    let error = after_delivery(
        AdapterError::new(ErrorCode::AppUnresponsive, "live evidence incomplete")
            .with_disposition(agent_desktop_core::DeliverySemantics::not_delivered()),
    );
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::DeliveredUnverified
    );
}

#[test]
fn post_state_absent_when_no_delivery() {
    with_fixture_button(|element| {
        let steps = vec![ActionStep::skipped("ValuePattern.SetValue")];
        let post = post_state_for_steps(element, &Action::SetValue("x".into()), &steps, deadline())
            .expect("no delivery skips post-state");
        assert!(post.is_none());
    });
}

#[test]
fn click_never_requests_post_state_even_after_delivery() {
    with_fixture_button(|element| {
        let steps = vec![ActionStep::succeeded("InvokePattern.Invoke").with_verified(false)];
        let post = post_state_for_steps(element, &Action::Click, &steps, deadline())
            .expect("click attaches none");
        assert!(post.is_none());
    });
}

#[test]
fn post_read_failure_after_delivery_is_delivered_unverified() {
    let error = after_delivery(AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Live element evidence was incomplete",
    ));
    assert_eq!(error.code, ErrorCode::AppUnresponsive);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::DeliveredUnverified
    );
    assert_ne!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
}
