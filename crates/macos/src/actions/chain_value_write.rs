#[cfg(target_os = "macos")]
mod imp {
    use agent_desktop_core::error::AdapterError;
    use std::time::Instant;

    use crate::actions::{ax_helpers, chain_verify};
    use crate::tree::AXElement;

    pub(crate) fn set_dynamic_verified(
        el: &AXElement,
        attr: &str,
        value: &str,
    ) -> Result<bool, AdapterError> {
        if attr == "AXValue" {
            ax_helpers::set_ax_value_coerced(el, value)?;
        } else {
            ax_helpers::set_ax_string_or_err(el, attr, value)?;
        }
        Ok(chain_verify::dynamic_write_had_effect(
            attr,
            ax_helpers::element_role(el).as_deref(),
            value,
            crate::tree::copy_value_typed(el).as_deref(),
        ))
    }

    /// Drives AXIncrement/AXDecrement until the control reaches `target`.
    /// Steppers and some sliders expose no settable AXValue but step through
    /// these actions. Stops on reaching the target or on no observable
    /// progress (the action stopped moving the value). Deadline expiry is a
    /// hard error: the control may sit at a half-applied value, and silently
    /// reporting "step failed" would mask that mutation as ACTION_FAILED with
    /// recovery guidance pointing the wrong way.
    pub(crate) fn increment_to_value(
        el: &AXElement,
        target: &str,
        deadline: Option<Instant>,
    ) -> Result<bool, AdapterError> {
        const MAX_INCREMENT_STEPS: usize = 1024;

        let target = match finite_target(target) {
            Some(target) => target,
            None => return Ok(false),
        };
        let read = || crate::tree::copy_value_typed(el).and_then(|v| v.parse::<f64>().ok());
        let mut current = match read() {
            Some(c) => c,
            None => return Ok(false),
        };
        let actions = ax_helpers::list_ax_actions(el);
        if !actions.iter().any(|action| action == "AXIncrement")
            && !actions.iter().any(|action| action == "AXDecrement")
        {
            return Ok(false);
        }
        let start = current;
        for _ in 0..MAX_INCREMENT_STEPS {
            if (current - target).abs() < 0.5 {
                return Ok(true);
            }
            if deadline.is_some_and(|dl| Instant::now() > dl) {
                return Err(chain_verify::increment_deadline_error(
                    start, current, target,
                ));
            }
            let action = if current < target {
                "AXIncrement"
            } else {
                "AXDecrement"
            };
            if !ax_helpers::try_ax_action(el, action) {
                break;
            }
            match read() {
                Some(next) if (next - current).abs() >= f64::EPSILON => current = next,
                _ => break,
            }
        }
        if (current - target).abs() < 0.5 {
            return Ok(true);
        }
        if (current - start).abs() >= f64::EPSILON {
            return Err(chain_verify::increment_step_limit_error(
                start, current, target,
            ));
        }
        Ok(false)
    }

    pub(crate) fn finite_target(target: &str) -> Option<f64> {
        target.parse::<f64>().ok().filter(|value| value.is_finite())
    }

    pub(crate) fn set_bool_verified(
        el: &AXElement,
        attr: &str,
        value: bool,
    ) -> Result<bool, AdapterError> {
        Ok(ax_helpers::set_ax_bool_or_err(el, attr, value)?
            && chain_verify::bool_write_had_effect(
                attr,
                value,
                crate::tree::copy_bool_attr(el, attr),
            ))
    }
}

#[cfg(target_os = "macos")]
pub(crate) use imp::{increment_to_value, set_bool_verified, set_dynamic_verified};

#[cfg(all(test, target_os = "macos"))]
use imp::finite_target;

#[cfg(test)]
#[path = "chain_value_write_tests.rs"]
mod tests;
