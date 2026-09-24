use agent_desktop_core::Deadline;

use crate::input::skylight;

/// Reads the frontmost application's pid from the most reliable source that
/// answers: the window server (`_SLPSGetFrontProcess`), then the AX
/// system-wide `AXFocusedApplication`, then `NSWorkspace`.
///
/// AX alone returned nothing while Comet (Chromium) was frontmost, which made
/// every report say `focus_change: unknown`, and `NSWorkspace` can be stale
/// in a CLI that never runs an AppKit run loop, so it is the last resort.
pub(crate) fn frontmost_pid(deadline: Deadline) -> Option<i32> {
    first_available([
        &mut skylight::front_process_pid,
        &mut || accessibility_frontmost(deadline),
        &mut || workspace_frontmost(deadline),
    ])
}

/// Returns the first positive pid, calling later readers only when earlier
/// ones fail.
pub(crate) fn first_available<const N: usize>(
    readers: [&mut dyn FnMut() -> Option<i32>; N],
) -> Option<i32> {
    readers
        .into_iter()
        .find_map(|read| read().filter(|pid| *pid > 0))
}

fn accessibility_frontmost(deadline: Deadline) -> Option<i32> {
    let instant = crate::tree::locator_deadline::from_operation(deadline).ok()?;
    crate::system::window_inventory::focused_application_pid(instant).ok()?
}

fn workspace_frontmost(deadline: Deadline) -> Option<i32> {
    let instant = crate::tree::locator_deadline::from_operation(deadline).ok()?;
    let snapshot = crate::system::workspace_apps::window_owner_snapshot_until(instant).ok()?;
    snapshot.frontmost().map(|owner| owner.pid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn a_failed_source_falls_through_to_the_next_one() {
        let later_calls = Cell::new(0);
        let pid = first_available([
            &mut || None,
            &mut || Some(0),
            &mut || Some(4242),
            &mut || {
                later_calls.set(later_calls.get() + 1);
                Some(7)
            },
        ]);
        assert_eq!(pid, Some(4242));
        assert_eq!(later_calls.get(), 0);
    }

    #[test]
    fn every_source_failing_is_unknown() {
        assert_eq!(first_available([&mut || None, &mut || Some(-1)]), None);
    }
}
