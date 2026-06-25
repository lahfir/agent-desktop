use crate::AdAdapter;
use crate::actions::conversion::action_from_c;
use crate::actions::result::action_result_to_c;
use crate::error::{self, AdResult};
use crate::ffi_try::trap_panic;
use crate::types::{AdAction, AdActionResult, AdNativeHandle, AdPolicyKind, AdRefEntry};
use agent_desktop_core::{action::Action, action_request::ActionRequest, adapter::NativeHandle};

/// Low-level native-handle action. This does not perform strict ref
/// re-identification or actionability preflight; callers that want CLI parity
/// should use `ad_execute_ref_action_with_policy`.
///
/// # Safety
///
/// `adapter` must be a non-null pointer returned by `ad_adapter_create`.
/// `handle` must be a non-null pointer to a valid `AdNativeHandle` produced by
/// the same live adapter. Free the handle before destroying that adapter.
/// `action` must be a non-null pointer to a valid `AdAction`.
/// `out` must be a non-null pointer to an `AdActionResult` to write the result into.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_execute_action(
    adapter: *const AdAdapter,
    handle: *const AdNativeHandle,
    action: *const AdAction,
    out: *mut AdActionResult,
) -> AdResult {
    unsafe {
        ad_execute_action_with_policy(adapter, handle, action, AdPolicyKind::Headless as i32, out)
    }
}

/// Low-level native-handle action with explicit interaction policy. This does
/// not perform strict ref re-identification or actionability preflight; callers
/// that want CLI parity should use `ad_execute_ref_action_with_policy`.
///
/// # Safety
///
/// `adapter` must be a non-null pointer returned by `ad_adapter_create`.
/// `handle` must be a non-null pointer to a valid `AdNativeHandle` produced by
/// the same live adapter. Free the handle before destroying that adapter.
/// `action` must be a non-null pointer to a valid `AdAction`.
/// `out` must be a non-null pointer to an `AdActionResult` to write the result into.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_execute_action_with_policy(
    adapter: *const AdAdapter,
    handle: *const AdNativeHandle,
    action: *const AdAction,
    policy: i32,
    out: *mut AdActionResult,
) -> AdResult {
    trap_panic(|| unsafe {
        crate::pointer_guard::guard_non_null!(out, c"out is null");
        *out = std::mem::zeroed();
        if let Err(rc) = crate::main_thread::require_main_thread() {
            return rc;
        }
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        crate::pointer_guard::guard_non_null!(handle, c"handle is null");
        crate::pointer_guard::guard_non_null!(action, c"action is null");
        let adapter = &*adapter;
        let handle_ref = &*handle;
        if handle_ref.ptr.is_null() {
            error::set_last_error(&agent_desktop_core::error::AdapterError::new(
                agent_desktop_core::error::ErrorCode::InvalidArgs,
                "handle.ptr is null — the handle has already been freed or was never resolved",
            ));
            return AdResult::ErrInvalidArgs;
        }
        let core_action = match decode_action(&*action) {
            Ok(action) => action,
            Err(result) => return result,
        };
        let native_handle = NativeHandle::from_ptr(handle_ref.ptr);
        let policy = match decode_policy(policy) {
            Ok(policy) => policy,
            Err(result) => return result,
        };
        let request = action_request(policy, core_action);
        match adapter.inner.execute_action(&native_handle, request) {
            Ok(result) => {
                *out = action_result_to_c(&result);
                AdResult::Ok
            }
            Err(e) => {
                error::set_last_error(&e);
                error::last_error_code()
            }
        }
    })
}

/// Strict ref action path matching CLI semantics: resolve the full ref
/// identity, run actionability preflight, then dispatch using the requested
/// policy.
///
/// # Safety
///
/// `adapter` must be a non-null pointer returned by `ad_adapter_create`.
/// `entry` must be a non-null pointer to a valid `AdRefEntry`.
/// `action` must be a non-null pointer to a valid `AdAction`.
/// `out` must be a non-null pointer to an `AdActionResult` to write the result into.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_execute_ref_action_with_policy(
    adapter: *const AdAdapter,
    entry: *const AdRefEntry,
    action: *const AdAction,
    policy: i32,
    out: *mut AdActionResult,
) -> AdResult {
    trap_panic(|| unsafe {
        crate::pointer_guard::guard_non_null!(out, c"out is null");
        *out = std::mem::zeroed();
        if let Err(rc) = crate::main_thread::require_main_thread() {
            return rc;
        }
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        crate::pointer_guard::guard_non_null!(entry, c"entry is null");
        crate::pointer_guard::guard_non_null!(action, c"action is null");
        let adapter = &*adapter;
        let entry_ref = &*entry;
        let core_entry = match crate::actions::resolve::core_ref_entry_from_ffi(entry_ref) {
            Ok(entry) => entry,
            Err(err) => {
                error::set_last_error(&err);
                return error::last_error_code();
            }
        };
        let core_action = match decode_action(&*action) {
            Ok(action) => action,
            Err(result) => return result,
        };
        let policy = match decode_policy(policy) {
            Ok(policy) => policy,
            Err(result) => return result,
        };
        let request = action_request(policy, core_action);
        match agent_desktop_core::ref_action::execute_entry(
            adapter.inner.as_ref(),
            &core_entry,
            request,
        ) {
            Ok(result) => {
                *out = action_result_to_c(&result);
                AdResult::Ok
            }
            Err(err) => {
                error::set_last_error(&err);
                error::last_error_code()
            }
        }
    })
}

fn decode_action(action: &AdAction) -> Result<Action, AdResult> {
    unsafe { action_from_c(action) }.map_err(|msg| {
        error::set_last_error(&agent_desktop_core::error::AdapterError::new(
            agent_desktop_core::error::ErrorCode::InvalidArgs,
            msg,
        ));
        AdResult::ErrInvalidArgs
    })
}

fn decode_policy(policy: i32) -> Result<AdPolicyKind, AdResult> {
    AdPolicyKind::from_c(policy).ok_or_else(|| {
        error::set_last_error(&agent_desktop_core::error::AdapterError::new(
            agent_desktop_core::error::ErrorCode::InvalidArgs,
            "invalid policy kind discriminant",
        ));
        AdResult::ErrInvalidArgs
    })
}

fn action_request(policy: AdPolicyKind, action: Action) -> ActionRequest {
    match policy {
        AdPolicyKind::Headless => ActionRequest::headless(action),
        AdPolicyKind::FocusFallback => ActionRequest::focus_fallback(action),
        AdPolicyKind::Headed => ActionRequest::headed(action),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_desktop_core::{action::Action, interaction_policy::InteractionPolicy};

    #[test]
    fn ffi_policy_kind_maps_to_core_interaction_policy() {
        assert_eq!(
            action_request(AdPolicyKind::Headless, Action::Click).policy,
            InteractionPolicy::headless()
        );
        assert_eq!(
            action_request(AdPolicyKind::FocusFallback, Action::Click).policy,
            InteractionPolicy::focus_fallback()
        );
        assert_eq!(
            action_request(AdPolicyKind::Headed, Action::Click).policy,
            InteractionPolicy::headed()
        );
    }
}
