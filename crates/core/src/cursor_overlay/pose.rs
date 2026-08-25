use crate::Point;

#[derive(Debug, Clone, PartialEq)]
pub struct CursorPose {
    pub point: Point,
    pub scale: f64,
    pub spin: f64,
    pub tilt: f64,
    pub ripple: f64,
}

impl CursorPose {
    pub const fn still(point: Point) -> Self {
        Self {
            point,
            scale: 1.0,
            spin: 0.0,
            tilt: 0.0,
            ripple: 0.0,
        }
    }
}
