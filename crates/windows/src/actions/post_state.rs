//! Post-delivery `ElementState` attachment for state-bearing actions.
//!
//! Pointer / scroll / focus actions attach none. Secure withholding is
//! inherited from the live-read path's `IsPassword` gate. A post-read
//! failure after delivery maps to `delivered_unverified` via
//! [`after_delivery`], never a bare `not_delivered` read error.

use agent_desktop_core::{
    Action, ActionStep, ActionStepOutcome, AdapterError, Deadline, DeliverySemantics, ElementState,
};

use crate::tree::element::UIAElement;

/// Remaps a post-delivery observation failure onto `delivered_unverified`.
pub(crate) fn after_delivery(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::delivered_unverified())
}

pub(crate) fn delivery_occurred(steps: &[ActionStep]) -> bool {
    steps
        .iter()
        .any(|step| matches!(step.outcome, ActionStepOutcome::Succeeded))
}

pub(crate) fn attaches_post_state(action: &Action) -> bool {
    matches!(
        action,
        Action::Toggle
            | Action::Check
            | Action::Uncheck
            | Action::SetValue(_)
            | Action::Clear
            | Action::Expand
            | Action::Collapse
    )
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{
        Action, AdapterError, Deadline, ElementState, UIAElement, after_delivery,
        attaches_post_state, delivery_occurred,
    };
    use crate::tree::live_read::{live_state, read_live_element};
    use agent_desktop_core::ActionStep;

    pub(crate) fn read_post_state(
        element: &UIAElement,
        action: &Action,
        deadline: Deadline,
    ) -> Result<Option<ElementState>, AdapterError> {
        if !attaches_post_state(action) {
            return Ok(None);
        }
        let handle = element.clone().into_native_handle();
        let read = read_live_element(&handle, deadline)?;
        Ok(Some(live_state(&read)?))
    }

    pub(crate) fn post_state_for_steps(
        element: &UIAElement,
        action: &Action,
        steps: &[ActionStep],
        deadline: Deadline,
    ) -> Result<Option<ElementState>, AdapterError> {
        if !delivery_occurred(steps) || deadline.is_expired() {
            return Ok(None);
        }
        read_post_state(element, action, deadline).map_err(after_delivery)
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use super::{Action, AdapterError, Deadline, ElementState, UIAElement};
    use agent_desktop_core::ActionStep;

    pub(crate) fn read_post_state(
        _element: &UIAElement,
        _action: &Action,
        _deadline: Deadline,
    ) -> Result<Option<ElementState>, AdapterError> {
        Ok(None)
    }

    pub(crate) fn post_state_for_steps(
        _element: &UIAElement,
        _action: &Action,
        _steps: &[ActionStep],
        _deadline: Deadline,
    ) -> Result<Option<ElementState>, AdapterError> {
        Ok(None)
    }
}

pub(crate) use imp::post_state_for_steps;

#[cfg(all(test, target_os = "windows"))]
#[path = "post_state_tests.rs"]
mod tests;
