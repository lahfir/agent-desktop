use crate::error::{clear_last_error, set_last_error, AdResult};
use crate::ffi_try::trap_panic;
use crate::types::{AdWindowInfo, AdWindowOp, AdWindowOpKind};
use crate::windows::to_core::ad_window_to_core;
use crate::AdAdapter;
use agent_desktop_core::action::WindowOp;

/// # Safety
/// `adapter` and `win` must be valid pointers.
#[no_mangle]
pub unsafe extern "C" fn ad_window_op(
    adapter: *const AdAdapter,
    win: *const AdWindowInfo,
    op: AdWindowOp,
) -> AdResult {
    trap_panic(|| unsafe {
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
    })
}
