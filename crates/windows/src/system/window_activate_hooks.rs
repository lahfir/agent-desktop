use std::cell::Cell;

pub(super) mod attempt_probe {
    use super::Cell;

    thread_local! {
        static CURRENT: Cell<u32> = const { Cell::new(0) };
        static COUNT: Cell<u32> = const { Cell::new(0) };
    }

    pub(crate) fn begin_attempt(attempt: u32) {
        CURRENT.with(|cell| cell.set(attempt));
        COUNT.with(|cell| cell.set(cell.get() + 1));
    }

    pub(crate) fn current() -> u32 {
        CURRENT.with(Cell::get)
    }

    pub(crate) fn count() -> u32 {
        COUNT.with(Cell::get)
    }

    pub(crate) fn reset() {
        CURRENT.with(|cell| cell.set(0));
        COUNT.with(|cell| cell.set(0));
    }
}

pub(super) mod never_foreground {
    use super::Cell;

    thread_local! {
        static ACTIVE: Cell<bool> = const { Cell::new(false) };
    }

    pub(crate) fn is_active() -> bool {
        ACTIVE.with(Cell::get)
    }

    pub(crate) fn with<R>(run: impl FnOnce() -> R) -> R {
        struct ResetOnDrop;
        impl Drop for ResetOnDrop {
            fn drop(&mut self) {
                ACTIVE.with(|flag| flag.set(false));
            }
        }
        ACTIVE.with(|flag| flag.set(true));
        let _reset = ResetOnDrop;
        run()
    }
}

pub(super) mod force_unowned_from_attempt {
    use super::Cell;

    thread_local! {
        static FROM: Cell<Option<u32>> = const { Cell::new(None) };
    }

    pub(crate) fn blocks(attempt: u32) -> bool {
        FROM.with(|cell| cell.get().is_some_and(|from| attempt >= from))
    }

    pub(crate) fn with<R>(from_attempt: u32, run: impl FnOnce() -> R) -> R {
        struct ResetOnDrop;
        impl Drop for ResetOnDrop {
            fn drop(&mut self) {
                FROM.with(|cell| cell.set(None));
            }
        }
        FROM.with(|cell| cell.set(Some(from_attempt)));
        let _reset = ResetOnDrop;
        run()
    }
}

pub(super) mod force_strictly_higher {
    use super::Cell;

    thread_local! {
        static ACTIVE: Cell<bool> = const { Cell::new(false) };
    }

    pub(crate) fn is_active() -> bool {
        ACTIVE.with(Cell::get)
    }

    pub(crate) fn with<R>(run: impl FnOnce() -> R) -> R {
        struct ResetOnDrop;
        impl Drop for ResetOnDrop {
            fn drop(&mut self) {
                ACTIVE.with(|flag| flag.set(false));
            }
        }
        ACTIVE.with(|flag| flag.set(true));
        let _reset = ResetOnDrop;
        run()
    }
}

pub(super) mod restore_probe {
    use super::Cell;

    thread_local! {
        static COUNT: Cell<u32> = const { Cell::new(0) };
    }

    pub(crate) fn record() {
        COUNT.with(|cell| cell.set(cell.get() + 1));
    }

    pub(crate) fn count() -> u32 {
        COUNT.with(Cell::get)
    }

    pub(crate) fn reset() {
        COUNT.with(|cell| cell.set(0));
    }
}
