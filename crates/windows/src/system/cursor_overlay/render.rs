//! One frame: where the follower surface sits, and what is on it.
//!
//! Pure apart from the label's text, which the caller draws afterwards onto
//! the bubble body this composes. Keeping the composition itself free of
//! Win32 is what lets the whole visual layer be asserted without a desktop.

use agent_desktop_core::{CursorOverlayStyle, Point, Rect, place_label};

use super::raster::{self, Surface, SurfaceMapping};
use super::{cursor_image, geometry, rounded};

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
    /// The whole card, in surface coordinates, when there is one. The text
    /// rectangle above is only its inset interior; a caller fading the card
    /// in has to dim the body and its border with the text, or the card
    /// arrives already drawn with only its writing catching up.
    pub(crate) card_rect: Option<Rect>,
}

pub(crate) fn compose(frame: &Frame<'_>) -> Composed {
    let scale = frame.style.size();
    let image = frame.style.image().and_then(|image| {
        let decoded = cursor_image::decoded(image)?;
        let rect = cursor_image::image_rect(&frame.tip, image, &decoded, scale);
        Some((decoded, rect))
    });
    let label_rect = frame
        .label
        .map(|_| place_label(&frame.tip, geometry::bubble_size(scale), &frame.screen));
    let highlighted = frame
        .target
        .as_ref()
        .filter(|_| frame.style.highlight() && frame.highlight_opacity > 0.0);
    let highlight_rect = highlighted.map(|target| geometry::highlight_rect(target, scale));

    let mut bounds = geometry::follower_rect(
        &frame.tip,
        scale,
        label_rect.as_ref(),
        highlight_rect.as_ref(),
    );
    if let Some((_, rect)) = &image {
        bounds = geometry::union_of(&bounds, rect);
    }
    let mapping = SurfaceMapping {
        origin: Point {
            x: bounds.x.floor(),
            y: bounds.y.floor(),
        },
        scale,
    };
    let Some(mut surface) =
        surface_extent(&bounds).and_then(|(width, height)| Surface::try_transparent(width, height))
    else {
        return Composed {
            origin: mapping.origin,
            surface: Surface::empty(),
            text_rect: None,
            card_rect: None,
        };
    };

    if let Some(highlight) = highlighted {
        rounded::draw_highlight(
            &mut surface,
            &mapping,
            highlight,
            frame.highlight_opacity,
            frame.style.accent_rgb(),
        );
    }
    if frame.style.ripple() {
        raster::draw_ripple(
            &mut surface,
            &mapping,
            &frame.tip,
            frame.ripple_phase,
            frame.style.accent_rgb(),
        );
    }

    let mut card_rect = None;
    let text_rect = label_rect.map(|rect| {
        let local = mapping.to_local(&rect);
        card_rect = Some(local);
        rounded::draw_bubble(
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

    match &image {
        Some((decoded, rect)) => cursor_image::draw_image(&mut surface, &mapping, rect, decoded),
        None => raster::draw_glyph(
            &mut surface,
            &mapping,
            &frame.tip,
            frame.style.fill_rgb(),
            frame.style.rim_rgb(),
        ),
    }

    Composed {
        origin: mapping.origin,
        surface,
        text_rect,
        card_rect,
    }
}

fn surface_extent(bounds: &Rect) -> Option<(i32, i32)> {
    let extent = |value: f64| {
        let pixels = value.ceil();
        (pixels.is_finite() && (0.0..f64::from(i32::MAX)).contains(&pixels))
            .then(|| pixels as i32 + 1)
    };
    Some((extent(bounds.width)?, extent(bounds.height)?))
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
