//! The overlay's shape, in the numbers the macOS renderer already uses.
//!
//! Pure. R16 claims Windows draws the same visual vocabulary as macOS, and a
//! claim like that needs numbers to fail against — core shares the motion,
//! the style and the label placement, but none of these dimensions. They are
//! carried across rather than invented, and scaled by the session's style
//! size so a caller that asks for a larger cursor gets a larger everything.

use agent_desktop_core::{Point, Rect};

/// The cursor glyph's box, and the four-point dart drawn inside it, with the
/// tip at the first point — which is what the pose's position names.
///
/// The dart is the macOS path **flipped vertically**, not copied. Core
/// Graphics authors that path in a y-up space where the tip's `35.0` sits
/// near the top of a 40-unit box; this surface is a top-down DIB where the
/// same number is near the bottom, so carrying the coordinates across
/// unchanged draws the pointer upside down. Flipping once here is what keeps
/// every consumer — the glyph rectangle and the rasterizer both — in one
/// space.
pub(crate) const GLYPH_WIDTH: f64 = 32.0;
pub(crate) const GLYPH_HEIGHT: f64 = 40.0;
pub(crate) const GLYPH_RIM_WIDTH: f64 = 6.5;
pub(crate) const DART: [(f64, f64); 4] = [(1.0, 5.0), (29.6, 22.5), (12.7, 23.7), (4.2, 38.4)];

/// The ripple's full extent, and the solid disc at its centre.
pub(crate) const RIPPLE_SIZE: f64 = 108.0;
pub(crate) const RIPPLE_CORE_RADIUS: f64 = 19.0;
pub(crate) const RIPPLE_RING_INSET: f64 = 4.0;

/// The label bubble. Its corner radius and border are why the bubble body is
/// composited per-pixel rather than alpha-forced across its bounding
/// rectangle: forcing a rectangle would paint opaque square corners.
pub(crate) const BUBBLE_WIDTH: f64 = 232.0;
pub(crate) const BUBBLE_HEIGHT: f64 = 38.0;
pub(crate) const BUBBLE_CORNER_RADIUS: f64 = 10.0;
pub(crate) const BUBBLE_BORDER_WIDTH: f64 = 1.5;
pub(crate) const BUBBLE_FONT_POINTS: f64 = 12.5;
pub(crate) const BUBBLE_TEXT_INSET: f64 = 10.0;

/// The highlight drawn around the element being acted on.
pub(crate) const HIGHLIGHT_BORDER_WIDTH: f64 = 2.5;
pub(crate) const HIGHLIGHT_CORNER_RADIUS: f64 = 8.0;
pub(crate) const HIGHLIGHT_PADDING: f64 = 5.0;

/// The glyph's rectangle for a pose, with the dart's tip at the pose point.
pub(crate) fn glyph_rect(tip: &Point, scale: f64) -> Rect {
    let (tip_x, tip_y) = DART[0];
    Rect {
        x: tip.x - tip_x * scale,
        y: tip.y - tip_y * scale,
        width: GLYPH_WIDTH * scale,
        height: GLYPH_HEIGHT * scale,
    }
}

/// The ripple's rectangle for a pose, centred on the pose point.
pub(crate) fn ripple_rect(tip: &Point, scale: f64) -> Rect {
    let size = RIPPLE_SIZE * scale;
    Rect {
        x: tip.x - size / 2.0,
        y: tip.y - size / 2.0,
        width: size,
        height: size,
    }
}

pub(crate) fn bubble_size(scale: f64) -> (f64, f64) {
    (BUBBLE_WIDTH * scale, BUBBLE_HEIGHT * scale)
}

/// The highlight's rectangle: the target's bounds, padded, so the outline
/// sits outside the element rather than over its edge.
pub(crate) fn highlight_rect(target: &Rect, scale: f64) -> Rect {
    let padding = HIGHLIGHT_PADDING * scale;
    Rect {
        x: target.x - padding,
        y: target.y - padding,
        width: target.width + padding * 2.0,
        height: target.height + padding * 2.0,
    }
}

/// The one surface the renderer paints, sized to contain everything this
/// frame draws.
///
/// A surface that spanned the virtual screen would cost a frame budget rather
/// than a rounding error on a large desktop — measured near-linear in pixel
/// count — so the window follows the pose instead, and has to be big enough
/// for the ripple at full extent or the effect would be clipped at its edges.
pub(crate) fn follower_rect(
    tip: &Point,
    scale: f64,
    label: Option<&Rect>,
    highlight: Option<&Rect>,
) -> Rect {
    let mut union = union_of(&glyph_rect(tip, scale), &ripple_rect(tip, scale));
    if let Some(label) = label {
        union = union_of(&union, label);
    }
    if let Some(highlight) = highlight {
        union = union_of(&union, highlight);
    }
    union
}

fn union_of(left: &Rect, right: &Rect) -> Rect {
    let x = left.x.min(right.x);
    let y = left.y.min(right.y);
    let right_edge = (left.x + left.width).max(right.x + right.width);
    let bottom_edge = (left.y + left.height).max(right.y + right.height);
    Rect {
        x,
        y,
        width: right_edge - x,
        height: bottom_edge - y,
    }
}

#[cfg(test)]
#[path = "geometry_tests.rs"]
mod tests;
