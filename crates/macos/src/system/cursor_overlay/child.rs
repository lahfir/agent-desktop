use agent_desktop_core::{
    AdapterError, CursorMotion, CursorOverlayControl, CursorOverlayInstruction, CursorPose,
    ErrorCode, Point, place_label,
};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use super::bridge;
use super::pose::{OverlayState, apply_landing_memory, fade_pose, instruction_to_render};

pub(super) const MARKER: &str = "AGENT_DESKTOP_CURSOR_OVERLAY_CHILD";
pub(super) const SOCKET_ENV: &str = "AGENT_DESKTOP_CURSOR_OVERLAY_SOCKET";
const MAX_INSTRUCTION_BYTES: u64 = 4 * 1024;
const BUBBLE_SIZE: (f64, f64) = (232.0, 38.0);

enum PreparedRender {
    Visible {
        screen: agent_desktop_core::Rect,
        fps: u32,
        reduce_motion: bool,
        bubble: agent_desktop_core::Rect,
    },
    Deferred,
}

fn prepare_render(
    point: &Point,
    screen_at: impl FnOnce(&Point) -> Result<(agent_desktop_core::Rect, u32, bool), AdapterError>,
) -> PreparedRender {
    let Ok((screen, fps, reduce_motion)) = screen_at(point) else {
        return PreparedRender::Deferred;
    };
    let bubble = place_label(point, BUBBLE_SIZE, &screen);
    PreparedRender::Visible {
        screen,
        fps,
        reduce_motion,
        bubble,
    }
}

pub(crate) fn entry_from_env() -> Option<Result<(), AdapterError>> {
    match std::env::var(MARKER) {
        Err(_) => None,
        Ok(value) if value == super::endpoint::PROTOCOL_VERSION => Some(run()),
        Ok(_) => Some(Err(AdapterError::internal(
            "Invalid cursor overlay child protocol marker",
        ))),
    }
}

