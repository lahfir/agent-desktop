//! @generated — produced by crates/ffi/build.rs codegen.
//! Edit the templates under crates/ffi/codegen_templates/, not this file.
//! Commands in alphabetical order: execute_by_ref, snapshot, status, trace_export, trace_show, version, wait.

use crate::AdAdapter;
use crate::actions::conversion::action_from_c;
use crate::commands::app_error_to_adapter;
use crate::commands::envelope_out::write_command_envelope;
use crate::convert::string::{
    decode_optional_filter, optional_adapter_string, required_adapter_string,
};
use crate::convert::surface::snapshot_surface_from_c;
use crate::error::{self, AdResult, set_last_error};
use crate::ffi_try::trap_panic;
use crate::main_thread::require_main_thread;
use crate::pointer_guard::guard_non_null;
use crate::types::wait_args::AdWaitArgs;
use crate::types::{AdAction, AdPolicyKind};
use agent_desktop_core::commands::snapshot::SnapshotArgs;
use agent_desktop_core::commands::status::execute_with_report_with_context;
use agent_desktop_core::commands::wait::{WaitArgs, WaitModeArgs, WaitPredicateArgs};
use agent_desktop_core::error::{AdapterError, AppError, ErrorCode};
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
/// `ref_id` must be a non-null pointer to a NUL-terminated C string within
/// `AD_MAX_STRING_BYTES + 1` bytes; null is **not** optional — it is defined
/// behaviour (no UB) but is rejected immediately with `ErrInvalidArgs`.
/// `snapshot_id` may be null (meaning: use the latest snapshot for this
/// session) or a non-null NUL-terminated C string within
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

        let caller_ip = caller_policy.to_interaction_policy();

        let adapter_ref = unsafe { &*adapter };
        let context = match adapter_ref.command_context() {
            Ok(ctx) => ctx,
            Err(e) => {
                let ae = app_error_to_adapter(e);
                set_last_error(&ae);
                return crate::error::last_error_code();
            }
        };

        let scope = context.command_scope("execute_by_ref");

        let result = agent_desktop_core::commands::execute_by_ref::execute(
            &ref_str,
            snapshot_str.as_deref(),
            core_action,
            caller_ip,
            adapter_ref.inner.as_ref(),
            &context,
        );
        scope.complete(&result);

        unsafe { write_command_envelope("execute_by_ref", result, out) }
    })
}

