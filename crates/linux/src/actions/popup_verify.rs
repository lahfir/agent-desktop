use super::click::{POLL_STEP, POPUP_SETTLE, opens_popup, poll_until};
use super::context::Target;
use crate::system::focus::focus_cause::Cause;
use crate::system::menu::menu_surface::roots_below;
use crate::system::menu::overview::overview_open;
use crate::tree::AtspiElement;
use crate::tree::capabilities::action_read::{NodeView, read_node_view};
use crate::tree::vocabulary::role_numbers::{PUSH_BUTTON, PUSH_BUTTON_MENU, TOGGLE_BUTTON};
use agent_desktop_core::{ActionResult, AdapterError, DeliverySemantics, ErrorCode};

fn opener(target: &Target<'_>, view: &NodeView) -> Result<Option<AtspiElement>, AdapterError> {
    if matches!(view.role(), Some(PUSH_BUTTON | PUSH_BUTTON_MENU)) && opens_popup(view) {
        return Ok(Some(target.element.clone()));
    }
    if view.role() != Some(TOGGLE_BUTTON) {
        return Ok(None);
    }
    let Some(parent) = target.element.ancestry().button_parent() else {
        return Ok(None);
    };
    let handle = parent.clone().into_native_handle();
    let view = read_node_view(target.bus, &handle, target.deadline)
        .map_err(|error| error.with_disposition(DeliverySemantics::not_delivered()))?;
    if view
        .state(crate::tree::vocabulary::state_bits::HAS_POPUP)
        .is_none()
    {
        return Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "The parent button's popup capability could not be read",
        )
        .with_disposition(DeliverySemantics::not_delivered()));
    }
    Ok(opens_popup(&view).then_some(parent))
}

fn matches(wanted: Option<bool>, before: &[AtspiElement], after: &[AtspiElement]) -> bool {
    match wanted {
        Some(open) => after.is_empty() != open,
        None => before != after,
    }
}

/// Names the Activities Overview as the cause of a popup that did not appear
/// when GNOME Shell reports it open, with the cause and the advice a
/// `focus-window` that the Overview refused gives. Any other answer leaves the
/// error as it is.
pub(super) fn name_open_overview(mut error: AdapterError, overview: Option<bool>) -> AdapterError {
    if overview != Some(true) {
        return error;
    }
    let cause = Cause::OverviewOpen;
    let details = error.details.get_or_insert_with(|| serde_json::json!({}));
    if let Some(fields) = details.as_object_mut() {
        fields.insert("cause".to_owned(), serde_json::json!(cause.as_str()));
    }
    error.with_suggestion(cause.suggestion())
}

/// The popup of an opener is verified or the action fails with
/// `popup_not_observed`. A failure with no popup on screen reads the Overview
/// once: an open Overview explains a popup that never appeared, not one that
/// stayed open or one that could not be read.
fn confirm(
    result: ActionResult,
    wanted: Option<bool>,
    before: &[AtspiElement],
    after: Option<&[AtspiElement]>,
    overview: impl FnOnce() -> Option<bool>,
) -> Result<ActionResult, AdapterError> {
    if after.is_some_and(|after| matches(wanted, before, after)) {
        return Ok(result);
    }
    let disposition = if result.disposition() == DeliverySemantics::not_delivered() {
        DeliverySemantics::not_delivered()
    } else {
        DeliverySemantics::delivered_unverified()
    };
    let mut steps = result.steps;
    for step in &mut steps {
        step.verified = Some(false);
    }
    let error = AdapterError::new(
        ErrorCode::ActionFailed,
        "The requested popup was not observed after the opener action",
    )
    .with_suggestion(
        "Inspect a fresh snapshot before taking another action; no automatic retry is safe.",
    )
    .with_details(serde_json::json!({
        "kind": "popup_not_observed",
        "action": result.action,
        "steps": steps,
        "popup_present": after.map(|roots| !roots.is_empty()),
    }))
    .with_disposition(disposition);
    if after.is_some_and(<[AtspiElement]>::is_empty) {
        return Err(name_open_overview(error, overview()));
    }
    Err(error)
}

pub(super) fn run(
    target: &Target<'_>,
    view: &NodeView,
    wanted: Option<bool>,
    invoke: impl FnOnce() -> Result<ActionResult, AdapterError>,
) -> Result<ActionResult, AdapterError> {
    let Some(opener) = opener(target, view)? else {
        return invoke();
    };
    attempt(
        wanted,
        || roots_below(target.bus, &opener, target.deadline),
        invoke,
        |before| {
            poll_until(
                target.deadline,
                POPUP_SETTLE,
                POLL_STEP,
                || roots_below(target.bus, &opener, target.deadline),
                |after| {
                    after
                        .as_deref()
                        .is_some_and(|after| matches(wanted, before, after))
                },
            )
        },
        overview_open,
    )
}

fn attempt(
    wanted: Option<bool>,
    read_before: impl FnOnce() -> Option<Vec<AtspiElement>>,
    invoke: impl FnOnce() -> Result<ActionResult, AdapterError>,
    read_after: impl FnOnce(&[AtspiElement]) -> Option<Vec<AtspiElement>>,
    overview: impl FnOnce() -> Option<bool>,
) -> Result<ActionResult, AdapterError> {
    let before = read_before().ok_or_else(|| {
        AdapterError::new(
            ErrorCode::ActionFailed,
            "The popup subtree could not be read before delivery",
        )
        .with_disposition(DeliverySemantics::not_delivered())
    })?;
    let result = invoke()?;
    let after = read_after(&before);
    confirm(result, wanted, &before, after.as_deref(), overview)
}

#[cfg(test)]
#[path = "popup_verify_tests.rs"]
mod tests;
