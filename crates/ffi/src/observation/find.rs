use crate::convert::string::decode_optional_filter;
use crate::error::{set_last_error, AdResult};
use crate::ffi_try::trap_panic;
use crate::observation::walk::find_first_match;
use crate::types::{AdFindQuery, AdNativeHandle, AdWindowInfo};
use crate::AdAdapter;
use agent_desktop_core::adapter::{SnapshotSurface, TreeOptions};
use agent_desktop_core::refs::RefEntry;

/// Finds the first element in `win`'s accessibility tree matching the
/// query and resolves it to an opaque `AdNativeHandle`. The caller owns
/// the handle and must release it with `ad_free_handle(adapter, handle)`
/// once done.
///
/// Matching is DFS order, first hit wins. All query fields are optional
/// (null = "don't care") and case-insensitive substring matches:
/// - `role` against `AccessibilityNode.role`
/// - `name_substring` against `AccessibilityNode.name`
/// - `value_substring` against `AccessibilityNode.value`
///
/// # Safety
/// `adapter`, `win`, and `query` must be valid pointers. `out_handle`
/// must be a valid writable `*mut AdNativeHandle`. On
/// `AD_RESULT_ERR_ELEMENT_NOT_FOUND` the out-handle is zero-initialized.
#[no_mangle]
pub unsafe extern "C" fn ad_find(
    adapter: *const AdAdapter,
    win: *const AdWindowInfo,
    query: *const AdFindQuery,
    out_handle: *mut AdNativeHandle,
) -> AdResult {
    trap_panic(|| unsafe {
        crate::pointer_guard::guard_non_null!(out_handle, c"out_handle is null");
        (*out_handle).ptr = std::ptr::null();
        if let Err(rc) = crate::main_thread::require_main_thread() {
            return rc;
        }
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        crate::pointer_guard::guard_non_null!(win, c"win is null");
        crate::pointer_guard::guard_non_null!(query, c"query is null");
        let adapter = &*adapter;
        let core_win = match crate::windows::ad_window_to_core(&*win) {
            Ok(w) => w,
            Err(e) => {
                set_last_error(&e);
                return crate::error::last_error_code();
            }
        };
        let q = &*query;
        let role_filter = decode_optional_filter!(q.role, "query.role");
        let name_filter = decode_optional_filter!(q.name_substring, "query.name_substring");
        let value_filter = decode_optional_filter!(q.value_substring, "query.value_substring");

        // include_bounds must be true: the resolver disambiguates
        // duplicate-label siblings using bounds_hash, and without the
        // bounds field populated on the matched node we would fall
        // back to role+name alone and drift to the wrong element.
        let tree = match adapter.inner.get_tree(
            &core_win,
            &TreeOptions {
                max_depth: 50,
                include_bounds: true,
                interactive_only: false,
                compact: false,
                surface: SnapshotSurface::Window,
                skeleton: false,
            },
        ) {
            Ok(t) => t,
            Err(e) => {
                set_last_error(&e);
                return crate::error::last_error_code();
            }
        };

        let matched = match find_first_match(
            &tree,
            role_filter.as_deref(),
            name_filter.as_deref(),
            value_filter.as_deref(),
        ) {
            Some(n) => n,
            None => {
                set_last_error(&agent_desktop_core::error::AdapterError::new(
                    agent_desktop_core::error::ErrorCode::ElementNotFound,
                    "no element matched the find query",
                ));
                return AdResult::ErrElementNotFound;
            }
        };

        let bounds_hash = matched.bounds.as_ref().map(|r| r.bounds_hash());
        let ref_entry = RefEntry {
            pid: core_win.pid,
            role: matched.role.clone(),
            name: matched.name.clone(),
            value: matched.value.clone(),
            states: matched.states.clone(),
            bounds: matched.bounds,
            bounds_hash,
            available_actions: Vec::new(),
            source_app: None,
            root_ref: None,
        };
        match adapter.inner.resolve_element(&ref_entry) {
            Ok(handle) => {
                (*out_handle).ptr = handle.as_raw();
                AdResult::Ok
            }
            Err(e) => {
                set_last_error(&e);
                crate::error::last_error_code()
            }
        }
    })
}
