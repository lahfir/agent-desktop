use super::{ReachOutcome, reach};
use agent_desktop_core::{CursorOverlayControl, CursorOverlayStyle};
use std::time::Duration;

/// A name nothing is serving must read as "no renderer", not as a failure -
/// it is the ordinary state before the first enable of a session, and the
/// only state from which spawning is correct.
#[test]
fn a_name_nobody_serves_reports_no_renderer_rather_than_an_error() {
    let control = CursorOverlayControl::enable("s0000001".into(), CursorOverlayStyle::default());

    let outcome = reach(
        r"\\.\pipe\agent-desktop-cursor-nothing-serves-this",
        &control,
        Duration::from_millis(50),
    );

    assert!(
        matches!(outcome, ReachOutcome::NoRenderer),
        "an absent renderer must be distinguishable from an unreachable one, or the caller \
         either forks a duplicate or never starts one at all"
    );
}

/// The budget bounds the attempt. A caller that could be held past its own
/// deadline here is the clipboard defect this crate carries a guard for,
/// reappearing in the renderer that is meant to be fail-soft.
#[test]
fn an_absent_renderer_answers_well_inside_the_budget() {
    let control = CursorOverlayControl::disable("s0000001".into());
    let started = std::time::Instant::now();

    let _ = reach(
        r"\\.\pipe\agent-desktop-cursor-nothing-serves-this-either",
        &control,
        Duration::from_millis(400),
    );

    assert!(
        started.elapsed() < Duration::from_millis(50),
        "a not-found answer is immediate. The bound sits far below the 400ms budget on \
         purpose: the regression it guards against is falling into the busy-retry loop, \
         which runs to the deadline, so a bound equal to the budget would be a coin flip \
         against itself"
    );
}
