use std::cell::Cell;
use std::time::{Duration, Instant};

use serde_json::json;

use crate::{AdapterError, ErrorCode, wait_timeout_duration};

pub const DEFAULT_OPERATION_TIMEOUT_MS: u64 = 5_000;

thread_local! {
    static INHERITED_DEADLINE: Cell<Option<Deadline>> = const { Cell::new(None) };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deadline {
    started_at: Instant,
    expires_at: Instant,
    timeout_ms: u64,
    capped: bool,
}

impl Deadline {
    pub fn after(timeout_ms: u64) -> Result<Self, AdapterError> {
        let deadline = Self::detached_after(timeout_ms)?;
        Ok(INHERITED_DEADLINE.with(|inherited| {
            inherited
                .get()
                .map_or(deadline, |parent| deadline.min_expiry(parent))
        }))
    }

    /// Creates a bounded deadline that is intentionally independent of the
    /// current command scope. Use only for recovery work that must run after
    /// an operation deadline expires.
    pub fn detached_after(timeout_ms: u64) -> Result<Self, AdapterError> {
        let started_at = Instant::now();
        let duration = wait_timeout_duration(timeout_ms)?;
        let expires_at = started_at.checked_add(duration).ok_or_else(|| {
            AdapterError::new(
                ErrorCode::InvalidArgs,
                "Timeout cannot be represented safely",
            )
            .with_details(json!({ "timeout_ms": timeout_ms }))
        })?;
        Ok(Self {
            started_at,
            expires_at,
            timeout_ms,
            capped: false,
        })
    }

    pub fn standard() -> Result<Self, AdapterError> {
        Self::after(DEFAULT_OPERATION_TIMEOUT_MS)
    }

    pub fn from_duration(duration: Duration) -> Result<Self, AdapterError> {
        let whole_ms = duration.as_millis();
        let rounded_ms = if duration.subsec_nanos().is_multiple_of(1_000_000) {
            whole_ms
        } else {
            whole_ms.saturating_add(1)
        };
        let timeout_ms = u64::try_from(rounded_ms)
            .map_err(|_| AdapterError::new(ErrorCode::InvalidArgs, "Timeout is too large"))?;
        Self::after(timeout_ms)
    }

    pub(crate) fn at(started_at: Instant, timeout_ms: u64) -> Result<Self, AdapterError> {
        let duration = wait_timeout_duration(timeout_ms)?;
        let expires_at = started_at.checked_add(duration).ok_or_else(|| {
            AdapterError::new(
                ErrorCode::InvalidArgs,
                "Timeout cannot be represented safely",
            )
            .with_details(json!({ "timeout_ms": timeout_ms }))
        })?;
        let deadline = Self {
            started_at,
            expires_at,
            timeout_ms,
            capped: false,
        };
        Ok(INHERITED_DEADLINE.with(|inherited| {
            inherited
                .get()
                .map_or(deadline, |parent| deadline.min_expiry(parent))
        }))
    }

    pub fn remaining(self) -> Duration {
        self.expires_at.saturating_duration_since(Instant::now())
    }

    pub fn remaining_slice(self, maximum: Duration) -> Result<Duration, AdapterError> {
        let remaining = self.remaining();
        if remaining.is_zero() {
            Err(self.timeout_error())
        } else {
            Ok(remaining.min(maximum))
        }
    }

    pub fn capped(self, maximum: Duration) -> Self {
        let expires_at = Instant::now()
            .checked_add(maximum)
            .map_or(self.expires_at, |slice| self.expires_at.min(slice));
        Self {
            capped: self.capped || expires_at < self.expires_at,
            expires_at,
            ..self
        }
    }

    pub fn is_expired(self) -> bool {
        self.remaining().is_zero()
    }

    pub fn remaining_ms(self) -> u64 {
        let remaining = self.remaining();
        let whole_ms = remaining.as_millis();
        let rounded_ms = if remaining.subsec_nanos().is_multiple_of(1_000_000) {
            whole_ms
        } else {
            whole_ms.saturating_add(1)
        };
        u64::try_from(rounded_ms).unwrap_or(u64::MAX)
    }

    pub fn elapsed(self) -> Duration {
        self.started_at.elapsed()
    }

    pub fn timeout_ms(self) -> u64 {
        self.timeout_ms
    }

    pub(crate) fn was_capped(self) -> bool {
        self.capped
    }

    pub fn timeout_error(self) -> AdapterError {
        AdapterError::timeout("Operation exceeded its deadline").with_details(json!({
            "kind": "deadline",
            "timeout_ms": self.timeout_ms,
            "elapsed_ms": self.elapsed().as_millis(),
        }))
    }

    fn min_expiry(self, other: Self) -> Self {
        Self {
            capped: self.capped || other.expires_at < self.expires_at,
            expires_at: self.expires_at.min(other.expires_at),
            ..self
        }
    }
}

pub(crate) struct DeadlineScope {
    previous: Option<Deadline>,
    _not_send: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl Drop for DeadlineScope {
    fn drop(&mut self) {
        INHERITED_DEADLINE.set(self.previous);
    }
}

pub(crate) fn enter_scope(deadline: Option<Deadline>) -> DeadlineScope {
    let previous = INHERITED_DEADLINE.replace(None);
    let effective = match (previous, deadline) {
        (Some(parent), Some(child)) => Some(child.min_expiry(parent)),
        (parent, child) => parent.or(child),
    };
    INHERITED_DEADLINE.set(effective);
    DeadlineScope {
        previous,
        _not_send: std::marker::PhantomData,
    }
}

#[cfg(test)]
#[path = "deadline_tests.rs"]
mod tests;
