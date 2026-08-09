use agent_desktop_core::{
    AdapterError, Deadline, DeliverySemantics, ErrorCode, WindowInfo, WindowOp,
};

use super::permissions::ensure_budget;
use super::window_identity::WindowIdentityEvidence;
use super::window_ops::parse_handle;

const MAX_DIMENSION: f64 = 1_000_000.0;
const MAX_COORDINATE: f64 = 1_000_000.0;
/// Placement re-read tolerance pinned by A21-5 (DWM animation headroom floor).
const PLACEMENT_TOLERANCE_PX: i32 = 8;
/// Wait-then-re-read delay pinned by A21-5 so an in-flight DWM animation is
/// not misread as a placement mismatch.
const PLACEMENT_REREAD_MS: u64 = 80;

pub(crate) fn window_op_impl(
    win: &WindowInfo,
    op: WindowOp,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    ensure_budget(deadline).map_err(before_write)?;
    let handle = parse_handle(&win.id);
    if handle.is_null() {
        return Err(before_write(AdapterError::new(
            ErrorCode::InvalidArgs,
            "window id is not a Windows HWND-shaped id",
        )));
    }
    let Some(evidence) = WindowIdentityEvidence::from_info(handle, win) else {
        return Err(before_write(AdapterError::new(
            ErrorCode::StaleRef,
            "window operation requires a process-instance token on the stored window",
        )));
    };
    evidence.verify_stored().map_err(before_write)?;
    match op {
        WindowOp::Resize { width, height } => resize(handle, &evidence, width, height, deadline),
        WindowOp::Move { x, y } => move_to(handle, &evidence, x, y, deadline),
        WindowOp::Minimize => show_verb(handle, &evidence, ShowVerb::Minimize, deadline),
        WindowOp::Maximize => show_verb(handle, &evidence, ShowVerb::Maximize, deadline),
        WindowOp::Restore => show_verb(handle, &evidence, ShowVerb::Restore, deadline),
    }
}

fn validate_size(width: f64, height: f64) -> Result<(), AdapterError> {
    if !width.is_finite()
        || !height.is_finite()
        || width <= 0.0
        || height <= 0.0
        || width > MAX_DIMENSION
        || height > MAX_DIMENSION
    {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "Window width and height must be finite, positive, and at most 1000000",
        ));
    }
    Ok(())
}

fn validate_position(x: f64, y: f64) -> Result<(), AdapterError> {
    if !x.is_finite() || !y.is_finite() || x.abs() > MAX_COORDINATE || y.abs() > MAX_COORDINATE {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "Window coordinates must be finite and within -1000000..=1000000",
        ));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn resize(
    handle: super::window_enum::WindowHandle,
    evidence: &WindowIdentityEvidence<'_>,
    width: f64,
    height: f64,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    validate_size(width, height).map_err(before_write)?;
    let cx = round_i32(width).map_err(before_write)?;
    let cy = round_i32(height).map_err(before_write)?;
    ensure_owned_before(evidence)?;
    set_window_pos(handle, None, Some((cx, cy)))?;
    wait_then_reread(deadline)?;
    ensure_owned_after(evidence)?;
    let rect = window_rect(handle)?;
    if within_tolerance(rect.2, cx) && within_tolerance(rect.3, cy) {
        return Ok(());
    }
    Err(placement_unverified("resize"))
}

#[cfg(not(target_os = "windows"))]
fn resize(
    _handle: super::window_enum::WindowHandle,
    _evidence: &WindowIdentityEvidence<'_>,
    width: f64,
    height: f64,
    _deadline: Deadline,
) -> Result<(), AdapterError> {
    validate_size(width, height)?;
    Err(AdapterError::not_supported("window_op"))
}

#[cfg(target_os = "windows")]
fn move_to(
    handle: super::window_enum::WindowHandle,
    evidence: &WindowIdentityEvidence<'_>,
    x: f64,
    y: f64,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    validate_position(x, y).map_err(before_write)?;
    let left = round_i32(x).map_err(before_write)?;
    let top = round_i32(y).map_err(before_write)?;
    ensure_owned_before(evidence)?;
    set_window_pos(handle, Some((left, top)), None)?;
    wait_then_reread(deadline)?;
    ensure_owned_after(evidence)?;
    let rect = window_rect(handle)?;
    if within_tolerance(rect.0, left) && within_tolerance(rect.1, top) {
        return Ok(());
    }
    Err(placement_unverified("move"))
}

#[cfg(not(target_os = "windows"))]
fn move_to(
    _handle: super::window_enum::WindowHandle,
    _evidence: &WindowIdentityEvidence<'_>,
    x: f64,
    y: f64,
    _deadline: Deadline,
) -> Result<(), AdapterError> {
    validate_position(x, y)?;
    Err(AdapterError::not_supported("window_op"))
}

#[derive(Clone, Copy)]
enum ShowVerb {
    Minimize,
    Maximize,
    Restore,
}

#[cfg(target_os = "windows")]
fn show_verb(
    handle: super::window_enum::WindowHandle,
    evidence: &WindowIdentityEvidence<'_>,
    verb: ShowVerb,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE, SW_SHOWMAXIMIZED, SW_SHOWMINIMIZED, SW_SHOWNORMAL,
        ShowWindow,
    };

    let (command, expected) = match verb {
        ShowVerb::Minimize => (SW_MINIMIZE, SW_SHOWMINIMIZED as u32),
        ShowVerb::Maximize => (SW_MAXIMIZE, SW_SHOWMAXIMIZED as u32),
        ShowVerb::Restore => (SW_RESTORE, SW_SHOWNORMAL as u32),
    };
    ensure_owned_before(evidence)?;
    unsafe { ShowWindow(handle, command) };
    wait_then_reread(deadline)?;
    ensure_owned_after(evidence)?;
    let show_cmd = placement_show_cmd(handle)?;
    if show_cmd == expected {
        return Ok(());
    }
    Err(placement_unverified(match verb {
        ShowVerb::Minimize => "minimize",
        ShowVerb::Maximize => "maximize",
        ShowVerb::Restore => "restore",
    }))
}

