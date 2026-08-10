//! Capture backend seam: support predicate, modern entry, and precedence.
//!
//! Modern is stubbed unsupported here so orchestration and Legacy fallback can
//! land before the WGC binding exists. Production fills
//! [`modern_is_supported`] and [`capture_modern`]; tests force unavailable or
//! fail-after-available through [`test_hooks`].

use std::time::Duration;

use agent_desktop_core::{AdapterError, Deadline, ErrorCode, ImageBuffer};

use super::capture_display::capture_display_at;
use super::capture_window::capture_window;
use super::permissions::ensure_budget;
use super::window_enum::WindowHandle;

/// Floor reserved for Legacy when Modern is attempted (A22-style silent
/// degradation must not starve the fallback of its deadline).
const LEGACY_DEADLINE_FLOOR: Duration = Duration::from_millis(200);

#[derive(Clone, Copy, Debug)]
pub(crate) enum CaptureSubject {
    Window {
        handle: WindowHandle,
        scale_factor: f64,
    },
    Display {
        index: usize,
    },
}

/// Whether the modern capture backend can serve this session.
///
/// Stubbed to unsupported until the Graphics Capture binding fills this read.
pub(crate) fn modern_is_supported() -> bool {
    #[cfg(test)]
    {
        if test_hooks::force_unsupported() {
            return false;
        }
        if test_hooks::force_fail_after_available() {
            return true;
        }
    }
    false
}

/// Modern capture entry. Stubbed to unsupported; production replaces the body.
pub(crate) fn capture_modern(
    subject: CaptureSubject,
    deadline: Deadline,
) -> Result<ImageBuffer, AdapterError> {
    let _ = subject;
    ensure_budget(deadline)?;
    #[cfg(test)]
    {
        if test_hooks::force_fail_after_available() {
            if test_hooks::consume_modern_slice() {
                burn_deadline(deadline);
            }
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                "modern capture forced failure after reporting available",
            ));
        }
    }
    Err(AdapterError::new(
        ErrorCode::ActionNotSupported,
        "modern capture is not available in this session",
    ))
}

pub(crate) fn capture_legacy(
    subject: CaptureSubject,
    deadline: Deadline,
) -> Result<ImageBuffer, AdapterError> {
    match subject {
        CaptureSubject::Window {
            handle,
            scale_factor,
        } => capture_window(handle, scale_factor, deadline),
        CaptureSubject::Display { index } => capture_display_at(index, deadline),
    }
}

/// Prefer Modern when supported; fall back to Legacy on unsupported or failure.
pub(crate) fn capture_with_precedence(
    subject: CaptureSubject,
    deadline: Deadline,
) -> Result<ImageBuffer, AdapterError> {
    ensure_budget(deadline)?;
    if modern_is_supported() {
        let modern_deadline = modern_deadline_slice(deadline);
        match capture_modern(subject, modern_deadline) {
            Ok(image) => return Ok(image),
            Err(error) => {
                tracing::debug!(
                    error = %error,
                    "modern capture unavailable or failed; falling back to legacy"
                );
            }
        }
    } else {
        tracing::debug!("modern capture unsupported; using legacy");
    }
    ensure_budget(deadline)?;
    capture_legacy(subject, deadline)
}

fn modern_deadline_slice(deadline: Deadline) -> Deadline {
    #[cfg(test)]
    if test_hooks::disable_deadline_slice() {
        return deadline;
    }
    let remaining = deadline.remaining();
    if remaining <= LEGACY_DEADLINE_FLOOR {
        return deadline.capped(Duration::ZERO);
    }
    deadline.capped(remaining - LEGACY_DEADLINE_FLOOR)
}

#[cfg(test)]
fn burn_deadline(deadline: Deadline) {
    while !deadline.is_expired() {
        let Ok(slice) = deadline.remaining_slice(Duration::from_millis(5)) else {
            break;
        };
        if slice.is_zero() {
            break;
        }
        std::thread::sleep(slice);
    }
}

#[cfg(test)]
pub(crate) mod test_hooks {
    use std::cell::Cell;

    thread_local! {
        static FORCE_UNSUPPORTED: Cell<bool> = const { Cell::new(false) };
        static FORCE_FAIL_AFTER_AVAILABLE: Cell<bool> = const { Cell::new(false) };
        static CONSUME_MODERN_SLICE: Cell<bool> = const { Cell::new(false) };
        static DISABLE_DEADLINE_SLICE: Cell<bool> = const { Cell::new(false) };
    }

    pub(crate) fn force_unsupported() -> bool {
        FORCE_UNSUPPORTED.with(Cell::get)
    }

    pub(crate) fn force_fail_after_available() -> bool {
        FORCE_FAIL_AFTER_AVAILABLE.with(Cell::get)
    }

    pub(crate) fn consume_modern_slice() -> bool {
        CONSUME_MODERN_SLICE.with(Cell::get)
    }

    pub(crate) fn disable_deadline_slice() -> bool {
        DISABLE_DEADLINE_SLICE.with(Cell::get)
    }

    pub(crate) fn with_force_unsupported<R>(run: impl FnOnce() -> R) -> R {
        with_flag(&FORCE_UNSUPPORTED, true, run)
    }

    pub(crate) fn with_force_fail_after_available<R>(run: impl FnOnce() -> R) -> R {
        with_flag(&FORCE_FAIL_AFTER_AVAILABLE, true, run)
    }

    pub(crate) fn with_consume_modern_slice<R>(run: impl FnOnce() -> R) -> R {
        with_flag(&CONSUME_MODERN_SLICE, true, run)
    }

    pub(crate) fn with_disable_deadline_slice<R>(run: impl FnOnce() -> R) -> R {
        with_flag(&DISABLE_DEADLINE_SLICE, true, run)
    }

    fn with_flag<R>(
        flag: &'static std::thread::LocalKey<Cell<bool>>,
        value: bool,
        run: impl FnOnce() -> R,
    ) -> R {
        struct Reset(&'static std::thread::LocalKey<Cell<bool>>);
        impl Drop for Reset {
            fn drop(&mut self) {
                self.0.with(|cell| cell.set(false));
            }
        }
        flag.with(|cell| cell.set(value));
        let _reset = Reset(flag);
        run()
    }
}
