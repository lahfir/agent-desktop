use super::*;
use agent_desktop_core::{ErrorCode, Modifier, Point};

const WINDOW_NUMBER: i64 = 9555;
const PID: libc::pid_t = 4242;

fn event(kind: MouseEventKind, button: MouseButton) -> MouseEvent {
    MouseEvent {
        kind,
        point: Point {
            x: -2850.0,
            y: 160.0,
        },
        button,
        modifiers: vec![Modifier::Shift],
    }
}

fn routing(route: bool) -> EventRouting {
    EventRouting {
        pid: PID,
        window_number: WINDOW_NUMBER,
        flags: CGEventFlags::CGEventFlagShift,
        route,
    }
}

fn summary(planned: &[PlannedEvent]) -> Vec<(Phase, f64, f64, i64, u128)> {
    planned
        .iter()
        .map(|event| {
            (
                event.phase,
                event.global.x,
                event.global.y,
                event.click_state,
                event.pause_after.as_millis(),
            )
        })
        .collect()
}

fn field(event: &CGEvent, field: u32) -> i64 {
    event.get_integer_value_field(field)
}

#[test]
fn routing_fields_use_the_documented_numbers() {
    assert_eq!(EventField::MOUSE_EVENT_CLICK_STATE, 1);
    assert_eq!(EventField::MOUSE_EVENT_PRESSURE, 2);
    assert_eq!(EventField::MOUSE_EVENT_BUTTON_NUMBER, 3);
    assert_eq!(EventField::MOUSE_EVENT_SUB_TYPE, 7);
    assert_eq!(EventField::EVENT_TARGET_UNIX_PROCESS_ID, 40);
    assert_eq!(FIELD_WINDOW_NUMBER, 51);
    assert_eq!(FIELD_CLICK_GROUP, 58);
    assert_eq!(EventField::MOUSE_EVENT_WINDOW_UNDER_MOUSE_POINTER, 91);
    assert_eq!(
        EventField::MOUSE_EVENT_WINDOW_UNDER_MOUSE_POINTER_THAT_CAN_HANDLE_THIS_EVENT,
        92
    );
}

#[test]
fn hover_plans_a_single_move_at_the_target() {
    let planned = plan(&event(MouseEventKind::Move, MouseButton::Left), true, None).unwrap();
    assert_eq!(summary(&planned), [(Phase::Move, -2850.0, 160.0, 0, 0)]);
}

#[test]
fn routed_double_click_moves_first_then_pairs_click_states() {
    let planned = plan(
        &event(MouseEventKind::Click { count: 2 }, MouseButton::Right),
        true,
        None,
    )
    .unwrap();

    assert_eq!(
        summary(&planned),
        [
            (Phase::Move, -2850.0, 160.0, 0, 15),
            (Phase::Down, -2850.0, 160.0, 1, 10),
            (Phase::Up, -2850.0, 160.0, 1, 30),
            (Phase::Down, -2850.0, 160.0, 2, 10),
            (Phase::Up, -2850.0, 160.0, 2, 0),
        ]
    );
}

#[test]
fn unrouted_click_is_the_bare_down_up_pair() {
    let planned = plan(
        &event(MouseEventKind::Click { count: 1 }, MouseButton::Left),
        false,
        None,
    )
    .unwrap();
    assert_eq!(
        summary(&planned),
        [
            (Phase::Down, -2850.0, 160.0, 1, 10),
            (Phase::Up, -2850.0, 160.0, 1, 0),
        ]
    );
}

#[test]
fn primer_clicks_left_button_just_outside_the_window_frame_before_the_target() {
    let origin = CGPoint::new(-3000.0, 100.0);
    let planned = plan(
        &event(MouseEventKind::Click { count: 1 }, MouseButton::Right),
        false,
        Some(origin),
    )
    .unwrap();

    assert_eq!(
        summary(&planned),
        [
            (Phase::Down, -3001.0, 99.0, 1, 10),
            (Phase::Up, -3001.0, 99.0, 1, 100),
            (Phase::Down, -2850.0, 160.0, 1, 10),
            (Phase::Up, -2850.0, 160.0, 1, 0),
        ]
    );
    assert!(matches!(planned[0].button, MouseButton::Left));
    assert!(matches!(planned[2].button, MouseButton::Right));
    let local = window_local(planned[0].global, origin);
    assert_eq!((local.x, local.y), (-1.0, -1.0));
}

#[test]
fn zero_wheel_standalone_state_and_zero_count_are_not_planned() {
    let rejected = |kind| {
        plan(&event(kind, MouseButton::Left), true, None)
            .expect_err("event must be rejected")
            .code
    };
    assert_eq!(
        rejected(MouseEventKind::Wheel {
            delta_x: 0.0,
            delta_y: 0.0
        }),
        ErrorCode::InvalidArgs
    );
    assert_eq!(
        rejected(MouseEventKind::Down),
        ErrorCode::ActionNotSupported
    );
    assert_eq!(
        rejected(MouseEventKind::Click { count: 0 }),
        ErrorCode::InvalidArgs
    );
}

#[test]
fn unrouted_events_carry_only_click_state_and_window_fields() {
    let planned = plan(
        &event(MouseEventKind::Click { count: 1 }, MouseButton::Left),
        false,
        None,
    )
    .unwrap();
    let built = build(&planned[0], &routing(false)).unwrap();

    assert_eq!(built.get_type() as u32, CGEventType::LeftMouseDown as u32);
    assert_eq!(field(&built, EventField::MOUSE_EVENT_CLICK_STATE), 1);
    assert_eq!(field(&built, 91), WINDOW_NUMBER);
    assert_eq!(field(&built, 92), WINDOW_NUMBER);
    assert_eq!(field(&built, FIELD_WINDOW_NUMBER), 0);
    assert_eq!(field(&built, FIELD_CLICK_GROUP), 0);
    assert_eq!(field(&built, EventField::MOUSE_EVENT_SUB_TYPE), 0);
    assert!(built.get_flags().contains(CGEventFlags::CGEventFlagShift));
}

