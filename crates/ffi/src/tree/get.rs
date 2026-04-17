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
/// This is a **raw adapter tree**, not the snapshot the CLI `snapshot`
/// subcommand returns. Differences the caller must know about:
///
/// - `ref_id` is always null on every `AdNode`. The FFI surface does
///   not run `ref_alloc::allocate_refs`; refs are a CLI/JSON pipeline
///   concern, so agent-facing code that needs them should drive them
///   externally (resolve via `ad_find` + `ad_free_handle`, or call the
///   CLI if refs are required).
/// - `include_bounds`, `interactive_only`, and `compact` are honored
///   after the adapter returns the raw tree, using
///   `ref_alloc::transform_tree`. Because refs are not allocated here,
///   the `interactive_only` cut is role-based rather than ref-based;
///   otherwise the semantics match the CLI snapshot path.
/// - No skeleton/drill-down pipeline is wired through — `skeleton` is
///   always false on the underlying `TreeOptions`.
///
/// If parity with the CLI snapshot is important to your consumer,
/// either use `ad_find` + `ad_get` / `ad_is` for point lookups (which
/// bypass tree shape entirely) or invoke the CLI binary for the
/// snapshot call. A future revision may layer a "normalized snapshot"
/// FFI function on top of this raw path.
///
/// On error `*out` is zeroed so `ad_free_tree` on it is a safe no-op.
///
/// # Safety
/// All pointers must be non-null. `win.id` and `win.title` must be
/// valid UTF-8 C strings. `out` must be writable.
#[no_mangle]
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
        let surface = match AdSnapshotSurface::from_c(opts_ref.surface) {
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
            skeleton: false,
        };

        match adapter.inner.get_tree(&core_win, &core_opts) {
            Ok(tree) => {
                // Adapters return a full raw tree; flags live on the core
                // TreeOptions but the macOS adapter only consumes
                // `max_depth` and `skeleton`. Apply shape transformations
                // here so the FFI behavior matches what AdTreeOptions
                // documents. ref_alloc::transform_tree is the ref-free
                // variant of allocate_refs and matches its semantics for
                // compact/interactive_only/include_bounds.
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
