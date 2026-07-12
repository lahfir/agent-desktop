use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};

use crate::tree::AXElement;

const MAX_SELECT_NODES: usize = 2_048;
const MAX_SELECT_DEPTH: u8 = 8;

pub(crate) fn select_from_menu(
    element: &AXElement,
    value: &str,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let pid = require_pid(element, deadline)?;
    let identity = require_identity(pid)?;
    if menu_root(pid, deadline)?.is_some() {
        return Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "Select refused to reuse a menu that was already open",
        )
        .with_suggestion("Dismiss the open menu and retry select."));
    }
    let exposes_value = crate::tree::copy_value_typed(element, deadline).is_some();
    let delivered = open_menu(element, deadline)?;
    if !delivered {
        return Err(AdapterError::new(
            ErrorCode::ActionNotSupported,
            "The target did not support AXShowMenu or AXPress",
        )
        .with_suggestion("Use click to open the control, then snapshot the menu."));
    }
    let result = select_open_menu_item(pid, identity, value, deadline);
    match result {
        Ok(_) if exposes_value => {
            verify_selected_value(element, value, deadline).map_err(after_menu_delivery)
        }
        Ok(verified) => Ok(verified),
        Err(error) => {
            let _ = crate::actions::ax_helpers::try_ax_action_or_err(element, "AXCancel", deadline);
            Err(error.with_disposition(DeliverySemantics::delivered_unverified()))
        }
    }
}

fn verify_selected_value(
    element: &AXElement,
    expected: &str,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let local_end = std::time::Instant::now() + std::time::Duration::from_millis(600);
    loop {
        prepare(element, deadline)?;
        if crate::tree::copy_value_typed(element, deadline)
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case(expected))
        {
            return Ok(true);
        }
        if deadline.is_expired() {
            return Err(deadline.timeout_error().with_details(serde_json::json!({
                "kind": "select_value_not_observed",
                "complete": false,
            })));
        }
        if std::time::Instant::now() >= local_end {
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                "The menu item was activated but the control value did not change",
            ));
        }
        let pause = deadline.remaining_slice(std::time::Duration::from_millis(25))?;
        std::thread::sleep(pause.min(std::time::Duration::from_millis(25)));
    }
}

fn after_menu_delivery(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::delivered_unverified())
}

pub(crate) fn select_collection_item(
    element: &AXElement,
    value: &str,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let Some(candidate) = find_named_descendant(element, value, deadline)? else {
        return Err(AdapterError::new(
            ErrorCode::ElementNotFound,
            format!(
                "No collection item matched the requested value ({} chars)",
                value.chars().count()
            ),
        )
        .with_suggestion("Use find to inspect the collection's available items."));
    };
    select_collection_candidate(&candidate, deadline)
}

fn open_menu(element: &AXElement, deadline: Deadline) -> Result<bool, AdapterError> {
    prepare(element, deadline)?;
    if crate::actions::ax_helpers::try_ax_action_or_err(element, "AXShowMenu", deadline)? {
        return Ok(true);
    }
    prepare(element, deadline)?;
    crate::actions::ax_helpers::try_ax_action_or_err(element, "AXPress", deadline)
}

fn select_open_menu_item(
    pid: i32,
    identity: crate::system::process_identity::ProcessIdentity,
    value: &str,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    loop {
        if !identity.still_matches()? {
            return Err(AdapterError::new(
                ErrorCode::AppUnresponsive,
                "The target process changed while selecting a menu item",
            ));
        }
        if let Some(menu) = menu_root(pid, deadline)?
            && let Some(candidate) = find_named_descendant(&menu, value, deadline)?
        {
            let verified = activate_menu_item(&candidate, deadline)?;
            return if verified {
                Ok(true)
            } else {
                wait_for_menu_to_close(pid, deadline)
            };
        }
        if deadline.is_expired() {
            return Err(deadline.timeout_error().with_details(serde_json::json!({
                "kind": "select_menu_item_not_found",
                "requested_chars": value.chars().count(),
                "complete": false,
            })));
        }
        let pause = deadline.remaining_slice(std::time::Duration::from_millis(25))?;
        std::thread::sleep(pause.min(std::time::Duration::from_millis(25)));
    }
}

fn find_named_descendant(
    root: &AXElement,
    value: &str,
    deadline: Deadline,
) -> Result<Option<AXElement>, AdapterError> {
    let mut stack = vec![(root.clone(), 0_u8)];
    let mut visited = 0_usize;
    while let Some((candidate, depth)) = stack.pop() {
        visited = visited.saturating_add(1);
        if visited > MAX_SELECT_NODES {
            return Err(AdapterError::new(
                ErrorCode::AppUnresponsive,
                "Select search exceeded its accessibility-node budget",
            )
            .with_details(serde_json::json!({
                "kind": "select_node_limit",
                "limit": MAX_SELECT_NODES,
                "complete": false,
            })));
        }
        let instant = crate::tree::locator_deadline::from_operation(deadline)?;
        if candidate_matches(&candidate, value, instant)? {
            return Ok(Some(candidate));
        }
        if depth >= MAX_SELECT_DEPTH {
            continue;
        }
        let instant = crate::tree::locator_deadline::from_operation(deadline)?;
        stack.extend(
            crate::tree::surface_read::elements(&candidate, "AXChildren", instant)?
                .into_iter()
                .rev()
                .map(|child| (child, depth.saturating_add(1))),
        );
    }
    Ok(None)
}

