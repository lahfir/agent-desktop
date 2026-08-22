use agent_desktop_core::{AdapterError, CursorOverlayInstruction, ErrorCode, Point, Rect};
use std::ffi::{CString, c_char};

const CLICK: u8 = 1 << 1;
const REDUCE_MOTION: u8 = 1 << 2;

#[repr(C)]
struct NativeRenderConfig {
    frame_seconds: f64,
    label: *const c_char,
    bubble_x: f64,
    bubble_y: f64,
    flags: u8,
}

unsafe extern "C" {
    fn agent_desktop_cursor_overlay_initial_point(output: *mut f64) -> bool;
    fn agent_desktop_cursor_overlay_screen(x: f64, y: f64, output: *mut f64) -> bool;
    fn agent_desktop_cursor_overlay_run(
        points: *const f64,
        point_count: usize,
        config: *const NativeRenderConfig,
    ) -> bool;
    fn agent_desktop_cursor_overlay_idle();
    fn agent_desktop_cursor_overlay_hide();
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
    points: &[Point],
    fps: u32,
    instruction: &CursorOverlayInstruction,
    reduce_motion: bool,
    bubble: &Rect,
) -> Result<(), AdapterError> {
    let native_points = points
        .iter()
        .flat_map(|point| [point.x, point.y])
        .collect::<Vec<_>>();
    let label = instruction
        .label()
        .map(|value| CString::new(value.replace('\0', " ")))
        .transpose()
        .map_err(|_| AdapterError::internal("Cursor overlay label encoding failed"))?;
    let mut flags = 0;
    if instruction.is_click() {
        flags |= CLICK;
    }
    if reduce_motion {
        flags |= REDUCE_MOTION;
    }
    let config = NativeRenderConfig {
        frame_seconds: 1.0 / f64::from(fps),
        label: label
            .as_ref()
            .map_or(std::ptr::null(), |value| value.as_ptr()),
        bubble_x: bubble.x,
        bubble_y: bubble.y,
        flags,
    };
    if unsafe {
        agent_desktop_cursor_overlay_run(native_points.as_ptr(), native_points.len() / 2, &config)
    } {
        Ok(())
    } else {
        Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "macOS cursor overlay renderer failed",
        ))
    }
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

pub(super) fn show() {
    unsafe { agent_desktop_cursor_overlay_show() }
}
