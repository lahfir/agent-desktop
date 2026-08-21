use super::*;
use crate::{Point, Rect};

#[test]
fn motion_reaches_both_endpoints_and_curves_between_them() {
    let motion = CursorMotion::new(Point { x: 20.0, y: 40.0 }, Point { x: 420.0, y: 240.0 });

    assert_eq!(motion.sample(0), Point { x: 20.0, y: 40.0 });
    assert_eq!(
        motion.sample(motion.duration_ms()),
        Point { x: 420.0, y: 240.0 }
    );
    let midpoint = motion.sample(motion.duration_ms() / 2);
    assert!((midpoint.x - 220.0).hypot(midpoint.y - 140.0) > 30.0);
}

#[test]
fn motion_eases_in_and_out() {
    let motion = CursorMotion::new(Point { x: 0.0, y: 0.0 }, Point { x: 1_000.0, y: 0.0 });

    assert!(motion.sample(motion.duration_ms() / 4).x < 150.0);
    assert!(motion.sample(motion.duration_ms() * 3 / 4).x > 850.0);
}

#[test]
fn motion_duration_is_distance_aware_and_bounded() {
    let short = CursorMotion::new(Point { x: 0.0, y: 0.0 }, Point { x: 10.0, y: 0.0 });
    let long = CursorMotion::new(Point { x: 0.0, y: 0.0 }, Point { x: 4_000.0, y: 0.0 });

    assert_eq!(short.duration_ms(), 420);
    assert_eq!(long.duration_ms(), 720);
}

#[test]
fn label_limit_handles_unicode_words_and_ellipsis() {
    let config = CursorOverlayConfig::enabled(
        Some("Opening the profile menu for this account now".into()),
        5,
    )
    .expect("valid config");

    assert_eq!(config.label(), Some("Opening the profile menu for…"));
}

#[test]
fn label_has_a_bounded_transport_size() {
    let error = CursorOverlayConfig::enabled(Some("x".repeat(513)), MAX_CURSOR_LABEL_WORDS)
        .expect_err("oversized labels must be rejected when configured");

    assert_eq!(error.code, crate::ErrorCode::InvalidArgs);
}

#[test]
fn label_placement_stays_on_screen_and_clear_of_destination() {
    let screen = Rect {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
    };
    let bubble = place_label(&Point { x: 790.0, y: 590.0 }, (232.0, 38.0), &screen);

    assert!(bubble.x >= screen.x);
    assert!(bubble.y >= screen.y);
    assert!(bubble.x + bubble.width <= screen.x + screen.width);
    assert!(bubble.y + bubble.height <= screen.y + screen.height);
    assert!(bubble.x + bubble.width <= 782.0 || bubble.y + bubble.height <= 582.0);
}

#[test]
fn instruction_rejects_invalid_destination() {
    let config = CursorOverlayConfig::enabled(None, 6).expect("valid config");
    let error = CursorOverlayInstruction::new(
        Point {
            x: f64::NAN,
            y: 10.0,
        },
        &config,
        true,
    )
    .expect_err("invalid point");

    assert_eq!(error.code, crate::ErrorCode::InvalidArgs);
}
