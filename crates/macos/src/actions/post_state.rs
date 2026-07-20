use agent_desktop_core::{
    Action, AdapterError, Deadline, ElementState, ErrorCode, EvidenceRequirements, LiveElement,
    LiveIdentity, LocatorField, ObservationBudget, Rect,
};
use std::time::{Duration, Instant};

use crate::tree::AXElement;

pub(crate) fn read_post_state(
    element: &AXElement,
    action: &Action,
    deadline: Deadline,
) -> Result<Option<ElementState>, AdapterError> {
    let delay = match action {
        Action::TypeText(_) => Duration::from_millis(50),
        Action::Toggle
        | Action::Check
        | Action::Uncheck
        | Action::SetValue(_)
        | Action::Clear
        | Action::Expand
        | Action::Collapse => Duration::ZERO,
        Action::Click
        | Action::DoubleClick
        | Action::RightClick
        | Action::TripleClick
        | Action::SetFocus
        | Action::Select(_)
        | Action::Scroll(_, _)
        | Action::ScrollTo
        | Action::PressKey(_)
        | Action::KeyDown(_)
        | Action::KeyUp(_)
        | Action::Hover
        | Action::Drag(_) => return Ok(None),
    };
    if !delay.is_zero() && !pause_if_budget_allows(deadline, delay) {
        return Ok(None);
    }
    read_element_state(element, deadline).map(Some)
}

pub(crate) fn read_element_state(
    element: &AXElement,
    deadline: Deadline,
) -> Result<ElementState, AdapterError> {
    let read = read_live_observation(element, deadline)?;
    let attrs = read.attrs;
    let role = known_role(&read.evidence.role)?;
    element_state_from_attrs(
        element,
        attrs,
        role,
        owning_window_bounds(element, deadline)?,
    )
}

pub(crate) fn read_live_element(
    element: &AXElement,
    deadline: Deadline,
) -> Result<LiveElement, AdapterError> {
    let read = read_live_observation(element, deadline)?;
    let role = known_role(&read.evidence.role)?;
    let identity = LiveIdentity {
        name: read.evidence.name,
        description: read.evidence.description,
        identifiers: read.evidence.identifiers,
    };
    let available_actions = known_actions(read.evidence.ref_evidence.available_actions)?;
    let attrs = read.attrs;
    let bounds = attrs.bounds;
    let window_bounds = owning_window_bounds(element, deadline)?;
    let state = element_state_from_attrs(element, attrs, role, window_bounds)?;
    Ok(LiveElement {
        identity,
        state,
        states_complete: true,
        bounds,
        available_actions,
    })
}

pub(crate) fn read_live_actions(
    element: &AXElement,
    deadline: Deadline,
) -> Result<Vec<String>, AdapterError> {
    Ok(read_live_element(element, deadline)?.available_actions)
}

fn read_live_observation(
    element: &AXElement,
    deadline: Deadline,
) -> Result<crate::tree::query::node_read::NodeRead, AdapterError> {
    let mut usage = new_usage();
    if !usage.claim_node() {
        return Err(incomplete_live_evidence());
    }
    let mut stats = agent_desktop_core::LocatorStats::default();
    let child_plan =
        crate::tree::query::child_read_plan::ChildReadPlan::load(usage.child_capacity());
    let read = crate::tree::query::node_read::read_node(
        element,
        crate::tree::query::node_read_context::NodeReadContext {
            tree: &crate::tree::TreeBuildContext::empty(false),
            stats: &mut stats,
            usage: &mut usage,
            requirements: EvidenceRequirements::snapshot(),
            deadline: deadline_instant(deadline)?,
            child_plan,
        },
    )?;
    usage.note_child_demand(read.child_read.total_count, &mut stats);
    usage.claim_edges(read.child_read.elements.len());
    if read.invalid_element {
        return Err(AdapterError::stale_ref(
            "Element became invalid while reading live state",
        ));
    }
    if deadline.is_expired() {
        return Err(deadline.timeout_error());
    }
    if !essential_live_evidence_complete(&read.evidence) {
        return Err(incomplete_live_evidence().with_details(serde_json::json!({
            "kind": "live_element_evidence",
            "complete": false,
            "query_stats": stats,
        })));
    }
    Ok(read)
}

fn essential_live_evidence_complete(evidence: &agent_desktop_core::LocatorEvidence) -> bool {
    !evidence.role.is_unknown()
        && !evidence.value.is_unknown()
        && !evidence.states.is_unknown()
        && !evidence.ref_evidence.bounds.is_unknown()
        && !evidence.ref_evidence.available_actions.is_unknown()
}

fn new_usage() -> crate::tree::observation_usage::ObservationUsage {
    crate::tree::observation_usage::ObservationUsage::new(ObservationBudget::default())
}