fn run() -> Result<(), AdapterError> {
    let initial = read_control(std::io::stdin())?;
    if !session_active(initial.session_id(), initial.agent_id()) {
        return Ok(());
    }
    let socket = socket_path(&initial)?;
    prepare_socket(&socket)?;
    let mut state = OverlayState::default();
    if !handle(&initial, &mut state)? {
        return cleanup(socket);
    }
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
    let mut checked_session = std::time::Instant::now();
    loop {
        if !bridge::drag_active() {
            fade_pose(&mut state, Instant::now(), bridge::opacity, bridge::rest);
        }
        if checked_session.elapsed() >= Duration::from_secs(1) {
            if !session_active(initial.session_id(), initial.agent_id()) {
                return cleanup(socket);
            }
            checked_session = std::time::Instant::now();
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                if prepare_stream(&stream).is_err() {
                    continue;
                }
                let Ok(control) = read_control(&mut stream) else {
                    let _ = stream.write_all(&[0]);
                    continue;
                };
                if control.session_id() != initial.session_id()
                    || (control.agent_id() != initial.agent_id()
                        && !(control.agent_id().is_none()
                            && (control.is_transient() || control.is_disable())))
                {
                    let _ = stream.write_all(&[2]);
                    continue;
                }
                if control.is_disable() {
                    drop(listener);
                    cleanup(socket)?;
                    let _ = stream.write_all(&[1]);
                    return Ok(());
                }
                let outcome = handle(&control, &mut state);
                if outcome.is_ok() && control.expects_acknowledgement() {
                    let _ = stream.write_all(&[1]);
                }
                if outcome.is_err() {
                    let _ = stream.write_all(&[3]);
                    continue;
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

fn prepare_stream(stream: &UnixStream) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;
    stream.set_write_timeout(Some(Duration::from_secs(1)))
}

fn handle(control: &CursorOverlayControl, state: &mut OverlayState) -> Result<bool, AdapterError> {
    control.validate()?;
    if !bridge::drag_active() {
        fade_pose(state, Instant::now(), bridge::opacity, bridge::rest);
    }
    if control.is_disable() {
        return Ok(false);
    }
    if let Some(style) = control.style() {
        state.style = style.clone();
        bridge::apply_style(&state.style);
    }
    if control.is_hide() {
        bridge::hide();
        apply_landing_memory(control, state, None);
        return Ok(true);
    }
    if control.is_show() {
        bridge::show();
        apply_landing_memory(control, state, None);
        return Ok(true);
    }
    let Some(instruction) = instruction_to_render(control) else {
        return Ok(true);
    };
    bridge::prepare(instruction);
    render(instruction, state)?;
    state.record_target_pose(Instant::now());
    apply_landing_memory(control, state, Some(instruction));
    Ok(true)
}

fn render(
    instruction: &CursorOverlayInstruction,
    state: &OverlayState,
) -> Result<(), AdapterError> {
    if instruction.phase() == agent_desktop_core::CursorPhase::Drag {
        let from = instruction.drag_from().unwrap_or(instruction.destination());
        let PreparedRender::Visible {
            fps,
            reduce_motion,
            bubble,
            ..
        } = prepare_render(from, bridge::screen_at)
        else {
            return bridge::run(
                &[CursorPose::still(from.clone())],
                60,
                instruction,
                false,
                None,
            );
        };
        bridge::run(
            &[CursorPose::still(from.clone())],
            fps,
            instruction,
            reduce_motion,
            Some(&bubble),
        )?;
        bridge::begin_drag(from, state.style.ripple() && !reduce_motion);
        return Ok(());
    }
    if instruction.phase() == agent_desktop_core::CursorPhase::Effect {
        bridge::end_drag(instruction.destination(), instruction.drag_from().is_some());
    }
    let shown = if state.style.highlight() {
        instruction.clone()
    } else {
        instruction.clone().with_target(None)
    };
    let PreparedRender::Visible {
        screen,
        fps,
        reduce_motion,
        bubble,
    } = prepare_render(instruction.destination(), bridge::screen_at)
    else {
        return bridge::run(
            &[CursorPose::still(instruction.destination().clone())],
            60,
            &shown,
            false,
            None,
        );
    };
    let frames = if reduce_motion {
        vec![CursorPose::still(instruction.destination().clone())]
    } else {
        motion_frames(state, &shown, &screen, fps)
    };
    bridge::run(&frames, fps, &shown, reduce_motion, Some(&bubble))
}

fn motion_frames(
    state: &OverlayState,
    instruction: &CursorOverlayInstruction,
    screen: &agent_desktop_core::Rect,
    fps: u32,
) -> Vec<CursorPose> {
    let destination = instruction.destination();
    if instruction.phase() == agent_desktop_core::CursorPhase::Effect {
        let mut frames = vec![CursorPose::still(destination.clone())];
        if instruction.is_click() && state.style.ripple() {
            frames.push(CursorPose {
                point: destination.clone(),
                ripple: 1.0,
            });
        }
        return frames;
    }
    let start = state.at.clone().unwrap_or_else(|| Point {
        x: (destination.x - 180.0).clamp(screen.x, screen.x + screen.width),
        y: (destination.y + 108.0).clamp(screen.y, screen.y + screen.height),
    });
    let motion = CursorMotion::new(start, destination.clone())
        .with_impact(instruction.is_click())
        .with_ripple(state.style.ripple());
    let frame_ms = 1_000.0 / f64::from(fps);
    let frame_count = (motion.total_ms() as f64 / frame_ms).ceil() as u64;
    (0..=frame_count)
        .map(|frame| {
            let elapsed = ((frame as f64 * frame_ms).round() as u64).min(motion.total_ms());
            motion.pose(elapsed)
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
    let expected = super::endpoint::path(control.session_id(), control.agent_id())?;
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

pub(super) fn session_active(session_id: &str, agent_id: Option<&str>) -> bool {
    agent_desktop_core::session::read_manifest(session_id)
        .ok()
        .flatten()
        .is_some_and(|manifest| {
            manifest.ended_at.is_none()
                && manifest.cursor_overlay.is_enabled()
                && !(manifest.cursor_overlay.is_multi_agent() && agent_id.is_none())
        })
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
#[path = "child_tests.rs"]
mod tests;