/// Takes a full CLI-format snapshot of the target application window,
/// allocates `@e` refs for all interactive elements, persists the refmap
/// to disk, and writes the JSON envelope into `*out`.
///
/// The JSON shape matches `agent-desktop snapshot`:
/// `{"version":"2.0","ok":true,"command":"snapshot","data":{"app":"...","window":{...},"ref_count":N,"snapshot_id":"...","tree":{...}}}`.
///
/// **`*out` ownership and error behaviour:**
/// - On success (`AD_RESULT_OK`): `*out` is a heap-allocated JSON string with `"ok":true`.
///   Caller must free it with `ad_free_string`.
/// - On a command-level error (e.g. app not found, snapshot failure): `*out` is a
///   heap-allocated JSON string with `"ok":false` and an `"error"` payload. Caller
///   must still free it with `ad_free_string`. The last-error slot is also set.
/// - On an argument or infrastructure error (null adapter, off-main-thread, invalid
///   UTF-8, bad surface discriminant, context failure): `*out` is set to null and no
///   allocation is made. Only the last-error slot is set.
///
/// `app` is tri-state:
/// - null — snapshot the currently focused window (same as running the command with no `--app`).
/// - valid UTF-8 string — snapshot the named application's focused window.
/// - non-null but invalid UTF-8 or exceeding `AD_MAX_STRING_BYTES` — returns `ErrInvalidArgs`.
///
/// `surface` is an `AdSnapshotSurface` discriminant (0 = Window, 1 = Focused, …).
/// An out-of-range value returns `ErrInvalidArgs`.
///
/// This entrypoint always targets the active focused window of the requested
/// application; explicit window targeting (`window_id`) is not yet exposed
/// over the ABI. Progressive traversal (skeleton mode and `--root` drill-down)
/// is likewise not exposed here. Both are planned fast-follows to this
/// entrypoint — agents needing them should use the CLI in the meantime.
///
/// **Dispatch-before-serialize ordering**: the snapshot and refmap persistence
/// occur before the result JSON is serialised. In the near-impossible event
/// that serialisation of an already-valid result fails, `*out` is set to null
/// and `ErrInternal` is returned while the refmap is already written.
///
/// # Safety
///
/// `adapter` must be a non-null pointer from `ad_adapter_create` or
/// `ad_adapter_create_with_session`. `out` must be a non-null writable
/// `*mut *mut c_char`. `app` must be null or a NUL-terminated string within
/// `AD_MAX_STRING_BYTES + 1` bytes. All pointers must remain valid for the
/// duration of the call. `adapter` must be used from the main thread on macOS.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_snapshot(
    adapter: *const AdAdapter,
    app: *const c_char,
    surface: i32,
    max_depth: u8,
    interactive_only: bool,
    compact: bool,
    out: *mut *mut c_char,
) -> AdResult {
    guard_non_null!(out, c"out is null");
    unsafe { *out = ptr::null_mut() };
    trap_panic(|| {
        if let Err(rc) = require_main_thread() {
            return rc;
        }
        guard_non_null!(adapter, c"adapter is null");

        let app_filter = unsafe { decode_optional_filter!(app, "app") };

        let core_surface = match snapshot_surface_from_c(surface, "surface") {
            Ok(s) => s,
            Err(e) => {
                set_last_error(&e);
                return AdResult::ErrInvalidArgs;
            }
        };

        let adapter_ref = unsafe { &*adapter };
        let context = match adapter_ref.command_context() {
            Ok(ctx) => ctx,
            Err(e) => {
                let ae = app_error_to_adapter(e);
                set_last_error(&ae);
                return crate::error::last_error_code();
            }
        };

        let args = SnapshotArgs {
            app: app_filter,
            window_id: None,
            max_depth,
            include_bounds: false,
            interactive_only,
            compact,
            surface: core_surface,
            skeleton: false,
            root_ref: None,
            snapshot_id: None,
        };

        let scope = context.command_scope("snapshot");

        let result = agent_desktop_core::commands::snapshot::execute(
            args,
            adapter_ref.inner.as_ref(),
            &context,
        );
        scope.complete(&result);

        unsafe { write_command_envelope("snapshot", result, out) }
    })
}

/// Returns the adapter's current health and permission state as a JSON
/// envelope matching the `agent-desktop status` CLI output.
///
/// `ad_status` does not query the accessibility tree; it reads the
/// permission report and ref-store metadata only, so it is safe to call
/// from any thread (unlike tree-traversal commands that require the
/// macOS main thread). On success `*out` is a NUL-terminated,
/// heap-allocated JSON string freed with `ad_free_string`.
///
/// On a command-level failure `*out` is set to a heap-allocated JSON string
/// with `"ok":false` and an `"error"` payload. The caller must still release
/// it with `ad_free_string(*out)`. The last-error slot is also set.
///
/// On an argument or infrastructure failure (null adapter, null out, context
/// error) `*out` is zeroed and only the last-error slot is populated.
///
/// # Safety
///
/// `adapter` must be a non-null pointer returned by `ad_adapter_create`
/// that has not been destroyed. `out` must be a non-null writable
/// `*mut *mut c_char`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_status(
    adapter: *const crate::AdAdapter,
    out: *mut *mut c_char,
) -> AdResult {
    guard_non_null!(out, c"out is null");
    unsafe { *out = ptr::null_mut() };
    guard_non_null!(adapter, c"adapter is null");

    trap_panic(|| {
        let adapter = unsafe { &*adapter };

        let ctx = match adapter.command_context() {
            Ok(c) => c,
            Err(app_err) => {
                let ae = app_error_to_adapter(app_err);
                error::set_last_error(&ae);
                return error::last_error_code();
            }
        };

        let report = adapter.inner.permission_report();

        let scope = ctx.command_scope("status");

        let result: Result<serde_json::Value, AppError> =
            execute_with_report_with_context(&*adapter.inner, &report, &ctx);
        scope.complete(&result);

        unsafe { write_command_envelope("status", result, out) }
    })
}

