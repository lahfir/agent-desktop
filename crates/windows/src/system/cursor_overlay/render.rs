//! One frame: where the follower surface sits, and what is on it.
//!
//! Pure apart from the label's text, which the caller draws afterwards onto
//! the bubble body this composes. Keeping the composition itself free of
//! Win32 is what lets the whole visual layer be asserted without a desktop.

use agent_desktop_core::{CursorOverlayStyle, Point, Rect, place_label};

use super::geometry;
use super::raster::{self, Surface};

/// Everything a frame needs, resolved from the control and the clock before
/// any pixel is touched.
pub(crate) struct Frame<'a> {
    pub(crate) tip: Point,
    pub(crate) style: &'a CursorOverlayStyle,
    pub(crate) ripple_phase: f64,
    pub(crate) target: Option<Rect>,
    pub(crate) highlight_opacity: f64,
    pub(crate) label: Option<&'a str>,
    pub(crate) screen: Rect,
}

pub(crate) struct Composed {
    pub(crate) origin: Point,
    pub(crate) surface: Surface,
    /// Where the label's text goes, in surface coordinates, when there is
    /// one. The caller draws into it and then forces it opaque, which is the
    /// only region GDI is allowed to touch.
    pub(crate) text_rect: Option<Rect>,
}

pub(crate) fn compose(frame: &Frame<'_>) -> Composed {
    let scale = frame.style.size();
    let label_rect = frame
        .label
        .map(|_| place_label(&frame.tip, geometry::bubble_size(scale), &frame.screen));
    let highlight_rect = frame
        .target
        .as_ref()
        .filter(|_| frame.style.highlight() && frame.highlight_opacity > 0.0)
        .map(|target| geometry::highlight_rect(target, scale));

    let bounds = geometry::follower_rect(
        &frame.tip,
        scale,
        label_rect.as_ref(),
        highlight_rect.as_ref(),
    );
    let origin = Point {
        x: bounds.x.floor(),
        y: bounds.y.floor(),
    };
    let mut surface = Surface::transparent(
        bounds.width.ceil() as i32 + 1,
        bounds.height.ceil() as i32 + 1,
    );

    if let Some(highlight) = frame.target.as_ref().filter(|_| highlight_rect.is_some()) {
        raster::draw_highlight(
            &mut surface,
            &origin,
            highlight,
            scale,
            frame.highlight_opacity,
            frame.style.accent_rgb(),
        );
    }
    if frame.style.ripple() {
        raster::draw_ripple(
            &mut surface,
            &origin,
            &frame.tip,
            scale,
            frame.ripple_phase,
            frame.style.accent_rgb(),
        );
    }

    let text_rect = label_rect.map(|rect| {
        let local = Rect {
            x: rect.x - origin.x,
            y: rect.y - origin.y,
            width: rect.width,
            height: rect.height,
        };
        raster::draw_bubble(
            &mut surface,
            &local,
            frame.style.fill_rgb(),
            frame.style.rim_rgb(),
        );
        let inset = geometry::BUBBLE_TEXT_INSET * scale;
        Rect {
            x: local.x + inset,
            y: local.y + inset,
            width: (local.width - inset * 2.0).max(0.0),
            height: (local.height - inset * 2.0).max(0.0),
        }
    });

    raster::draw_glyph(
        &mut surface,
        &origin,
        &frame.tip,
        scale,
        frame.style.fill_rgb(),
        frame.style.rim_rgb(),
    );

    Composed {
        origin,
        surface,
        text_rect,
    }
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
