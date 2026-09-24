use crate::{BackgroundFocusGuard, ProcessId};

/// Evidence an adapter gathers around one background pointer delivery.
///
/// Background delivery tries not to activate the target, but the target
/// process decides for itself how to react to posted events, so the command
/// reports what it observed instead of claiming a silent success. A `None`
/// frontmost sample means the frontmost application could not be read.
/// `layers` names the platform delivery techniques that were requested and
/// `degradations` explains any of them that were unavailable or failed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BackgroundPointerReport {
    pub frontmost_pid_before: Option<ProcessId>,
    pub frontmost_pid_after: Option<ProcessId>,
    pub layers: Vec<String>,
    pub degradations: Vec<String>,
    pub focus_guard: Option<BackgroundFocusGuard>,
}

impl BackgroundPointerReport {
    /// `"unchanged"`, `"restored"` when the frontmost application was
    /// observed away from the user's app during delivery (whether the focus
    /// guard took it back or it returned on its own), `"changed"`, or
    /// `"unknown"` when either sample is missing.
    pub fn focus_change(&self) -> &'static str {
        let intervened = self
            .focus_guard
            .as_ref()
            .is_some_and(|guard| guard.interventions > 0 || guard.max_steal_ms > 0);
        match (self.frontmost_pid_before, self.frontmost_pid_after) {
            (Some(before), Some(after)) if before != after => "changed",
            (Some(_), Some(_)) if intervened => "restored",
            (Some(_), Some(_)) => "unchanged",
            _ => "unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(
        before: Option<u32>,
        after: Option<u32>,
        interventions: u32,
    ) -> BackgroundPointerReport {
        BackgroundPointerReport {
            frontmost_pid_before: before.map(ProcessId::new),
            frontmost_pid_after: after.map(ProcessId::new),
            focus_guard: Some(BackgroundFocusGuard {
                interventions,
                restored: interventions > 0 && before == after,
                max_steal_ms: 0,
                yielded: false,
            }),
            ..BackgroundPointerReport::default()
        }
    }

    #[test]
    fn a_guarded_steal_is_never_reported_as_unchanged() {
        assert_eq!(report(Some(7), Some(7), 0).focus_change(), "unchanged");
        assert_eq!(report(Some(7), Some(7), 1).focus_change(), "restored");
        assert_eq!(report(Some(7), Some(9), 2).focus_change(), "changed");
        assert_eq!(report(None, Some(7), 1).focus_change(), "unknown");
    }

    #[test]
    fn a_steal_that_ended_without_intervention_is_still_reported() {
        let mut observed = report(Some(7), Some(7), 0);
        observed.focus_guard = Some(BackgroundFocusGuard {
            interventions: 0,
            restored: false,
            max_steal_ms: 25,
            yielded: false,
        });
        assert_eq!(observed.focus_change(), "restored");
    }
}
