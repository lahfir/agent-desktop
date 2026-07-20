use agent_desktop_core::{
    ActionStep, AdapterError, Deadline, DeliverySemantics, ErrorCode, InteractionPolicy,
    StepMechanism,
};

use crate::{
    actions::{
        ax_helpers,
        chain::{ChainContext, execute_chain},
        chain_defs,
    },
    tree::AXElement,
};

const TOGGLE_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(600);
const TOGGLE_STABLE: std::time::Duration = std::time::Duration::from_millis(200);

pub(crate) fn toggle(
    el: &AXElement,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<Vec<ActionStep>, AdapterError> {
    let role = read_role(el, deadline)?;
    if !role
        .as_deref()
        .is_some_and(crate::tree::roles::is_toggleable_role)
    {
        return Err(AdapterError::new(
            ErrorCode::ActionNotSupported,
            format!(
                "Toggle not supported on role '{}'",
                role.as_deref().unwrap_or("unknown")
            ),
        )
        .with_suggestion(
            "Toggle works on checkboxes, switches, and radio buttons. Use 'click' for other elements.",
        ));
    }
    let before = read_value(el, deadline)?;
    let ctx = ChainContext {
        dynamic_value: None,
        verified_point: None,
        deadline,
    };
    let mut steps = execute_chain(el, &chain_defs::SEMANTIC_CLICK_CHAIN, &ctx, policy)?;
    let verified = if let Some(before) = before {
        wait_for_value_change(el, &before, deadline).map_err(after_delivery)?;
        true
    } else {
        false
    };
    mark_last_verified(&mut steps, verified);
    Ok(steps)
}

pub(crate) fn check_uncheck(
    el: &AXElement,
    want_checked: bool,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<Vec<ActionStep>, AdapterError> {
    let role = read_role(el, deadline)?;
    if !role
        .as_deref()
        .is_some_and(crate::tree::roles::is_toggleable_role)
    {
        return Err(AdapterError::new(
            ErrorCode::ActionNotSupported,
            format!(
                "check/uncheck not supported on role '{}'",
                role.as_deref().unwrap_or("unknown")
            ),
        )
        .with_suggestion("Only works on checkboxes, switches, and radio buttons."));
    }
    if checked_state(el, deadline)? == Some(want_checked) {
        return Ok(vec![already_in_state_step()]);
    }
    prepare(el, deadline)?;
    if ax_helpers::is_attr_settable(el, "AXValue", deadline)? && {
        prepare(el, deadline)?;
        ax_helpers::set_ax_bool_or_err(el, "AXValue", want_checked, deadline)?
    } {
        wait_for_checked_state(el, want_checked, deadline).map_err(after_delivery)?;
        return Ok(vec![
            ActionStep::succeeded("AXValue")
                .with_mechanism(StepMechanism::SemanticApi)
                .with_verified(true),
        ]);
    }
    let ctx = ChainContext {
        dynamic_value: None,
        verified_point: None,
        deadline,
    };
    let mut steps = execute_chain(el, &chain_defs::SEMANTIC_CLICK_CHAIN, &ctx, policy)?;
    wait_for_checked_state(el, want_checked, deadline).map_err(after_delivery)?;
    mark_last_verified(&mut steps, true);
    Ok(steps)
}

fn already_in_state_step() -> ActionStep {
    ActionStep::skipped("AlreadyInState").with_verified(true)
}

fn after_delivery(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::delivered_unverified())
}

fn mark_last_verified(steps: &mut [ActionStep], verified: bool) {
    if let Some(step) = steps.last_mut() {
        if verified || step.verified.is_none() {
            step.verified = Some(verified);
        }
    }
}

fn checked_state(el: &AXElement, deadline: Deadline) -> Result<Option<bool>, AdapterError> {
    Ok(read_value(el, deadline)?.and_then(|value| parse_checked_value(&value)))
}

fn parse_checked_value(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" | "checked" => Some(true),
        "0" | "false" | "no" | "off" | "unchecked" => Some(false),
        "2" | "mixed" | "indeterminate" => None,
        _ => None,
    }
}

fn wait_for_checked_state(
    el: &AXElement,
    want_checked: bool,
    action_deadline: Deadline,
) -> Result<(), AdapterError> {
    let deadline = verification_deadline(action_deadline)?;
    loop {
        if checked_state(el, action_deadline)? == Some(want_checked) {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                "check/uncheck did not reach the requested state",
            )
            .with_details(serde_json::json!({
                "verification": "requested_checked_state_not_observed"
            }))
            .with_suggestion(
                "Refresh the snapshot and inspect the checked state before deciding whether to retry.",
            ));
        }
        sleep_poll(deadline, action_deadline)?;
    }
}

