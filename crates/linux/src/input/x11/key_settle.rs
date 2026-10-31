//! How long this process stays on the accessibility bus after its last key.
//! When the last AT-SPI client leaves the bus, at-spi2's ATK bridge removes
//! its key listener, and the focused GTK, Chromium or Electron application
//! can still be inside that listener's nested main loop: it then crashes.
//! The CLI is often that last client, so its exit waits until the last key it
//! sent is old enough to have been handled. The wait is paid once per
//! process, so a `batch` of key steps pays it once, and work done after the
//! key counts towards it.

use std::sync::{Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

const SETTLE: Duration = Duration::from_millis(200);

static LAST_KEY: Mutex<Option<Instant>> = Mutex::new(None);

pub(crate) fn mark_key_sent() {
    *LAST_KEY.lock().unwrap_or_else(PoisonError::into_inner) = Some(Instant::now());
}

pub(crate) fn wait_for_last_key() {
    let last = *LAST_KEY.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(left) = remaining(last, Instant::now()) {
        thread::sleep(left);
    }
}

fn remaining(last_key: Option<Instant>, now: Instant) -> Option<Duration> {
    let left = (last_key? + SETTLE).saturating_duration_since(now);
    (!left.is_zero()).then_some(left)
}

#[cfg(test)]
#[path = "key_settle_tests.rs"]
mod tests;
