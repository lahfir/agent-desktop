use super::super::geometry;
use super::super::raster::{Surface, SurfaceMapping};
use super::{draw_bubble, draw_highlight};
use agent_desktop_core::{Point, Rect};

const FILL: [f64; 3] = [1.0, 1.0, 1.0];
const RIM: [f64; 3] = [0.07, 0.07, 0.09];
const ACCENT: [f64; 3] = [0.26, 0.60, 1.0];

fn surface() -> Surface {
    Surface::transparent(200, 200)
}

fn mapping(scale: f64) -> SurfaceMapping {
    SurfaceMapping {
        origin: Point { x: 0.0, y: 0.0 },
        scale,
    }
}

/// The highlight is an outline, not a filled box: the element underneath has
/// to stay visible through it.
#[test]
fn the_highlight_is_an_outline_with_its_centre_untouched() {
    let mut surface = surface();
    let target = Rect {
        x: 60.0,
        y: 60.0,
        width: 80.0,
        height: 60.0,
    };

    draw_highlight(&mut surface, &mapping(1.0), &target, 1.0, ACCENT);

    assert_eq!(
        surface.alpha_at(100, 90),
        0,
        "the middle of the element stays clear, or the highlight hides what it points at"
    );
    assert!(
        surface.alpha_at(target.x as i32 - 3, 90) > 0,
        "the outline is painted just outside the element's edge"
    );
}

/// The curve is what keeps the cue from blinking on and off; a static outline
/// would paint identically at every opacity.
#[test]
fn the_highlights_opacity_follows_its_curve() {
    let mut faint = surface();
    let mut full = surface();
    let target = Rect {
        x: 60.0,
        y: 60.0,
        width: 80.0,
        height: 60.0,
    };

    draw_highlight(&mut faint, &mapping(1.0), &target, 0.2, ACCENT);
    draw_highlight(&mut full, &mapping(1.0), &target, 1.0, ACCENT);

    let peak = |surface: &Surface| {
        (0..surface.height)
            .flat_map(|y| (0..surface.width).map(move |x| (x, y)))
            .map(|(x, y)| surface.alpha_at(x, y))
            .max()
            .unwrap_or(0)
    };

    assert!(peak(&faint) < peak(&full));
    assert!(peak(&faint) > 0, "a faint highlight is still drawn");
}

#[test]
fn a_highlight_at_zero_opacity_paints_nothing() {
    let mut surface = surface();

    draw_highlight(
        &mut surface,
        &mapping(1.0),
        &Rect {
            x: 60.0,
            y: 60.0,
            width: 80.0,
            height: 60.0,
        },
        0.0,
        ACCENT,
    );

    assert!(surface.pixels.iter().all(|pixel| *pixel == 0));
}

/// The bubble's corners are why its body is composited rather than
/// alpha-forced across its bounding rectangle: forcing would paint opaque
/// square corners and the bubble would read unlike the reference.
#[test]
fn the_bubbles_corners_are_rounded_rather_than_squared_off() {
    let mut surface = surface();
    let rect = Rect {
        x: 10.0,
        y: 10.0,
        width: geometry::BUBBLE_WIDTH.min(180.0),
        height: geometry::BUBBLE_HEIGHT,
    };

    draw_bubble(&mut surface, &rect, FILL, RIM);

    assert_eq!(
        surface.alpha_at(rect.x as i32, rect.y as i32),
        0,
        "the very corner of the bounding rectangle is outside a rounded bubble"
    );
    assert_eq!(
        surface.alpha_at(
            (rect.x + rect.width / 2.0) as i32,
            (rect.y + rect.height / 2.0) as i32
        ),
        255,
        "the bubble's body is opaque, which is what lets GDI draw text onto it"
    );
}

#[path = "rounded_equivalence_tests.rs"]
mod equivalence;
