use crate::AdAdapter;
use crate::actions::conversion::action_from_c;
use crate::commands::app_error_to_adapter;
use crate::commands::envelope_out::write_command_envelope;
use crate::convert::string::{optional_adapter_string, required_adapter_string};
use crate::error::{AdResult, set_last_error};
use crate::ffi_try::trap_panic;
use crate::main_thread::require_main_thread;
use crate::pointer_guard::guard_non_null;
use crate::types::{AdAction, AdPolicyKind};
use agent_desktop_core::error::{AdapterError, ErrorCode};
use agent_desktop_core::interaction_policy::InteractionPolicy;
use agent_desktop_core::refs::validate_ref_id;
use std::ffi::c_char;
use std::ptr;

/// Drives a ref action (`@e5`, action) through the canonical ref-action
/// pipeline: `RefStore` load → `RefMap` lookup (→ `STALE_REF` on missing) →
/// strict element resolution (→ `STALE_REF`/`AMBIGUOUS_TARGET`) → live
/// actionability preflight → dispatch → handle release.
///
/// Policy: `TypeText` defaults to `focus_fallback` (matching the CLI `type`
/// command); `PressKey` shares that `focus_fallback` base (a ref-targeted key
/// press may need the target focused); every other action defaults to
/// `headless`. An explicit `policy` discriminant may *elevate* to headed but
/// must not downgrade an action below its base. Base and elevation are computed
/// by `agent_desktop_core::commands::execute_by_ref::execute` via
/// `Action::base_interaction_policy` + `InteractionPolicy::join`, so CLI and
/// FFI share a single source of policy truth.
///
/// `ref_id` tri-state: null → `ErrInvalidArgs`; non-null invalid UTF-8 →
/// `ErrInvalidArgs`; valid UTF-8 but bad `@e{N}` format → `ErrInvalidArgs`.
///
/// `snapshot_id` tri-state: null → use the latest snapshot for the session
/// (CLI `--snapshot` omitted); valid UTF-8 → pin to that snapshot id; non-null
/// invalid UTF-8 → `ErrInvalidArgs`.
///
/// `policy` is an `AdPolicyKind` discriminant (0=Headless, 1=FocusFallback,
/// 2=Headed). An out-of-range value returns `ErrInvalidArgs`. `Headless (0)`
/// accepts the action's own CLI base (so `TypeText` still uses
/// `focus_fallback`). `Headed (2)` opts in to cursor-based fallbacks.
///
/// On success `*out` is set to a NUL-terminated JSON envelope (command
/// `"execute_by_ref"`); free with `ad_free_string`. On guard or decode
/// failure (invalid args before the command runs) `*out` remains null.
/// On a command-level error (STALE_REF, AMBIGUOUS_TARGET, etc.) `*out`
/// holds the error JSON envelope and must still be freed with
/// `ad_free_string`. The last-error slot is populated on all failures.
///
/// **Dispatch-before-serialize ordering**: the action is dispatched (and any
/// side effects committed) before the result JSON is serialized. In the
/// near-impossible event that serialization of an already-valid
/// `ActionResult` fails, `*out` is null and `ErrInternal` is returned while
/// the side effect has already occurred. No pre-validation machinery is
/// needed because serialization of a valid envelope effectively never fails.
///
/// # Safety
///
/// `adapter` must be a non-null pointer from `ad_adapter_create[_with_session]`.
/// `ref_id` must be null or NUL-terminated within `AD_MAX_STRING_BYTES + 1`
/// bytes. `snapshot_id` must be null or NUL-terminated within
/// `AD_MAX_STRING_BYTES + 1` bytes. `action` must be a non-null pointer to a
/// valid `AdAction`. `out` must be a non-null writable pointer. All pointers
/// must remain valid for the duration of the call. Must be called from the
/// main thread on macOS.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_execute_by_ref(
    adapter: *const AdAdapter,
    ref_id: *const c_char,
    snapshot_id: *const c_char,
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

        let ref_str = match required_adapter_string(ref_id, "ref_id") {
            Ok(s) => s,
            Err(e) => {
                set_last_error(&e);
                return AdResult::ErrInvalidArgs;
            }
        };

        if let Err(app_err) = validate_ref_id(&ref_str) {
            let ae = app_error_to_adapter(app_err);
            set_last_error(&ae);
            return crate::error::last_error_code();
        }

        let snapshot_str = match optional_adapter_string(snapshot_id, "snapshot_id") {
            Ok(opt) => opt,
            Err(e) => {
                set_last_error(&e);
                return AdResult::ErrInvalidArgs;
            }
        };

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

        let caller_ip = policy_kind_to_interaction_policy(caller_policy);

        let adapter_ref = unsafe { &*adapter };
        let context = match adapter_ref.command_context() {
            Ok(ctx) => ctx,
            Err(e) => {
                let ae = app_error_to_adapter(e);
                set_last_error(&ae);
                return crate::error::last_error_code();
            }
        };

        let result = agent_desktop_core::commands::execute_by_ref::execute(
            &ref_str,
            snapshot_str.as_deref(),
            core_action,
            caller_ip,
            adapter_ref.inner.as_ref(),
            &context,
        );

        unsafe { write_command_envelope("execute_by_ref", result, out) }
    })
}

