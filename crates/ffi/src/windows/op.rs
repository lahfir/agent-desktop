use crate::error::{set_last_error, AdResult};
use crate::ffi_try::trap_panic;
use crate::types::{AdWindowInfo, AdWindowOp, AdWindowOpKind};
use crate::windows::to_core::ad_window_to_core;
use crate::AdAdapter;
use agent_desktop_core::action::WindowOp;

/// Performs a window-manager operation (`Resize`, `Move`, `Minimize`,
/// `Maximize`, `Restore`) on `win`. Width / height / x / y are consulted
/// only for the variants that use them; other kinds ignore them.
///
/// An invalid `op.kind` discriminant is rejected with
/// `AD_RESULT_ERR_INVALID_ARGS` before any adapter call.
///
/// # Safety
/// `adapter` and `win` must be non-null pointers. `win.id` and
/// `win.title` must be non-null valid UTF-8 C strings.
#[no_mangle]
pub unsafe extern "C" fn ad_window_op(
    adapter: *const AdAdapter,
    win: *const AdWindowInfo,
    op: AdWindowOp,
) -> AdResult {
    trap_panic(|| unsafe {
        if let Err(rc) = crate::main_thread::require_main_thread() {
            return rc;
        }
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        crate::pointer_guard::guard_non_null!(win, c"win is null");
        let adapter = &*adapter;
        let core_win = match ad_window_to_core(&*win) {
            Ok(w) => w,
            Err(e) => {
                set_last_error(&e);
                return crate::error::last_error_code();
            }
        };
        let kind = match AdWindowOpKind::from_c(op.kind) {
            Some(k) => k,
            None => {
                set_last_error(&agent_desktop_core::error::AdapterError::new(
                    agent_desktop_core::error::ErrorCode::InvalidArgs,
                    "invalid window op kind discriminant",
                ));
                return AdResult::ErrInvalidArgs;
            }
        };
        let core_op = match kind {
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
            Ok(()) => AdResult::Ok,
            Err(e) => {
                set_last_error(&e);
                crate::error::last_error_code()
            }
        }
    })
}
