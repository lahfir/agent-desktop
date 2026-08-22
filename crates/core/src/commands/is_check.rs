use crate::{
    AppError,
    adapter::{PlatformAdapter, optional_live_read},
    commands::helpers::resolve_ref_with_context,
    context::CommandContext,
    element_state::ElementState,
    refs::RefEntry,
    state::{self, CHECKED, DISABLED, EXPANDED, FOCUSED, SELECTED, VisibilityEvidence},
};
use serde_json::{Value, json};

pub struct IsArgs {
    pub ref_id: String,
    pub snapshot_id: Option<String>,
    pub property: IsProperty,
}

pub enum IsProperty {
    Visible,
    Enabled,
    Checked,
    Focused,
    Expanded,
    Selected,
}

pub fn execute(
    args: IsArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let (entry, handle) =
        resolve_ref_with_context(&args.ref_id, args.snapshot_id.as_deref(), adapter, context)?;

    let prop_name = match args.property {
        IsProperty::Visible => "visible",
        IsProperty::Enabled => "enabled",
        IsProperty::Checked => "checked",
        IsProperty::Focused => "focused",
        IsProperty::Expanded => "expanded",
        IsProperty::Selected => "selected",
    };

    let deadline = crate::Deadline::standard()?;
    let live_state = optional_live_read(adapter.get_live_state(&handle, deadline))?;
    let state = live_state
        .clone()
        .unwrap_or_else(|| state_from_ref_entry(&entry));
    let states_from_live = live_state.is_some();

    let (applicable, result) = match args.property {
        IsProperty::Visible => {
            let live_bounds = optional_live_read(adapter.get_element_bounds(&handle, deadline))?;
            let visibility = VisibilityEvidence {
                bounds: live_bounds.or(entry.geometry.bounds),
                states: state.states.clone(),
                bounds_from_live: live_bounds.is_some(),
                states_from_live,
            };
            (visibility.applicable(), visibility.result())
        }
        IsProperty::Enabled => (true, !state::has_state(&state.states, DISABLED)),
        IsProperty::Focused => (true, state::has_state(&state.states, FOCUSED)),
        IsProperty::Checked => (
            crate::roles::is_toggleable_role(&entry.identity.role)
                || state::has_state(&state.states, CHECKED)
                || crate::capability::contains_any(
                    &entry.capabilities.available_actions,
                    crate::capability::CHECKED_APPLICABILITY,
                ),
            state::has_state(&state.states, CHECKED),
        ),
        IsProperty::Expanded => (
            crate::roles::is_expandable_role(&entry.identity.role)
                || state::has_state(&state.states, EXPANDED)
                || crate::capability::contains_any(
                    &entry.capabilities.available_actions,
                    crate::capability::EXPANDED_APPLICABILITY,
                ),
            state::has_state(&state.states, EXPANDED),
        ),
        IsProperty::Selected => (true, state::has_state(&state.states, SELECTED)),
    };

    Ok(
        json!({ "property": prop_name, "ref": args.ref_id, "result": result, "applicable": applicable }),
    )
}

fn state_from_ref_entry(entry: &RefEntry) -> ElementState {
    ElementState {
        role: entry.identity.role.clone(),
        states: entry.capabilities.states.clone(),
        value: entry.identity.value.clone(),
        enabled: None,
        hidden: None,
        offscreen: None,
    }
}

#[cfg(test)]
#[path = "is_check_tests.rs"]
mod tests;
