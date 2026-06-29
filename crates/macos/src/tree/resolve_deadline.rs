use agent_desktop_core::error::{AdapterError, ErrorCode};
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
pub(super) fn remaining_before_deadline(deadline: Instant) -> Result<Duration, AdapterError> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(timeout_error());
    }
    Ok(remaining)
}

#[cfg(target_os = "macos")]
pub(super) fn ensure_before_deadline(deadline: Instant) -> Result<(), AdapterError> {
    remaining_before_deadline(deadline).map(|_| ())
}

#[cfg(target_os = "macos")]
pub(super) fn timeout_error() -> AdapterError {
    AdapterError::new(ErrorCode::Timeout, "Element resolution timed out")
        .with_suggestion("Retry the command, or run 'snapshot' if the UI changed.")
}

#[cfg(target_os = "macos")]
pub(super) fn sleep_before_retry(deadline: Instant) {
    if let Ok(remaining) = remaining_before_deadline(deadline) {
        if remaining > Duration::from_millis(100) {
            std::thread::sleep(remaining.min(Duration::from_millis(75)));
        }
    }
}
