use super::CursorPose;
use super::hand_path::{HandPath, minimum_jerk};
use crate::Point;
use std::f64::consts::{PI, TAU};

const FLOURISH_MS: u64 = 620;
const LIFT_SPAN: f64 = 0.44;
const BAM_SPAN: f64 = 0.56;
const RIPPLE_START: f64 = 0.52;
const LIFT_SCALE: f64 = 1.7;
const BAM_SCALE: f64 = 0.74;
const MAX_TILT: f64 = 0.34;

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
        if self.click && elapsed_ms >= self.duration_ms {
            let pose = flourish(point, elapsed_ms - self.duration_ms);
            return if self.ripple {
                pose
            } else {
                CursorPose {
                    ripple: 0.0,
                    ..pose
                }
            };
        }
        CursorPose::still(point)
    }
}

fn flourish(point: Point, elapsed_ms: u64) -> CursorPose {
    let t = (elapsed_ms as f64 / FLOURISH_MS as f64).clamp(0.0, 1.0);
    let lift = minimum_jerk((t / LIFT_SPAN).clamp(0.0, 1.0));
    let scale = if t < LIFT_SPAN {
        1.0 + (LIFT_SCALE - 1.0) * lift
    } else if t < BAM_SPAN {
        LIFT_SCALE
            + (BAM_SCALE - LIFT_SCALE) * minimum_jerk((t - LIFT_SPAN) / (BAM_SPAN - LIFT_SPAN))
    } else {
        BAM_SCALE + (1.0 - BAM_SCALE) * minimum_jerk((t - BAM_SPAN) / (1.0 - BAM_SPAN))
    };
    CursorPose {
        point,
        scale,
        spin: TAU * lift,
        tilt: MAX_TILT * (PI * (t / LIFT_SPAN).clamp(0.0, 1.0)).sin(),
        ripple: ((t - RIPPLE_START) / (1.0 - RIPPLE_START)).clamp(0.0, 1.0),
    }
}
