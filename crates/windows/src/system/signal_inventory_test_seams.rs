#![cfg(test)]

pub(super) mod enum_windows_calls {
    use std::cell::Cell;

    thread_local! {
        static COUNT: Cell<usize> = const { Cell::new(0) };
    }

    pub(crate) fn record() {
        COUNT.with(|cell| cell.set(cell.get() + 1));
    }

    pub(crate) fn take() -> usize {
        COUNT.with(|cell| {
            let value = cell.get();
            cell.set(0);
            value
        })
    }
}

pub(super) mod force_token_none {
    use std::cell::Cell;

    use agent_desktop_core::ProcessId;

    thread_local! {
        static TARGET: Cell<Option<u32>> = const { Cell::new(None) };
    }

    struct ResetOnDrop;

    impl Drop for ResetOnDrop {
        fn drop(&mut self) {
            TARGET.with(|cell| cell.set(None));
        }
    }

    pub(crate) fn with<R>(pid: ProcessId, run: impl FnOnce() -> R) -> R {
        TARGET.with(|cell| cell.set(Some(u32::from(pid))));
        let _reset = ResetOnDrop;
        run()
    }

    pub(crate) fn matches(pid: ProcessId) -> bool {
        TARGET.with(|cell| cell.get() == Some(u32::from(pid)))
    }
}

#[cfg(target_os = "windows")]
pub(super) mod force_race {
    use std::cell::Cell;

    thread_local! {
        static REMAINING: Cell<usize> = const { Cell::new(0) };
    }

    pub(crate) fn with<R>(times: usize, run: impl FnOnce() -> R) -> R {
        crate::system::test_support::with_usize_flag(&REMAINING, times, run)
    }

    pub(crate) fn consume_if_armed() -> bool {
        REMAINING.with(|cell| {
            let remaining = cell.get();
            if remaining == 0 {
                false
            } else {
                cell.set(remaining - 1);
                true
            }
        })
    }
}

#[cfg(target_os = "windows")]
pub(super) mod force_truncation {
    use std::cell::Cell;

    thread_local! {
        static ACTIVE: Cell<bool> = const { Cell::new(false) };
    }

    pub(crate) fn with<R>(run: impl FnOnce() -> R) -> R {
        crate::system::test_support::with_flag(&ACTIVE, true, run)
    }

    pub(crate) fn is_active() -> bool {
        ACTIVE.with(Cell::get)
    }
}
