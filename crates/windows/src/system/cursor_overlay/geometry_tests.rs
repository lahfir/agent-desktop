use super::{
    BUBBLE_CORNER_RADIUS, BUBBLE_HEIGHT, BUBBLE_WIDTH, DART, GLYPH_HEIGHT, GLYPH_WIDTH,
    RIPPLE_SIZE, bubble_size, follower_rect, glyph_rect, highlight_rect, ripple_rect,
};
use agent_desktop_core::{Point, Rect};

fn tip() -> Point {
    Point { x: 400.0, y: 300.0 }
}

/// The dimensions are the macOS renderer's, carried across rather than
/// invented. Without them R16's parity claim has nothing to fail against.
#[test]
fn the_ported_dimensions_match_the_reference() {
    assert_eq!((GLYPH_WIDTH, GLYPH_HEIGHT), (32.0, 40.0));
    assert_eq!(RIPPLE_SIZE, 108.0);
    assert_eq!((BUBBLE_WIDTH, BUBBLE_HEIGHT), (232.0, 38.0));
    assert_eq!(BUBBLE_CORNER_RADIUS, 10.0);
}

/// The pose names where the cursor points, not where its box starts, so the
/// dart's tip lands on the destination.
#[test]
fn the_glyphs_tip_sits_on_the_pose_point() {
    let rect = glyph_rect(&tip(), 1.0);

    assert_eq!(rect.x + DART[0].0, tip().x);
    assert_eq!(rect.y + DART[0].1, tip().y);
    assert_eq!((rect.width, rect.height), (GLYPH_WIDTH, GLYPH_HEIGHT));
}

#[test]
fn the_ripple_is_centred_on_the_pose_point() {
    let rect = ripple_rect(&tip(), 1.0);

    assert_eq!(rect.x + rect.width / 2.0, tip().x);
    assert_eq!(rect.y + rect.height / 2.0, tip().y);
}

/// A caller that asks for a larger cursor gets a larger everything, rather
/// than a large glyph beside a reference-sized ripple.
#[test]
fn every_dimension_scales_with_the_sessions_style_size() {
    let doubled = glyph_rect(&tip(), 2.0);
    let ripple = ripple_rect(&tip(), 2.0);

    assert_eq!((doubled.width, doubled.height), (64.0, 80.0));
    assert_eq!(ripple.width, RIPPLE_SIZE * 2.0);
    assert_eq!(bubble_size(2.0), (BUBBLE_WIDTH * 2.0, BUBBLE_HEIGHT * 2.0));
    assert_eq!(doubled.x + DART[0].0 * 2.0, tip().x, "the tip stays put");
}

#[test]
fn the_highlight_sits_outside_the_element_rather_than_over_its_edge() {
    let target = Rect {
        x: 100.0,
        y: 100.0,
        width: 200.0,
        height: 50.0,
    };

    let rect = highlight_rect(&target, 1.0);

    assert!(rect.x < target.x && rect.y < target.y);
    assert!(rect.x + rect.width > target.x + target.width);
    assert!(rect.y + rect.height > target.y + target.height);
}

/// The follower surface exists so the renderer does not paint the whole
/// virtual screen. It still has to contain the ripple at full extent, or the
/// effect is clipped at the window's own edge.
#[test]
fn the_follower_surface_contains_the_ripple_at_full_extent() {
    let surface = follower_rect(&tip(), 1.0, None, None);
    let ripple = ripple_rect(&tip(), 1.0);

    assert!(surface.x <= ripple.x);
    assert!(surface.y <= ripple.y);
    assert!(surface.x + surface.width >= ripple.x + ripple.width);
    assert!(surface.y + surface.height >= ripple.y + ripple.height);
}

#[test]
fn the_follower_surface_contains_the_glyph_the_label_and_the_highlight() {
    let label = Rect {
        x: 900.0,
        y: 700.0,
        width: BUBBLE_WIDTH,
        height: BUBBLE_HEIGHT,
    };
    let highlight = Rect {
        x: 20.0,
        y: 30.0,
        width: 60.0,
        height: 40.0,
    };

    let surface = follower_rect(&tip(), 1.0, Some(&label), Some(&highlight));

    for contained in [glyph_rect(&tip(), 1.0), label, highlight] {
        assert!(
            surface.x <= contained.x,
            "surface starts left of the content"
        );
        assert!(surface.y <= contained.y, "surface starts above the content");
        assert!(surface.x + surface.width >= contained.x + contained.width);
        assert!(surface.y + surface.height >= contained.y + contained.height);
    }
}

/// A surface spanning the whole virtual screen costs a frame budget rather
/// than a rounding error, which is why the window follows the pose. The
/// follower has to stay small to be worth having.
#[test]
fn the_follower_surface_stays_far_smaller_than_a_screen() {
    let surface = follower_rect(&tip(), 1.0, None, None);

    assert!(
        surface.width <= RIPPLE_SIZE * 1.5 && surface.height <= RIPPLE_SIZE * 1.5,
        "a {}x{} surface is no longer a follower",
        surface.width,
        surface.height
    );
}
