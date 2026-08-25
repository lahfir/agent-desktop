use crate::Point;

const OVERSHOOT: f64 = 1.035;
const PRIMARY_SPAN: f64 = 0.74;
const CORRECTION_START: f64 = 0.62;
const BOW_RATIO: f64 = 0.055;
const MAX_BOW: f64 = 38.0;
const TREMOR_PIXELS: f64 = 1.1;
const TREMOR_CYCLES: f64 = 7.0;

pub(super) struct HandPath {
    start: Point,
    destination: Point,
    bow: f64,
    seed: u64,
}

impl HandPath {
    pub(super) fn new(start: Point, destination: Point) -> Self {
        let distance = distance_between(&start, &destination);
        let seed =
            (start.x.to_bits() ^ destination.y.to_bits()).rotate_left(17) ^ destination.x.to_bits();
        Self {
            bow: (BOW_RATIO * distance).min(MAX_BOW),
            start,
            destination,
            seed,
        }
    }

    pub(super) fn destination(&self) -> Point {
        self.destination.clone()
    }

    pub(super) fn duration_ms(&self) -> u64 {
        let distance = distance_between(&self.start, &self.destination);
        if distance < 1.5 {
            return 0;
        }
        (60.0 + 120.0 * (distance / 40.0 + 1.0).log2()).clamp(180.0, 620.0) as u64
    }

    pub(super) fn at(&self, t: f64) -> Point {
        let dx = self.destination.x - self.start.x;
        let dy = self.destination.y - self.start.y;
        let distance = dx.hypot(dy);
        let travelled = self.submovements(t);
        let (normal_x, normal_y) = if distance > f64::EPSILON {
            (-dy / distance, dx / distance)
        } else {
            (0.0, 0.0)
        };
        let settle = (std::f64::consts::PI * t).sin();
        let sideways = self.bow * (std::f64::consts::PI * t.powf(0.8)).sin()
            + TREMOR_PIXELS * tremor(self.seed, t) * settle;
        Point {
            x: self.start.x + dx * travelled + normal_x * sideways,
            y: self.start.y + dy * travelled + normal_y * sideways,
        }
    }

    fn submovements(&self, t: f64) -> f64 {
        let primary = minimum_jerk((t / PRIMARY_SPAN).clamp(0.0, 1.0));
        let correction =
            minimum_jerk(((t - CORRECTION_START) / (1.0 - CORRECTION_START)).clamp(0.0, 1.0));
        OVERSHOOT * primary + (1.0 - OVERSHOOT) * correction
    }
}

fn distance_between(start: &Point, destination: &Point) -> f64 {
    (destination.x - start.x).hypot(destination.y - start.y)
}

pub(super) fn minimum_jerk(t: f64) -> f64 {
    t * t * t * (10.0 + t * (-15.0 + 6.0 * t))
}

fn tremor(seed: u64, t: f64) -> f64 {
    let x = t * TREMOR_CYCLES;
    let step = x.floor();
    let fraction = x - step;
    let low = unit_hash(seed, step as u64);
    let high = unit_hash(seed, step as u64 + 1);
    let blend = fraction * fraction * (3.0 - 2.0 * fraction);
    (low + (high - low) * blend) * 2.0 - 1.0
}

fn unit_hash(seed: u64, index: u64) -> f64 {
    let mut hash = seed ^ index.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    hash ^= hash >> 33;
    hash = hash.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    hash ^= hash >> 33;
    (hash >> 11) as f64 / (1u64 << 53) as f64
}
