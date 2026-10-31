use crate::{Rect, tree::element_state::ElementState};

pub(super) struct ActionabilityEvidence {
    pub(super) nonspatial: bool,
    pub(super) state: ElementState,
    pub(super) states_complete: bool,
    pub(super) bounds: Option<Rect>,
    pub(super) available_actions: Vec<String>,
}

impl ActionabilityEvidence {
    pub(super) fn can_wait_for_geometry(
        &self,
        request: &crate::ActionRequest,
        report: &super::report::ActionabilityReport,
        requirement: Option<crate::HeadedRequirement>,
    ) -> bool {
        !report.actionable
            && request.policy.is_headed()
            && request.timeout_ms.is_some_and(|timeout| timeout > 0)
            && self.states_complete
            && self.state.hidden != Some(true)
            && self.state.offscreen != Some(true)
            && !crate::tree::state::has_state(&self.state.states, crate::tree::state::HIDDEN)
            && !crate::tree::state::has_state(&self.state.states, crate::tree::state::OFFSCREEN)
            && self.bounds.is_none_or(|bounds| {
                bounds.validate().is_ok() && (bounds.width == 0.0 || bounds.height == 0.0)
            })
            && report.checks.iter().all(|check| {
                check.status == super::status::ActionabilityStatus::Pass
                    || (check.terminal_code.is_none()
                        && matches!(check.check, "visible" | "stable"))
            })
            && requirement == Some(crate::HeadedRequirement::FocusedApplicationAndCursor)
    }
}
