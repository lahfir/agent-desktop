//! The overlay's pixels, composited by hand into a premultiplied buffer.
//!
//! Every GDI raster primitive — `Ellipse`, `Polygon`, `FillRect`, `DrawTextW`
//! alike — writes RGB into a 32bpp DIB and leaves the alpha byte at zero, so
//! anything drawn through GDI is invisible under `ULW_ALPHA`. Forcing alpha
//! across a rectangle afterwards rescues an opaque rectangle and nothing
//! else: an anti-aliased edge and a soft ring need per-pixel coverage, which
//! is the very thing being destroyed.
//!
//! So the glyph, the ripple and the highlight are composited here instead,
//! with coverage computed rather than approximated. GDI draws only the label
//! text, onto a bubble body this module has already made opaque - and even
//! there the alpha byte is written directly as the glyphs are copied back,
//! rather than forced across a rectangle afterwards, so the bubble's rounded
//! corners are never in reach of it.
//!
//! Being pure is the other half of the point: the alpha behaviour that makes
//! this necessary is assertable without a window.

use agent_desktop_core::{Point, Rect};

use super::geometry;

/// A premultiplied 32bpp BGRA surface, top-down, ready for
/// `UpdateLayeredWindow`.
pub(crate) struct Surface {
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) pixels: Vec<u32>,
}

#[derive(Clone, Copy)]
pub(crate) struct Paint {
    pub(crate) rgb: [f64; 3],
    pub(crate) alpha: f64,
}

impl Surface {
    pub(crate) fn transparent(width: i32, height: i32) -> Self {
        let count = (width.max(0) as usize) * (height.max(0) as usize);
        Self {
            width,
            height,
            pixels: vec![0; count],
        }
    }

    #[cfg(test)]
    pub(crate) fn alpha_at(&self, x: i32, y: i32) -> u8 {
        self.pixel_at(x, y).map_or(0, |value| (value >> 24) as u8)
    }

    pub(crate) fn pixel_at(&self, x: i32, y: i32) -> Option<u32> {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return None;
        }
        self.pixels
            .get((y as usize) * (self.width as usize) + (x as usize))
            .copied()
    }

    /// Composites one sample over what is already there, premultiplied.
    fn blend(&mut self, x: i32, y: i32, paint: &Paint, coverage: f64) {
        if coverage <= 0.0 || x < 0 || y < 0 || x >= self.width || y >= self.height {
            return;
        }
        let index = (y as usize) * (self.width as usize) + (x as usize);
        let Some(existing) = self.pixels.get(index).copied() else {
            return;
        };
        let source_alpha = (paint.alpha * coverage).clamp(0.0, 1.0);
        if source_alpha <= 0.0 {
            return;
        }
        let existing_alpha = f64::from((existing >> 24) as u8) / 255.0;
        let out_alpha = source_alpha + existing_alpha * (1.0 - source_alpha);
        let channel = |shift: u32, source: f64| {
            let under = f64::from(((existing >> shift) & 0xFF) as u8) / 255.0;
            let over = source * source_alpha + under * (1.0 - source_alpha);
            (over.clamp(0.0, 1.0) * 255.0).round() as u32
        };
        let blue = channel(0, paint.rgb[2]);
        let green = channel(8, paint.rgb[1]);
        let red = channel(16, paint.rgb[0]);
        let alpha = (out_alpha.clamp(0.0, 1.0) * 255.0).round() as u32;
        self.pixels[index] = (alpha << 24) | (red << 16) | (green << 8) | blue;
    }
}

/// How many samples per axis each pixel is tested with. Three is enough for
/// an edge that reads smooth at these sizes and cheap enough to stay well
/// inside a frame.
const SAMPLES: i32 = 3;

fn coverage(x: i32, y: i32, inside: impl Fn(f64, f64) -> bool) -> f64 {
    let mut hits = 0;
    for sy in 0..SAMPLES {
        for sx in 0..SAMPLES {
            let px = f64::from(x) + (f64::from(sx) + 0.5) / f64::from(SAMPLES);
            let py = f64::from(y) + (f64::from(sy) + 0.5) / f64::from(SAMPLES);
            if inside(px, py) {
                hits += 1;
            }
        }
    }
    f64::from(hits) / f64::from(SAMPLES * SAMPLES)
}

