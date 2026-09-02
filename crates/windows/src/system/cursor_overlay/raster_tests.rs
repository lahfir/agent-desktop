use super::super::geometry;
use super::{Surface, draw_bubble, draw_glyph, draw_highlight, draw_ripple};
use agent_desktop_core::{Point, Rect};

const FILL: [f64; 3] = [1.0, 1.0, 1.0];
const RIM: [f64; 3] = [0.07, 0.07, 0.09];
const ACCENT: [f64; 3] = [0.26, 0.60, 1.0];

fn surface() -> Surface {
    Surface::transparent(200, 200)
}

fn origin() -> Point {
    Point { x: 0.0, y: 0.0 }
}

fn edge_alphas(surface: &Surface) -> Vec<u8> {
    let mut seen = Vec::new();
    for y in 0..surface.height {
        for x in 0..surface.width {
            let alpha = surface.alpha_at(x, y);
            if alpha > 0 && alpha < 255 {
                seen.push(alpha);
            }
        }
    }
    seen
}

#[test]
fn an_untouched_surface_is_fully_transparent() {
    let surface = surface();

    assert!(surface.pixels.iter().all(|pixel| *pixel == 0));
    assert_eq!(surface.alpha_at(10, 10), 0);
}

/// The whole reason this module exists. A GDI `Polygon` would leave the alpha
/// byte at zero across the glyph, so nothing would appear under `ULW_ALPHA`;
/// forcing alpha across its rectangle afterwards would square off the edges.
/// Intermediate alpha at the edge is the proof the per-pixel path ran.
#[test]
fn the_glyph_carries_full_alpha_inside_and_partial_alpha_at_its_edge() {
    let mut surface = surface();

    draw_glyph(
        &mut surface,
        &origin(),
        &Point { x: 40.0, y: 40.0 },
        1.0,
        FILL,
        RIM,
    );

    assert!(
        surface.pixels.iter().any(|pixel| (pixel >> 24) == 0xFF),
        "the glyph's interior must be opaque, or nothing is visible"
    );
    assert!(
        !edge_alphas(&surface).is_empty(),
        "an anti-aliased edge carries partial alpha; a GDI draw would have left every pixel \
         at zero, and a rectangle alpha-force would have left every pixel at 255"
    );
    assert!(
        surface.pixels.contains(&0),
        "the surface outside the glyph stays transparent"
    );
}

#[test]
fn the_glyphs_tip_is_painted_where_the_pose_points() {
    let mut surface = surface();
    let tip = Point { x: 60.0, y: 60.0 };

    draw_glyph(&mut surface, &origin(), &tip, 1.0, FILL, RIM);

    let near_tip = surface.alpha_at(tip.x as i32, tip.y as i32);
    assert!(
        near_tip > 0,
        "the dart's tip lands on the pose point, not the corner of its box"
    );
}

/// A ripple that never fades would be a solid disc rather than an effect.
#[test]
fn the_ripple_expands_and_fades_across_its_phase() {
    let tip = Point { x: 100.0, y: 100.0 };
    let mut early = surface();
    let mut late = surface();

    draw_ripple(&mut early, &origin(), &tip, 1.0, 0.15, ACCENT);
    draw_ripple(&mut late, &origin(), &tip, 1.0, 0.9, ACCENT);

    let radius_of = |surface: &Surface| {
        let mut furthest = 0.0_f64;
        for y in 0..surface.height {
            for x in 0..surface.width {
                if surface.alpha_at(x, y) > 0 {
                    furthest = furthest.max((f64::from(x) - 100.0).hypot(f64::from(y) - 100.0));
                }
            }
        }
        furthest
    };

    assert!(
        radius_of(&late) > radius_of(&early),
        "the ring expands as the phase advances"
    );
    assert!(
        !edge_alphas(&early).is_empty(),
        "the ripple's edge is soft, which a GDI Ellipse could not produce here"
    );
}

#[test]
fn a_ripple_outside_its_phase_paints_nothing() {
    let mut surface = surface();

    draw_ripple(
        &mut surface,
        &origin(),
        &Point { x: 100.0, y: 100.0 },
        1.0,
        0.0,
        ACCENT,
    );

    assert!(surface.pixels.iter().all(|pixel| *pixel == 0));
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

    draw_highlight(&mut surface, &origin(), &target, 1.0, 1.0, ACCENT);

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

    draw_highlight(&mut faint, &origin(), &target, 1.0, 0.2, ACCENT);
    draw_highlight(&mut full, &origin(), &target, 1.0, 1.0, ACCENT);

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
        &origin(),
        &Rect {
            x: 60.0,
            y: 60.0,
            width: 80.0,
            height: 60.0,
        },
        1.0,
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

#[test]
fn nothing_is_written_outside_the_surface() {
    let mut surface = Surface::transparent(24, 24);

    draw_glyph(
        &mut surface,
        &origin(),
        &Point { x: 500.0, y: 500.0 },
        4.0,
        FILL,
        RIM,
    );
    draw_ripple(
        &mut surface,
        &origin(),
        &Point {
            x: -400.0,
            y: -400.0,
        },
        4.0,
        0.5,
        ACCENT,
    );

    assert_eq!(surface.pixels.len(), 24 * 24);
}