fn policy_kind_to_interaction_policy(kind: AdPolicyKind) -> InteractionPolicy {
    match kind {
        AdPolicyKind::Headless => InteractionPolicy::headless(),
        AdPolicyKind::FocusFallback => InteractionPolicy::focus_fallback(),
        AdPolicyKind::Headed => InteractionPolicy::headed(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_desktop_core::action::Action;

    #[test]
    fn policy_kind_headless_maps_to_headless() {
        assert_eq!(
            policy_kind_to_interaction_policy(AdPolicyKind::Headless),
            InteractionPolicy::headless()
        );
    }

    #[test]
    fn policy_kind_focus_fallback_maps_to_focus_fallback() {
        assert_eq!(
            policy_kind_to_interaction_policy(AdPolicyKind::FocusFallback),
            InteractionPolicy::focus_fallback()
        );
    }

    #[test]
    fn policy_kind_headed_maps_to_headed() {
        assert_eq!(
            policy_kind_to_interaction_policy(AdPolicyKind::Headed),
            InteractionPolicy::headed()
        );
    }

    #[test]
    fn type_text_base_plus_headless_caller_gives_focus_fallback() {
        let base = Action::TypeText("hi".into()).base_interaction_policy();
        let effective = base.join(InteractionPolicy::headless());
        assert_eq!(effective, InteractionPolicy::focus_fallback());
    }

    #[test]
    fn click_base_plus_headless_caller_stays_headless() {
        let base = Action::Click.base_interaction_policy();
        let effective = base.join(InteractionPolicy::headless());
        assert_eq!(effective, InteractionPolicy::headless());
    }

    #[test]
    fn click_base_plus_headed_caller_becomes_headed() {
        let base = Action::Click.base_interaction_policy();
        let effective = base.join(InteractionPolicy::headed());
        assert_eq!(effective, InteractionPolicy::headed());
    }

    #[test]
    fn type_text_base_plus_headed_caller_becomes_headed() {
        let base = Action::TypeText("x".into()).base_interaction_policy();
        let effective = base.join(InteractionPolicy::headed());
        assert_eq!(effective, InteractionPolicy::headed());
    }

    #[test]
    fn headless_caller_cannot_downgrade_type_text_base() {
        let base = Action::TypeText("x".into()).base_interaction_policy();
        assert_eq!(base, InteractionPolicy::focus_fallback());
        let effective = base.join(InteractionPolicy::headless());
        assert_eq!(effective, InteractionPolicy::focus_fallback());
    }

    #[test]
    fn click_base_plus_focus_fallback_caller_gives_focus_fallback() {
        let base = Action::Click.base_interaction_policy();
        let effective = base.join(InteractionPolicy::focus_fallback());
        assert_eq!(effective, InteractionPolicy::focus_fallback());
    }
}
