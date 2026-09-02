use super::{clamp_refresh, frame_interval, has_arrived, highlight_progress};
use std::time::Duration;

/// A29-7 measured the obvious refresh call failing on this host and leaving
/// its frequency at zero. A renderer that divided by that reading would take
/// an infinite frame rate; the floor is why it cannot.
#[test]
fn an_absent_refresh_reading_falls_back_rather_than_producing_no_interval() {
    assert_eq!(clamp_refresh(0), 60);
    assert_eq!(clamp_refresh(1), 60);
    assert_eq!(frame_interval(0), Duration::from_nanos(1_000_000_000 / 60));
}

#[test]
fn an_absurd_refresh_reading_is_capped_rather_than_believed() {
    assert_eq!(clamp_refresh(1_000), 240);
    assert!(frame_interval(1_000) >= Duration::from_nanos(1_000_000_000 / 240));
}

/// The reading this host actually gives, so the clamp is not silently
/// rewriting every real value.
#[test]
fn an_ordinary_refresh_reading_passes_through_unchanged() {
    assert_eq!(clamp_refresh(64), 64);
    assert_eq!(clamp_refresh(60), 60);
    assert_eq!(clamp_refresh(144), 144);
}

#[test]
fn arrival_is_answered_by_elapsed_time_not_by_a_frame_count() {
    assert!(!has_arrived(0, 200));
    assert!(!has_arrived(199, 200));
    assert!(has_arrived(200, 200));
    assert!(has_arrived(5_000, 200));
}

/// Core returns a zero duration when the cursor is already where it is going,
/// so a click on the current position must not wait for a travel.
#[test]
fn a_motion_with_nowhere_to_go_arrives_immediately() {
    assert!(has_arrived(0, 0));
}

/// A static outline would hold full opacity throughout. The curve is what keeps
/// Windows highlight from reading as a cruder cue than the macOS one it is
/// measured against.
#[test]
fn the_highlight_rises_holds_and_falls_rather_than_blinking() {
    let hold = 900;

    assert_eq!(highlight_progress(0, hold), 0.0);
    assert!(highlight_progress(30, hold) > 0.0 && highlight_progress(30, hold) < 1.0);
    assert_eq!(highlight_progress(400, hold), 1.0);
    assert!(highlight_progress(800, hold) > 0.0 && highlight_progress(800, hold) < 1.0);
    assert_eq!(highlight_progress(hold, hold), 0.0);
    assert_eq!(highlight_progress(hold + 1, hold), 0.0);
}

#[test]
fn the_highlight_never_leaves_the_unit_range() {
    for elapsed in (0..1_200).step_by(7) {
        let value = highlight_progress(elapsed, 900);
        assert!(
            (0.0..=1.0).contains(&value),
            "progress {value} at {elapsed}ms is outside the range a paint can use"
        );
    }
}

#[test]
fn a_zero_hold_never_shows_the_highlight() {
    assert_eq!(highlight_progress(0, 0), 0.0);
}
