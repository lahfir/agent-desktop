use crate::AdAdapter;
use crate::actions::conversion::action_from_c;
use crate::convert::string::{CStrDecodeError, string_to_c, try_c_to_string};
use crate::error::{AdResult, set_last_error};
use crate::ffi_try::trap_panic;
use crate::main_thread::require_main_thread;
use crate::pointer_guard::guard_non_null;
use crate::types::{AdAction, AdPolicyKind};
use agent_desktop_core::action_request::ActionRequest;
use agent_desktop_core::error::{AdapterError, AppError, ErrorCode};
use agent_desktop_core::output::{ErrorPayload, Response};
use agent_desktop_core::refs::validate_ref_id;
use agent_desktop_core::refs_store::RefStore;
use std::ffi::c_char;
use std::ptr;

/// Drives a ref action (`@e5`, action) through the full strict-resolution
/// ladder: `RefStore` load → `RefMap` lookup (→ `STALE_REF` on missing) →
/// `resolve_element_strict` (→ `STALE_REF`/`AMBIGUOUS_TARGET`) → live
/// actionability preflight → dispatch → handle release.
///
/// Policy follows CLI parity (KTD6): `TypeText` actions default to
/// `focus_fallback`; every other action defaults to `headless`. An explicit
/// `policy` discriminant may *elevate* to headed but must not downgrade an
/// action below its CLI base.
///
/// `ref_id` tri-state: null → `ErrInvalidArgs`; non-null invalid UTF-8 →
/// `ErrInvalidArgs`; valid UTF-8 but bad `@e{N}` format → `ErrInvalidArgs`.
///
/// `policy` is an `AdPolicyKind` discriminant (0=Headless, 1=FocusFallback,
/// 2=Headed). An out-of-range value returns `ErrInvalidArgs`. `Headless (0)`
/// accepts the action's own CLI base (so `TypeText` still uses
/// `focus_fallback`). `Headed (2)` opts in to cursor-based fallbacks.
///
/// On success `*out` is set to a NUL-terminated JSON envelope (command
/// `"execute_by_ref"`); free with `ad_free_string`. On error `*out` is
/// zeroed and the last-error slot is populated.
///
/// # Safety
///
/// `adapter` must be a non-null pointer from `ad_adapter_create[_with_session]`.
/// `ref_id` must be null or NUL-terminated within `AD_MAX_STRING_BYTES + 1`
/// bytes. `action` must be a non-null pointer to a valid `AdAction`.
/// `out` must be a non-null writable pointer. All pointers must remain valid
/// for the duration of the call. Must be called from the main thread on macOS.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_execute_by_ref(
    adapter: *const AdAdapter,
    ref_id: *const c_char,
    action: *const AdAction,
    policy: i32,
    out: *mut *mut c_char,
) -> AdResult {
    guard_non_null!(out, c"out is null");
    unsafe { *out = ptr::null_mut() };
    trap_panic(|| {
        if let Err(rc) = require_main_thread() {
            return rc;
        }
        guard_non_null!(adapter, c"adapter is null");
        guard_non_null!(action, c"action is null");

        let ref_str = match unsafe { try_c_to_string(ref_id) } {
            Ok(None) => {
                set_last_error(&AdapterError::new(
                    ErrorCode::InvalidArgs,
                    "ref_id is null — must be a valid @e{N} ref string",
                ));
                return AdResult::ErrInvalidArgs;
            }
            Ok(Some(s)) => s,
            Err(CStrDecodeError::NotUtf8) => {
                set_last_error(&AdapterError::new(
                    ErrorCode::InvalidArgs,
                    "ref_id is not valid UTF-8",
                ));
                return AdResult::ErrInvalidArgs;
            }
            Err(CStrDecodeError::TooLong) => {
                set_last_error(&AdapterError::new(
                    ErrorCode::InvalidArgs,
                    "ref_id exceeds AD_MAX_STRING_BYTES",
                ));
                return AdResult::ErrInvalidArgs;
            }
        };

        if let Err(app_err) = validate_ref_id(&ref_str) {
            let ae = app_error_to_adapter(app_err);
            set_last_error(&ae);
            return AdResult::ErrInvalidArgs;
        }

        let caller_policy = match AdPolicyKind::from_c(policy) {
            Some(p) => p,
            None => {
                set_last_error(&AdapterError::new(
                    ErrorCode::InvalidArgs,
                    "invalid policy kind discriminant",
                ));
                return AdResult::ErrInvalidArgs;
            }
        };

        let core_action = match unsafe { action_from_c(&*action) } {
            Ok(a) => a,
            Err(msg) => {
                set_last_error(&AdapterError::new(ErrorCode::InvalidArgs, msg));
                return AdResult::ErrInvalidArgs;
            }
        };

        let effective_policy = effective_action_policy(&core_action, caller_policy);

        let adapter_ref = unsafe { &*adapter };
        let context = match adapter_ref.command_context() {
            Ok(ctx) => ctx,
            Err(e) => {
                let ae = app_error_to_adapter(e);
                set_last_error(&ae);
                return crate::error::last_error_code();
            }
        };

        let result = run_ref_action(
            adapter_ref,
            &ref_str,
            context.session_id(),
            effective_policy,
            core_action,
        );

        let (envelope, had_error) = match result {
            Ok(data) => (Response::ok("execute_by_ref", data), false),
            Err(app_err) => {
                let payload = ErrorPayload::from_app_error(&app_err);
                let ae = app_error_to_adapter(app_err);
                set_last_error(&ae);
                (Response::err("execute_by_ref", payload), true)
            }
        };

        let json = match serde_json::to_string(&envelope) {
            Ok(s) => s,
            Err(e) => {
                let ae = AdapterError::new(
                    ErrorCode::Internal,
                    format!("failed to serialize execute_by_ref envelope: {e}"),
                );
                set_last_error(&ae);
                return AdResult::ErrInternal;
            }
        };

        let c_ptr = string_to_c(&json);
        if c_ptr.is_null() {
            let ae = AdapterError::new(
                ErrorCode::Internal,
                "execute_by_ref JSON contains interior NUL",
            );
            set_last_error(&ae);
            return AdResult::ErrInternal;
        }

        unsafe { *out = c_ptr };

        if had_error {
            crate::error::last_error_code()
        } else {
            AdResult::Ok
        }
    })
}

