use crate::{Point, Rect};

const DESTINATION_GAP: f64 = 18.0;

pub fn place_label(destination: &Point, size: (f64, f64), screen: &Rect) -> Rect {
    let (width, height) = size;
    let right = screen.x + screen.width;
    let bottom = screen.y + screen.height;
    let preferred_x = destination.x + DESTINATION_GAP;
    let preferred_y = destination.y + DESTINATION_GAP;
    let x = if preferred_x + width <= right {
        preferred_x
    } else {
        destination.x - width - DESTINATION_GAP
    };
    let y = if preferred_y + height <= bottom {
        preferred_y
    } else {
        destination.y - height - DESTINATION_GAP
    };
    Rect {
        x: x.clamp(screen.x, (right - width).max(screen.x)),
        y: y.clamp(screen.y, (bottom - height).max(screen.y)),
        width: width.min(screen.width.max(0.0)),
        height: height.min(screen.height.max(0.0)),
    }
}
