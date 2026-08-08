use super::*;
use crate::input::mouse_modifier::modifier_fake_sink as key_sink;
use crate::input::mouse_send::mouse_send_fake_sink as mouse_sink;
use agent_desktop_core::{
    Deadline, DeliveryDisposition, ErrorCode, MAX_MOUSE_CLICK_COUNT, Modifier,
    MouseButton, MouseEvent, MouseEventKind, Point,
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

fn move_event(point: Point) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Move,
        point,
        button: MouseButton::Left,
        modifiers: Vec::new(),
    }
}

fn click_event(
    point: Point,
    count: u32,
    modifiers: Vec<Modifier>,
    button: MouseButton,
) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Click { count },
        point,
        button,
        modifiers,
    }
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
fn move_posts_exactly_one_absolute_move_event() {
    reset_sinks();
    synthesize_mouse(move_event(origin()), deadline()).expect("move succeeds");

    let recorded = mouse_sink::recorded();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].flags & MOUSEEVENTF_MOVE, MOUSEEVENTF_MOVE);
    assert_eq!(
        recorded[0].flags & MOUSEEVENTF_ABSOLUTE,
        MOUSEEVENTF_ABSOLUTE
    );
}

#[test]
fn a_single_click_posts_a_move_then_one_down_up_pair() {
    reset_sinks();
    synthesize_mouse(
        click_event(origin(), 1, Vec::new(), MouseButton::Left),
        deadline(),
    )
    .expect("click succeeds");

    let recorded = mouse_sink::recorded();
    assert_eq!(recorded.len(), 3, "move + down + up");
    assert_eq!(recorded[0].flags & MOUSEEVENTF_MOVE, MOUSEEVENTF_MOVE);
    assert_eq!(recorded[1].flags, MOUSEEVENTF_LEFTDOWN);
    assert_eq!(recorded[2].flags, MOUSEEVENTF_LEFTUP);
}

#[test]
fn a_triple_click_posts_a_move_then_three_down_up_pairs() {
    reset_sinks();
    synthesize_mouse(
        click_event(origin(), 3, Vec::new(), MouseButton::Left),
        deadline(),
    )
    .expect("triple click succeeds");

    let recorded = mouse_sink::recorded();
    assert_eq!(recorded.len(), 1 + 3 * 2);
    let downs = recorded
        .iter()
        .filter(|event| event.flags == MOUSEEVENTF_LEFTDOWN)
        .count();
    let ups = recorded
        .iter()
        .filter(|event| event.flags == MOUSEEVENTF_LEFTUP)
        .count();
    assert_eq!(downs, 3);
    assert_eq!(ups, 3);
}

#[test]
fn right_button_click_uses_the_right_button_flags_never_the_left() {
    reset_sinks();
    synthesize_mouse(
        click_event(origin(), 1, Vec::new(), MouseButton::Right),
        deadline(),
    )
    .expect("right click succeeds");

    let recorded = mouse_sink::recorded();
    assert!(
        recorded
            .iter()
            .any(|event| event.flags == MOUSEEVENTF_RIGHTDOWN)
    );
    assert!(
        recorded
            .iter()
            .any(|event| event.flags == MOUSEEVENTF_RIGHTUP)
    );
    assert!(
        !recorded
            .iter()
            .any(|event| event.flags == MOUSEEVENTF_LEFTDOWN || event.flags == MOUSEEVENTF_LEFTUP)
    );
}