fn fill_region(
    surface: &mut Surface,
    bounds: &Rect,
    paint: &Paint,
    inside: impl Fn(f64, f64) -> bool,
) {
    let x0 = bounds.x.floor().max(0.0) as i32;
    let y0 = bounds.y.floor().max(0.0) as i32;
    let x1 = (bounds.x + bounds.width)
        .ceil()
        .min(f64::from(surface.width)) as i32;
    let y1 = (bounds.y + bounds.height)
        .ceil()
        .min(f64::from(surface.height)) as i32;
    for y in y0..y1 {
        for x in x0..x1 {
            let value = coverage(x, y, &inside);
            surface.blend(x, y, paint, value);
        }
    }
}

fn point_in_polygon(points: &[(f64, f64)], x: f64, y: f64) -> bool {
    let mut inside = false;
    let mut j = points.len() - 1;
    for i in 0..points.len() {
        let (xi, yi) = points[i];
        let (xj, yj) = points[j];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn distance_to_polygon(points: &[(f64, f64)], x: f64, y: f64) -> f64 {
    let mut best = f64::MAX;
    let mut j = points.len() - 1;
    for i in 0..points.len() {
        best = best.min(distance_to_segment(points[j], points[i], x, y));
        j = i;
    }
    best
}

fn distance_to_segment(a: (f64, f64), b: (f64, f64), x: f64, y: f64) -> f64 {
    let (ax, ay) = a;
    let (bx, by) = b;
    let dx = bx - ax;
    let dy = by - ay;
    let length_squared = dx * dx + dy * dy;
    let t = if length_squared <= f64::EPSILON {
        0.0
    } else {
        (((x - ax) * dx + (y - ay) * dy) / length_squared).clamp(0.0, 1.0)
    };
    (x - (ax + t * dx)).hypot(y - (ay + t * dy))
}

/// The cursor glyph: a rimmed dart, its tip on the pose point.
pub(crate) fn draw_glyph(
    surface: &mut Surface,
    origin: &Point,
    tip: &Point,
    scale: f64,
    fill: [f64; 3],
    rim: [f64; 3],
) {
    let rect = geometry::glyph_rect(tip, scale);
    let points: Vec<(f64, f64)> = geometry::DART
        .iter()
        .map(|(x, y)| (rect.x - origin.x + x * scale, rect.y - origin.y + y * scale))
        .collect();
    let local = Rect {
        x: rect.x - origin.x - geometry::GLYPH_RIM_WIDTH * scale,
        y: rect.y - origin.y - geometry::GLYPH_RIM_WIDTH * scale,
        width: rect.width + geometry::GLYPH_RIM_WIDTH * scale * 2.0,
        height: rect.height + geometry::GLYPH_RIM_WIDTH * scale * 2.0,
    };
    let rim_width = geometry::GLYPH_RIM_WIDTH * scale * 0.5;

    let rim_points = points.clone();
    fill_region(
        surface,
        &local,
        &Paint {
            rgb: rim,
            alpha: 1.0,
        },
        |x, y| {
            point_in_polygon(&rim_points, x, y)
                || distance_to_polygon(&rim_points, x, y) <= rim_width
        },
    );
    let fill_points = points;
    fill_region(
        surface,
        &local,
        &Paint {
            rgb: fill,
            alpha: 1.0,
        },
        |x, y| point_in_polygon(&fill_points, x, y),
    );
}

/// The click ripple: a solid centre and an expanding ring, both fading as the
/// ripple's phase advances.
pub(crate) fn draw_ripple(
    surface: &mut Surface,
    origin: &Point,
    tip: &Point,
    scale: f64,
    phase: f64,
    accent: [f64; 3],
) {
    if phase <= 0.0 || phase > 1.0 {
        return;
    }
    let rect = geometry::ripple_rect(tip, scale);
    let local = Rect {
        x: rect.x - origin.x,
        y: rect.y - origin.y,
        width: rect.width,
        height: rect.height,
    };
    let centre_x = local.x + local.width / 2.0;
    let centre_y = local.y + local.height / 2.0;
    let fade = 1.0 - phase;

    let core = geometry::RIPPLE_CORE_RADIUS * scale * (1.0 - phase * 0.4);
    fill_region(
        surface,
        &local,
        &Paint {
            rgb: accent,
            alpha: 0.55 * fade,
        },
        |x, y| (x - centre_x).hypot(y - centre_y) <= core,
    );

    let ring = (geometry::RIPPLE_SIZE * 0.5 - geometry::RIPPLE_RING_INSET) * scale * phase;
    let thickness = 3.0 * scale;
    fill_region(
        surface,
        &local,
        &Paint {
            rgb: accent,
            alpha: 0.85 * fade,
        },
        |x, y| {
            let distance = (x - centre_x).hypot(y - centre_y);
            distance <= ring && distance >= ring - thickness
        },
    );
}

/// The outline around the element being acted on, at the opacity its own
/// curve reports.
pub(crate) fn draw_highlight(
    surface: &mut Surface,
    origin: &Point,
    target: &Rect,
    scale: f64,
    opacity: f64,
    accent: [f64; 3],
) {
    if opacity <= 0.0 {
        return;
    }
    let rect = geometry::highlight_rect(target, scale);
    let local = Rect {
        x: rect.x - origin.x,
        y: rect.y - origin.y,
        width: rect.width,
        height: rect.height,
    };
    let radius = geometry::HIGHLIGHT_CORNER_RADIUS * scale;
    let border = geometry::HIGHLIGHT_BORDER_WIDTH * scale;
    let outer = local;
    let inner = Rect {
        x: local.x + border,
        y: local.y + border,
        width: (local.width - border * 2.0).max(0.0),
        height: (local.height - border * 2.0).max(0.0),
    };
    fill_region(
        surface,
        &outer,
        &Paint {
            rgb: accent,
            alpha: opacity,
        },
        |x, y| {
            in_rounded_rect(&outer, radius, x, y)
                && !in_rounded_rect(&inner, (radius - border).max(0.0), x, y)
        },
    );
}

/// The label bubble's body: an opaque rounded rectangle with a rim, drawn
/// per-pixel so its corners keep their coverage. GDI writes the text on top.
pub(crate) fn draw_bubble(surface: &mut Surface, rect: &Rect, fill: [f64; 3], rim: [f64; 3]) {
    let radius = geometry::BUBBLE_CORNER_RADIUS;
    let border = geometry::BUBBLE_BORDER_WIDTH;
    let inner = Rect {
        x: rect.x + border,
        y: rect.y + border,
        width: (rect.width - border * 2.0).max(0.0),
        height: (rect.height - border * 2.0).max(0.0),
    };
    let outer = *rect;
    fill_region(
        surface,
        &outer,
        &Paint {
            rgb: rim,
            alpha: 1.0,
        },
        |x, y| in_rounded_rect(&outer, radius, x, y),
    );
    fill_region(
        surface,
        &inner,
        &Paint {
            rgb: fill,
            alpha: 1.0,
        },
        |x, y| in_rounded_rect(&inner, (radius - border).max(0.0), x, y),
    );
}

fn in_rounded_rect(rect: &Rect, radius: f64, x: f64, y: f64) -> bool {
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return false;
    }
    let radius = radius.min(rect.width / 2.0).min(rect.height / 2.0).max(0.0);
    let left = rect.x + radius;
    let right = rect.x + rect.width - radius;
    let top = rect.y + radius;
    let bottom = rect.y + rect.height - radius;
    if x < rect.x || y < rect.y || x > rect.x + rect.width || y > rect.y + rect.height {
        return false;
    }
    let nearest_x = x.clamp(left, right);
    let nearest_y = y.clamp(top, bottom);
    (x - nearest_x).hypot(y - nearest_y) <= radius
}

#[cfg(test)]
#[path = "raster_tests.rs"]
mod tests;