#[test]
fn routed_events_carry_pid_window_group_button_subtype_and_pressure() {
    let planned = plan(
        &event(MouseEventKind::Click { count: 1 }, MouseButton::Right),
        true,
        None,
    )
    .unwrap();
    let built: Vec<CGEvent> = planned
        .iter()
        .map(|planned| build(planned, &routing(true)).unwrap())
        .collect();

    let types: Vec<u32> = built.iter().map(|event| event.get_type() as u32).collect();
    assert_eq!(
        types,
        [
            CGEventType::MouseMoved as u32,
            CGEventType::RightMouseDown as u32,
            CGEventType::RightMouseUp as u32,
        ]
    );
    for event in &built {
        assert_eq!(
            field(event, EventField::EVENT_TARGET_UNIX_PROCESS_ID),
            i64::from(PID)
        );
        assert_eq!(field(event, FIELD_WINDOW_NUMBER), WINDOW_NUMBER);
        assert_eq!(field(event, FIELD_CLICK_GROUP), 1);
        assert_eq!(field(event, 91), WINDOW_NUMBER);
        assert_eq!(field(event, 92), WINDOW_NUMBER);
        assert_eq!(field(event, EventField::MOUSE_EVENT_BUTTON_NUMBER), 1);
        assert_eq!(
            field(event, EventField::MOUSE_EVENT_SUB_TYPE),
            SUBTYPE_TOUCH
        );
    }
    let pressures: Vec<f64> = built
        .iter()
        .map(|event| event.get_double_value_field(EventField::MOUSE_EVENT_PRESSURE))
        .collect();
    assert_eq!(pressures, [0.0, 1.0, 0.0]);
    let location = built[1].location();
    assert_eq!((location.x, location.y), (-2850.0, 160.0));
}

#[test]
fn window_local_conversion_handles_negative_origins() {
    let cases = [
        ((100.0, 50.0), (0.0, 0.0), (100.0, 50.0)),
        ((-1580.5, 966.5), (-1920.0, 900.0), (339.5, 66.5)),
        ((3521.0, 2034.0), (3440.0, 1440.0), (81.0, 594.0)),
        ((-10.0, -300.0), (-200.0, -400.0), (190.0, 100.0)),
    ];
    for ((gx, gy), (ox, oy), expected) in cases {
        let local = window_local(CGPoint::new(gx, gy), CGPoint::new(ox, oy));
        assert_eq!((local.x, local.y), expected);
    }
}

fn wheel(delta_x: f64, delta_y: f64) -> MouseEvent {
    event(
        MouseEventKind::Wheel { delta_x, delta_y },
        MouseButton::Left,
    )
}

#[test]
fn routed_wheel_moves_to_the_target_first_then_posts_line_chunks() {
    let planned = plan(&wheel(12.0, -25.0), true, None).unwrap();

    assert_eq!(
        summary(&planned),
        [
            (Phase::Move, -2850.0, 160.0, 0, 15),
            (Phase::Wheel { dy: -10, dx: 10 }, -2850.0, 160.0, 0, 5),
            (Phase::Wheel { dy: -10, dx: 2 }, -2850.0, 160.0, 0, 5),
            (Phase::Wheel { dy: -5, dx: 0 }, -2850.0, 160.0, 0, 0),
        ]
    );
}

#[test]
fn unrouted_wheel_is_only_the_line_chunks_and_ignores_the_primer() {
    let planned = plan(&wheel(0.0, 3.0), false, Some(CGPoint::new(-3000.0, 100.0))).unwrap();
    assert_eq!(
        summary(&planned),
        [(Phase::Wheel { dy: 3, dx: 0 }, -2850.0, 160.0, 0, 0)]
    );
}

/// CoreGraphics keeps fields 91/92 only on mouse events, so a wheel names its
/// window through the `route` fields (and the window-local location).
#[test]
fn wheel_events_are_line_unit_scrolls_carrying_the_routing_fields() {
    let planned = plan(&wheel(-2.0, 4.0), true, None).unwrap();
    let routed = build(&planned[1], &routing(true)).unwrap();
    let bare = build(&planned[1], &routing(false)).unwrap();

    for event in [&routed, &bare] {
        assert_eq!(event.get_type() as u32, CGEventType::ScrollWheel as u32);
        assert_eq!(field(event, EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_1), 4);
        assert_eq!(
            field(event, EventField::SCROLL_WHEEL_EVENT_DELTA_AXIS_2),
            -2
        );
        assert_eq!(
            field(event, EventField::SCROLL_WHEEL_EVENT_IS_CONTINUOUS),
            0
        );
        assert!(event.get_flags().contains(CGEventFlags::CGEventFlagShift));
        let location = event.location();
        assert_eq!((location.x, location.y), (-2850.0, 160.0));
    }
    assert_eq!(
        field(&routed, EventField::EVENT_TARGET_UNIX_PROCESS_ID),
        i64::from(PID)
    );
    assert_eq!(field(&routed, FIELD_WINDOW_NUMBER), WINDOW_NUMBER);
    assert_eq!(field(&routed, FIELD_CLICK_GROUP), 1);
    assert_eq!(field(&bare, FIELD_WINDOW_NUMBER), 0);
    assert_eq!(field(&bare, FIELD_CLICK_GROUP), 0);
}
