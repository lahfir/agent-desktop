use agent_desktop_core::{
    error::{AdapterError, ErrorCode},
    interaction_policy::InteractionPolicy,
};

use crate::{
    actions::{
        ax_helpers,
        chain::{ChainContext, execute_chain},
        chain_defs, discovery,
    },
    tree::AXElement,
};

const DEFAULT_TOGGLE_TIMEOUT_MS: u64 = 600;
const MAX_TOGGLE_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_TOGGLE_STABLE_MS: u64 = 200;
const MAX_TOGGLE_STABLE_MS: u64 = 2_000;

pub(crate) fn toggle(el: &AXElement, policy: InteractionPolicy) -> Result<(), AdapterError> {
    let role = ax_helpers::element_role(el);
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
    let before = crate::tree::copy_value_typed(el);
    let caps = discovery::discover(el);
    let ctx = ChainContext {
        dynamic_value: None,
        deadline: None,
    };
    execute_chain(el, &caps, &chain_defs::CLICK_CHAIN, &ctx, policy)?;
    if let Some(before) = before {
        wait_for_value_change(el, &before)?;
    }
    Ok(())
}

pub(crate) fn check_uncheck(
    el: &AXElement,
    want_checked: bool,
    policy: InteractionPolicy,
) -> Result<(), AdapterError> {
    let role = ax_helpers::element_role(el);
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
    if checked_state(el) == Some(want_checked) {
        return Ok(());
    }
    if ax_helpers::is_attr_settable(el, "AXValue")
        && ax_helpers::set_ax_bool(el, "AXValue", want_checked)
        && wait_for_checked_state(el, want_checked).is_ok()
    {
        return Ok(());
    }
    let caps = discovery::discover(el);
    let ctx = ChainContext {
        dynamic_value: None,
        deadline: None,
    };
    execute_chain(el, &caps, &chain_defs::CLICK_CHAIN, &ctx, policy)?;
    wait_for_checked_state(el, want_checked)
}

fn checked_state(el: &AXElement) -> Option<bool> {
    crate::tree::copy_value_typed(el).and_then(|value| parse_checked_value(&value))
}

fn parse_checked_value(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" | "checked" => Some(true),
        "0" | "false" | "no" | "off" | "unchecked" => Some(false),
        "2" | "mixed" | "indeterminate" => None,
        _ => None,
    }
}

fn wait_for_checked_state(el: &AXElement, want_checked: bool) -> Result<(), AdapterError> {
    let deadline = std::time::Instant::now() + toggle_timeout();
    loop {
        if checked_state(el) == Some(want_checked) {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                "check/uncheck did not reach the requested state",
            )
            .with_suggestion("Retry after refreshing the snapshot."));
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

fn wait_for_value_change(el: &AXElement, before: &str) -> Result<(), AdapterError> {
    let deadline = std::time::Instant::now() + toggle_timeout();
    let stable_for = toggle_stable_duration();
    let mut candidate: Option<(String, std::time::Instant)> = None;
    loop {
        if let Some(changed) = crate::tree::copy_value_typed(el) {
            if changed != before {
                match &mut candidate {
                    Some((candidate_value, since)) if candidate_value == &changed => {
                        if since.elapsed() >= stable_for {
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
            .with_suggestion("Use 'click' for controls that do not expose stable toggle state."));
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

fn toggle_timeout() -> std::time::Duration {
    env_duration_ms(
        "AGENT_DESKTOP_TOGGLE_TIMEOUT_MS",
        DEFAULT_TOGGLE_TIMEOUT_MS,
        MAX_TOGGLE_TIMEOUT_MS,
    )
}

fn toggle_stable_duration() -> std::time::Duration {
    env_duration_ms(
        "AGENT_DESKTOP_TOGGLE_STABLE_MS",
        DEFAULT_TOGGLE_STABLE_MS,
        MAX_TOGGLE_STABLE_MS,
    )
}

fn env_duration_ms(name: &str, default_ms: u64, max_ms: u64) -> std::time::Duration {
    let ms = std::env::var(name)
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .filter(|ms| *ms > 0)
        .map(|ms| ms.min(max_ms))
        .unwrap_or(default_ms);
    std::time::Duration::from_millis(ms)
}

#[cfg(test)]
mod tests {
    use super::parse_checked_value;

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
}
