use crate::error::{set_last_error, AdResult};
use crate::ffi_try::trap_panic;
use crate::tree::flatten::flatten_tree;
use crate::types::{AdNodeTree, AdTreeOptions, AdWindowInfo};
use crate::AdAdapter;
use std::ptr;

/// # Safety
/// All pointers must be valid. `out` must be writable.
#[no_mangle]
pub unsafe extern "C" fn ad_get_tree(
    adapter: *const AdAdapter,
    win: *const AdWindowInfo,
    opts: *const AdTreeOptions,
    out: *mut AdNodeTree,
) -> AdResult {
    trap_panic(|| {
        unsafe {
            (*out).nodes = ptr::null_mut();
            (*out).count = 0;
        }

        let adapter = unsafe { &*adapter };
        let opts_ref = unsafe { &*opts };
        let core_win = crate::windows::ad_window_to_core(unsafe { &*win });
        let core_opts = agent_desktop_core::adapter::TreeOptions {
            max_depth: opts_ref.max_depth,
            include_bounds: opts_ref.include_bounds,
            interactive_only: opts_ref.interactive_only,
            compact: opts_ref.compact,
            surface: agent_desktop_core::adapter::SnapshotSurface::Window,
        };

        match adapter.inner.get_tree(&core_win, &core_opts) {
            Ok(tree) => {
                unsafe { *out = flatten_tree(&tree) };
                AdResult::Ok
            }
            Err(e) => {
                set_last_error(&e);
                crate::error::last_error_code()
            }
        }
    })
}
