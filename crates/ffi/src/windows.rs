use crate::convert::{c_to_str, free_window_info_fields, window_info_to_c};
use crate::error::{clear_last_error, set_last_error, AdResult};
use crate::types::{AdWindowInfo, AdWindowOp, AdWindowOpKind};
use crate::AdAdapter;
use agent_desktop_core::action::WindowOp;
use agent_desktop_core::adapter::WindowFilter;
use std::os::raw::c_char;
use std::ptr;

pub(crate) fn ad_window_to_core(w: &AdWindowInfo) -> agent_desktop_core::node::WindowInfo {
    agent_desktop_core::node::WindowInfo {
        id: unsafe { c_to_str(w.id) }.unwrap_or("").to_string(),
        title: unsafe { c_to_str(w.title) }.unwrap_or("").to_string(),
        app: unsafe { c_to_str(w.app_name) }.unwrap_or("").to_string(),
        pid: w.pid,
        bounds: if w.has_bounds {
            Some(agent_desktop_core::node::Rect {
                x: w.bounds.x,
                y: w.bounds.y,
                width: w.bounds.width,
                height: w.bounds.height,
            })
        } else {
            None
        },
        is_focused: w.is_focused,
    }
}

/// # Safety
/// `adapter` must be valid. `out` and `out_count` must be writable.
#[no_mangle]
pub unsafe extern "C" fn ad_list_windows(
    adapter: *const AdAdapter,
    app_filter: *const c_char,
    out: *mut *mut AdWindowInfo,
    out_count: *mut u32,
) -> AdResult {
    *out = ptr::null_mut();
    *out_count = 0;
    let adapter = &*adapter;
    let filter = WindowFilter {
        focused_only: false,
        app: c_to_str(app_filter).map(str::to_string),
    };
    match adapter.inner.list_windows(&filter) {
        Ok(windows) => {
            clear_last_error();
            let c_wins: Vec<AdWindowInfo> = windows.iter().map(window_info_to_c).collect();
            let count = c_wins.len() as u32;
            if c_wins.is_empty() {
                return AdResult::Ok;
            }
            let mut boxed = c_wins.into_boxed_slice();
            *out = boxed.as_mut_ptr();
            *out_count = count;
            std::mem::forget(boxed);
            AdResult::Ok
        }
        Err(e) => {
            set_last_error(&e);
            crate::error::last_error_code()
        }
    }
}

/// # Safety
/// `windows` must be null or from `ad_list_windows`.
#[no_mangle]
pub unsafe extern "C" fn ad_free_windows(windows: *mut AdWindowInfo, count: u32) {
    if windows.is_null() {
        return;
    }
    let slice = std::slice::from_raw_parts_mut(windows, count as usize);
    for w in slice.iter_mut() {
        free_window_info_fields(w);
    }
    drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
        windows,
        count as usize,
    )));
}

/// # Safety
/// `win` must be null or point to a valid `AdWindowInfo`.
#[no_mangle]
pub unsafe extern "C" fn ad_free_window(win: *mut AdWindowInfo) {
    if win.is_null() {
        return;
    }
    free_window_info_fields(&mut *win);
}

/// # Safety
/// `adapter` and `win` must be valid pointers.
#[no_mangle]
pub unsafe extern "C" fn ad_focus_window(
    adapter: *const AdAdapter,
    win: *const AdWindowInfo,
) -> AdResult {
    let adapter = &*adapter;
    let core_win = ad_window_to_core(&*win);
    match adapter.inner.focus_window(&core_win) {
        Ok(()) => {
            clear_last_error();
            AdResult::Ok
        }
        Err(e) => {
            set_last_error(&e);
            crate::error::last_error_code()
        }
    }
}

/// # Safety
/// `adapter` and `win` must be valid pointers.
#[no_mangle]
pub unsafe extern "C" fn ad_window_op(
    adapter: *const AdAdapter,
    win: *const AdWindowInfo,
    op: AdWindowOp,
) -> AdResult {
    let adapter = &*adapter;
    let core_win = ad_window_to_core(&*win);
    let core_op = match op.kind {
        AdWindowOpKind::Resize => WindowOp::Resize {
            width: op.width,
            height: op.height,
        },
        AdWindowOpKind::Move => WindowOp::Move { x: op.x, y: op.y },
        AdWindowOpKind::Minimize => WindowOp::Minimize,
        AdWindowOpKind::Maximize => WindowOp::Maximize,
        AdWindowOpKind::Restore => WindowOp::Restore,
    };
    match adapter.inner.window_op(&core_win, core_op) {
        Ok(()) => {
            clear_last_error();
            AdResult::Ok
        }
        Err(e) => {
            set_last_error(&e);
            crate::error::last_error_code()
        }
    }
}
