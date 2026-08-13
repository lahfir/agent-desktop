//! Shared `#[cfg(test)]` thread-local flag helper for Windows capture test
//! hooks.
//!
//! Every capture test hook needs the same shape: flip a thread-local `Cell`
//! on, run the closure, and flip it back off even if the closure panics.
//! Each hook module still owns its own flag (its meaning is module-specific),
//! but the arm/run/reset sequence lives here once.

#![cfg(all(test, target_os = "windows"))]

use std::cell::Cell;

pub(crate) fn with_flag<R>(
    flag: &'static std::thread::LocalKey<Cell<bool>>,
    value: bool,
    run: impl FnOnce() -> R,
) -> R {
    struct ResetOnDrop(&'static std::thread::LocalKey<Cell<bool>>);
    impl Drop for ResetOnDrop {
        fn drop(&mut self) {
            self.0.with(|cell| cell.set(false));
        }
    }
    flag.with(|cell| cell.set(value));
    let _reset = ResetOnDrop(flag);
    run()
}

pub(crate) fn with_usize_flag<R>(
    flag: &'static std::thread::LocalKey<Cell<usize>>,
    value: usize,
    run: impl FnOnce() -> R,
) -> R {
    struct ResetOnDrop(&'static std::thread::LocalKey<Cell<usize>>);
    impl Drop for ResetOnDrop {
        fn drop(&mut self) {
            self.0.with(|cell| cell.set(0));
        }
    }
    flag.with(|cell| cell.set(value));
    let _reset = ResetOnDrop(flag);
    run()
}
