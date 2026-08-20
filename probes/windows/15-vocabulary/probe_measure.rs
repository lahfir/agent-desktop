//! The nine vocabulary measurements sub-phase 2.3's plan refuses to infer.
//!
//! Every function here reports shape - variant type, length, boolean or
//! integer content, HRESULT - and never the fixture's text, except through the
//! `contains_marker` flag that proves a secret did not travel.



use serde_json::{Value, json};
use uiautomation::types::UIProperty;
use uiautomation::variants::Variant;
use uiautomation::{Error as UiaError, UIAutomation, UIElement, UITreeWalker};


use crate::properties::PATTERN_STATE;

/// The marker written into the fixture's secure control, so a capture can
/// assert the secret did not travel rather than assuming it did not.
pub const SECRET_MARKER: &str = "zzvocabsecretzz";

pub fn failure_shape(error: &UiaError) -> Value {
    json!({
        "code": error.code(),
        "result_is_none": error.result().is_none(),
        "result_hex": error.result().map(|hresult| format!("0x{:08X}", hresult.0 as u32)),
    })
}

/// Describes a variant without disclosing its text.
///
/// `not_supported` is the pointer comparison against UI Automation's reserved
/// singleton, which is the only reliable test - the sentinel arrives as
/// `VT_UNKNOWN` and no field distinguishes it from any other interface.
pub fn variant_shape(variant: &Variant) -> Value {
    let rendered = variant.get_string().unwrap_or_default();
    let mut shape = serde_json::Map::new();
    shape.insert(
        "variant_type".into(),
        json!(format!("{:?}", variant.get_type())),
    );
    if is_not_supported(variant) {
        shape.insert("not_supported".into(), json!(true));
    }
    if variant.is_null() {
        shape.insert("is_null".into(), json!(true));
    }
    if let Some(flag) = boolean_of(variant) {
        shape.insert("bool".into(), json!(flag));
    }
    if let Some(number) = number_of(variant) {
        shape.insert("number".into(), json!(number));
    }
    if !rendered.is_empty() {
        shape.insert("length".into(), json!(rendered.chars().count()));
        shape.insert("non_empty".into(), json!(!rendered.trim().is_empty()));
    }
    if rendered.contains(SECRET_MARKER) {
        shape.insert("contains_marker".into(), json!(true));
    }
    Value::Object(shape)
}

/// Collapses a per-element reading to the **distinct** shapes observed.
///
/// A capture that prints one shape per element records the same answer as many
/// times as the provider happens to expose that control - ten identical rows
/// for ten static texts - which is bulk, not evidence. The distinct set plus a
/// count says exactly as much and is what a later reader can actually scan.
fn distinct_shapes(shapes: Vec<Value>) -> Value {
    let mut seen: Vec<Value> = Vec::new();
    for shape in shapes {
        if !seen.contains(&shape) {
            seen.push(shape);
        }
    }
    json!(seen)
}

pub fn boolean_of(variant: &Variant) -> Option<bool> {
    match variant.get_value() {
        Ok(uiautomation::variants::Value::BOOL(flag)) => Some(flag),
        _ => None,
    }
}

fn number_of(variant: &Variant) -> Option<i32> {
    match variant.get_value() {
        Ok(uiautomation::variants::Value::I4(number) | uiautomation::variants::Value::INT(number)) => {
            Some(number)
        }
        _ => None,
    }
}

fn is_not_supported(variant: &Variant) -> bool {
    use windows::Win32::UI::Accessibility::UiaGetReservedNotSupportedValue;
    use windows::core::Interface;
    let Ok(uiautomation::variants::Value::UNKNOWN(candidate)) = variant.get_value() else {
        return false;
    };
    unsafe { UiaGetReservedNotSupportedValue() }
        .ok()
        .is_some_and(|sentinel| candidate.as_raw() == sentinel.as_raw())
}

