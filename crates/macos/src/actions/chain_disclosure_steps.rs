#[cfg(target_os = "macos")]
mod imp {
    use crate::actions::{ax_helpers, discovery::ElementCaps};
    use crate::tree::AXElement;
    use agent_desktop_core::error::AdapterError;

    /// Expands a disclosure that toggles via press (no settable `AXExpanded`).
    /// Idempotent: a no-op when already expanded; otherwise presses and
    /// confirms the disclosed state flipped.
    pub(crate) fn press_to_expand(
        el: &AXElement,
        _caps: &ElementCaps,
        chain_deadline: Option<std::time::Instant>,
    ) -> Result<bool, AdapterError> {
        press_toggle_disclosure(el, true, chain_deadline)
    }

    /// Collapses a press-toggled disclosure, mirroring [`press_to_expand`].
    pub(crate) fn press_to_collapse(
        el: &AXElement,
        _caps: &ElementCaps,
        chain_deadline: Option<std::time::Instant>,
    ) -> Result<bool, AdapterError> {
        press_toggle_disclosure(el, false, chain_deadline)
    }

    /// Tries the semantic action / settable attribute, then a press. Each is
    /// confirmed against the disclosed state; an action that succeeds at the AX
    /// layer but does not move the control is not counted.
    fn press_toggle_disclosure(
        el: &AXElement,
        want_expanded: bool,
        chain_deadline: Option<std::time::Instant>,
    ) -> Result<bool, AdapterError> {
        if disclosed_state(el) == Some(want_expanded) {
            return Ok(true);
        }
        let action = if want_expanded {
            "AXExpand"
        } else {
            "AXCollapse"
        };
        if ax_helpers::has_ax_action(el, action) {
            let _ = ax_helpers::try_ax_action_retried_or_err(el, action)?;
            if disclosure_settled(el, want_expanded, chain_deadline) {
                return Ok(true);
            }
        }
        if ax_helpers::is_attr_settable(el, "AXExpanded") {
            let _ = ax_helpers::set_ax_bool_or_err(el, "AXExpanded", want_expanded)?;
            if disclosure_settled(el, want_expanded, chain_deadline) {
                return Ok(true);
            }
        }
        if ax_helpers::has_ax_action(el, "AXPress")
            && ax_helpers::try_ax_action_retried_or_err(el, "AXPress")?
            && disclosure_settled(el, want_expanded, chain_deadline)
        {
            return Ok(true);
        }
        Ok(false)
    }

    /// Polls for the disclosed state instead of a fixed settle sleep: fast UIs
    /// confirm on the first read, while animated disclosures get up to the
    /// settle budget — capped to the chain's remaining deadline so a settle
    /// wait can never overshoot the chain's own budget.
    fn disclosure_settled(
        el: &AXElement,
        want_expanded: bool,
        chain_deadline: Option<std::time::Instant>,
    ) -> bool {
        use std::time::{Duration, Instant};

        const POLL_INTERVAL: Duration = Duration::from_millis(20);
        const SETTLE_BUDGET: Duration = Duration::from_millis(200);

        let mut deadline = Instant::now() + SETTLE_BUDGET;
        if let Some(chain_dl) = chain_deadline {
            deadline = deadline.min(chain_dl);
        }
        loop {
            if disclosed_state(el) == Some(want_expanded) {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    }

    fn disclosed_state(el: &AXElement) -> Option<bool> {
        crate::tree::copy_bool_attr(el, "AXExpanded")
            .or_else(|| crate::tree::copy_bool_attr(el, "AXDisclosing"))
            .or_else(|| value_as_bool(el))
    }

    fn value_as_bool(el: &AXElement) -> Option<bool> {
        match crate::tree::copy_value_typed(el).as_deref() {
            Some("1" | "true" | "True") => Some(true),
            Some("0" | "false" | "False") => Some(false),
            _ => None,
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) use imp::*;
