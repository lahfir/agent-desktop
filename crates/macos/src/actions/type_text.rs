use agent_desktop_core::{
    ActionStep, AdapterError, Deadline, DeliverySemantics, ErrorCode, InteractionPolicy,
    StepMechanism,
};

use crate::tree::AXElement;

/// Types by replacing the current selection through `AXSelectedText`.
///
/// Chromium and Electron text inputs accept that write but ignore it, while
/// they do honour `AXValue`. When the insertion left a known value unchanged,
/// the full composed value is written once through `AXValue`. That write is
/// absolute, and the app applies both writes in order, so an insertion that
/// only landed late is overwritten rather than duplicated. Secure fields never
/// take this path.
pub(crate) fn execute_type(
    element: &AXElement,
    text: &str,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<Vec<ActionStep>, AdapterError> {
    let role = text_target_role(element, deadline)?;
    if policy.is_headed() {
        crate::actions::physical_keyboard::type_text(element, text, policy, deadline)?;
        return Ok(vec![
            ActionStep::succeeded("PIDTargetedUnicodeText")
                .with_mechanism(StepMechanism::PhysicalSynthetic)
                .with_verified(false),
        ]);
    }
    type_semantically(element, &role, text, deadline)
}

/// The accessibility reads and writes headless `type` performs, so the write
/// sequence can be exercised without a live element.
trait SemanticTextTarget {
    fn read_subrole(&self, deadline: Deadline) -> Result<Option<String>, i32>;
    fn read_value(&self, deadline: Deadline) -> Option<String>;
    fn read_selection(&self, deadline: Deadline) -> Option<std::ops::Range<usize>>;
    fn value_is_settable(&self, deadline: Deadline) -> bool;
    fn write_selected_text(&self, text: &str, deadline: Deadline) -> Result<(), AdapterError>;
    fn write_value(&self, value: &str, deadline: Deadline) -> Result<(), AdapterError>;
}

fn type_semantically(
    target: &impl SemanticTextTarget,
    role: &str,
    text: &str,
    deadline: Deadline,
) -> Result<Vec<ActionStep>, AdapterError> {
    if may_be_secure(role, target.read_subrole(deadline)) {
        target.write_selected_text(text, deadline)?;
        return Ok(vec![semantic_step("AXSelectedText")]);
    }

    let before = target.read_value(deadline);
    let selection = target.read_selection(deadline);
    target.write_selected_text(text, deadline)?;
    let after = target.read_value(deadline);

    let fallback = fallback_value(before.as_deref(), selection, after.as_deref(), text);
    let Some(value) = fallback.filter(|_| target.value_is_settable(deadline)) else {
        return Ok(vec![semantic_step("AXSelectedText")]);
    };
    target
        .write_value(&value, deadline)
        .map_err(insertion_was_accepted)?;
    Ok(vec![
        semantic_step("AXSelectedText"),
        semantic_step("AXValue"),
    ])
}

/// Password fields can be `AXTextField` with an `AXSecureTextField` subrole.
/// Their value readback is masked, so composing from it would overwrite the
/// secret. A subrole that cannot be read is treated as possibly secure.
fn may_be_secure(role: &str, subrole: Result<Option<String>, i32>) -> bool {
    role == SECURE_TEXT_FIELD
        || match subrole {
            Ok(subrole) => subrole.as_deref() == Some(SECURE_TEXT_FIELD),
            Err(_) => true,
        }
}

const SECURE_TEXT_FIELD: &str = "AXSecureTextField";

/// Returns the value `AXValue` should receive when an accepted insertion left
/// the field unchanged. Any change, unknown value, or unknown insertion point
/// returns `None`, because the insertion may then have landed.
fn fallback_value(
    before: Option<&str>,
    selection: Option<std::ops::Range<usize>>,
    after: Option<&str>,
    text: &str,
) -> Option<String> {
    let before = before?;
    if after? != before {
        return None;
    }
    let composed = agent_desktop_core::expected_insertion(before, text, selection)?;
    (composed != before).then_some(composed)
}

/// The preceding `AXSelectedText` write was accepted, so a failed fallback
/// cannot report the action as never delivered or as safe to retry.
fn insertion_was_accepted(error: AdapterError) -> AdapterError {
    match error.disposition {
        DeliverySemantics::NotDelivered | DeliverySemantics::Unknown => {
            error.with_disposition(DeliverySemantics::delivered_unverified())
        }
        _ => error,
    }
}

fn semantic_step(label: &'static str) -> ActionStep {
    ActionStep::succeeded(label)
        .with_mechanism(StepMechanism::SemanticApi)
        .with_verified(false)
}

impl SemanticTextTarget for AXElement {
    fn read_subrole(&self, deadline: Deadline) -> Result<Option<String>, i32> {
        crate::tree::attributes::copy_string_attr_result(self, "AXSubrole", deadline)
    }

    fn read_value(&self, deadline: Deadline) -> Option<String> {
        crate::tree::attributes::copy_string_attr_result(self, "AXValue", deadline)
            .ok()
            .flatten()
    }

    fn read_selection(&self, deadline: Deadline) -> Option<std::ops::Range<usize>> {
        crate::tree::attributes::selected_text_range(self, deadline)
    }

    fn value_is_settable(&self, deadline: Deadline) -> bool {
        matches!(
            crate::actions::ax_helpers::is_attr_settable(self, "AXValue", deadline),
            Ok(true)
        )
    }

    fn write_selected_text(&self, text: &str, deadline: Deadline) -> Result<(), AdapterError> {
        prepare(self, deadline)?;
        crate::actions::ax_helpers::set_ax_string_or_err(self, "AXSelectedText", text, deadline)?;
        if deadline.is_expired() {
            return Err(deadline
                .timeout_error()
                .with_details(serde_json::json!({ "operation": "AXSelectedText" }))
                .with_disposition(DeliverySemantics::delivered_unverified()));
        }
        Ok(())
    }

    fn write_value(&self, value: &str, deadline: Deadline) -> Result<(), AdapterError> {
        crate::actions::ax_helpers::set_ax_string_or_err(self, "AXValue", value, deadline)
    }
}

fn text_target_role(element: &AXElement, deadline: Deadline) -> Result<String, AdapterError> {
    prepare(element, deadline)?;
    let result = crate::tree::attributes::copy_string_attr_result(element, "AXRole", deadline);
    if deadline.is_expired() {
        return Err(deadline.timeout_error());
    }
    let role = result.map_err(|error| {
        AdapterError::new(
            ErrorCode::ActionFailed,
            "Could not read keyboard target role",
        )
        .with_details(serde_json::json!({ "ax_error": error }))
    })?;
    match role {
        Some(role)
            if matches!(
                role.as_str(),
                "AXTextField" | "AXTextArea" | "AXSecureTextField" | "AXComboBox"
            ) =>
        {
            Ok(role)
        }
        _ => Err(AdapterError::new(
            ErrorCode::ActionNotSupported,
            "Type requires a text field, secure text field, or combo box",
        )),
    }
}

fn prepare(element: &AXElement, deadline: Deadline) -> Result<(), AdapterError> {
    crate::tree::attributes::set_messaging_timeout(element, deadline)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn execute_type(
    _element: &crate::tree::AXElement,
    _text: &str,
    _policy: agent_desktop_core::InteractionPolicy,
    _deadline: Deadline,
) -> Result<Vec<agent_desktop_core::ActionStep>, AdapterError> {
    Err(AdapterError::not_supported("type_text"))
}

#[cfg(test)]
#[path = "type_text_tests.rs"]
mod tests;
