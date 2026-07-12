use std::time::Duration;

use crate::{Deadline, PlatformAdapter, RefEntry, resolve_attempt_outcome::ResolveAttemptOutcome};

pub(crate) const POLL_INTERVAL: Duration = Duration::from_millis(100);
const RESOLVE_ATTEMPT: Duration = Duration::from_millis(750);

pub(crate) fn resolve_within_deadline(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    deadline: Deadline,
) -> ResolveAttemptOutcome {
    if deadline.remaining_slice(RESOLVE_ATTEMPT).is_err() {
        return ResolveAttemptOutcome::DeadlinePassed;
    }
    match adapter.resolve_element_strict(entry, deadline.capped(RESOLVE_ATTEMPT)) {
        Ok(handle) => ResolveAttemptOutcome::Resolved(handle),
        Err(error) => ResolveAttemptOutcome::Failed(error),
    }
}
