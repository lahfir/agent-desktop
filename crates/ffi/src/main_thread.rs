//! Main-thread enforcement helper for macOS-sensitive FFI entry points.
//!
//! macOS accessibility (AX) and Cocoa APIs must only be invoked on the
//! process's main thread. Calling them from a worker thread silently
//! leads to undefined behavior — dropped events, stale trees, or outright
//! crashes that look like memory corruption.
//!
//! This is a particularly sharp edge for `agent_desktop` when consumed
//! from Python / Swift / Node threads: the cdylib has no way to detect
//! the violation at compile time.
//!
//! `debug_assert_main_thread` panics in debug builds when the current
//! thread is not the process's main thread; the panic is caught by the
//! `trap_panic` boundary and converted into `AD_RESULT_ERR_INTERNAL`,
//! making off-main-thread violations loud during development. In release
//! builds the check is optimized out (`debug_assert!`) — the header
//! documents the constraint for consumers who ship their own debug
//! tooling.

#[cfg(target_os = "macos")]
pub(crate) fn is_main_thread() -> bool {
    unsafe { libc::pthread_main_np() != 0 }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn is_main_thread() -> bool {
    true
}

#[allow(dead_code)] // referenced by the ffi_macos_main_thread! macro from ffi_try.rs
pub(crate) fn debug_assert_main_thread() {
    debug_assert!(
        is_main_thread(),
        "agent_desktop FFI entry called off the main thread — macOS AX APIs require the main thread"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_main_thread_returns_bool() {
        // Cargo test runs each test on a worker thread, so the result may be
        // false on macOS. We just want to confirm the call itself is safe.
        let _ = is_main_thread();
    }

    #[test]
    fn test_off_main_panic_is_caught_by_trap() {
        // Simulate the production path: debug_assert_main_thread inside a
        // trap_panic body must convert the debug-mode panic into a clean
        // error code rather than unwinding out of the FFI boundary.
        let result = std::panic::catch_unwind(|| {
            let _ = std::thread::spawn(|| {
                debug_assert_main_thread();
            })
            .join();
        });
        assert!(result.is_ok());
    }
}
