use super::CursorPose;
use super::hand_path::{HandPath, minimum_jerk};
use crate::Point;
use std::f64::consts::{PI, TAU};

const FLOURISH_MS: u64 = 620;
const WANDER_SPAN: f64 = 0.46;
const PRESS_SPAN: f64 = 0.58;
const RIPPLE_START: f64 = 0.54;
const WANDER_LOOPS: f64 = 1.25;
const WANDER_RADIUS: f64 = 11.0;
const HOVER_SCALE: f64 = 1.22;
const PRESS_SCALE: f64 = 0.78;

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
        if self.click {
            self.duration_ms + FLOURISH_MS
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
        if !self.click || elapsed_ms < self.duration_ms {
            return CursorPose::still(point);
        }
        let mut pose = flourish(point, elapsed_ms - self.duration_ms);
        if !self.ripple {
            pose.ripple = 0.0;
        }
        pose
    }
}

fn flourish(destination: Point, elapsed_ms: u64) -> CursorPose {
    let t = (elapsed_ms as f64 / FLOURISH_MS as f64).clamp(0.0, 1.0);
    let wander = (t / WANDER_SPAN).clamp(0.0, 1.0);
    let radius = if wander >= 1.0 {
        0.0
    } else {
        WANDER_RADIUS * (PI * wander).sin().powf(0.8)
    };
    let angle = TAU * WANDER_LOOPS * minimum_jerk(wander);
    let scale = if t < WANDER_SPAN {
        1.0 + (HOVER_SCALE - 1.0) * minimum_jerk(wander)
    } else if t < PRESS_SPAN {
        HOVER_SCALE
            + (PRESS_SCALE - HOVER_SCALE)
                * minimum_jerk((t - WANDER_SPAN) / (PRESS_SPAN - WANDER_SPAN))
    } else {
        PRESS_SCALE + (1.0 - PRESS_SCALE) * spring((t - PRESS_SPAN) / (1.0 - PRESS_SPAN))
    };
    CursorPose {
        point: Point {
            x: destination.x + angle.cos() * radius * 1.15,
            y: destination.y + angle.sin() * radius * 0.8,
        },
        scale,
        ripple: ((t - RIPPLE_START) / (1.0 - RIPPLE_START)).clamp(0.0, 1.0),
    }
}

fn spring(t: f64) -> f64 {
    if t >= 1.0 {
        return 1.0;
    }
    1.0 - (-7.0 * t).exp() * (9.0 * t).cos()
}