fn wait_for_value_change(
    el: &AXElement,
    before: &str,
    action_deadline: Deadline,
) -> Result<(), AdapterError> {
    let deadline = verification_deadline(action_deadline)?;
    let mut candidate: Option<(String, std::time::Instant)> = None;
    loop {
        if let Some(changed) = read_value(el, action_deadline)? {
            if changed != before {
                match &mut candidate {
                    Some((candidate_value, since)) if candidate_value == &changed => {
                        if since.elapsed() >= TOGGLE_STABLE {
                            return Ok(());
                        }
                    }
                    _ => {
                        candidate = Some((changed, std::time::Instant::now()));
                    }
                }
            } else {
                candidate = None;
            }
        }
        if std::time::Instant::now() >= deadline {
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                "toggle did not change the element value",
            )
            .with_details(serde_json::json!({
                "verification": "stable_value_change_not_observed"
            }))
            .with_suggestion(
                "Refresh the snapshot and inspect the value before deciding whether to retry or use 'click'.",
            ));
        }
        sleep_poll(deadline, action_deadline)?;
    }
}

fn verification_deadline(action_deadline: Deadline) -> Result<std::time::Instant, AdapterError> {
    let local = std::time::Instant::now() + TOGGLE_TIMEOUT;
    let remaining = action_deadline.remaining();
    if remaining.is_zero() {
        Err(action_deadline.timeout_error())
    } else {
        Ok(std::time::Instant::now()
            .checked_add(remaining)
            .map_or(local, |deadline| deadline.min(local)))
    }
}

fn sleep_poll(deadline: std::time::Instant, action_deadline: Deadline) -> Result<(), AdapterError> {
    let remaining = deadline.saturating_duration_since(std::time::Instant::now());
    if !remaining.is_zero() {
        std::thread::sleep(remaining.min(std::time::Duration::from_millis(25)));
    }
    if action_deadline.is_expired() {
        Err(action_deadline
            .timeout_error()
            .with_details(serde_json::json!({
                "verification": "action_deadline_elapsed",
            })))
    } else {
        Ok(())
    }
}

fn read_value(el: &AXElement, deadline: Deadline) -> Result<Option<String>, AdapterError> {
    use accessibility_sys::{
        kAXErrorAPIDisabled, kAXErrorCannotComplete, kAXErrorInvalidUIElement,
    };

    crate::tree::attributes::set_messaging_timeout(el, deadline)?;
    let result = crate::tree::attributes::copy_value_typed_result(el, deadline);
    if deadline.is_expired() {
        return Err(deadline.timeout_error());
    }
    result.map_err(|error| {
        let code = if error == kAXErrorAPIDisabled {
            ErrorCode::PermDenied
        } else if error == kAXErrorCannotComplete {
            ErrorCode::Timeout
        } else if error == kAXErrorInvalidUIElement {
            ErrorCode::StaleRef
        } else {
            ErrorCode::ActionFailed
        };
        AdapterError::new(code, "Could not verify the live toggle value")
            .with_details(serde_json::json!({ "ax_error": error }))
    })
}

fn read_role(el: &AXElement, deadline: Deadline) -> Result<Option<String>, AdapterError> {
    ax_helpers::element_role(el, deadline)
}

fn prepare(el: &AXElement, deadline: Deadline) -> Result<(), AdapterError> {
    crate::tree::attributes::set_messaging_timeout(el, deadline)
}

#[cfg(test)]
mod tests {
    use agent_desktop_core::action_step::ActionStep;

    use super::{already_in_state_step, mark_last_verified, parse_checked_value};

    #[test]
    fn parses_checked_values_from_common_ax_strings() {
        for value in ["1", "true", "TRUE", "YES", "on", "checked"] {
            assert_eq!(parse_checked_value(value), Some(true));
        }
        for value in ["0", "false", "FALSE", "NO", "off", "unchecked"] {
            assert_eq!(parse_checked_value(value), Some(false));
        }
    }

    #[test]
    fn treats_mixed_and_unknown_checked_values_as_indeterminate() {
        for value in ["2", "mixed", "indeterminate", "maybe", ""] {
            assert_eq!(parse_checked_value(value), None);
        }
    }

    #[test]
    fn absent_toggle_state_does_not_erase_existing_verification() {
        let mut steps = vec![ActionStep::succeeded("verified_press").with_verified(true)];
        mark_last_verified(&mut steps, false);
        assert_eq!(steps[0].verified(), Some(true));
    }

    #[test]
    fn toggle_state_verification_upgrades_an_unverified_step() {
        let mut steps = vec![ActionStep::succeeded("AXPress").with_verified(false)];
        mark_last_verified(&mut steps, true);
        assert_eq!(steps[0].verified(), Some(true));
    }

    #[test]
    fn already_in_state_is_verified_without_claiming_delivery() {
        let step = already_in_state_step();
        assert!(matches!(
            step.outcome,
            agent_desktop_core::action_step_outcome::ActionStepOutcome::Skipped
        ));
        assert!(step.mechanism().is_none());
        assert_eq!(step.verified(), Some(true));
    }
}
