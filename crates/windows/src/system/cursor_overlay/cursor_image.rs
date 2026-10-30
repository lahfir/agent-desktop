//! A caller's cursor image, drawn in place of the built-in dart.
//!
//! The PNG is decoded once per path and kept premultiplied, in the surface's
//! own BGRA layout, so a frame only composites it. It is drawn at the style's
//! size, with its hotspot on the pose point, the way the macOS renderer draws
//! it.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use agent_desktop_core::{CursorImage, Point, Rect};

use super::raster::{Surface, SurfaceMapping};

pub(crate) struct DecodedImage {
    width: u32,
    height: u32,
    pixels: Vec<u32>,
}

thread_local! {
    static DECODED: RefCell<HashMap<String, Option<Rc<DecodedImage>>>> =
        RefCell::new(HashMap::new());
}

/// The decoded image for `image`, or `None` when its file is not a readable
/// PNG; the dart is drawn instead.
pub(crate) fn decoded(image: &CursorImage) -> Option<Rc<DecodedImage>> {
    DECODED.with(|cache| {
        cache
            .borrow_mut()
            .entry(image.path().to_owned())
            .or_insert_with(|| decode(image.path()).map(Rc::new))
            .clone()
    })
}

/// Where the image lands for a pose: its hotspot on the tip, scaled.
pub(crate) fn image_rect(
    tip: &Point,
    image: &CursorImage,
    decoded: &DecodedImage,
    scale: f64,
) -> Rect {
    let hotspot = image.hotspot();
    Rect {
        x: tip.x - hotspot.x * scale,
        y: tip.y - hotspot.y * scale,
        width: f64::from(decoded.width) * scale,
        height: f64::from(decoded.height) * scale,
    }
}

/// Composites the image over the surface, sampling the nearest source pixel
/// for each destination pixel so a 1:1 draw is exact.
pub(crate) fn draw_image(
    surface: &mut Surface,
    mapping: &SurfaceMapping,
    at: &Rect,
    decoded: &DecodedImage,
) {
    let local = mapping.to_local(at);
    if local.width <= 0.0 || local.height <= 0.0 {
        return;
    }
    let left = local.x.floor() as i32;
    let top = local.y.floor() as i32;
    let right = (local.x + local.width).ceil() as i32;
    let bottom = (local.y + local.height).ceil() as i32;
    for y in top.max(0)..bottom.min(surface.height) {
        let v = ((f64::from(y) + 0.5 - local.y) / local.height * f64::from(decoded.height)).floor();
        if v < 0.0 || v >= f64::from(decoded.height) {
            continue;
        }
        for x in left.max(0)..right.min(surface.width) {
            let u =
                ((f64::from(x) + 0.5 - local.x) / local.width * f64::from(decoded.width)).floor();
            if u < 0.0 || u >= f64::from(decoded.width) {
                continue;
            }
            let source = decoded.pixels[(v as usize) * (decoded.width as usize) + (u as usize)];
            composite(surface, x, y, source);
        }
    }
}

/// Source-over for one premultiplied pixel onto the premultiplied surface.
fn composite(surface: &mut Surface, x: i32, y: i32, source: u32) {
    let source_alpha = source >> 24;
    if source_alpha == 0 {
        return;
    }
    let index = (y as usize) * (surface.width as usize) + (x as usize);
    let Some(under) = surface.pixels.get(index).copied() else {
        return;
    };
    let keep = 255 - source_alpha;
    let channel = |shift: u32| {
        let over = (source >> shift) & 0xFF;
        let below = (under >> shift) & 0xFF;
        (over + (below * keep + 127) / 255).min(255)
    };
    surface.pixels[index] =
        (channel(24) << 24) | (channel(16) << 16) | (channel(8) << 8) | channel(0);
}

fn decode(path: &str) -> Option<DecodedImage> {
    use png::{ColorType, Decoder, Transformations};

    let file = std::fs::File::open(path).ok()?;
    let mut decoder = Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(Transformations::EXPAND | Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut frame = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut frame).ok()?;
    let channels = match info.color_type {
        ColorType::Rgba => 4,
        ColorType::Rgb => 3,
        ColorType::GrayscaleAlpha => 2,
        ColorType::Grayscale => 1,
        ColorType::Indexed => return None,
    };
    let count = (info.width as usize).checked_mul(info.height as usize)?;
    let pixels = (0..count)
        .map(|index| premultiplied(&frame[index * channels..index * channels + channels]))
        .collect();
    Some(DecodedImage {
        width: info.width,
        height: info.height,
        pixels,
    })
}

fn premultiplied(sample: &[u8]) -> u32 {
    let (red, green, blue, alpha) = match *sample {
        [r, g, b, a] => (r, g, b, a),
        [r, g, b] => (r, g, b, 255),
        [l, a] => (l, l, l, a),
        [l] => (l, l, l, 255),
        _ => (0, 0, 0, 0),
    };
    let scale = |channel: u8| (u32::from(channel) * u32::from(alpha) + 127) / 255;
    (u32::from(alpha) << 24) | (scale(red) << 16) | (scale(green) << 8) | scale(blue)
}

#[cfg(test)]
#[path = "cursor_image_tests.rs"]
mod tests;
