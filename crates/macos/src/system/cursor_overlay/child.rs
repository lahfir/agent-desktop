use agent_desktop_core::{
    AdapterError, CursorMotion, CursorOverlayConfig, CursorOverlayControl,
    CursorOverlayInstruction, ErrorCode, Point, place_label,
};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use super::bridge;

pub(super) const MARKER: &str = "AGENT_DESKTOP_CURSOR_OVERLAY_CHILD";
pub(super) const PROTOCOL_VERSION: &str = "v1";
pub(super) const SOCKET_ENV: &str = "AGENT_DESKTOP_CURSOR_OVERLAY_SOCKET";
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
    let initial = read_control(std::io::stdin())?;
    let socket = socket_path(&initial)?;
    prepare_socket(&socket)?;
    let listener = UnixListener::bind(&socket).map_err(|error| {
        AdapterError::internal("Could not bind the cursor overlay session socket")
            .with_platform_detail(error.to_string())
    })?;
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600)).map_err(|error| {
        AdapterError::internal("Could not protect the cursor overlay session socket")
            .with_platform_detail(error.to_string())
    })?;
    listener.set_nonblocking(true).map_err(|error| {
        AdapterError::internal("Could not configure the cursor overlay session socket")
            .with_platform_detail(error.to_string())
    })?;
    let mut current = None;
    if !handle(&initial, &mut current)? {
        return cleanup(socket);
    }
    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let Ok(control) = read_control(&mut stream) else {
                    continue;
                };
                if control.session_id() != initial.session_id() {
                    continue;
                }
                if control.is_disable() {
                    drop(listener);
                    cleanup(socket)?;
                    let _ = stream.write_all(&[1]);
                    return Ok(());
                }
                if handle(&control, &mut current).is_err() {
                    continue;
                }
                if control.is_hide() {
                    let _ = stream.write_all(&[1]);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                bridge::idle();
                thread::sleep(Duration::from_millis(8));
            }
            Err(error) => {
                let _ = cleanup(socket);
                return Err(
                    AdapterError::internal("Cursor overlay session socket failed")
                        .with_platform_detail(error.to_string()),
                );
            }
        }
    }
}

fn handle(
    control: &CursorOverlayControl,
    current: &mut Option<Point>,
) -> Result<bool, AdapterError> {
    control.validate()?;
    if control.is_disable() {
        return Ok(false);
    }
    if control.is_hide() {
        bridge::hide();
        return Ok(true);
    }
    if control.is_show() {
        bridge::show();
        return Ok(true);
    }
    let owned;
    let instruction = if let Some(instruction) = control.instruction() {
        instruction
    } else {
        let config = CursorOverlayConfig::enabled(control.label().map(str::to_owned), 12)?;
        owned = CursorOverlayInstruction::new(bridge::initial_point()?, &config, false)?;
        &owned
    };
    render(instruction, current)?;
    *current = Some(instruction.destination().clone());
    Ok(true)
}

fn render(
    instruction: &CursorOverlayInstruction,
    current: &Option<Point>,
) -> Result<(), AdapterError> {
    let (screen, fps, reduce_motion) = bridge::screen_at(instruction.destination())?;
    let bubble = place_label(instruction.destination(), BUBBLE_SIZE, &screen);
    let points = if reduce_motion {
        vec![instruction.destination().clone()]
    } else {
        motion_points(current.as_ref(), instruction.destination(), &screen, fps)
    };
    bridge::run(&points, fps, instruction, reduce_motion, &bubble)
}

fn motion_points(
    current: Option<&Point>,
    destination: &Point,
    screen: &agent_desktop_core::Rect,
    fps: u32,
) -> Vec<Point> {
    let start = current.cloned().unwrap_or_else(|| Point {
        x: (destination.x - 180.0).clamp(screen.x, screen.x + screen.width),
        y: (destination.y + 108.0).clamp(screen.y, screen.y + screen.height),
    });
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

fn read_control(reader: impl Read) -> Result<CursorOverlayControl, AdapterError> {
    let mut payload = Vec::new();
    reader
        .take(MAX_INSTRUCTION_BYTES + 1)
        .read_to_end(&mut payload)
        .map_err(|error| {
            AdapterError::internal("Could not read cursor overlay control")
                .with_platform_detail(error.to_string())
        })?;
    if payload.len() as u64 > MAX_INSTRUCTION_BYTES {
        return Err(AdapterError::internal(
            "Cursor overlay instruction exceeds the transport limit",
        ));
    }
    let control: CursorOverlayControl = serde_json::from_slice(&payload).map_err(|error| {
        AdapterError::internal("Could not decode cursor overlay control")
            .with_platform_detail(error.to_string())
    })?;
    control.validate()?;
    Ok(control)
}

fn socket_path(control: &CursorOverlayControl) -> Result<PathBuf, AdapterError> {
    let expected = super::endpoint::path(control.session_id())?;
    let supplied = std::env::var_os(SOCKET_ENV)
        .map(PathBuf::from)
        .ok_or_else(|| AdapterError::internal("Cursor overlay child socket is missing"))?;
    if supplied != expected {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "Cursor overlay child socket does not match its session",
        ));
    }
    Ok(expected)
}

fn prepare_socket(path: &Path) -> Result<(), AdapterError> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(
            AdapterError::internal("Could not replace stale cursor overlay socket")
                .with_platform_detail(error.to_string()),
        ),
    }
}

fn cleanup(path: PathBuf) -> Result<(), AdapterError> {
    bridge::stop();
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(
            AdapterError::internal("Could not remove cursor overlay socket")
                .with_platform_detail(error.to_string()),
        ),
    }
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
        let points = motion_points(None, &destination, &screen, 120);

        assert_eq!(points.last(), Some(&destination));
        assert!(points.len() >= 51);
    }

    #[test]
    fn subsequent_motion_starts_from_the_previous_destination() {
        let screen = agent_desktop_core::Rect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let start = Point { x: 200.0, y: 300.0 };
        let destination = Point { x: 900.0, y: 500.0 };
        let points = motion_points(Some(&start), &destination, &screen, 120);

        assert_eq!(points.first(), Some(&start));
        assert_eq!(points.last(), Some(&destination));
    }
}