/// Loads the `RefEntry` for `ref_id` from the session's `RefStore`, then
/// dispatches the action through `ref_action::execute_entry`.
///
/// Resolution ladder: `RefStore::for_session` → `store.load` →
/// `refmap.get` → `STALE_REF` on miss → `execute_entry` (strict resolve →
/// `STALE_REF`/`AMBIGUOUS_TARGET` → actionability → dispatch).
fn run_ref_action(
    adapter: &AdAdapter,
    ref_id: &str,
    session_id: Option<&str>,
    policy: AdPolicyKind,
    action: agent_desktop_core::action::Action,
) -> Result<serde_json::Value, AppError> {
    let store = RefStore::for_session(session_id)?;
    let refmap = store.load(None)?;
    let entry = match refmap.get(ref_id) {
        Some(e) => e.clone(),
        None => return Err(AppError::stale_ref(ref_id)),
    };
    let request = match policy {
        AdPolicyKind::Headless => ActionRequest::headless(action),
        AdPolicyKind::FocusFallback => ActionRequest::focus_fallback(action),
        AdPolicyKind::Headed => ActionRequest::headed(action),
    };
    let result =
        agent_desktop_core::ref_action::execute_entry(adapter.inner.as_ref(), &entry, request)?;
    Ok(serde_json::to_value(result)?)
}

/// Derive the effective interaction policy for this action (KTD6).
///
/// CLI base policies:
/// - `TypeText` → `focus_fallback` (typing requires focus to land in the
///   right field; headless would fail on unfocused elements).
/// - Everything else → `headless` (pure AX action, no cursor movement).
///
/// The caller-supplied `policy` may *elevate* — passing `Headed` escalates
/// to cursor + OS-input fallbacks — but never downgrades below the action's
/// own base. `Headless (0)` accepts the action's base (so `TypeText` keeps
/// `focus_fallback` rather than being degraded).
fn effective_action_policy(
    action: &agent_desktop_core::action::Action,
    caller: AdPolicyKind,
) -> AdPolicyKind {
    use agent_desktop_core::action::Action;
    let base = match action {
        Action::TypeText(_) => AdPolicyKind::FocusFallback,
        _ => AdPolicyKind::Headless,
    };
    if caller as i32 > base as i32 {
        caller
    } else {
        base
    }
}

fn app_error_to_adapter(err: AppError) -> AdapterError {
    match err {
        AppError::Adapter(e) => e,
        AppError::Io(e) => AdapterError::new(ErrorCode::Internal, e.to_string()),
        AppError::Json(e) => AdapterError::new(ErrorCode::Internal, e.to_string()),
        AppError::Internal(msg) => AdapterError::new(ErrorCode::Internal, msg),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_desktop_core::action::Action;

    #[test]
    fn type_text_base_is_focus_fallback() {
        let result =
            effective_action_policy(&Action::TypeText("hello".into()), AdPolicyKind::Headless);
        assert_eq!(result, AdPolicyKind::FocusFallback);
    }

    #[test]
    fn click_base_is_headless() {
        let result = effective_action_policy(&Action::Click, AdPolicyKind::Headless);
        assert_eq!(result, AdPolicyKind::Headless);
    }

    #[test]
    fn headed_caller_elevates_above_click_base() {
        let result = effective_action_policy(&Action::Click, AdPolicyKind::Headed);
        assert_eq!(result, AdPolicyKind::Headed);
    }

    #[test]
    fn headed_caller_also_elevates_type_text() {
        let result = effective_action_policy(&Action::TypeText("x".into()), AdPolicyKind::Headed);
        assert_eq!(result, AdPolicyKind::Headed);
    }

    #[test]
    fn headless_caller_cannot_downgrade_type_text() {
        let result = effective_action_policy(&Action::TypeText("x".into()), AdPolicyKind::Headless);
        assert_eq!(result, AdPolicyKind::FocusFallback);
    }

    #[test]
    fn focus_fallback_caller_elevates_click() {
        let result = effective_action_policy(&Action::Click, AdPolicyKind::FocusFallback);
        assert_eq!(result, AdPolicyKind::FocusFallback);
    }
}
