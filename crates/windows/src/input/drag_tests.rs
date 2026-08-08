use super::*;
use crate::input::mouse_send::mouse_send_fake_sink as sink;
use crate::input::mouse_send::{MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE};
use agent_desktop_core::{Deadline, Point};

fn base_params() -> DragParams {
    DragParams {
        from: Point { x: 0.0, y: 0.0 },
        to: Point { x: 10.0, y: 10.0 },
        duration_ms: None,
        drop_delay_ms: None,
    }
}

#[test]
fn drag_limits_reject_unbounded_work() {
    let too_long = DragParams {
        duration_ms: Some(MAX_DRAG_MS + 1),
        ..base_params()
    };
    assert!(validate_drag(&too_long).is_err());

    let too_slow_to_release = DragParams {
        duration_ms: None,
        drop_delay_ms: Some(MAX_DROP_DELAY_MS + 1),
        ..base_params()
    };
    assert!(validate_drag(&too_slow_to_release).is_err());
}

#[test]
fn impossible_drag_deadline_fails_before_mouse_down() {
    sink::reset();
    let params = DragParams {
        duration_ms: Some(1),
        drop_delay_ms: Some(0),
        ..base_params()
    };

    let error = synthesize_drag(params, Deadline::after(1).expect("bounded deadline"))
        .expect_err("deadline too tight to complete the drag");

    assert_eq!(error.code, ErrorCode::Timeout);
    let details = error.details.expect("timeout carries preflight details");
    assert_eq!(details["physical_delivery_started"], false);
    assert!(
        sink::recorded().is_empty(),
        "an impossible deadline must not post mouse-down"
    );
}

#[test]
fn zero_drop_delay_has_no_forced_dwell_ticks() {
    assert_eq!(0_u64.div_ceil(DWELL_TICK_MS), 0);
}

#[test]
fn sub_tick_drag_uses_one_nonzero_duration_step() {
    let duration_ms = 1_u64;
    let steps = duration_ms.div_ceil(DWELL_TICK_MS).clamp(1, MAX_STEPS);
    let step_delay = Duration::from_secs_f64(duration_ms as f64 / steps as f64 / 1_000.0);

    assert_eq!(steps, 1);
    assert_eq!(step_delay, Duration::from_millis(1));
}

#[test]
fn a_successful_drag_posts_down_move_and_up_with_no_corrective_release() {
    sink::reset();
    let params = DragParams {
        duration_ms: Some(0),
        drop_delay_ms: Some(0),
        ..base_params()
    };

    synthesize_drag(params, Deadline::after(5_000).expect("bounded deadline"))
        .expect("a well-formed drag with an ample deadline succeeds");

    let recorded = sink::recorded();
    assert_eq!(
        recorded.get(1).map(|event| event.flags),
        Some(MOUSEEVENTF_LEFTDOWN),
        "the move to the origin is immediately followed by the button down"
    );
    assert!(
        recorded
            .iter()
            .any(|event| event.flags & MOUSEEVENTF_MOVE == MOUSEEVENTF_MOVE),
        "at least one move event steps toward the destination"
    );
    assert_eq!(
        recorded.last().map(|event| event.flags),
        Some(MOUSEEVENTF_LEFTUP),
        "the sequence ends with the release and nothing after it"
    );
    let down_count = recorded
        .iter()
        .filter(|event| event.flags == MOUSEEVENTF_LEFTDOWN)
        .count();
    let up_count = recorded
        .iter()
        .filter(|event| event.flags == MOUSEEVENTF_LEFTUP)
        .count();
    assert_eq!(down_count, 1, "no corrective mouse-down is ever posted");
    assert_eq!(
        up_count, 1,
        "a completed drag disarms the guard, so Drop posts no second release"
    );
}
