use crate::convert::string::{c_to_string, decode_optional_filter};
use crate::error::{set_last_error, AdResult};
use crate::ffi_try::trap_panic;
use crate::types::{AdFindQuery, AdWindowInfo};
use crate::AdAdapter;
use agent_desktop_core::adapter::{SnapshotSurface, TreeOptions};
use agent_desktop_core::node::AccessibilityNode;
use std::os::raw::c_char;

/// Checks whether a named boolean state is set on the first element
/// matching `query` inside `win`'s accessibility tree. Intended for
/// the common agent idiom `find → is("focused") → if yes, act`.
///
/// Supported property names reflect the strings the macOS tree
/// builder actually emits in `AccessibilityNode.states`:
///
/// - `"focused"` — true when the node carries the `focused` state.
/// - `"disabled"` — true when the adapter surfaced `disabled`.
/// - `"enabled"` — derived: true iff `disabled` is NOT present. There
///   is no `enabled` string in the adapter output; asking for it
///   returns the logical negation so agents don't have to invert
///   themselves.
///
/// `"selected"`, `"checked"`, and `"expanded"` are not currently
/// emitted by any platform adapter; asking for them returns
/// `AD_RESULT_ERR_INVALID_ARGS` with a diagnostic last-error rather
/// than silently answering `false`. The set will widen as adapters
/// grow support; future additions stay backwards-compatible
/// (unknown → InvalidArgs, known → deterministic answer).
///
/// On entry `*out` is always cleared to `false` so a caller inspecting
/// the slot after an error sees a predictable sentinel, not whatever
/// was there before. If the query matches nothing, returns
/// `AD_RESULT_ERR_ELEMENT_NOT_FOUND` with `*out` still `false`.
///
/// # Safety
/// All pointers must be valid. `property` must be a non-null UTF-8
/// C string. `out` must be a valid writable `*mut bool`.
#[no_mangle]
pub unsafe extern "C" fn ad_is(
    adapter: *const AdAdapter,
    win: *const AdWindowInfo,
    query: *const AdFindQuery,
    property: *const c_char,
    out: *mut bool,
) -> AdResult {
    trap_panic(|| unsafe {
        crate::pointer_guard::guard_non_null!(out, c"out is null");
        *out = false;
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
        let property = match SupportedProperty::from_name(&prop) {
            Some(p) => p,
            None => {
                set_last_error(&agent_desktop_core::error::AdapterError::new(
                    agent_desktop_core::error::ErrorCode::InvalidArgs,
                    "unknown property — expected one of: focused, disabled, enabled",
                ));
                return AdResult::ErrInvalidArgs;
            }
        };

        let tree = match adapter.inner.get_tree(
            &core_win,
            &TreeOptions {
                max_depth: 50,
                include_bounds: false,
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

        *out = property.evaluate(matched);
        AdResult::Ok
    })
}

/// Compile-time set of property names `ad_is` answers. Each variant
/// encodes how the answer is derived from `AccessibilityNode.states`,
/// which keeps the documented contract and the implementation in
/// lockstep — an addition here is the only way to add a supported name.
enum SupportedProperty {
    Focused,
    Disabled,
    EnabledDerivedFromDisabled,
}

impl SupportedProperty {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "focused" => Some(SupportedProperty::Focused),
            "disabled" => Some(SupportedProperty::Disabled),
            "enabled" => Some(SupportedProperty::EnabledDerivedFromDisabled),
            _ => None,
        }
    }

    fn evaluate(&self, node: &AccessibilityNode) -> bool {
        match self {
            SupportedProperty::Focused => has_state(node, "focused"),
            SupportedProperty::Disabled => has_state(node, "disabled"),
            SupportedProperty::EnabledDerivedFromDisabled => !has_state(node, "disabled"),
        }
    }
}

fn has_state(node: &AccessibilityNode, state: &str) -> bool {
    node.states.iter().any(|s| s.eq_ignore_ascii_case(state))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node_with_states(states: &[&str]) -> AccessibilityNode {
        AccessibilityNode {
            ref_id: None,
            role: "button".into(),
            name: None,
            value: None,
            description: None,
            hint: None,
            states: states.iter().map(|s| s.to_string()).collect(),
            bounds: None,
            children: vec![],
            children_count: None,
        }
    }

    #[test]
    fn focused_mirrors_state_presence() {
        let prop = SupportedProperty::from_name("focused").unwrap();
        assert!(prop.evaluate(&node_with_states(&["focused"])));
        assert!(!prop.evaluate(&node_with_states(&[])));
    }

    #[test]
    fn disabled_mirrors_state_presence() {
        let prop = SupportedProperty::from_name("disabled").unwrap();
        assert!(prop.evaluate(&node_with_states(&["disabled"])));
        assert!(!prop.evaluate(&node_with_states(&["focused"])));
    }

    #[test]
    fn enabled_is_derived_negation_of_disabled() {
        let prop = SupportedProperty::from_name("enabled").unwrap();
        assert!(!prop.evaluate(&node_with_states(&["disabled"])));
        assert!(prop.evaluate(&node_with_states(&[])));
        assert!(prop.evaluate(&node_with_states(&["focused"])));
    }

    #[test]
    fn unsupported_names_do_not_resolve() {
        assert!(SupportedProperty::from_name("selected").is_none());
        assert!(SupportedProperty::from_name("checked").is_none());
        assert!(SupportedProperty::from_name("expanded").is_none());
        assert!(SupportedProperty::from_name("bogus").is_none());
    }
}
