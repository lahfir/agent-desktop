//! Shared `#[cfg(test)]` thread-local flag helper for Windows capture test
//! hooks, plus one cross-test serialization primitive.
//!
//! Every capture test hook needs the same shape: flip a thread-local `Cell`
//! on, run the closure, and flip it back off even if the closure panics.
//! Each hook module still owns its own flag (its meaning is module-specific),
//! but the arm/run/reset sequence lives here once.
//!
//! `FIXTURE_APP_NAME_LOCK` addresses a different, process-wide hazard: every
//! re-executed test fixture (`HostedFixture`, `ModalFixture`, `MenuFixture`,
//! ...) launches `std::env::current_exe()` verbatim, so every fixture child
//! spawned anywhere in this test binary reports the identical process image
//! name. A live test that resolves its fixture by `--app <that name>` while
//! a second such test's fixture is concurrently alive observes two
//! processes sharing the name and gets `AMBIGUOUS_TARGET` instead of its own
//! fixture. Most `wait --event` live tests avoid this entirely by seeding
//! `CommandContext` with a `capture_signal_baseline` already scoped to their
//! own `(pid, process_instance)` (`wait_event_live_tests.rs`'s module doc
//! explains why that closes the hazard rather than merely narrowing it), so
//! this lock is the remaining mitigation for the two paths with no seeding
//! escape hatch: `app-launched` (the process does not exist yet, so there is
//! nothing to seed against) and `wait --menu` (`wait_for_menu` resolves its
//! target through `list_apps`, not `capture_signal_baseline`, so it never
//! sees a seed at all). A live test on either of those two paths holds this
//! lock for its fixture's entire lifetime plus the wait, so at most one such
//! test's fixture exists at a time.

#![cfg(all(test, target_os = "windows"))]

use std::cell::Cell;
use std::sync::Mutex;

pub(crate) static FIXTURE_APP_NAME_LOCK: Mutex<()> = Mutex::new(());

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
