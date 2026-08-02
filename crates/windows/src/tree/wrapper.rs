use super::properties::ElementProperties;
use super::property_ids::TreeProperty;
use super::property_outcome::{PropertyOutcome, PropertyValue};

/// Whether an element is a transparent web wrapper that consumes raw depth but
/// no logical depth (KTD6).
///
/// The predicate consumes only evidence this walk has already read - control
/// type, name, value, `AutomationId`, and actions - so it costs nothing extra
/// per node. A node is a transparent wrapper only when its control type is
/// `Group` (50026) or `Custom` (50025) **and** its name, value and
/// `AutomationId` are all empty **and** it advertises no action. This mirrors
/// macOS's rule that a named or actionable generic element consumes depth
/// (`crates/macos/src/tree/query/node_evidence.rs:6-38`).
///
/// Gate note: this function is **ungated** - it decides emptiness only. The
/// Chromium-provenance gate (that a skip only fires under detected
/// Chromium/WebView2 provenance) lives at the call site, because the same
/// predicate would otherwise skip the anonymous `Group`/`Pane` containers
/// native stacks are full of (KTD6's silent-deepening guard).
pub(crate) fn is_web_wrapper(properties: &ElementProperties) -> bool {
    let control_type = non_zero_number(properties.get(TreeProperty::ControlType));
    let is_group_or_custom = control_type == Some(50026) || control_type == Some(50025);
    if !is_group_or_custom {
        return false;
    }
    if non_empty_text(properties.get(TreeProperty::Name)).is_some() {
        return false;
    }
    if non_empty_text(properties.get(TreeProperty::Value)).is_some() {
        return false;
    }
    if non_empty_text(properties.get(TreeProperty::AutomationId)).is_some() {
        return false;
    }
    if advertises_action(properties) {
        return false;
    }
    true
}

fn non_empty_text(outcome: PropertyOutcome) -> Option<String> {
    match outcome {
        PropertyOutcome::Known(PropertyValue::Text(value)) if !value.trim().is_empty() => {
            Some(value)
        }
        _ => None,
    }
}

fn non_zero_number(outcome: PropertyOutcome) -> Option<i32> {
    match outcome {
        PropertyOutcome::Known(PropertyValue::Number(value)) if value != 0 => Some(value),
        _ => None,
    }
}

/// Whether the element advertises any affordance. The action set is resolved
/// from the same available-pattern flags the walk read, so the wrapper check
/// never asks the provider for a pattern.
fn advertises_action(properties: &ElementProperties) -> bool {
    super::actions::resolve_actions(properties)
        .known()
        .is_some_and(|actions| !actions.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(reads: &[(TreeProperty, PropertyOutcome)]) -> ElementProperties {
        ElementProperties::from_reads(reads.to_vec())
    }

    fn group() -> Vec<(TreeProperty, PropertyOutcome)> {
        vec![(
            TreeProperty::ControlType,
            PropertyOutcome::Known(PropertyValue::Number(50026)),
        )]
    }

    /// KTD6's silent-deepening guard: an empty `Group` is a wrapper, but only
    /// under Chromium provenance - the gate lives at the call site. This pins
    /// the emptiness half of the predicate.
    #[test]
    fn an_empty_group_without_identity_is_a_wrapper_shape() {
        let properties = props(&group());
        assert!(is_web_wrapper(&properties));
    }

    #[test]
    fn a_named_group_consumes_depth() {
        let mut reads = group();
        reads.push((
            TreeProperty::Name,
            PropertyOutcome::Known(PropertyValue::Text("Root".into())),
        ));
        assert!(!is_web_wrapper(&props(&reads)));
    }

    #[test]
    fn a_group_with_a_value_consumes_depth() {
        let mut reads = group();
        reads.push((
            TreeProperty::Value,
            PropertyOutcome::Known(PropertyValue::Text("v".into())),
        ));
        assert!(!is_web_wrapper(&props(&reads)));
    }

    #[test]
    fn a_group_with_an_automation_id_consumes_depth() {
        let mut reads = group();
        reads.push((
            TreeProperty::AutomationId,
            PropertyOutcome::Known(PropertyValue::Text("id".into())),
        ));
        assert!(!is_web_wrapper(&props(&reads)));
    }

    #[test]
    fn a_group_with_an_action_consumes_depth() {
        let mut reads = group();
        reads.push((
            TreeProperty::InvokeAvailable,
            PropertyOutcome::Known(PropertyValue::Flag(true)),
        ));
        assert!(!is_web_wrapper(&props(&reads)));
    }

    #[test]
    fn a_non_group_control_type_is_never_a_wrapper() {
        let reads = vec![(
            TreeProperty::ControlType,
            PropertyOutcome::Known(PropertyValue::Number(50033)),
        )];
        assert!(!is_web_wrapper(&props(&reads)));
    }
}