fn owning_window_bounds(
    element: &AXElement,
    deadline: Deadline,
) -> Result<Option<Rect>, AdapterError> {
    crate::tree::attributes::set_messaging_timeout(element, deadline)?;
    let window = first_owning_container(|attribute| {
        crate::tree::attributes::copy_element_attr_result(element, attribute, deadline)
    })
    .map_err(|(attribute, error)| read_error(attribute, error))?;
    if deadline.is_expired() {
        return Err(deadline.timeout_error());
    }
    let Some(window) = window else {
        return Ok(None);
    };
    crate::tree::element_bounds::read_bounds_with_deadline(&window, deadline_instant(deadline)?)
}

fn first_owning_container(
    mut read: impl FnMut(&'static str) -> Result<Option<AXElement>, i32>,
) -> Result<Option<AXElement>, (&'static str, i32)> {
    for attribute in ["AXWindow", "AXTopLevelUIElement"] {
        match read(attribute) {
            Ok(Some(element)) => return Ok(Some(element)),
            Ok(None) => {}
            Err(error) => return Err((attribute, error)),
        }
    }
    Ok(None)
}

fn known_role(role: &LocatorField<String>) -> Result<String, AdapterError> {
    role.known().cloned().ok_or_else(incomplete_live_evidence)
}

fn known_actions(actions: LocatorField<Vec<String>>) -> Result<Vec<String>, AdapterError> {
    match actions {
        LocatorField::Known(actions) => Ok(actions),
        LocatorField::Absent => Ok(Vec::new()),
        LocatorField::Unknown => Err(incomplete_live_evidence()),
    }
}

fn incomplete_live_evidence() -> AdapterError {
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Live element evidence was incomplete",
    )
    .with_details(serde_json::json!({
        "kind": "live_element_evidence",
        "complete": false,
        "retryable": true,
    }))
}

fn element_state_from_attrs(
    element: &AXElement,
    attrs: crate::tree::NodeAttrs,
    role: String,
    window_bounds: Option<Rect>,
) -> Result<ElementState, AdapterError> {
    let is_secure = attrs.role.as_deref() == Some("AXSecureTextField")
        || attrs.subrole.as_deref() == Some("AXSecureTextField");
    let context = crate::tree::state_reader::StateReaderContext {
        focused: None,
        window_bounds,
        is_secure_text: is_secure,
    };
    let states = crate::tree::state_reader::states_from_element(element, &attrs, &role, &context);
    let enabled = Some(attrs.states.enabled);
    let hidden = hidden_state(attrs.states.semantic.hidden);
    let offscreen = offscreen(attrs.bounds, window_bounds);
    Ok(ElementState {
        role,
        states,
        value: (!is_secure).then_some(attrs.value).flatten(),
        enabled,
        hidden,
        offscreen,
    })
}

fn hidden_state(reported: Option<bool>) -> Option<bool> {
    reported
}

fn offscreen(bounds: Option<Rect>, window: Option<Rect>) -> Option<bool> {
    let (bounds, window) = bounds.zip(window)?;
    Some(
        bounds.x + bounds.width <= window.x
            || bounds.x >= window.x + window.width
            || bounds.y + bounds.height <= window.y
            || bounds.y >= window.y + window.height,
    )
}

#[cfg(test)]
fn normalized_role(ax_role: Option<&str>, ax_subrole: Option<&str>) -> String {
    ax_role
        .map(|role| crate::tree::roles::ax_role_and_subrole_to_str(role, ax_subrole))
        .unwrap_or("unknown")
        .to_string()
}

fn pause_if_budget_allows(deadline: Deadline, delay: Duration) -> bool {
    let remaining = deadline.remaining();
    if remaining <= delay {
        return false;
    }
    std::thread::sleep(delay);
    !deadline.is_expired()
}

fn deadline_instant(deadline: Deadline) -> Result<Instant, AdapterError> {
    let remaining = deadline.remaining();
    if remaining.is_zero() {
        return Err(deadline.timeout_error());
    }
    Instant::now()
        .checked_add(remaining)
        .ok_or_else(|| AdapterError::new(ErrorCode::InvalidArgs, "Deadline is out of range"))
}

fn read_error(attribute: &str, error: i32) -> AdapterError {
    AdapterError::new(
        if error == accessibility_sys::kAXErrorCannotComplete {
            ErrorCode::Timeout
        } else if error == accessibility_sys::kAXErrorAPIDisabled {
            ErrorCode::PermDenied
        } else if error == accessibility_sys::kAXErrorInvalidUIElement {
            ErrorCode::StaleRef
        } else {
            ErrorCode::ActionFailed
        },
        format!("Could not read {attribute} for live state"),
    )
    .with_details(serde_json::json!({ "attribute": attribute, "ax_error": error }))
}

#[cfg(test)]
#[path = "post_state_tests.rs"]
mod tests;