pub fn read_shape(element: &UIElement, property: UIProperty) -> Value {
    match element.get_property_value(property) {
        Ok(variant) => variant_shape(&variant),
        Err(error) => json!({ "failed": failure_shape(&error) }),
    }
}

pub fn control_type_of(element: &UIElement) -> i32 {
    element
        .get_property_value(UIProperty::ControlType)
        .ok()
        .and_then(|variant| number_of(&variant))
        .unwrap_or(0)
}

pub fn enumerate_children(walker: &UITreeWalker, parent: &UIElement) -> Vec<UIElement> {
    let mut children = Vec::new();
    let mut current = match walker.get_first_child(parent) {
        Ok(first) => first,
        Err(_) => return children,
    };
    loop {
        let next = walker.get_next_sibling(&current);
        children.push(current);
        match next {
            Ok(sibling) => current = sibling,
            Err(_) => return children,
        }
    }
}

fn descendants(walker: &UITreeWalker, root: &UIElement, depth: u32, out: &mut Vec<UIElement>) {
    if depth >= 12 {
        return;
    }
    for child in enumerate_children(walker, root) {
        descendants(walker, &child, depth + 1, out);
        out.push(child);
    }
}

pub fn collect_descendants(walker: &UITreeWalker, root: &UIElement) -> Vec<UIElement> {
    let mut out = Vec::new();
    descendants(walker, root, 0, &mut out);
    out
}

/// Groups the fixture's controls by `ControlType`, so every measurement below
/// selects its subject by control class rather than by the text it carries.
pub fn by_control_type(elements: &[UIElement]) -> Vec<(i32, Vec<&UIElement>)> {
    let mut grouped: Vec<(i32, Vec<&UIElement>)> = Vec::new();
    for element in elements {
        let control_type = control_type_of(element);
        match grouped.iter_mut().find(|(key, _)| *key == control_type) {
            Some((_, bucket)) => bucket.push(element),
            None => grouped.push((control_type, vec![element])),
        }
    }
    grouped.sort_by_key(|(key, _)| *key);
    grouped
}

fn is_secure(element: &UIElement) -> bool {
    element
        .get_property_value(UIProperty::IsPassword)
        .ok()
        .and_then(|variant| boolean_of(&variant))
        .unwrap_or(false)
}

/// Question 1: does `LabeledBy` resolve, and does the target's `Name` read
/// across the process boundary?
pub fn measure_labeled_by(elements: &[UIElement]) -> Value {
    let rows: Vec<Value> = elements
        .iter()
        .map(|element| match element.get_labeled_by() {
            Err(error) => json!({
                "control_type": control_type_of(element),
                "secure": is_secure(element),
                "resolved": false,
                "failed": failure_shape(&error),
            }),
            Ok(target) => json!({
                "control_type": control_type_of(element),
                "secure": is_secure(element),
                "resolved": true,
                "target_control_type": control_type_of(&target),
                "target_secure": is_secure(&target),
                "target_name": read_shape(&target, UIProperty::Name),
            }),
        })
        .collect();
    let resolved: Vec<&Value> = rows
        .iter()
        .filter(|row| row["resolved"] == json!(true))
        .collect();
    let mut unresolved: Vec<Value> = rows
        .iter()
        .filter(|row| row["resolved"] != json!(true))
        .map(|row| json!({ "control_type": row["control_type"], "failed": row["failed"] }))
        .collect();
    unresolved.dedup();
    json!({
        "elements": rows.len(),
        "resolved": resolved.len(),
        "non_secure_labelled_by_secure": rows.iter().any(|row| {
            row["secure"] == json!(false) && row["target_secure"] == json!(true)
        }),
        "resolved_rows": resolved,
        "unresolved_shapes": distinct_shapes(unresolved),
    })
}

