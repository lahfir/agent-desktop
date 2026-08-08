use super::*;
use crate::input::mouse_modifier::modifier_fake_sink as key_sink;
use crate::input::mouse_send::mouse_send_fake_sink as mouse_sink;
use agent_desktop_core::{
    Deadline, ErrorCode, Modifier, MouseButton, MouseEvent, MouseEventKind, Point,
};

fn reset_sinks() {
    mouse_sink::reset();
    key_sink::reset();
}

fn origin() -> Point {
    Point { x: 1.0, y: 1.0 }
}

fn deadline() -> Deadline {
    Deadline::after(1_000).expect("bounded test deadline")
}

fn wheel_event(point: Point, delta_x: f64, delta_y: f64) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Wheel { delta_x, delta_y },
        point,
        button: MouseButton::Left,
        modifiers: Vec::new(),
    }
}

#[test]
fn wheel_up_posts_a_positive_wheel_delta_event() {
    reset_sinks();
    synthesize_mouse(wheel_event(origin(), 0.0, 1.0), deadline()).expect("wheel succeeds");

    let recorded = mouse_sink::recorded();
    let wheel = recorded
        .iter()
        .find(|event| event.flags == MOUSEEVENTF_WHEEL)
        .expect("a vertical wheel event");
    assert_eq!(wheel.mouse_data, WHEEL_DELTA);
}

#[test]
fn wheel_down_posts_a_negative_wheel_delta_event() {
    reset_sinks();
    synthesize_mouse(wheel_event(origin(), 0.0, -1.0), deadline()).expect("wheel succeeds");

    let recorded = mouse_sink::recorded();
    let wheel = recorded
        .iter()
        .find(|event| event.flags == MOUSEEVENTF_WHEEL)
        .expect("a vertical wheel event");
    assert_eq!(wheel.mouse_data, -WHEEL_DELTA);
}

#[test]
fn horizontal_wheel_uses_the_hwheel_flag_never_the_vertical_one() {
    reset_sinks();
    synthesize_mouse(wheel_event(origin(), 2.0, 0.0), deadline()).expect("wheel succeeds");

    let recorded = mouse_sink::recorded();
    assert!(
        recorded
            .iter()
            .any(|event| event.flags == MOUSEEVENTF_HWHEEL)
    );
    assert!(
        !recorded
            .iter()
            .any(|event| event.flags == MOUSEEVENTF_WHEEL)
    );
}

#[test]
fn a_large_wheel_delta_splits_into_bounded_chunks_that_sum_to_the_request() {
    reset_sinks();
    synthesize_mouse(wheel_event(origin(), 0.0, 25.0), deadline()).expect("wheel succeeds");

    let recorded = mouse_sink::recorded();
    let wheel_events: Vec<_> = recorded
        .iter()
        .filter(|event| event.flags == MOUSEEVENTF_WHEEL)
        .collect();
    assert_eq!(
        wheel_events.len(),
        3,
        "25 lines at 10 lines per event splits into three chunks"
    );
    let total: i32 = wheel_events
        .iter()
        .map(|event| event.mouse_data / WHEEL_DELTA)
        .sum();
    assert_eq!(total, 25);
}

#[test]
fn a_zero_wheel_delta_is_rejected_before_any_injection() {
    reset_sinks();
    let error =
        synthesize_mouse(wheel_event(origin(), 0.0, 0.0), deadline()).expect_err("zero delta");

    assert_eq!(error.code, ErrorCode::InvalidArgs);
    assert!(mouse_sink::recorded().is_empty());
}

#[test]
fn an_out_of_bounds_wheel_delta_is_rejected_before_any_injection() {
    reset_sinks();
    let error = synthesize_mouse(wheel_event(origin(), 0.0, 2_000.0), deadline())
        .expect_err("out-of-bounds delta");

    assert_eq!(error.code, ErrorCode::InvalidArgs);
    assert!(mouse_sink::recorded().is_empty());
}

#[test]
fn a_wheel_with_modifiers_holds_them_down_for_the_scroll_then_releases_them() {
    reset_sinks();
    let event = MouseEvent {
        kind: MouseEventKind::Wheel {
            delta_x: 0.0,
            delta_y: 1.0,
        },
        point: origin(),
        button: MouseButton::Left,
        modifiers: vec![Modifier::Shift],
    };
    synthesize_mouse(event, deadline()).expect("modified wheel succeeds");

    let keys = key_sink::recorded();
    assert_eq!(
        keys.first().map(|(_, up)| *up),
        Some(false),
        "the modifier must go down before the wheel event"
    );
    assert_eq!(
        keys.last().map(|(_, up)| *up),
        Some(true),
        "the modifier must come back up after the wheel event"
    );
}
