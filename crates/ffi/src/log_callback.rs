//! `ad_set_log_callback` — forward `tracing` events to a consumer callback.
//!
//! # Thread-safety
//!
//! `tracing` events fire from arbitrary threads. The callback pointer is
//! stored in a global `AtomicPtr` (lock-free). A `tracing_subscriber` layer
//! is installed exactly once (via [`OnceLock`]) when the first non-null callback
//! is registered; subsequent registrations only swap the pointer. If a foreign
//! global subscriber already owns the process at the time of the first
//! registration, the install fails and `AD_RESULT_ERR_INTERNAL` is returned —
//! the callback is never stored and events will not be delivered.
//!
//! Re-entrancy is prevented by a per-thread flag: if a consumer callback
//! itself emits a `tracing` event, the recursive `on_event` invocation is
//! silently discarded rather than overflowing the stack.
//!
//! # Level mapping
//!
//! | `tracing` level | `level` passed to callback |
//! |-----------------|---------------------------|
//! | ERROR           | 1                         |
//! | WARN            | 2                         |
//! | INFO            | 3                         |
//! | DEBUG           | 4                         |
//! | TRACE           | 5                         |
//!
//! # Pointer lifetime
//!
//! The `msg` pointer passed to the callback is valid **only for the duration
//! of the call**. The consumer must copy the string before returning.
//!
//! # Redaction
//!
//! Fields whose keys match `SENSITIVE_KEYS` (password, token, text, …) are
//! replaced with `{"redacted":true}` before the message is formatted, using
//! the same logic as the file-trace writer.

use std::cell::Cell;
use std::ffi::{CString, c_char};
use std::os::raw::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicPtr, Ordering};

use agent_desktop_core::sanitize_trace_value;
use serde_json::{Map, Value};
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::error::AdResult;

/// Raw storage for the consumer callback. Null means no callback registered.
/// Stored as `*mut c_void` to avoid `fn` pointer restrictions on `AtomicPtr`.
static CALLBACK_SLOT: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

/// Memoises the outcome of the one-time subscriber install attempt.
///
/// `true`  — our `CallbackLayer` subscriber is installed and callback delivery works.
/// `false` — `try_init` failed because a foreign global subscriber already owns the
///           process; our layer was never installed.
static INSTALL_RESULT: OnceLock<bool> = OnceLock::new();

thread_local! {
    static IN_CALLBACK: Cell<bool> = const { Cell::new(false) };
}

/// RAII guard that resets [`IN_CALLBACK`] on drop, so a panicking callback
/// does not permanently poison the flag on its thread.
struct CallbackGuard;

impl Drop for CallbackGuard {
    fn drop(&mut self) {
        IN_CALLBACK.with(|g| g.set(false));
    }
}

/// The concrete type of the consumer callback.
type LogCb = unsafe extern "C" fn(level: i32, msg: *const c_char);

/// Soundness guard: `fn` pointer and data pointer must be the same size so
/// the `transmute` in `on_event` is layout-safe on every target.
const _: () = assert!(
    std::mem::size_of::<LogCb>() == std::mem::size_of::<*mut c_void>(),
    "fn pointer size must equal data pointer size for LogCb transmute"
);

fn level_to_i32(level: &Level) -> i32 {
    match *level {
        Level::ERROR => 1,
        Level::WARN => 2,
        Level::INFO => 3,
        Level::DEBUG => 4,
        Level::TRACE => 5,
    }
}

/// Collects event fields into a `serde_json::Map`.
struct FieldCollector {
    map: Map<String, Value>,
}

impl Visit for FieldCollector {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.map
            .insert(field.name().to_owned(), Value::String(value.to_owned()));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.map
            .insert(field.name().to_owned(), Value::String(format!("{value:?}")));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.map.insert(field.name().to_owned(), Value::from(value));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.map.insert(field.name().to_owned(), Value::from(value));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.map.insert(field.name().to_owned(), Value::Bool(value));
    }
}

/// `tracing_subscriber` layer that forwards events to the registered callback.
struct CallbackLayer;

