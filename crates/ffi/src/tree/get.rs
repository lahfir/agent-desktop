use crate::enum_validation::enum_raw_i32;
use crate::error::{set_last_error, AdResult};
use crate::ffi_try::trap_panic;
use crate::tree::flatten::flatten_tree;
use crate::types::{AdNodeTree, AdSnapshotSurface, AdTreeOptions, AdWindowInfo};
use crate::AdAdapter;
use agent_desktop_core::adapter::SnapshotSurface;
use std::ptr;

fn core_surface(s: AdSnapshotSurface) -> SnapshotSurface {
    match s {
        AdSnapshotSurface::Window => SnapshotSurface::Window,
        AdSnapshotSurface::Focused => SnapshotSurface::Focused,
        AdSnapshotSurface::Menu => SnapshotSurface::Menu,
        AdSnapshotSurface::Menubar => SnapshotSurface::Menubar,
        AdSnapshotSurface::Sheet => SnapshotSurface::Sheet,
        AdSnapshotSurface::Popover => SnapshotSurface::Popover,
        AdSnapshotSurface::Alert => SnapshotSurface::Alert,
    }
}

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
        crate::main_thread::debug_assert_main_thread();
        unsafe {
            (*out).nodes = ptr::null_mut();
            (*out).count = 0;
        }

        let adapter = unsafe { &*adapter };
        let opts_ref = unsafe { &*opts };
        let core_win = match crate::windows::ad_window_to_core(unsafe { &*win }) {
            Ok(w) => w,
            Err(e) => {
                set_last_error(&e);
                return crate::error::last_error_code();
            }
        };
        let surface = match AdSnapshotSurface::from_c(enum_raw_i32(&opts_ref.surface)) {
            Some(s) => core_surface(s),
            None => {
                set_last_error(&agent_desktop_core::error::AdapterError::new(
                    agent_desktop_core::error::ErrorCode::InvalidArgs,
                    "invalid snapshot surface discriminant",
                ));
                return AdResult::ErrInvalidArgs;
            }
        };
        let core_opts = agent_desktop_core::adapter::TreeOptions {
            max_depth: opts_ref.max_depth,
            include_bounds: opts_ref.include_bounds,
            interactive_only: opts_ref.interactive_only,
            compact: opts_ref.compact,
            surface,
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
