use super::CursorPose;
use super::hand_path::HandPath;
use crate::Point;

const RIPPLE_MS: u64 = 300;

pub struct CursorMotion {
    path: HandPath,
    duration_ms: u64,
    click: bool,
    ripple: bool,
}

impl CursorMotion {
    pub fn new(start: Point, destination: Point) -> Self {
        let path = HandPath::new(start, destination);
        let duration_ms = path.duration_ms();
        Self {
            path,
            duration_ms,
            click: false,
            ripple: true,
        }
    }

    pub const fn with_impact(mut self, click: bool) -> Self {
        self.click = click;
        self
    }

    pub const fn with_ripple(mut self, ripple: bool) -> Self {
        self.ripple = ripple;
        self
    }

    pub const fn duration_ms(&self) -> u64 {
        self.duration_ms
    }

    pub const fn total_ms(&self) -> u64 {
        if self.plays_ripple() {
            self.duration_ms + RIPPLE_MS
        } else {
            self.duration_ms
        }
    }

    pub fn sample(&self, elapsed_ms: u64) -> Point {
        if elapsed_ms >= self.duration_ms {
            return self.path.destination();
        }
        self.path.at(elapsed_ms as f64 / self.duration_ms as f64)
    }

    pub fn pose(&self, elapsed_ms: u64) -> CursorPose {
        let point = self.sample(elapsed_ms.min(self.duration_ms));
        if !self.plays_ripple() || elapsed_ms <= self.duration_ms {
            return CursorPose::still(point);
        }
        CursorPose {
            point,
            ripple: ((elapsed_ms - self.duration_ms) as f64 / RIPPLE_MS as f64).clamp(0.0, 1.0),
        }
    }

    const fn plays_ripple(&self) -> bool {
        self.click && self.ripple
    }
}
