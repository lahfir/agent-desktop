use agent_desktop_core::{
    ActionResult, AdapterError, Deadline, DeliverySemantics, ErrorCode, InteractionPolicy,
    NotificationFilter, NotificationIdentity, NotificationInfo,
};

use crate::tree::element::UIAElement;

use super::list::list_entries;
use super::session::{ActionCenterSession, close_session};
use super::verify::{
    action_changed_state, dismiss_survived_error, entry_gone, read_settling_without, same_identity,
    survivor_failures,
};

#[cfg(test)]
#[path = "actions_tests.rs"]
mod tests;

#[cfg(all(test, target_os = "windows"))]
#[path = "actions_live_tests.rs"]
mod live_tests;

#[cfg(all(test, target_os = "windows"))]
#[path = "actions_gate_tests.rs"]
mod gate_tests;

/// Removes exactly the identified notification and proves the removal by
/// re-reading the surface.
///
/// The foreground floor fires before the center is raised, the entry at the
/// requested index is located by a fresh read, the caller's identity is
/// verified against what that read found - the index is a position, never the
/// identity - and only then is the entry's own dismiss control invoked. An
/// invoke the shell accepts without acting on is reported `ACTION_FAILED`,
/// never success.
pub fn dismiss_notification(
    index: usize,
    app_filter: Option<&str>,
    identity: Option<&NotificationIdentity>,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<NotificationInfo, AdapterError> {
    require_foreground_policy(policy)?;
    let session = ActionCenterSession::open(policy, deadline)?;
    let result = dismiss_impl(index, app_filter, identity, session.hwnd(), deadline);
    close_session(session, result)
}

/// Clears the center and reports the outcome against the identity set captured
/// before the clear.
///
/// Only members of the captured set still present after the clear are
/// failures; entries outside it arrived during the clear and are new arrivals.
/// A partial clear is therefore a success shape carrying per-entry failures,
/// not a refusal - the envelope distinguishes them the way the command's
/// `dismissed_count` and `failures` fields do.
pub fn dismiss_all(
    app_filter: Option<&str>,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<(Vec<NotificationInfo>, Vec<String>), AdapterError> {
    require_foreground_policy(policy)?;
    let session = ActionCenterSession::open(policy, deadline)?;
    let result = dismiss_all_impl(app_filter, session.hwnd(), deadline);
    close_session(session, result)
}

/// Invokes the named action button on the identified notification.
///
/// The name must match one of the entry's action buttons exactly,
/// case-insensitively, the way the macOS adapter matches action names; a
/// notification offering no such button is refused with `ACTION_NOT_SUPPORTED`
/// and is left unchanged, because nothing was invoked.
pub fn notification_action(
    index: usize,
    identity: Option<&NotificationIdentity>,
    action_name: &str,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<ActionResult, AdapterError> {
    require_foreground_policy(policy)?;
    let session = ActionCenterSession::open(policy, deadline)?;
    let result = action_impl(index, identity, action_name, session.hwnd(), deadline);
    close_session(session, result)
}

/// Removes the entry at `index` with its own dismiss control, then verifies.
///
/// Measured on this build (A26-3's own cleanup protocol beside this module's
/// live runs): the shell can accept the entry's dismiss invoke without acting
/// on it, while the top-level clear-all control does act. The clear-all is
/// only a faithful substitute for the entry's own button when the target is
/// the center's sole entry - with any other entry present it would remove
/// notifications nobody asked to remove, so an accepted-and-ignored invoke
/// stays an honest `ACTION_FAILED` there.
fn dismiss_impl(
    index: usize,
    app_filter: Option<&str>,
    identity: Option<&NotificationIdentity>,
    hwnd: isize,
    deadline: Deadline,
) -> Result<NotificationInfo, AdapterError> {
    let filter = build_filter(app_filter);
    let entries = list_entries(&filter, hwnd, deadline)?;
    let entry = entries
        .iter()
        .find(|entry| entry.info.index == index)
        .ok_or_else(|| AdapterError::notification_not_found(index))?;
    verify_identity(index, identity, &entry.info)?;
    let info = entry.info.clone();

    invoke_dismiss(&entry.element, index)?;
    let mut current = read_settling_without(
        std::slice::from_ref(&info),
        hwnd,
        &NotificationFilter::default(),
        deadline,
    )?;
    if !entry_gone(&info, &current)
        && current.len() == 1
        && clear_all_still_the_sole_target(hwnd, &info, deadline)?
    {
        let root = crate::tree::automation::root_from_hwnd(hwnd, deadline)?;
        if let Some(clear) = super::read::find_clear_all_button(&root)? {
            invoke_element(&clear)?;
            current = read_settling_without(
                std::slice::from_ref(&info),
                hwnd,
                &NotificationFilter::default(),
                deadline,
            )?;
        }
    }

    if !entry_gone(&info, &current) {
        return Err(dismiss_survived_error(index));
    }
    Ok(info)
}

/// The clear-all substitute re-verifies, immediately before it fires, that
/// the center still holds exactly one entry and that it is still the target.
///
/// An entry arriving between the settle read and the invoke would be cleared
/// too if the substitute fired on the stale read, and the substitute exists
/// only as a faithful stand-in for the target's own dismiss control - never
/// as a way to clear more than the one identified notification. A re-read
/// that answers anything other than exactly-one-matching aborts the
/// substitute; the caller's survival check then reports the honest
/// `ACTION_FAILED`. A re-read that cannot run surfaces as an error, for the
/// same reason.
#[cfg(target_os = "windows")]
fn clear_all_still_the_sole_target(
    hwnd: isize,
    target: &NotificationInfo,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let current = super::list::list_infos(&NotificationFilter::default(), hwnd, deadline)?;
    Ok(current.len() == 1 && same_identity(target, &current[0]))
}

fn dismiss_all_impl(
    app_filter: Option<&str>,
    hwnd: isize,
    deadline: Deadline,
) -> Result<(Vec<NotificationInfo>, Vec<String>), AdapterError> {
    let filter = build_filter(app_filter);
    let captured: Vec<NotificationInfo> = list_entries(&filter, hwnd, deadline)?
        .into_iter()
        .map(|entry| entry.info)
        .collect();
    if captured.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let root = crate::tree::automation::root_from_hwnd(hwnd, deadline)?;
    let clear = super::read::find_clear_all_button(&root)?
        .ok_or_else(super::read::missing_clear_all_error)?;
    invoke_element(&clear)?;
    let current = read_settling_without(&captured, hwnd, &filter, deadline)?;
    let failures = survivor_failures(&captured, &current);
    let dismissed = captured
        .iter()
        .filter(|member| entry_gone(member, &current))
        .cloned()
        .collect();
    Ok((dismissed, failures))
}

fn action_impl(
    index: usize,
    identity: Option<&NotificationIdentity>,
    action_name: &str,
    hwnd: isize,
    deadline: Deadline,
) -> Result<ActionResult, AdapterError> {
    let filter = NotificationFilter::default();
    let entries = list_entries(&filter, hwnd, deadline)?;
    let entry = entries
        .iter()
        .find(|entry| entry.info.index == index)
        .ok_or_else(|| AdapterError::notification_not_found(index))?;
    verify_identity(index, identity, &entry.info)?;
    let original = entry.info.clone();
    invoke_notification_action(&entry.element, action_name, index)?;
    let current = read_settling_without(std::slice::from_ref(&original), hwnd, &filter, deadline)?;
    let mut result = ActionResult::delivered_unverified(action_name);
    if action_changed_state(&original, &current) {
        result = result.with_verified_delivery();
    }
    Ok(result)
}

fn invoke_dismiss(entry: &UIAElement, index: usize) -> Result<(), AdapterError> {
    let button =
        super::read::find_dismiss_button(entry)?.ok_or_else(|| missing_dismiss_error(index))?;
    invoke_element(&button)
}

fn invoke_notification_action(
    entry: &UIAElement,
    action_name: &str,
    index: usize,
) -> Result<(), AdapterError> {
    let wanted = action_name.to_lowercase();
    for verb in super::read::find_verb_buttons(entry)? {
        if super::read::name_of(&verb).to_lowercase() == wanted {
            return invoke_element(&verb);
        }
    }
    Err(unknown_action_error(action_name, index))
}

#[cfg(target_os = "windows")]
fn invoke_element(element: &UIAElement) -> Result<(), AdapterError> {
    use uiautomation::patterns::UIInvokePattern;

    let pattern: UIInvokePattern = element
        .0
        .get_pattern()
        .map_err(|error| crate::tree::automation::uia_error(&error, "read the Invoke pattern"))?;
    pattern.invoke().map_err(|error| {
        crate::tree::automation::uia_error(&error, "invoke a notification control")
    })
}

#[cfg(not(target_os = "windows"))]
fn invoke_element(_element: &UIAElement) -> Result<(), AdapterError> {
    Err(AdapterError::not_supported("invoke a notification control"))
}

/// The floor every chrome-raising notification command answers to, checked
/// before the session raises anything - a policy that forbids the foreground
/// to move is refused while the desktop is still untouched.
pub(crate) fn require_foreground_policy(policy: InteractionPolicy) -> Result<(), AdapterError> {
    if policy.allow_focus_steal {
        return Ok(());
    }
    Err(AdapterError::policy_denied_for_policy(
        "Action Center interaction requires foreground access",
        policy,
    ))
}

/// The index is not trusted as identity: a caller-supplied expected app or
/// title is compared against what the re-read actually found at that index,
/// and a mismatch is `NOTIFICATION_NOT_FOUND` - the surface reordered under
/// the caller, so acting anyway would dismiss the wrong notification.
fn verify_identity(
    index: usize,
    identity: Option<&NotificationIdentity>,
    info: &NotificationInfo,
) -> Result<(), AdapterError> {
    if identity.is_none_or(|value| value.is_empty() || value.matches(info)) {
        return Ok(());
    }
    Err(AdapterError::new(
        ErrorCode::NotificationNotFound,
        format!("Notification at index {index} no longer matches its expected identity"),
    )
    .with_suggestion("Run list-notifications again and retry with the freshly-observed index"))
}

fn build_filter(app_filter: Option<&str>) -> NotificationFilter {
    NotificationFilter {
        app: app_filter.map(String::from),
        ..Default::default()
    }
}

fn missing_dismiss_error(index: usize) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        format!("The notification at index {index} exposes no 'DismissButton' control to invoke"),
    )
    .with_suggestion("Re-run list-notifications and retry with a freshly observed entry")
    .with_disposition(DeliverySemantics::not_delivered())
}

fn unknown_action_error(action_name: &str, index: usize) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionNotSupported,
        format!("Action '{action_name}' is not offered by the notification at index {index}"),
    )
    .with_suggestion("Run list-notifications and use one of the entry's reported actions")
    .with_disposition(DeliverySemantics::not_delivered())
}
