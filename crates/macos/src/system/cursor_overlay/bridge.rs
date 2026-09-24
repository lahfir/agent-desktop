use agent_desktop_core::{
    AdapterError, CURSOR_HIGHLIGHT_HOLD_MS, CursorOverlayInstruction, CursorOverlayStyle,
    CursorPose, ErrorCode, Point, Rect,
};
use std::ffi::{CString, c_char};

const REDUCE_MOTION: u8 = 1 << 2;
const HIGHLIGHT: u8 = 1 << 3;
const LAYOUT_DEFERRED: u8 = 1 << 4;

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
    highlight_seconds: f64,
    flags: u8,
}

unsafe extern "C" {
    fn agent_desktop_cursor_overlay_target(pid: u32, window: u32, x: f64, y: f64);
    fn agent_desktop_cursor_overlay_screen(x: f64, y: f64, output: *mut f64) -> bool;
    fn agent_desktop_cursor_overlay_run(
        frames: *const NativeCursorFrame,
        frame_count: usize,
        config: *const NativeRenderConfig,
    ) -> bool;
    fn agent_desktop_cursor_overlay_drag_begin(x: f64, y: f64, trail: bool);
    fn agent_desktop_cursor_overlay_drag_end(x: f64, y: f64, completed: bool);
    fn agent_desktop_cursor_overlay_drag_active() -> bool;
    fn agent_desktop_cursor_overlay_style(style: *const NativeCursorStyle);
    fn agent_desktop_cursor_overlay_idle();
    fn agent_desktop_cursor_overlay_hide();
    fn agent_desktop_cursor_overlay_opacity(alpha: f64);
    fn agent_desktop_cursor_overlay_rest();
    fn agent_desktop_cursor_overlay_show();
    fn agent_desktop_cursor_overlay_stop();
}

pub(super) fn begin_drag(from: &Point, trail: bool) {
    unsafe { agent_desktop_cursor_overlay_drag_begin(from.x, from.y, trail) }
}

pub(super) fn end_drag(to: &Point, completed: bool) {
    unsafe { agent_desktop_cursor_overlay_drag_end(to.x, to.y, completed) }
}

pub(super) fn drag_active() -> bool {
    unsafe { agent_desktop_cursor_overlay_drag_active() }
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

pub(super) fn prepare(instruction: &CursorOverlayInstruction) {
    let (pid, window) = instruction.window().map_or((0, 0), |(pid, window)| {
        let number = super::super::window_resolve::parse_window_number(window)
            .and_then(|number| u32::try_from(number).ok())
            .unwrap_or(0);
        (pid.get(), number)
    });
    unsafe {
        agent_desktop_cursor_overlay_target(
            pid,
            window,
            instruction.destination().x,
            instruction.destination().y,
        )
    };
}

pub(super) fn run(
    poses: &[CursorPose],
    fps: u32,
    instruction: &CursorOverlayInstruction,
    reduce_motion: bool,
    bubble: Option<&Rect>,
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
    if bubble.is_none() {
        flags |= LAYOUT_DEFERRED;
    }
    let bubble = bubble.cloned().unwrap_or(Rect {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    });
    let config = NativeRenderConfig {
        frame_seconds: 1.0 / f64::from(fps),
        label: label
            .as_ref()
            .map_or(std::ptr::null(), |value| value.as_ptr()),
        bubble_x: bubble.x,
        bubble_y: bubble.y,
        target: target.unwrap_or_default(),
        highlight_seconds: CURSOR_HIGHLIGHT_HOLD_MS as f64 / 1_000.0,
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

pub(super) fn hide() {
    unsafe { agent_desktop_cursor_overlay_hide() }
}

pub(super) fn opacity(alpha: f64) {
    unsafe { agent_desktop_cursor_overlay_opacity(alpha) }
}

pub(super) fn rest() {
    unsafe { agent_desktop_cursor_overlay_rest() }
}

pub(super) fn show() {
    unsafe { agent_desktop_cursor_overlay_show() }
}
