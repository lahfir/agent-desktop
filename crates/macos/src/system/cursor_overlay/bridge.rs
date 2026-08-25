use agent_desktop_core::{
    AdapterError, CursorOverlayInstruction, CursorOverlayStyle, CursorPose, ErrorCode, Point, Rect,
};
use std::ffi::{CString, c_char};

const REDUCE_MOTION: u8 = 1 << 2;
const HIGHLIGHT: u8 = 1 << 3;

#[repr(C)]
struct NativeCursorStyle {
    fill: [f64; 3],
    rim: [f64; 3],
    accent: [f64; 3],
    size: f64,
}

#[repr(C)]
struct NativeCursorFrame {
    x: f64,
    y: f64,
    ripple: f64,
}

#[repr(C)]
struct NativeRenderConfig {
    frame_seconds: f64,
    label: *const c_char,
    bubble_x: f64,
    bubble_y: f64,
    target: [f64; 4],
    flags: u8,
}

unsafe extern "C" {
    fn agent_desktop_cursor_overlay_initial_point(output: *mut f64) -> bool;
    fn agent_desktop_cursor_overlay_screen(x: f64, y: f64, output: *mut f64) -> bool;
    fn agent_desktop_cursor_overlay_run(
        frames: *const NativeCursorFrame,
        frame_count: usize,
        config: *const NativeRenderConfig,
    ) -> bool;
    fn agent_desktop_cursor_overlay_style(style: *const NativeCursorStyle);
    fn agent_desktop_cursor_overlay_idle();
    fn agent_desktop_cursor_overlay_hide();
    fn agent_desktop_cursor_overlay_rest();
    fn agent_desktop_cursor_overlay_show();
    fn agent_desktop_cursor_overlay_stop();
}

pub(super) fn initial_point() -> Result<Point, AdapterError> {
    let mut output = [0.0; 2];
    if !unsafe { agent_desktop_cursor_overlay_initial_point(output.as_mut_ptr()) } {
        return Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "macOS cursor overlay could not select an initial position",
        ));
    }
    Ok(Point {
        x: output[0],
        y: output[1],
    })
}

pub(super) fn screen_at(point: &Point) -> Result<(Rect, u32, bool), AdapterError> {
    let mut output = [0.0; 6];
    if !unsafe { agent_desktop_cursor_overlay_screen(point.x, point.y, output.as_mut_ptr()) } {
        return Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "No macOS display contains the cursor overlay destination",
        ));
    }
    Ok((
        Rect {
            x: output[0],
            y: output[1],
            width: output[2],
            height: output[3],
        },
        (output[4] as u32).clamp(60, 120),
        output[5] != 0.0,
    ))
}

pub(super) fn run(
    poses: &[CursorPose],
    fps: u32,
    instruction: &CursorOverlayInstruction,
    reduce_motion: bool,
    bubble: &Rect,
) -> Result<(), AdapterError> {
    let frames = poses
        .iter()
        .map(|pose| NativeCursorFrame {
            x: pose.point.x,
            y: pose.point.y,
            ripple: pose.ripple,
        })
        .collect::<Vec<_>>();
    let label = instruction
        .label()
        .map(|value| CString::new(value.replace('\0', " ")))
        .transpose()
        .map_err(|_| AdapterError::internal("Cursor overlay label encoding failed"))?;
    let mut flags = 0;
    if reduce_motion {
        flags |= REDUCE_MOTION;
    }
    let target = instruction
        .target()
        .map(|rect| [rect.x, rect.y, rect.width, rect.height]);
    if target.is_some() {
        flags |= HIGHLIGHT;
    }
    let config = NativeRenderConfig {
        frame_seconds: 1.0 / f64::from(fps),
        label: label
            .as_ref()
            .map_or(std::ptr::null(), |value| value.as_ptr()),
        bubble_x: bubble.x,
        bubble_y: bubble.y,
        target: target.unwrap_or_default(),
        flags,
    };
    if unsafe { agent_desktop_cursor_overlay_run(frames.as_ptr(), frames.len(), &config) } {
        Ok(())
    } else {
        Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "macOS cursor overlay renderer failed",
        ))
    }
}

pub(super) fn apply_style(style: &CursorOverlayStyle) {
    let native = NativeCursorStyle {
        fill: style.fill_rgb(),
        rim: style.rim_rgb(),
        accent: style.accent_rgb(),
        size: style.size(),
    };
    unsafe { agent_desktop_cursor_overlay_style(&native) }
}

pub(super) fn idle() {
    unsafe { agent_desktop_cursor_overlay_idle() }
}

pub(super) fn stop() {
    unsafe { agent_desktop_cursor_overlay_stop() }
}

pub(super) fn rest() {
    unsafe { agent_desktop_cursor_overlay_rest() }
}

pub(super) fn hide() {
    unsafe { agent_desktop_cursor_overlay_hide() }
}

pub(super) fn show() {
    unsafe { agent_desktop_cursor_overlay_show() }
}
