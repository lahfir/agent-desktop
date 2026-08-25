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
fn motion_is_ballistic_then_corrective_like_a_hand() {
    let motion = CursorMotion::new(Point { x: 0.0, y: 0.0 }, Point { x: 1_000.0, y: 0.0 });

    let early = motion.sample(motion.duration_ms() / 2).x;
    let overshoot = motion.sample(motion.duration_ms() * 3 / 4).x;

    assert!(
        early > 700.0,
        "the hand covers most of the gap early: {early}"
    );
    assert!(
        overshoot > 1_000.0,
        "the hand overshoots before it corrects: {overshoot}"
    );
    assert_eq!(
        motion.sample(motion.duration_ms()),
        Point { x: 1_000.0, y: 0.0 }
    );
}

#[test]
fn motion_duration_follows_fitts_law_and_stays_bounded() {
    let short = CursorMotion::new(Point { x: 0.0, y: 0.0 }, Point { x: 10.0, y: 0.0 });
    let medium = CursorMotion::new(Point { x: 0.0, y: 0.0 }, Point { x: 600.0, y: 0.0 });
    let long = CursorMotion::new(Point { x: 0.0, y: 0.0 }, Point { x: 4_000.0, y: 0.0 });

    assert_eq!(short.duration_ms(), 260);
    assert!((600..760).contains(&medium.duration_ms()));
    assert_eq!(long.duration_ms(), 950);
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

#[test]
fn control_protocol_carries_the_session_lifecycle() {
    let enable = CursorOverlayControl::enable("run-1".into(), CursorOverlayStyle::default());
    let disable = CursorOverlayControl::disable("run-1".into());

    assert_eq!(enable.session_id(), "run-1");
    assert_eq!(enable.label(), Some("Hey, let's play with this computer!"));
    assert!(enable.is_enable());
    assert_eq!(disable.session_id(), "run-1");
    assert!(disable.is_disable());
}

#[test]
fn travel_stays_flat_so_only_the_click_is_a_flourish() {
    let motion =
        CursorMotion::new(Point { x: 0.0, y: 0.0 }, Point { x: 600.0, y: 0.0 }).with_impact(true);

    let cruise = motion.pose(motion.duration_ms() / 2);

    assert_eq!(cruise.scale, 1.0);
    assert_eq!(cruise.ripple, 0.0);
}

#[test]
fn the_click_loops_playfully_then_presses_and_ripples() {
    let motion =
        CursorMotion::new(Point { x: 0.0, y: 0.0 }, Point { x: 600.0, y: 0.0 }).with_impact(true);
    let flourish = motion.total_ms() - motion.duration_ms();

    let wandering = motion.pose(motion.duration_ms() + flourish * 22 / 100);
    let pressed = motion.pose(motion.duration_ms() + flourish * 58 / 100);
    let settled = motion.pose(motion.total_ms());

    let drift = (wandering.point.x - 600.0).hypot(wandering.point.y);
    assert!(drift > 4.0, "the cursor loops around the target: {drift}");
    assert!(wandering.scale > 1.0, "it grows while it loops");
    assert!(
        pressed.scale < 0.85,
        "it presses into the element: {}",
        pressed.scale
    );
    assert!(motion.pose(motion.total_ms() - 40).ripple > 0.0);
    assert!((settled.scale - 1.0).abs() < 1e-9);
    assert_eq!(settled.point, Point { x: 600.0, y: 0.0 });
}

#[test]
fn a_move_without_a_click_never_flourishes() {
    let motion = CursorMotion::new(Point { x: 0.0, y: 0.0 }, Point { x: 600.0, y: 0.0 });

    let settled = motion.pose(motion.total_ms());

    assert_eq!(motion.total_ms(), motion.duration_ms());
    assert_eq!(settled, CursorPose::still(Point { x: 600.0, y: 0.0 }));
}

#[test]
fn a_clicked_target_rect_reaches_the_renderer() {
    let config = CursorOverlayConfig::enabled(None, 6).expect("valid config");
    let bounds = Rect {
        x: 10.0,
        y: 20.0,
        width: 80.0,
        height: 24.0,
    };
    let point = Point { x: 50.0, y: 32.0 };
    let instruction = CursorOverlayInstruction::new(point.clone(), &config, true)
        .expect("valid instruction")
        .with_target(Some(bounds));
    let degenerate = CursorOverlayInstruction::new(point, &config, true)
        .expect("valid instruction")
        .with_target(Some(Rect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 5.0,
        }));

    assert_eq!(instruction.target(), Some(&bounds));
    assert_eq!(degenerate.target(), None);
}
