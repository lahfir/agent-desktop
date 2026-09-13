//! The secure-field arm of the resolution probe (A17-6).
//!
//! Two readings of the same password control, deliberately kept side by side.
//!
//! The **provider reading** takes the value-bearing properties straight off the
//! UI Automation element with no adapter code in the path. That is the reading
//! A14-6 took of the walk's cached fetch and the one A17-6 records for the live
//! fetch: whether the platform itself hands a live reader the control's
//! content. Its three original keys - `name_contains_marker`,
//! `value_contains_marker`, `value_length` - keep their names and their meaning
//! so the row's recorded numbers stay comparable across runs.
//!
//! The **product reading** drives the adapter's own live-read composition on
//! the same element: `properties::read_live_bounded`,
//! `name_evidence::read_label`, `walker::walk_vocabulary` and
//! `ElementProperties::into_locator_evidence`, the four calls the adapter's
//! shared single-element read makes in that order. The withholding those calls
//! apply to a control reporting `IsPassword` is the only thing standing between
//! the provider reading and this one, so removing it changes this reading and
//! nothing else. (Process corroboration and the completeness gate are the other
//! two things the shared read does; neither touches content, and neither is
//! reachable from outside the adapter crate.)
//!
//! **A provider that published nothing would make the product reading
//! vacuous** - withholding nothing is indistinguishable from withholding
//! everything. So `provider_published_slots` lists the value-bearing slots the
//! provider answered with non-empty text, and `withholding_observable` is true
//! only when the provider offered such a slot and the adapter then refused to
//! pass it on. A reading with `withholding_observable` false proves nothing
//! about the adapter and must not be read as if it did.
//!
//! `withholding_applied` is a slot-shape verdict, not a content one: an empty
//! string the adapter published is `Known("")` where withholding would have
//! produced `Absent`, and the difference between those two is exactly the rule.

use serde_json::{Value, json};

use agent_desktop_core::Deadline;
use agent_desktop_windows::tree::element::UIAElement;
use agent_desktop_windows::tree::name_evidence::read_label;
use agent_desktop_windows::tree::properties::{PropertyOutcome, read_live_bounded};
use agent_desktop_windows::tree::property_ids::{TreeProperty, uia_property};
use agent_desktop_windows::tree::walker::walk_vocabulary;
use uiautomation::UIElement;
use uiautomation::types::UIProperty;

/// Written into the fixture's password control by `probe_window.rs`. Its
/// appearance in any reading is the leak the arm exists to catch.
const MARKER: &str = "obs-pwd-marker-15ch";

/// Budget for the adapter's own bounded property read. Generous on purpose:
/// a truncated read leaves slots `Unknown`, which withholds for the wrong
/// reason and would flatter the arm.
const PRODUCT_READ_BUDGET_MS: u64 = 10_000;

/// The provider's own answer for one control, taken with no adapter code in
/// the path.
struct ProviderReading {
    name_contains_marker: bool,
    value_contains_marker: bool,
    value_length: Option<usize>,
    published: Vec<&'static str>,
}

/// Measures the first control in the walk that reports `IsPassword`.
pub(crate) fn measure_secure(elements: &[UIElement]) -> Value {
    let Some(element) = elements.iter().find(|element| is_password(element)) else {
        return json!({ "password_control_present": false });
    };
    let provider = provider_reading(element);
    let product = product_reading(element, &provider);
    json!({
        "password_control_present": true,
        "name_contains_marker": provider.name_contains_marker,
        "value_contains_marker": provider.value_contains_marker,
        "value_length": provider.value_length,
        "provider_published_slots": provider.published,
        "product_live_read": product,
    })
}

fn is_password(element: &UIElement) -> bool {
    element
        .get_property_value(UIProperty::IsPassword)
        .ok()
        .and_then(|variant| crate::measure::boolean_of(&variant))
        .unwrap_or(false)
}

fn raw_text(element: &UIElement, property: TreeProperty) -> Option<String> {
    element
        .get_property_value(uia_property(property))
        .ok()
        .and_then(|variant| variant.get_string().ok())
}

fn provider_reading(element: &UIElement) -> ProviderReading {
    let name = raw_text(element, TreeProperty::Name);
    let value = raw_text(element, TreeProperty::Value);
    let published = TreeProperty::VALUE_BEARING
        .iter()
        .copied()
        .filter(|property| raw_text(element, *property).is_some_and(|text| !text.trim().is_empty()))
        .map(TreeProperty::as_str)
        .collect();
    ProviderReading {
        name_contains_marker: name.is_some_and(|text| text.contains(MARKER)),
        value_contains_marker: value.as_ref().is_some_and(|text| text.contains(MARKER)),
        value_length: value.map(|text| text.chars().count()),
        published,
    }
}

/// Runs the adapter's live-read composition over the same element and reports
/// which value-bearing slots survive into the evidence a caller would receive.
fn product_reading(element: &UIElement, provider: &ProviderReading) -> Value {
    let Ok(deadline) = Deadline::after(PRODUCT_READ_BUDGET_MS) else {
        return json!({ "reached": false, "reason": "no operation deadline could be built" });
    };
    let target = UIAElement::from(element.clone());
    let (properties, _errors) = read_live_bounded(&target, deadline);
    let label = read_label(&target, false);
    let vocabulary = walk_vocabulary(&properties, &label);
    let evidence = properties.clone().into_locator_evidence(vocabulary);

    let published: Vec<&'static str> = TreeProperty::VALUE_BEARING
        .iter()
        .copied()
        .filter(|property| matches!(properties.get(*property), PropertyOutcome::Known(_)))
        .map(TreeProperty::as_str)
        .collect();
    let name = evidence.name.known().cloned();
    let value = evidence.value.known().cloned();

    json!({
        "reached": true,
        "drives": [
            "properties::read_live_bounded",
            "name_evidence::read_label",
            "walker::walk_vocabulary",
            "ElementProperties::into_locator_evidence",
        ],
        "secure_gate_closed": properties.is_secure(),
        "value_bearing_slots_published": published.clone(),
        "withholding_applied": published.is_empty(),
        "name_slot_published": name.is_some(),
        "value_slot_published": value.is_some(),
        "name_contains_marker": name.is_some_and(|text| text.contains(MARKER)),
        "value_contains_marker": value.is_some_and(|text| text.contains(MARKER)),
        "withholding_observable": !provider.published.is_empty() && published.is_empty(),
    })
}