/// Exports the merged trace timeline for the adapter's active session as a
/// single self-contained HTML file matching `agent-desktop trace export`.
///
/// `limit` controls tail semantics: `0` embeds all events; the default `5000`
/// matches the CLI. Pass `-1` to use the CLI default explicitly.
///
/// `out_path` may be null; when set it must be a NUL-terminated UTF-8 path
/// within `AD_MAX_STRING_BYTES + 1` bytes.
///
/// On success `*out` is a heap-allocated JSON envelope freed with
/// `ad_free_string`. On command-level failure `*out` still holds an error
/// envelope that must be freed.
///
/// # Safety
///
/// `adapter` must be a non-null pointer from `ad_adapter_create` or
/// `ad_adapter_create_with_session`. `out` must be non-null. `out_path`
/// may be null or a NUL-terminated UTF-8 string within `AD_MAX_STRING_BYTES + 1`
/// bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_trace_export(
    adapter: *const AdAdapter,
    limit: i32,
    out_path: *const c_char,
    out: *mut *mut c_char,
) -> AdResult {
    guard_non_null!(out, c"out is null");
    unsafe { *out = ptr::null_mut() };
    trap_panic(|| {
        guard_non_null!(adapter, c"adapter is null");

        let path = match optional_adapter_string(out_path, "out_path") {
            Ok(value) => value,
            Err(e) => {
                set_last_error(&e);
                return AdResult::ErrInvalidArgs;
            }
        };

        let effective_limit = if limit < 0 {
            agent_desktop_core::trace_read::TRACE_EXPORT_DEFAULT_LIMIT
        } else {
            limit as usize
        };

        let adapter_ref = unsafe { &*adapter };
        let context = match adapter_ref.command_context() {
            Ok(ctx) => ctx,
            Err(e) => {
                let ae = app_error_to_adapter(e);
                set_last_error(&ae);
                return crate::error::last_error_code();
            }
        };

        let scope = context.command_scope("trace");
        let result = agent_desktop_core::commands::trace::execute(
            agent_desktop_core::commands::trace::TraceAction::Export {
                limit: effective_limit,
                out: path.map(std::path::PathBuf::from),
            },
            &context,
        );
        scope.complete(&result);

        unsafe { write_command_envelope("trace", result, out) }
    })
}

/// Returns the merged trace timeline for the adapter's active session as a
/// JSON envelope matching `agent-desktop trace show`.
///
/// `limit` controls tail semantics: `0` embeds all events; the default `500`
/// matches the CLI. Pass `-1` to use the CLI default explicitly.
///
/// `event_prefix` may be null; when set, only events whose name starts with the
/// prefix are returned before the tail limit is applied.
///
/// On success `*out` is a heap-allocated JSON envelope freed with
/// `ad_free_string`. On command-level failure `*out` still holds an error
/// envelope that must be freed.
///
/// # Safety
///
/// `adapter` must be a non-null pointer from `ad_adapter_create` or
/// `ad_adapter_create_with_session`. `out` must be non-null. `event_prefix`
/// may be null or a NUL-terminated UTF-8 string within `AD_MAX_STRING_BYTES + 1`
/// bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_trace_show(
    adapter: *const AdAdapter,
    limit: i32,
    event_prefix: *const c_char,
    out: *mut *mut c_char,
) -> AdResult {
    guard_non_null!(out, c"out is null");
    unsafe { *out = ptr::null_mut() };
    trap_panic(|| {
        guard_non_null!(adapter, c"adapter is null");

        let event = match optional_adapter_string(event_prefix, "event_prefix") {
            Ok(value) => value,
            Err(e) => {
                set_last_error(&e);
                return AdResult::ErrInvalidArgs;
            }
        };

        let effective_limit = if limit < 0 {
            agent_desktop_core::commands::trace::TRACE_SHOW_DEFAULT_LIMIT
        } else {
            limit as usize
        };

        let adapter_ref = unsafe { &*adapter };
        let context = match adapter_ref.command_context() {
            Ok(ctx) => ctx,
            Err(e) => {
                let ae = app_error_to_adapter(e);
                set_last_error(&ae);
                return crate::error::last_error_code();
            }
        };

        let scope = context.command_scope("trace");
        let result = agent_desktop_core::commands::trace::execute(
            agent_desktop_core::commands::trace::TraceAction::Show {
                limit: effective_limit,
                event,
            },
            &context,
        );
        scope.complete(&result);

        unsafe { write_command_envelope("trace", result, out) }
    })
}