#[cfg(not(target_os = "windows"))]
fn show_verb(
    _handle: super::window_enum::WindowHandle,
    _evidence: &WindowIdentityEvidence<'_>,
    _verb: ShowVerb,
    _deadline: Deadline,
) -> Result<(), AdapterError> {
    Err(AdapterError::not_supported("window_op"))
}

#[cfg(target_os = "windows")]
fn set_window_pos(
    handle: super::window_enum::WindowHandle,
    origin: Option<(i32, i32)>,
    size: Option<(i32, i32)>,
) -> Result<(), AdapterError> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos,
    };

    let (x, y, move_flag) = match origin {
        Some((x, y)) => (x, y, 0),
        None => (0, 0, SWP_NOMOVE),
    };
    let (cx, cy, size_flag) = match size {
        Some((cx, cy)) => (cx, cy, 0),
        None => (0, 0, SWP_NOSIZE),
    };
    let ok = unsafe {
        SetWindowPos(
            handle,
            std::ptr::null_mut(),
            x,
            y,
            cx,
            cy,
            SWP_NOZORDER | SWP_NOACTIVATE | move_flag | size_flag,
        )
    };
    if ok == 0 {
        return Err(before_write(win32_last_error("SetWindowPos failed")));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn window_rect(
    handle: super::window_enum::WindowHandle,
) -> Result<(i32, i32, i32, i32), AdapterError> {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect;

    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    let ok = unsafe { GetWindowRect(handle, &mut rect) };
    if ok == 0 {
        return Err(after_write(win32_last_error("GetWindowRect failed")));
    }
    Ok((
        rect.left,
        rect.top,
        rect.right - rect.left,
        rect.bottom - rect.top,
    ))
}

#[cfg(target_os = "windows")]
fn placement_show_cmd(handle: super::window_enum::WindowHandle) -> Result<u32, AdapterError> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowPlacement, WINDOWPLACEMENT};

    let mut placement = WINDOWPLACEMENT {
        length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
        flags: 0,
        showCmd: 0,
        ptMinPosition: windows_sys::Win32::Foundation::POINT { x: 0, y: 0 },
        ptMaxPosition: windows_sys::Win32::Foundation::POINT { x: 0, y: 0 },
        rcNormalPosition: windows_sys::Win32::Foundation::RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        },
    };
    let ok = unsafe { GetWindowPlacement(handle, &mut placement) };
    if ok == 0 {
        return Err(after_write(win32_last_error("GetWindowPlacement failed")));
    }
    Ok(placement.showCmd)
}

#[cfg(target_os = "windows")]
fn wait_then_reread(deadline: Deadline) -> Result<(), AdapterError> {
    ensure_budget(deadline).map_err(after_write)?;
    let pause = deadline
        .remaining_slice(std::time::Duration::from_millis(PLACEMENT_REREAD_MS))
        .map_err(after_write)?;
    std::thread::sleep(pause);
    ensure_budget(deadline).map_err(after_write)
}

#[cfg(not(target_os = "windows"))]
fn wait_then_reread(_deadline: Deadline) -> Result<(), AdapterError> {
    Ok(())
}

fn ensure_owned_before(evidence: &WindowIdentityEvidence<'_>) -> Result<(), AdapterError> {
    if !evidence.owns_handle_now().map_err(before_write)? {
        return Err(recycled_before_write());
    }
    Ok(())
}

fn ensure_owned_after(evidence: &WindowIdentityEvidence<'_>) -> Result<(), AdapterError> {
    if !evidence.owns_handle_now().map_err(after_write)? {
        return Err(after_write(AdapterError::new(
            ErrorCode::WindowNotFound,
            "Target window handle changed ownership after window operation",
        )));
    }
    Ok(())
}

fn round_i32(value: f64) -> Result<i32, AdapterError> {
    if !value.is_finite() || value < f64::from(i32::MIN) || value > f64::from(i32::MAX) {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "Window geometry is outside the Win32 integer coordinate range",
        ));
    }
    Ok(value.round() as i32)
}

fn within_tolerance(actual: i32, expected: i32) -> bool {
    actual.abs_diff(expected) <= PLACEMENT_TOLERANCE_PX as u32
}

fn placement_unverified(operation: &str) -> AdapterError {
    after_write(AdapterError::new(
        ErrorCode::ActionFailed,
        format!("Window {operation} did not reach its requested placement"),
    ))
}

fn recycled_before_write() -> AdapterError {
    before_write(AdapterError::new(
        ErrorCode::WindowNotFound,
        "Target window handle changed ownership before window operation",
    ))
}

#[cfg(target_os = "windows")]
fn win32_last_error(message: &str) -> AdapterError {
    use super::process_state::hresult_from_win32;

    let error = unsafe { windows_sys::Win32::Foundation::GetLastError() };
    let hresult = hresult_from_win32(error);
    let record = super::hresult::hresult_record(hresult);
    let mut err = AdapterError::new(record.code, message)
        .with_platform_detail(super::hresult::com_hresult_detail(hresult));
    if let Some(suggestion) = record.suggestion {
        err = err.with_suggestion(suggestion);
    }
    err
}

fn before_write(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::not_delivered())
}

fn after_write(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::delivered_unverified())
}

#[cfg(test)]
#[path = "window_op_tests.rs"]
mod tests;
