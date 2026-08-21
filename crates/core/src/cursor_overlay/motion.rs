use crate::Point;

#[derive(Debug, Clone, PartialEq)]
pub struct CursorMotion {
    start: Point,
    destination: Point,
    duration_ms: u64,
    bend: f64,
}

impl CursorMotion {
    pub fn new(start: Point, destination: Point) -> Self {
        let dx = destination.x - start.x;
        let dy = destination.y - start.y;
        let distance = dx.hypot(dy);
        let duration_ms = ((0.32 + distance / 2_600.0).clamp(0.42, 0.72) * 1_000.0).round();
        Self {
            start,
            destination,
            duration_ms: duration_ms as u64,
            bend: (distance * 0.09).min(54.0),
        }
    }

    pub const fn duration_ms(&self) -> u64 {
        self.duration_ms
    }

    pub fn sample(&self, elapsed_ms: u64) -> Point {
        if elapsed_ms == 0 {
            return self.start.clone();
        }
        if elapsed_ms >= self.duration_ms {
            return self.destination.clone();
        }
        let t = elapsed_ms as f64 / self.duration_ms as f64;
        let progress = minimum_jerk(t);
        let dx = self.destination.x - self.start.x;
        let dy = self.destination.y - self.start.y;
        let distance = dx.hypot(dy);
        let arc = 4.0 * t * (1.0 - t) * self.bend;
        let (normal_x, normal_y) = if distance > f64::EPSILON {
            (-dy / distance, dx / distance)
        } else {
            (0.0, 0.0)
        };
        Point {
            x: self.start.x + dx * progress + normal_x * arc,
            y: self.start.y + dy * progress + normal_y * arc,
        }
    }
}

fn minimum_jerk(t: f64) -> f64 {
    t * t * t * (10.0 + t * (-15.0 + 6.0 * t))
}