/// Questions 2, 3, 4 and 5, reported per control type: the naming properties,
/// the `LegacyIAccessible` gate KTD4 rests on, the two state feeds, and the
/// view-membership flags KTD11 turns into evidence.
pub fn measure_per_control_type(elements: &[UIElement]) -> Value {
    let rows: Vec<Value> = by_control_type(elements)
        .into_iter()
        .map(|(control_type, bucket)| {
            let sample = bucket[0];
            json!({
                "control_type": control_type,
                "count": bucket.len(),
                "secure": bucket.iter().any(|element| is_secure(element)),
                "help_text": read_shape(sample, UIProperty::HelpText),
                "full_description": read_shape(sample, UIProperty::FullDescription),
                "item_status": read_shape(sample, UIProperty::ItemStatus),
                "legacy_default_action": read_shape(sample, UIProperty::LegacyIAccessibleDefaultAction),
                "legacy_state": read_shape(sample, UIProperty::LegacyIAccessibleState),
                "legacy_available": read_shape(sample, UIProperty::IsLegacyIAccessiblePatternAvailable),
                "value_is_readonly": distinct_shapes(
                    bucket
                        .iter()
                        .map(|element| read_shape(element, UIProperty::ValueIsReadOnly))
                        .collect(),
                ),
                "selection_can_select_multiple": distinct_shapes(
                    bucket
                        .iter()
                        .map(|element| read_shape(element, UIProperty::SelectionCanSelectMultiple))
                        .collect(),
                ),
                "is_control_element": read_shape(sample, UIProperty::IsControlElement),
                "is_content_element": read_shape(sample, UIProperty::IsContentElement),
                "is_dialog": read_shape(sample, UIProperty::IsDialog),
            })
        })
        .collect();
    json!({ "rows": rows })
}

/// Question 6: what a pattern-state property returns on an element whose
/// provider does not implement the pattern.
///
/// The subject is chosen by advertised availability rather than by control
/// class, so the answer describes the property rather than this fixture.
pub fn measure_pattern_state_without_pattern(elements: &[UIElement]) -> Value {
    let rows: Vec<Value> = PATTERN_STATE
        .iter()
        .filter_map(|(name, property)| {
            let availability = availability_for(*property);
            let subject = elements.iter().find(|element| {
                element
                    .get_property_value(availability)
                    .ok()
                    .and_then(|variant| boolean_of(&variant))
                    == Some(false)
            })?;
            Some(json!({
                "property": name,
                "availability_property_reported": false,
                "control_type": control_type_of(subject),
                "outcome": read_shape(subject, *property),
            }))
        })
        .collect();
    json!({ "rows": rows })
}

fn availability_for(property: UIProperty) -> UIProperty {
    match property {
        UIProperty::ToggleToggleState => UIProperty::IsTogglePatternAvailable,
        UIProperty::ExpandCollapseExpandCollapseState => {
            UIProperty::IsExpandCollapsePatternAvailable
        }
        UIProperty::SelectionItemIsSelected => UIProperty::IsSelectionItemPatternAvailable,
        UIProperty::SelectionCanSelectMultiple => UIProperty::IsSelectionPatternAvailable,
        _ => UIProperty::IsValuePatternAvailable,
    }
}

/// Question 7: the node and `ControlType` delta between the two tree views
/// KTD11 leaves unreconciled.
pub fn measure_view_delta(automation: &UIAutomation, root: &UIElement) -> Value {
    let view = |walker: Result<UITreeWalker, UiaError>| match walker {
        Err(error) => json!({ "failed": failure_shape(&error) }),
        Ok(walker) => {
            let nodes = collect_descendants(&walker, root);
            let mut types: Vec<i32> = nodes.iter().map(control_type_of).collect();
            types.sort_unstable();
            types.dedup();
            json!({ "nodes": nodes.len(), "control_types": types })
        }
    };
    json!({
        "raw_view": view(automation.get_raw_view_walker()),
        "control_view": view(automation.get_control_view_walker()),
    })
}

