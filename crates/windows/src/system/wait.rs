use std::time::Duration;

use agent_desktop_core::{AdapterError, Deadline, ErrorCode, ProcessId, ProcessIdentity};

use super::menu_state::menu_is_open;
use super::process_identity;

const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Polls the [`menu_is_open`] predicate until the target process's
/// menu-open state equals `open`, then returns `Ok(())`.
///
/// A deliberate structural port of `crates/macos/src/system/wait.rs`: the
/// process identity is re-verified before the first predicate read on every
/// iteration and a second time immediately before success is declared - the
/// macOS double-check that closes the window where the target dies between
/// the read and the return. The deadline is consulted only after a mismatch,
/// so the predicate is evaluated at least once even for a near-zero timeout
/// and `--menu-closed` against an already-closed menu succeeds immediately.
///
/// Core makes exactly one call here and owns no retry of its own
/// (`crates/core/src/commands/wait.rs:123-129`), so every condition this
/// loop can absorb - the 50ms poll interval, the identity re-checks, and a
/// predicate read that itself times out mid-poll - is absorbed here rather
/// than propagated raw: a `Timeout` from the predicate falls through to this
/// loop's own deadline check so the direction-specific message below still
/// applies, instead of surfacing the predicate's own undecorated timeout.
///
/// Diverges from macOS's `AppUnresponsive` mapping for an identity mismatch:
/// this method reports the exited-or-recycled target as `StaleRef`, the code
/// this crate already uses for a resolved reference that no longer matches
/// live state, rather than the transient-condition code macOS reuses there.
pub(crate) fn wait_for_menu(
    process: ProcessIdentity,
    open: bool,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    loop {
        verify_process_alive(&process)?;
        match evaluate_menu_state(process.pid, deadline) {
            Ok(state) if state == open => {
                verify_process_alive(&process)?;
                return Ok(());
            }
            Ok(_) => {}
            Err(error) if error.code == ErrorCode::Timeout => {}
            Err(error) => return Err(error),
        }
        if deadline.is_expired() {
            return Err(deadline
                .timeout_error()
                .with_platform_detail(direction_message(open)));
        }
        let pause = deadline.remaining_slice(POLL_INTERVAL)?;
        std::thread::sleep(pause);
    }
}

fn direction_message(open: bool) -> &'static str {
    if open {
        "No menu opened before the deadline"
    } else {
        "Menu did not close before the deadline"
    }
}

/// Routes every predicate read through one call site so a test can count
/// polls without changing what production code executes.
fn evaluate_menu_state(pid: ProcessId, deadline: Deadline) -> Result<bool, AdapterError> {
    #[cfg(test)]
    poll_calls::record();
    menu_is_open(pid, deadline)
}

fn verify_process_alive(process: &ProcessIdentity) -> Result<(), AdapterError> {
    if process_identity::matches_instance(process.pid, &process.instance)? {
        Ok(())
    } else {
        Err(stale_process_error(process))
    }
}

fn stale_process_error(process: &ProcessIdentity) -> AdapterError {
    AdapterError::new(
        ErrorCode::StaleRef,
        "Target process instance is no longer running",
    )
    .with_details(serde_json::json!({
        "pid": u32::from(process.pid),
        "process_instance": process.instance,
    }))
}

#[cfg(test)]
pub(super) mod poll_calls {
    use std::cell::Cell;

    thread_local! {
        static COUNT: Cell<usize> = const { Cell::new(0) };
    }

    pub(super) fn record() {
        COUNT.with(|cell| cell.set(cell.get() + 1));
    }

    pub(super) fn take() -> usize {
        COUNT.with(|cell| {
            let value = cell.get();
            cell.set(0);
            value
        })
    }
}

#[cfg(test)]
#[path = "wait_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "wait_menu_command_live_tests.rs"]
mod menu_command_live_tests;