#[test]
fn a_click_with_modifiers_holds_them_down_for_the_click_then_releases_them() {
    reset_sinks();
    synthesize_mouse(
        click_event(origin(), 1, vec![Modifier::Ctrl], MouseButton::Left),
        deadline(),
    )
    .expect("modified click succeeds");

    let keys = key_sink::recorded();
    assert_eq!(
        keys.first().map(|(_, up)| *up),
        Some(false),
        "the modifier must go down before the click"
    );
    assert_eq!(
        keys.last().map(|(_, up)| *up),
        Some(true),
        "the modifier must come back up after the click"
    );
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
fn an_off_primary_move_sets_the_virtual_desktop_flag() {
    reset_sinks();
    synthesize_mouse(move_event(Point { x: -5.0, y: -5.0 }), deadline())
        .expect("off-primary move still succeeds");

    let recorded = mouse_sink::recorded();
    assert_eq!(recorded.len(), 1);
    assert_eq!(
        recorded[0].flags & MOUSEEVENTF_VIRTUALDESK,
        MOUSEEVENTF_VIRTUALDESK
    );
}

#[test]
fn a_primary_move_never_sets_the_virtual_desktop_flag() {
    reset_sinks();
    synthesize_mouse(move_event(origin()), deadline()).expect("primary move succeeds");

    let recorded = mouse_sink::recorded();
    assert_eq!(recorded[0].flags & MOUSEEVENTF_VIRTUALDESK, 0);
}

#[test]
fn an_expired_deadline_rejects_before_any_injection() {
    reset_sinks();
    let expired = Deadline::after(0).expect("zero-length deadline");

    let error = synthesize_mouse(
        click_event(origin(), 1, Vec::new(), MouseButton::Left),
        expired,
    )
    .expect_err("expired deadline");

    assert_eq!(error.code, ErrorCode::Timeout);
    assert!(mouse_sink::recorded().is_empty());
    assert!(key_sink::recorded().is_empty());
}

#[test]
fn an_expired_deadline_mid_click_posts_corrective_up_with_enrichment() {
    reset_sinks();
    let tight = Deadline::after(8).expect("tight deadline");

    let error = synthesize_mouse(
        click_event(origin(), 1, Vec::new(), MouseButton::Left),
        tight,
    )
    .expect_err("deadline must abort mid-click");

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::DeliveredUnverified
    );
    let details = error.details.expect("abort-after-down enrichment");
    assert_eq!(details["delivered_events"], 1);
    assert_eq!(details["emergency_release_posted"], true);
    assert_eq!(details["emergency_release_acknowledged"], false);

    let recorded = mouse_sink::recorded();
    assert!(
        recorded
            .iter()
            .any(|event| event.flags == MOUSEEVENTF_LEFTDOWN),
        "down must have posted before abort"
    );
    assert!(
        recorded
            .iter()
            .any(|event| event.flags == MOUSEEVENTF_LEFTUP),
        "corrective up must post on abort"
    );
}

#[test]
fn down_is_rejected_as_a_standalone_state_error_with_zero_injection() {
    reset_sinks();
    let event = MouseEvent {
        kind: MouseEventKind::Down,
        point: origin(),
        button: MouseButton::Left,
        modifiers: Vec::new(),
    };
    let error = synthesize_mouse(event, deadline()).expect_err("standalone down is rejected");

    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    let details = error
        .details
        .as_ref()
        .expect("standalone error carries details");
    assert_eq!(details["raw_input_emitted"].as_bool(), Some(false));
    assert_eq!(
        details["requires_daemon_owned_transaction"].as_bool(),
        Some(true)
    );
    assert!(mouse_sink::recorded().is_empty());
}

#[test]
fn up_is_rejected_as_a_standalone_state_error_with_zero_injection() {
    reset_sinks();
    let event = MouseEvent {
        kind: MouseEventKind::Up,
        point: origin(),
        button: MouseButton::Left,
        modifiers: Vec::new(),
    };
    let error = synthesize_mouse(event, deadline()).expect_err("standalone up is rejected");

    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    assert!(mouse_sink::recorded().is_empty());
}

#[test]
fn a_zero_click_count_is_rejected_with_zero_injection() {
    reset_sinks();
    let error = synthesize_mouse(
        click_event(origin(), 0, Vec::new(), MouseButton::Left),
        deadline(),
    )
    .expect_err("zero click count is rejected");

    assert_eq!(error.code, ErrorCode::InvalidArgs);
    assert!(mouse_sink::recorded().is_empty());
}

#[test]
fn a_click_count_beyond_the_maximum_is_rejected_with_zero_injection() {
    reset_sinks();
    let error = synthesize_mouse(
        click_event(
            origin(),
            MAX_MOUSE_CLICK_COUNT + 1,
            Vec::new(),
            MouseButton::Left,
        ),
        deadline(),
    )
    .expect_err("out-of-bounds click count is rejected");

    assert_eq!(error.code, ErrorCode::InvalidArgs);
    assert!(mouse_sink::recorded().is_empty());
}

#[test]
fn an_invalid_point_is_rejected_with_zero_injection() {
    reset_sinks();
    let event = move_event(Point {
        x: f64::NAN,
        y: 0.0,
    });
    let error = synthesize_mouse(event, deadline()).expect_err("NaN point is rejected");

    assert_eq!(error.code, ErrorCode::InvalidArgs);
    assert!(mouse_sink::recorded().is_empty());
}
