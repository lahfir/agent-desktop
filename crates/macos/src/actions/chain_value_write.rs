#[cfg(target_os = "macos")]
mod imp {
    use agent_desktop_core::{AdapterError, Deadline};

    use crate::actions::{ax_helpers, chain_delivery::DeliveryOutcome, chain_verify};
    use crate::tree::AXElement;

    pub(crate) fn set_dynamic_verified(
        element: &AXElement,
        attribute: &str,
        value: &str,
        deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        prepare(element, deadline)?;
        if attribute == "AXValue" {
            ax_helpers::set_ax_value_coerced(element, value, deadline)?;
        } else {
            ax_helpers::set_ax_string_or_err(element, attribute, value, deadline)?;
        }
        let mut delivery = crate::delivery_tracker::DeliveryTracker::default();
        delivery.mark_delivered();
        prepare(element, deadline).map_err(|error| delivery.annotate(error))?;
        let role = ax_helpers::element_role(element, deadline)
            .map_err(|error| delivery.annotate(error))?;
        prepare(element, deadline).map_err(|error| delivery.annotate(error))?;
        let observed = crate::tree::copy_value_typed(element, deadline);
        Ok(DeliveryOutcome::from_delivery(
            true,
            chain_verify::dynamic_write_had_effect(
                attribute,
                role.as_deref(),
                value,
                observed.as_deref(),
            ),
        ))
    }

    pub(crate) fn increment_to_value(
        element: &AXElement,
        target: &str,
        deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        const MAX_INCREMENT_STEPS: usize = 1_024;

        let Some(target) = finite_target(target) else {
            return Ok(DeliveryOutcome::NotDelivered);
        };
        let Some(mut current) = read_number(element, deadline)? else {
            return Ok(DeliveryOutcome::NotDelivered);
        };
        let start = current;
        let mut delivered = false;
        let mut delivery = crate::delivery_tracker::DeliveryTracker::default();
        for _ in 0..MAX_INCREMENT_STEPS {
            if (current - target).abs() < 0.5 {
                return Ok(if delivered {
                    DeliveryOutcome::DeliveredVerified
                } else {
                    DeliveryOutcome::SatisfiedNoDelivery
                });
            }
            if deadline.is_expired() {
                return Err(chain_verify::increment_deadline_error(
                    start, current, target,
                ));
            }
            let action = if current < target {
                "AXIncrement"
            } else {
                "AXDecrement"
            };
            prepare(element, deadline).map_err(|error| delivery.annotate(error))?;
            let delivered_step = match ax_helpers::try_ax_action_or_err(element, action, deadline) {
                Ok(delivered) => delivered,
                Err(error) if delivered => return Err(delivery.annotate(error)),
                Err(error) => return Err(error),
            };
            if !delivered_step {
                break;
            }
            delivered = true;
            delivery.mark_delivered();
            match read_number(element, deadline).map_err(|error| delivery.annotate(error))? {
                Some(next) if (next - current).abs() >= f64::EPSILON => current = next,
                _ => break,
            }
        }
        if (current - target).abs() < 0.5 {
            return Ok(DeliveryOutcome::DeliveredVerified);
        }
        if (current - start).abs() >= f64::EPSILON {
            return Err(chain_verify::increment_step_limit_error(
                start, current, target,
            ));
        }
        Ok(DeliveryOutcome::from_delivery(delivered, false))
    }

    pub(crate) fn set_bool_verified(
        element: &AXElement,
        attribute: &str,
        value: bool,
        deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        prepare(element, deadline)?;
        let delivered = ax_helpers::set_ax_bool_or_err(element, attribute, value, deadline)?;
        if !delivered {
            return Ok(DeliveryOutcome::NotDelivered);
        }
        let mut delivery = crate::delivery_tracker::DeliveryTracker::default();
        delivery.mark_delivered();
        prepare(element, deadline).map_err(|error| delivery.annotate(error))?;
        let observed = crate::tree::copy_bool_attr(element, attribute, deadline);
        Ok(DeliveryOutcome::from_delivery(
            delivered,
            delivered && chain_verify::bool_write_had_effect(attribute, value, observed),
        ))
    }

    pub(crate) fn finite_target(target: &str) -> Option<f64> {
        target.parse::<f64>().ok().filter(|value| value.is_finite())
    }

    fn read_number(element: &AXElement, deadline: Deadline) -> Result<Option<f64>, AdapterError> {
        prepare(element, deadline)?;
        Ok(crate::tree::copy_value_typed(element, deadline)
            .and_then(|value| value.parse::<f64>().ok()))
    }

    fn prepare(element: &AXElement, deadline: Deadline) -> Result<(), AdapterError> {
        crate::tree::attributes::set_messaging_timeout(element, deadline)
    }

    #[cfg(test)]
    pub(crate) fn verification_failure_after_write(error: AdapterError) -> AdapterError {
        let mut delivery = crate::delivery_tracker::DeliveryTracker::default();
        delivery.mark_delivered();
        delivery.annotate(error)
    }
}

#[cfg(target_os = "macos")]
pub(crate) use imp::{increment_to_value, set_bool_verified, set_dynamic_verified};

#[cfg(all(test, target_os = "macos"))]
use imp::finite_target;

#[cfg(all(test, target_os = "macos"))]
use imp::verification_failure_after_write;

#[cfg(test)]
#[path = "chain_value_write_tests.rs"]
mod tests;
