use crate::convert::string::c_to_string;
use crate::error::{set_last_error, AdResult};
use crate::ffi_try::trap_panic;
use crate::types::{AdFindQuery, AdWindowInfo};
use crate::AdAdapter;
use agent_desktop_core::adapter::{SnapshotSurface, TreeOptions};
use agent_desktop_core::node::AccessibilityNode;
use std::os::raw::c_char;

/// Checks whether a named boolean state is set on the first element
/// matching `query` inside `win`'s accessibility tree. Intended for the
/// common agent idiom `find → is(focused) → if yes, act`.
///
/// Recognized property names (match the strings the platform adapter
/// emits in `AccessibilityNode.states`):
///
/// - `"focused"`
/// - `"enabled"`
/// - `"selected"`
/// - `"checked"`
/// - `"expanded"`
///
/// Any other property name returns `AD_RESULT_ERR_INVALID_ARGS`. If no
/// element matches the query, returns `AD_RESULT_ERR_ELEMENT_NOT_FOUND`
/// and `*out` is untouched.
///
/// # Safety
/// All pointers must be valid. `property` must be a non-null UTF-8
/// C string. `out` must be a valid writable `*mut bool`; it is set to
/// `false` on entry.
#[no_mangle]
pub unsafe extern "C" fn ad_is(
    adapter: *const AdAdapter,
    win: *const AdWindowInfo,
    query: *const AdFindQuery,
    property: *const c_char,
    out: *mut bool,
) -> AdResult {
    trap_panic(|| unsafe {
        crate::main_thread::debug_assert_main_thread();
        *out = false;
        let adapter = &*adapter;
        let core_win = match crate::windows::ad_window_to_core(&*win) {
            Ok(w) => w,
            Err(e) => {
                set_last_error(&e);
                return crate::error::last_error_code();
            }
        };
        let q = &*query;
        let role_filter = c_to_string(q.role);
        let name_filter = c_to_string(q.name_substring);
        let value_filter = c_to_string(q.value_substring);
        let prop = match c_to_string(property) {
            Some(s) => s,
            None => {
                set_last_error(&agent_desktop_core::error::AdapterError::new(
                    agent_desktop_core::error::ErrorCode::InvalidArgs,
                    "property is null or invalid UTF-8",
                ));
                return AdResult::ErrInvalidArgs;
            }
        };
        if !is_known_property(&prop) {
            set_last_error(&agent_desktop_core::error::AdapterError::new(
                agent_desktop_core::error::ErrorCode::InvalidArgs,
                "unknown property — expected one of: focused, enabled, selected, checked, expanded",
            ));
            return AdResult::ErrInvalidArgs;
        }

        let tree = match adapter.inner.get_tree(
            &core_win,
            &TreeOptions {
                max_depth: 50,
                include_bounds: false,
                interactive_only: false,
                compact: false,
                surface: SnapshotSurface::Window,
            },
        ) {
            Ok(t) => t,
            Err(e) => {
                set_last_error(&e);
                return crate::error::last_error_code();
            }
        };

        let matched = match crate::observation::walk::find_first_match(
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

        *out = element_has_state(matched, &prop);
        AdResult::Ok
    })
}

fn is_known_property(p: &str) -> bool {
    matches!(p, "focused" | "enabled" | "selected" | "checked" | "expanded")
}

fn element_has_state(node: &AccessibilityNode, prop: &str) -> bool {
    node.states.iter().any(|s| s.eq_ignore_ascii_case(prop))
}
