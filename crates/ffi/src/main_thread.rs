//! Main-thread enforcement for macOS-sensitive FFI entrypoints.
//!
//! macOS accessibility (AX) and Cocoa APIs must run on the process's
//! main thread. Off-thread calls silently corrupt state or crash, and
//! the crash looks like memory corruption from the consumer side —
//! impossible to debug from a Python / Node / Swift host.
//!
//! `require_main_thread()` performs a **runtime** check (always, in
//! every build profile) and returns `AD_RESULT_ERR_INTERNAL` with a
//! `'static` diagnostic last-error when a worker-thread call is
//! detected. The check compiles away on non-macOS targets — AT-SPI
//! and UIA don't impose the same affinity rule.
//!
//! Exempt from the rule: `ad_adapter_create`, `ad_adapter_destroy`,
//! `ad_last_error_*`, and the entire `ad_free_*` / `ad_*_list_free` /
//! `ad_image_buffer_free` / `ad_release_window_fields` / `ad_free_handle`
//! / `ad_free_string` / `ad_free_action_result` / `ad_free_tree` family.
//! Those paths touch no AX/Cocoa state and are safe from any thread.

use crate::error::{set_last_error_static, AdResult};
use std::ffi::CStr;

static OFF_MAIN_THREAD_MESSAGE: &CStr =
    c"agent_desktop FFI entry called off the main thread (macOS requires main-thread AX/Cocoa calls)";

#[cfg(target_os = "macos")]
pub(crate) fn is_main_thread() -> bool {
    unsafe { libc::pthread_main_np() != 0 }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn is_main_thread() -> bool {
    true
}

/// Fail-closed runtime main-thread check. Returns
/// `Ok(())` on the main thread (always, on non-macOS). Returns
/// `Err(AdResult::ErrInternal)` on a worker thread with the last-error
/// slot populated with a `'static` diagnostic message.
///
/// Unlike a `debug_assert!`, this variant still fires in
/// `--profile release-ffi` and other optimized builds.
#[inline]
pub(crate) fn require_main_thread() -> Result<(), AdResult> {
    if is_main_thread() {
        Ok(())
    } else {
        set_last_error_static(AdResult::ErrInternal, OFF_MAIN_THREAD_MESSAGE);
        Err(AdResult::ErrInternal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_main_thread_call_is_always_safe_even_on_workers() {
        let _ = is_main_thread();
    }

    #[test]
    fn require_main_thread_returns_err_on_worker() {
        let outcome = std::thread::spawn(require_main_thread).join().unwrap();
        #[cfg(target_os = "macos")]
        assert!(matches!(outcome, Err(AdResult::ErrInternal)));
        #[cfg(not(target_os = "macos"))]
        assert!(outcome.is_ok());
    }
}
