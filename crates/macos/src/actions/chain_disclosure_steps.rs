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
        let (satisfied, allow_toggle) =
            disclosure_plan(disclosed_state(element, deadline)?, expanded);
        if satisfied {
            return Ok(DeliveryOutcome::SatisfiedNoDelivery);
        }
        let action = if expanded { "AXExpand" } else { "AXCollapse" };
        prepare(element, deadline)?;
        if crate::actions::ax_helpers::try_ax_action_or_err(element, action, deadline)? {
            if verify_disclosure(element, expanded, deadline).map_err(after_delivery)?
                == DeliveryOutcome::DeliveredVerified
            {
                return Ok(DeliveryOutcome::DeliveredVerified);
            }
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
                if verify_disclosure(element, expanded, deadline).map_err(after_delivery)?
                    == DeliveryOutcome::DeliveredVerified
                {
                    return Ok(DeliveryOutcome::DeliveredVerified);
                }
            }
        }
        if allow_toggle && disclosed_state(element, deadline)? == Some(!expanded) {
            prepare(element, deadline)?;
            if crate::actions::ax_helpers::try_ax_action_or_err(element, "AXPress", deadline)? {
                if verify_disclosure(element, expanded, deadline).map_err(after_delivery)?
                    == DeliveryOutcome::DeliveredVerified
                {
                    return Ok(DeliveryOutcome::DeliveredVerified);
                }
            }
        }
        Ok(DeliveryOutcome::NotDelivered)
    }

    fn disclosure_plan(current: Option<bool>, desired: bool) -> (bool, bool) {
        (current == Some(desired), current == Some(!desired))
    }

    fn verify_disclosure(
        element: &AXElement,
        expanded: bool,
        deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        let local_end = Instant::now() + Duration::from_millis(200);
        loop {
            let current_state = disclosed_state(element, deadline)?;
            match current_state {
                Some(current) if current == expanded => {
                    return Ok(DeliveryOutcome::DeliveredVerified);
                }
                _ => {}
            }
            if deadline.is_expired() {
                return Err(deadline.timeout_error().with_details(serde_json::json!({
                    "verification": "expanded_state_not_observed",
                })));
            }
            if Instant::now() >= local_end {
                return unobserved_delivery(current_state, expanded);
            }
            let pause = deadline.remaining_slice(Duration::from_millis(20))?;
            std::thread::sleep(pause.min(Duration::from_millis(20)));
        }
    }

    fn unobserved_delivery(
        last_state: Option<bool>,
        expanded: bool,
    ) -> Result<DeliveryOutcome, AdapterError> {
        if last_state == Some(!expanded) {
            Ok(DeliveryOutcome::NotDelivered)
        } else {
            Ok(DeliveryOutcome::DeliveredUnverified)
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

    #[cfg(test)]
    mod tests {
        use super::{DeliveryOutcome, disclosure_plan, unobserved_delivery};

        #[test]
        fn disclosure_plan_never_blindly_toggles_unknown_state() {
            assert_eq!(disclosure_plan(Some(true), true), (true, false));
            assert_eq!(disclosure_plan(Some(false), true), (false, true));
            assert_eq!(disclosure_plan(None, true), (false, false));
            assert_eq!(disclosure_plan(Some(false), false), (true, false));
            assert_eq!(disclosure_plan(Some(true), false), (false, true));
            assert_eq!(disclosure_plan(None, false), (false, false));
        }

        #[test]
        fn unobserved_delivery_preserves_honest_unknown_state() {
            assert_eq!(
                unobserved_delivery(Some(false), true).expect("known opposite state"),
                DeliveryOutcome::NotDelivered
            );
            assert_eq!(
                unobserved_delivery(None, true).expect("unreadable state after delivery"),
                DeliveryOutcome::DeliveredUnverified
            );
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) use imp::{press_to_collapse, press_to_expand};