/// Returns the `agent-desktop` version envelope as an owned JSON C string.
///
/// The returned string has the same `{version, ok, command, data}` shape
/// as `agent-desktop version` on the CLI. Free it with `ad_free_string`.
///
/// On success `*out` points to the envelope JSON.
/// On error `*out` is null and the last-error slot is populated.
///
/// # Safety
/// `out` must be a non-null writable `*mut *mut c_char`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_version(out: *mut *mut c_char) -> AdResult {
    trap_panic(|| unsafe {
        guard_non_null!(out, c"out is null");
        *out = ptr::null_mut();
        let context = match agent_desktop_core::context::CommandContext::new(None, None, false) {
            Ok(ctx) => ctx,
            Err(app_err) => {
                let ae = app_error_to_adapter(app_err);
                set_last_error(&ae);
                return crate::error::last_error_code();
            }
        };
        let scope = context.command_scope("version");
        let result = agent_desktop_core::commands::version::execute();
        scope.complete(&result);
        write_command_envelope("version", result, out)
    })
}

/// Runs `wait` with the given args, blocking the calling thread until the
/// condition is met or `timeout_ms` elapses.
///
/// On success `*out` is set to a freshly allocated JSON string containing the
/// CLI-format wait envelope (`{version, ok, command, data}`). The caller must
/// release the string with `ad_free_string(*out)`.
///
/// On a command-level failure (e.g. `TIMEOUT`, `ELEMENT_NOT_FOUND`) `*out` is
/// set to a freshly allocated JSON string with `"ok":false` and an `"error"`
/// payload. The caller must still release it with `ad_free_string(*out)`. The
/// last-error slot is also set.
///
/// On an argument or infrastructure failure (null adapter, null args, null out,
/// off-main-thread, invalid UTF-8 field) `*out` is zeroed, the last-error slot
/// is set, and a negative `AdResult` code is returned. No allocation is made.
///
/// # Safety
///
/// `adapter` must be a non-null pointer returned by `ad_adapter_create` that
/// has not been destroyed. `args` must be non-null and point to a valid
/// zero-initialized `AdWaitArgs`. `out` must be non-null and point to a
/// writable `*mut c_char`.
///
/// All `*const c_char` fields inside `AdWaitArgs` must be null or point to
/// readable, NUL-terminated memory within `AD_MAX_STRING_BYTES + 1` bytes.
///
/// `ad_wait` blocks the calling thread for up to `timeout_ms` milliseconds
/// while it holds a live reference into the adapter's allocation. The adapter
/// must outlive the call: do not call `ad_adapter_destroy` on this handle from
/// another thread while `ad_wait` is running — that is a use-after-free. Ensure
/// the wait has returned before destroying the adapter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_wait(
    adapter: *const AdAdapter,
    args: *const AdWaitArgs,
    out: *mut *mut c_char,
) -> AdResult {
    guard_non_null!(out, c"out is null");
    unsafe { *out = ptr::null_mut() };
    guard_non_null!(args, c"args is null");

    trap_panic(|| {
        if let Err(rc) = require_main_thread() {
            return rc;
        }
        guard_non_null!(adapter, c"adapter is null");

        let args = unsafe { &*args };
        let adapter_ref = unsafe { &*adapter };

        let ms = args.has_ms.then_some(args.ms);

        let element = unsafe { decode_optional_filter!(args.element, "element") };
        let window = unsafe { decode_optional_filter!(args.window, "window") };
        let text = unsafe { decode_optional_filter!(args.text, "text") };
        let snapshot_id = unsafe { decode_optional_filter!(args.snapshot_id, "snapshot_id") };
        let predicate = unsafe { decode_optional_filter!(args.predicate, "predicate") };
        let value = unsafe { decode_optional_filter!(args.value, "value") };
        let action_field = unsafe { decode_optional_filter!(args.action, "action") };
        let app = unsafe { decode_optional_filter!(args.app, "app") };

        let wait_args = WaitArgs {
            mode: WaitModeArgs {
                ms,
                element,
                window,
                text,
                menu: args.menu,
                menu_closed: args.menu_closed,
                notification: args.notification,
            },
            predicate: WaitPredicateArgs {
                snapshot_id,
                predicate,
                value,
                action: action_field,
                count: args.has_count.then_some(args.count),
            },
            timeout_ms: args.timeout_ms,
            app,
        };

        let ctx = match adapter_ref.command_context() {
            Ok(c) => c,
            Err(app_err) => {
                let adapter_err = app_error_to_adapter(app_err);
                error::set_last_error(&adapter_err);
                return error::last_error_code();
            }
        };

        let scope = ctx.command_scope("wait");

        let result = agent_desktop_core::commands::wait::execute(
            wait_args,
            adapter_ref.inner.as_ref(),
            &ctx,
        );
        scope.complete(&result);

        unsafe { write_command_envelope("wait", result, out) }
    })
}
