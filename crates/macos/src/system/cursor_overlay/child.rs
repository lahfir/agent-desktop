use agent_desktop_core::{
    AdapterError, CursorMotion, CursorOverlayInstruction, Point, place_label,
};
use std::io::Read;

use super::bridge;

pub(super) const MARKER: &str = "AGENT_DESKTOP_CURSOR_OVERLAY_CHILD";
pub(super) const PROTOCOL_VERSION: &str = "v1";
const MAX_INSTRUCTION_BYTES: u64 = 4 * 1024;
const BUBBLE_SIZE: (f64, f64) = (232.0, 38.0);

pub(crate) fn entry_from_env() -> Option<Result<(), AdapterError>> {
    match std::env::var(MARKER) {
        Err(_) => None,
        Ok(value) if value == PROTOCOL_VERSION => Some(run()),
        Ok(_) => Some(Err(AdapterError::internal(
            "Invalid cursor overlay child protocol marker",
        ))),
    }
}

fn run() -> Result<(), AdapterError> {
    let mut payload = Vec::new();
    std::io::stdin()
        .take(MAX_INSTRUCTION_BYTES + 1)
        .read_to_end(&mut payload)
        .map_err(|error| {
            AdapterError::internal("Could not read cursor overlay instruction")
                .with_platform_detail(error.to_string())
        })?;
    if payload.len() as u64 > MAX_INSTRUCTION_BYTES {
        return Err(AdapterError::internal(
            "Cursor overlay instruction exceeds the transport limit",
        ));
    }
    let instruction: CursorOverlayInstruction =
        serde_json::from_slice(&payload).map_err(|error| {
            AdapterError::internal("Could not decode cursor overlay instruction")
                .with_platform_detail(error.to_string())
        })?;
    instruction.validate()?;
    render(&instruction)
}

fn render(instruction: &CursorOverlayInstruction) -> Result<(), AdapterError> {
    let (screen, fps, reduce_motion) = bridge::screen_at(instruction.destination())?;
    let bubble = place_label(instruction.destination(), BUBBLE_SIZE, &screen);
    let points = if reduce_motion {
        vec![instruction.destination().clone()]
    } else {
        motion_points(instruction.destination(), &screen, fps)
    };
    bridge::run(&points, fps, instruction, reduce_motion, &bubble)
}

fn motion_points(destination: &Point, screen: &agent_desktop_core::Rect, fps: u32) -> Vec<Point> {
    let start = Point {
        x: (destination.x - 180.0).clamp(screen.x, screen.x + screen.width),
        y: (destination.y + 108.0).clamp(screen.y, screen.y + screen.height),
    };
    let motion = CursorMotion::new(start, destination.clone());
    let frame_ms = 1_000.0 / f64::from(fps);
    let frame_count = (motion.duration_ms() as f64 / frame_ms).ceil() as u64;
    (0..=frame_count)
        .map(|frame| {
            let elapsed = ((frame as f64 * frame_ms).round() as u64).min(motion.duration_ms());
            motion.sample(elapsed)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sampled_motion_ends_at_the_requested_destination() {
        let screen = agent_desktop_core::Rect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let destination = Point { x: 900.0, y: 500.0 };
        let points = motion_points(&destination, &screen, 120);

        assert_eq!(points.last(), Some(&destination));
        assert!(points.len() >= 51);
    }
}
