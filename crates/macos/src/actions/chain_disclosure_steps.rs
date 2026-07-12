#[cfg(target_os = "macos")]
mod imp {
    use crate::actions::chain_delivery::DeliveryOutcome;
    use crate::tree::AXElement;
    use agent_desktop_core::{AdapterError, Deadline};
    use std::time::{Duration, Instant};

    pub(crate) fn press_to_expand(
        element: &AXElement,
        deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        set_disclosure(element, true, deadline)
    }

    pub(crate) fn press_to_collapse(
        element: &AXElement,
        deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        set_disclosure(element, false, deadline)
    }

    fn set_disclosure(
        element: &AXElement,
        expanded: bool,
        deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        let Some(current) = disclosed_state(element, deadline)? else {
            return Ok(DeliveryOutcome::NotDelivered);
        };
        if current == expanded {
            return Ok(DeliveryOutcome::SatisfiedNoDelivery);
        }
        let action = if expanded { "AXExpand" } else { "AXCollapse" };
        prepare(element, deadline)?;
        if crate::actions::ax_helpers::try_ax_action_or_err(element, action, deadline)? {
            return verify_disclosure(element, expanded, deadline).map_err(after_delivery);
        }
        prepare(element, deadline)?;
        if crate::actions::ax_helpers::is_attr_settable(element, "AXExpanded", deadline)? {
            prepare(element, deadline)?;
            if crate::actions::ax_helpers::set_ax_bool_or_err(
                element,
                "AXExpanded",
                expanded,
                deadline,
            )? {
                return verify_disclosure(element, expanded, deadline).map_err(after_delivery);
            }
        }
        prepare(element, deadline)?;
        if crate::actions::ax_helpers::try_ax_action_or_err(element, "AXPress", deadline)? {
            return verify_disclosure(element, expanded, deadline).map_err(after_delivery);
        }
        Ok(DeliveryOutcome::NotDelivered)
    }

    fn verify_disclosure(
        element: &AXElement,
        expanded: bool,
        deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        let local_end = Instant::now() + Duration::from_millis(200);
        loop {
            if disclosed_state(element, deadline)? == Some(expanded) {
                return Ok(DeliveryOutcome::DeliveredVerified);
            }
            if deadline.is_expired() {
                return Err(deadline.timeout_error().with_details(serde_json::json!({
                    "verification": "expanded_state_not_observed",
                })));
            }
            if Instant::now() >= local_end {
                return Ok(DeliveryOutcome::DeliveredUnverified);
            }
            let pause = deadline.remaining_slice(Duration::from_millis(20))?;
            std::thread::sleep(pause.min(Duration::from_millis(20)));
        }
    }

    fn disclosed_state(
        element: &AXElement,
        deadline: Deadline,
    ) -> Result<Option<bool>, AdapterError> {
        let instant = crate::tree::locator_deadline::from_operation(deadline)?;
        if let Some(value) = crate::tree::surface_read::boolean(element, "AXExpanded", instant)? {
            return Ok(Some(value));
        }
        crate::tree::surface_read::boolean(element, "AXDisclosing", instant)
    }

    fn prepare(element: &AXElement, deadline: Deadline) -> Result<(), AdapterError> {
        crate::tree::attributes::set_messaging_timeout(element, deadline)
    }

    fn after_delivery(error: AdapterError) -> AdapterError {
        let mut delivery = crate::delivery_tracker::DeliveryTracker::default();
        delivery.mark_delivered();
        delivery.annotate(error)
    }
}

#[cfg(target_os = "macos")]
pub(crate) use imp::{press_to_collapse, press_to_expand};