impl<S: Subscriber> Layer<S> for CallbackLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        let ptr = CALLBACK_SLOT.load(Ordering::Acquire);
        if ptr.is_null() {
            return;
        }

        if IN_CALLBACK.with(Cell::get) {
            return;
        }

        let _ = catch_unwind(AssertUnwindSafe(|| {
            let level_i32 = level_to_i32(event.metadata().level());

            let mut collector = FieldCollector { map: Map::new() };
            event.record(&mut collector);

            let sanitized = sanitize_trace_value(Value::Object(collector.map));
            let msg_str = serde_json::to_string(&sanitized).unwrap_or_default();

            let Ok(c_msg) = CString::new(msg_str) else {
                return;
            };

            IN_CALLBACK.with(|g| g.set(true));
            let cb: LogCb = unsafe { std::mem::transmute(ptr) };
            let _reset = CallbackGuard;
            unsafe { cb(level_i32, c_msg.as_ptr()) };
        }));
    }
}

fn install_layer_once() -> bool {
    *INSTALL_RESULT.get_or_init(|| {
        tracing_subscriber::registry()
            .with(CallbackLayer)
            .try_init()
            .is_ok()
    })
}

/// Maps an install outcome to the `AdResult` that `ad_set_log_callback` should
/// return when registering a non-null callback.
///
/// Extracted as a named function so the routing logic can be unit-tested
/// independently of global subscriber state.
#[cfg(test)]
fn result_for_install(succeeded: bool) -> AdResult {
    if succeeded {
        AdResult::Ok
    } else {
        AdResult::ErrInternal
    }
}

/// Registers a callback to receive `tracing` events, or unregisters the
/// current callback when `cb` is `NULL`.
///
/// # Install semantics
///
/// The subscriber layer is installed exactly once — on the first call with a
/// non-null `cb`. If a foreign global subscriber already owns the process at
/// that point, the install fails and this function returns
/// `AD_RESULT_ERR_INTERNAL` with a diagnostic last-error. No callback pointer
/// is stored in that case; events will never be delivered until the consumer
/// remedies the conflict. Subsequent calls with a non-null `cb` after a
/// **successful** install only swap the stored pointer and always return
/// `AD_RESULT_OK`.
///
/// `NULL` always returns `AD_RESULT_OK` — unregistering cannot fail.
///
/// # Callback contract
///
/// - `level` — 1 (ERROR) … 5 (TRACE)
/// - `msg` — a NUL-terminated JSON string; valid only for the call's duration
///
/// Sensitive field values (password, token, text, …) are replaced with
/// `{"redacted":true}` before the message is formatted.
///
/// Invocations are best-effort. A panicking callback is caught and silently
/// discarded; no command fails because of a trace delivery error. A callback
/// that emits `tracing` events is safe: the recursive `on_event` is dropped
/// by a per-thread guard before it reaches the callback again.
///
/// # Safety
///
/// `cb` must be null or a valid function pointer with the declared signature.
/// The pointer is stored atomically; the subscriber may call it from threads
/// other than the registering thread.
///
/// A callback unregistered via `NULL` may still be invoked from another thread
/// for a brief window after this call returns. The callback (and any data it
/// captures) must remain valid for the process lifetime, or the caller must
/// quiesce all tracing sources before unregistering.
#[unsafe(no_mangle)]
pub extern "C" fn ad_set_log_callback(
    cb: Option<unsafe extern "C" fn(level: i32, msg: *const c_char)>,
) -> AdResult {
    crate::ffi_try::trap_panic(|| {
        match cb {
            Some(f) => {
                if !install_layer_once() {
                    crate::error::set_last_error_static(
                        AdResult::ErrInternal,
                        c"ad_set_log_callback: a foreign tracing subscriber already owns this \
                          process; our CallbackLayer was not installed and events will not be \
                          delivered",
                    );
                    return AdResult::ErrInternal;
                }
                let raw = f as *mut c_void;
                CALLBACK_SLOT.store(raw, Ordering::Release);
            }
            None => {
                CALLBACK_SLOT.store(std::ptr::null_mut(), Ordering::Release);
            }
        }
        AdResult::Ok
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_for_install_success_returns_ok() {
        assert_eq!(result_for_install(true), AdResult::Ok);
    }

    #[test]
    fn result_for_install_failure_returns_err_internal() {
        assert_eq!(result_for_install(false), AdResult::ErrInternal);
    }

    #[test]
    fn null_unregister_always_ok_independent_of_install_state() {
        let result = ad_set_log_callback(None);
        assert_eq!(
            result,
            AdResult::Ok,
            "NULL unregister must return Ok regardless of install state"
        );
    }
}