fn candidate_matches(
    candidate: &AXElement,
    value: &str,
    deadline: std::time::Instant,
) -> Result<bool, AdapterError> {
    for attribute in ["AXTitle", "AXDescription"] {
        if crate::tree::surface_read::string(candidate, attribute, deadline)?
            .as_deref()
            .is_some_and(|text| text.eq_ignore_ascii_case(value))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn activate_menu_item(candidate: &AXElement, deadline: Deadline) -> Result<bool, AdapterError> {
    deliver_candidate(false, || Ok(None), || press_candidate(candidate, deadline))
}

fn select_collection_candidate(
    candidate: &AXElement,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    deliver_candidate(
        true,
        || select_candidate_attribute(candidate, deadline),
        || press_candidate(candidate, deadline),
    )
}

fn deliver_candidate(
    allow_selected_attribute: bool,
    mut select_attribute: impl FnMut() -> Result<Option<bool>, AdapterError>,
    mut press: impl FnMut() -> Result<bool, AdapterError>,
) -> Result<bool, AdapterError> {
    let selected_error = if allow_selected_attribute {
        match select_attribute() {
            Ok(Some(verified)) => return Ok(verified),
            Ok(None) => None,
            Err(error) => Some(error),
        }
    } else {
        None
    };
    if press()? {
        return Ok(false);
    }
    if let Some(error) = selected_error {
        return Err(error);
    }
    Err(AdapterError::new(
        ErrorCode::ActionNotSupported,
        "The matching selection item did not support AXSelected or AXPress",
    )
    .with_disposition(DeliverySemantics::not_delivered()))
}

fn select_candidate_attribute(
    candidate: &AXElement,
    deadline: Deadline,
) -> Result<Option<bool>, AdapterError> {
    prepare(candidate, deadline)?;
    if !crate::actions::ax_helpers::is_attr_settable(candidate, "AXSelected", deadline)? {
        return Ok(None);
    }
    prepare(candidate, deadline)?;
    if !crate::actions::ax_helpers::set_ax_bool_or_err(candidate, "AXSelected", true, deadline)? {
        return Ok(None);
    }
    let instant = crate::tree::locator_deadline::from_operation(deadline)?;
    Ok(Some(
        crate::tree::surface_read::boolean(candidate, "AXSelected", instant)? == Some(true),
    ))
}

fn press_candidate(candidate: &AXElement, deadline: Deadline) -> Result<bool, AdapterError> {
    prepare(candidate, deadline)?;
    crate::actions::ax_helpers::try_ax_action_or_err(candidate, "AXPress", deadline)
}

fn wait_for_menu_to_close(pid: i32, deadline: Deadline) -> Result<bool, AdapterError> {
    let local_end = std::time::Instant::now() + std::time::Duration::from_millis(600);
    loop {
        if menu_root(pid, deadline)?.is_none() {
            return Ok(true);
        }
        if deadline.is_expired() || std::time::Instant::now() >= local_end {
            return Ok(false);
        }
        let pause = deadline.remaining_slice(std::time::Duration::from_millis(25))?;
        std::thread::sleep(pause.min(std::time::Duration::from_millis(25)));
    }
}

fn menu_root(pid: i32, deadline: Deadline) -> Result<Option<AXElement>, AdapterError> {
    let instant = crate::tree::locator_deadline::from_operation(deadline)?;
    crate::tree::surfaces::menu_element_for_pid(pid, instant)
}

fn require_pid(element: &AXElement, deadline: Deadline) -> Result<i32, AdapterError> {
    crate::system::app_ops::pid_from_element(element, deadline).ok_or_else(|| {
        AdapterError::new(
            ErrorCode::AppUnresponsive,
            "Could not determine the target process for menu selection",
        )
    })
}

fn require_identity(
    pid: i32,
) -> Result<crate::system::process_identity::ProcessIdentity, AdapterError> {
    crate::system::process_identity::ProcessIdentity::capture(pid)?.ok_or_else(|| {
        AdapterError::new(
            ErrorCode::AppUnresponsive,
            "The target process exited before menu selection",
        )
    })
}

fn prepare(element: &AXElement, deadline: Deadline) -> Result<(), AdapterError> {
    crate::tree::attributes::set_messaging_timeout(element, deadline)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn select_traversal_limits_are_bounded() {
        assert_eq!(MAX_SELECT_NODES, 2_048);
        assert_eq!(MAX_SELECT_DEPTH, 8);
    }

    #[test]
    fn menu_items_are_pressed_without_setting_selected_first() {
        let selected_calls = Cell::new(0);
        let press_calls = Cell::new(0);

        let verified = deliver_candidate(
            false,
            || {
                selected_calls.set(selected_calls.get() + 1);
                Ok(Some(true))
            },
            || {
                press_calls.set(press_calls.get() + 1);
                Ok(true)
            },
        )
        .expect("menu candidate delivery");

        assert!(!verified);
        assert_eq!(selected_calls.get(), 0);
        assert_eq!(press_calls.get(), 1);
    }

    #[test]
    fn collection_selection_falls_back_to_press_when_selected_write_fails() {
        let press_calls = Cell::new(0);
        let verified = deliver_candidate(
            true,
            || {
                Err(AdapterError::new(
                    ErrorCode::ActionFailed,
                    "AXSelected is unavailable",
                ))
            },
            || {
                press_calls.set(press_calls.get() + 1);
                Ok(true)
            },
        )
        .expect("press fallback");

        assert!(!verified);
        assert_eq!(press_calls.get(), 1);
    }
}
