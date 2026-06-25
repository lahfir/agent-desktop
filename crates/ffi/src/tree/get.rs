use crate::AdAdapter;
use crate::convert::surface::snapshot_surface_from_c;
use crate::error::{AdResult, set_last_error};
use crate::ffi_try::trap_panic;
use crate::tree::flatten::flatten_tree;
use crate::types::{AdNodeTree, AdTreeOptions, AdWindowInfo};
use std::ptr;

/// Snapshots `win`'s accessibility tree into the flat BFS layout
/// described in the types module. The result is written into `*out`
/// and must be freed with `ad_free_tree`. Direct children of any node
/// live contiguously at `nodes[child_start..child_start + child_count]`.
///
/// `opts.max_depth` caps tree depth. `opts.surface` selects which
/// surface to snapshot (window body, menu, menubar, sheet, popover,
/// alert, or focused subtree); see `AdSnapshotSurface`.
/// `opts.interactive_only` prunes non-interactive nodes; `opts.compact`
/// collapses containers with no semantic payload.
///
/// # Raw-tree contract
///
/// This is a **raw adapter tree** — ref-less, no refmap persistence, and
/// no JSON envelope. Differences the caller must know about:
///
/// - `ref_id` is always null on every `AdNode`. `ref_alloc::allocate_refs`
///   is not run; `@e` ref assignment is a snapshot-pipeline concern.
/// - `include_bounds`, `interactive_only`, and `compact` are honoured via
///   `ref_alloc::transform_tree` after the adapter returns. Because refs are
///   not allocated, the `interactive_only` cut is role-based rather than
///   ref-based; otherwise the semantics match the snapshot path.
/// - No skeleton/drill-down pipeline is wired through — `skeleton` is
///   always false on the underlying `TreeOptions`.
///
/// # When to use this function vs `ad_snapshot`
///
/// **Observe–act agents** that need `@e` refs and refmap persistence should
/// call `ad_snapshot` instead. `ad_snapshot` runs the full snapshot pipeline
/// (ref allocation, refmap write to disk, JSON envelope with
/// `{"version":"2.0","ok":true,...}`) and is the correct starting point for
/// any workflow that drives subsequent `ad_click`, `ad_type_text`, or other
/// ref-based actions.
///
/// Use `ad_get_tree` when you need the raw flat BFS layout without refs —
/// for example, to drive your own traversal logic or to populate a UI
/// inspector that does not use the ref-based action API. For point lookups
/// that bypass tree shape entirely, `ad_find` + `ad_get` / `ad_is` are
/// another alternative.
///
/// On error `*out` is zeroed so `ad_free_tree` on it is a safe no-op.
///
/// # Safety
/// All pointers must be non-null. `win.id` and `win.title` must be
/// valid UTF-8 C strings. `out` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_get_tree(
    adapter: *const AdAdapter,
    win: *const AdWindowInfo,
    opts: *const AdTreeOptions,
    out: *mut AdNodeTree,
) -> AdResult {
    trap_panic(|| {
        crate::pointer_guard::guard_non_null!(out, c"out is null");
        unsafe {
            (*out).nodes = ptr::null_mut();
            (*out).count = 0;
        }
        if let Err(rc) = crate::main_thread::require_main_thread() {
            return rc;
        }
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        crate::pointer_guard::guard_non_null!(win, c"win is null");
        crate::pointer_guard::guard_non_null!(opts, c"opts is null");

        let adapter = unsafe { &*adapter };
        let opts_ref = unsafe { &*opts };
        let core_win = match crate::windows::ad_window_to_core(unsafe { &*win }) {
            Ok(w) => w,
            Err(e) => {
                set_last_error(&e);
                return crate::error::last_error_code();
            }
        };
        let surface = match snapshot_surface_from_c(opts_ref.surface, "snapshot surface") {
            Ok(surface) => surface,
            Err(e) => {
                set_last_error(&e);
                return AdResult::ErrInvalidArgs;
            }
        };
        let core_opts = agent_desktop_core::adapter::TreeOptions {
            max_depth: opts_ref.max_depth,
            include_bounds: opts_ref.include_bounds,
            interactive_only: opts_ref.interactive_only,
            compact: opts_ref.compact,
            surface,
            skeleton: false,
        };

        match adapter.inner.get_tree(&core_win, &core_opts) {
            Ok(tree) => {
                let shaped = agent_desktop_core::ref_alloc::transform_tree(
                    tree,
                    core_opts.include_bounds,
                    core_opts.interactive_only,
                    core_opts.compact,
                );
                unsafe { *out = flatten_tree(&shaped) };
                AdResult::Ok
            }
            Err(e) => {
                set_last_error(&e);
                crate::error::last_error_code()
            }
        }
    })
}
