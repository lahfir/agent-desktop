use super::{Frame, compose};
use agent_desktop_core::{CursorOverlayStyle, Point, Rect};

fn screen() -> Rect {
    Rect {
        x: 0.0,
        y: 0.0,
        width: 1608.0,
        height: 780.0,
    }
}

fn frame<'a>(style: &'a CursorOverlayStyle, label: Option<&'a str>) -> Frame<'a> {
    Frame {
        tip: Point { x: 400.0, y: 300.0 },
        style,
        ripple_phase: 0.0,
        target: None,
        highlight_opacity: 0.0,
        label,
        screen: screen(),
    }
}

fn painted_pixels(composed: &super::Composed) -> usize {
    composed
        .surface
        .pixels
        .iter()
        .filter(|pixel| (*pixel >> 24) > 0)
        .count()
}

#[test]
fn a_bare_frame_draws_the_cursor_and_nothing_else() {
    let style = CursorOverlayStyle::default();

    let composed = compose(&frame(&style, None));

    assert!(painted_pixels(&composed) > 0, "the cursor is drawn");
    assert!(composed.text_rect.is_none(), "no label, no text rectangle");
}

/// The style's effect switches are the operator's, so turning them off has to
/// actually remove the pixels rather than merely skip a branch.
#[test]
fn disabling_the_ripple_removes_it_while_the_cursor_stays() {
    let mut off = CursorOverlayStyle::default();
    off.set_effects(false, true);
    let on = CursorOverlayStyle::default();

    let mut with_ripple = frame(&on, None);
    with_ripple.ripple_phase = 0.5;
    let mut without = frame(&off, None);
    without.ripple_phase = 0.5;

    let drawn = compose(&with_ripple);
    let suppressed = compose(&without);

    assert!(painted_pixels(&drawn) > painted_pixels(&suppressed));
    assert!(
        painted_pixels(&suppressed) > 0,
        "the cursor itself is not a ripple and must survive"
    );
}

#[test]
fn disabling_the_highlight_removes_it() {
    let mut off = CursorOverlayStyle::default();
    off.set_effects(true, false);
    let on = CursorOverlayStyle::default();
    let target = Rect {
        x: 380.0,
        y: 280.0,
        width: 120.0,
        height: 40.0,
    };

    let mut shown = frame(&on, None);
    shown.target = Some(target);
    shown.highlight_opacity = 1.0;
    let mut hidden = frame(&off, None);
    hidden.target = Some(target);
    hidden.highlight_opacity = 1.0;

    assert!(painted_pixels(&compose(&shown)) > painted_pixels(&compose(&hidden)));
}

/// The label's rectangle is where GDI is allowed to write, and it has to sit
/// inside the surface or the text lands outside the window entirely.
#[test]
fn a_label_yields_a_text_rectangle_inside_the_surface() {
    let style = CursorOverlayStyle::default();

    let composed = compose(&frame(&style, Some("open the file")));

    let text = composed.text_rect.expect("a label yields a text rectangle");
    assert!(text.x >= 0.0 && text.y >= 0.0);
    assert!(text.x + text.width <= f64::from(composed.surface.width));
    assert!(text.y + text.height <= f64::from(composed.surface.height));
    assert!(text.width > 0.0 && text.height > 0.0);
}

/// The bubble body must be opaque under the text, because that is what makes
/// forcing the text rectangle's alpha correct rather than a workaround.
#[test]
fn the_bubble_body_is_opaque_beneath_where_the_text_will_go() {
    let style = CursorOverlayStyle::default();

    let composed = compose(&frame(&style, Some("open the file")));
    let text = composed.text_rect.expect("a text rectangle");

    let alpha = composed.surface.alpha_at(
        (text.x + text.width / 2.0) as i32,
        (text.y + text.height / 2.0) as i32,
    );
    assert_eq!(
        alpha, 255,
        "GDI writes RGB without alpha, so it may only draw where the body is already opaque"
    );
}

/// A destination near the screen edge must not push the bubble off it: core
/// places the label, and the surface has to follow that placement.
#[test]
fn a_label_near_the_screen_edge_stays_on_screen() {
    let style = CursorOverlayStyle::default();
    let mut edge = frame(&style, Some("open the file"));
    edge.tip = Point {
        x: 1600.0,
        y: 770.0,
    };

    let composed = compose(&edge);
    let text = composed.text_rect.expect("a text rectangle");

    let absolute_right = composed.origin.x + text.x + text.width;
    let absolute_bottom = composed.origin.y + text.y + text.height;
    assert!(
        absolute_right <= screen().width + 1.0,
        "{absolute_right} runs off the right"
    );
    assert!(
        absolute_bottom <= screen().height + 1.0,
        "{absolute_bottom} runs off the bottom"
    );
}

/// The surface follows the pose rather than spanning the screen; painting a
/// virtual screen every frame is a frame budget rather than a rounding error.
#[test]
fn the_surface_follows_the_pose_rather_than_spanning_the_screen() {
    let style = CursorOverlayStyle::default();

    let composed = compose(&frame(&style, None));

    assert!(
        f64::from(composed.surface.width) < screen().width / 4.0,
        "a {}px surface is no longer a follower",
        composed.surface.width
    );
}

#[test]
fn a_larger_style_size_yields_a_larger_surface() {
    let small = CursorOverlayStyle::default();
    let mut large = CursorOverlayStyle::default();
    large.set_size(3.0);

    let small_surface = compose(&frame(&small, None)).surface.width;
    let large_surface = compose(&frame(&large, None)).surface.width;

    assert!(large_surface > small_surface);
}
