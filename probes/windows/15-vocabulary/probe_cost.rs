//! The cost half of the vocabulary probe: what the expanded property set
//! costs, whether the conditional split recovers it, and whether a cached
//! element-returning property carries the request's properties.

use std::time::Instant;

use serde_json::{Value, json};
use uiautomation::types::{ElementMode, TreeScope, UIProperty};
use uiautomation::variants::Variant;
use uiautomation::{Error as UiaError, UIAutomation, UIElement, UITreeWalker};

use crate::measure::{boolean_of, control_type_of, failure_shape, variant_shape};
use crate::properties::{BATCH_REPEATS, BASE_SET, CORE_PLUS_AVAILABILITY, CORE_SET, FULL_SET};

fn build_request(
    automation: &UIAutomation,
    properties: &[UIProperty],
) -> Result<uiautomation::core::UICacheRequest, UiaError> {
    let request = automation.create_cache_request()?;
    request.set_element_mode(ElementMode::Full)?;
    request.set_tree_scope(TreeScope::Element)?;
    for property in properties {
        request.add_property(*property)?;
    }
    Ok(request)
}

fn cached_walk(walker: &UITreeWalker, root: &UIElement, request: &uiautomation::core::UICacheRequest) -> Vec<UIElement> {
    let mut nodes = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(parent) = stack.pop() {
        let mut current = match walker.get_first_child_build_cache(&parent, request) {
            Ok(first) => first,
            Err(_) => continue,
        };
        loop {
            let next = walker.get_next_sibling_build_cache(&current, request);
            stack.push(current.clone());
            nodes.push(current);
            match next {
                Ok(sibling) => current = sibling,
                Err(_) => break,
            }
        }
    }
    nodes
}

/// Question 8: where the marginal cost of the expanded property set actually
/// lands, so U2's flat-or-split decision rests on a number per property group
/// rather than on one aggregate.
///
/// Reported as A6-1 reported it - the prefetch phase and the read phase
/// separately, because the cache request is built once per walk and every node
/// pays the prefetch whether the property applies to it or not.
///
/// A warm-up walk runs first and its result is discarded: a lazily-built
/// provider charges the first caller for peer construction, which would
/// otherwise be attributed to whichever property set happened to run first.
/// The baseline is then measured again last, so any remaining drift is visible
/// rather than folded into the comparison.
pub fn measure_batch_cost(automation: &UIAutomation, root: &UIElement) -> Value {
    let walker = match automation.get_raw_view_walker() {
        Ok(walker) => walker,
        Err(error) => return json!({ "failed": failure_shape(&error) }),
    };
    let once = |properties: &[UIProperty]| -> Option<(u128, u128, usize, u32)> {
        let request = build_request(automation, properties).ok()?;
        let started = Instant::now();
        let nodes = cached_walk(&walker, root, &request);
        let prefetch_micros = started.elapsed().as_micros();
        let started = Instant::now();
        let mut reads = 0_u32;
        for node in &nodes {
            for property in properties {
                if node.get_cached_property_value(*property).is_ok() {
                    reads += 1;
                }
            }
        }
        Some((
            prefetch_micros,
            started.elapsed().as_micros(),
            nodes.len(),
            reads,
        ))
    };
    let phase = |properties: &[UIProperty]| -> Value {
        let mut prefetch = Vec::new();
        let mut read = Vec::new();
        let mut nodes = 0;
        let mut reads = 0;
        for _ in 0..BATCH_REPEATS {
            match once(properties) {
                None => return json!({ "failed": "the cache request could not be built" }),
                Some((prefetch_micros, read_micros, node_count, successful)) => {
                    prefetch.push(prefetch_micros);
                    read.push(read_micros);
                    nodes = node_count;
                    reads = successful;
                }
            }
        }
        prefetch.sort_unstable();
        read.sort_unstable();
        json!({
            "properties": properties.len(),
            "nodes": nodes,
            "repeats": BATCH_REPEATS,
            "prefetch_min_micros": prefetch[0],
            "prefetch_median_micros": prefetch[prefetch.len() / 2],
            "prefetch_max_micros": prefetch[prefetch.len() - 1],
            "read_median_micros": read[read.len() / 2],
            "successful_reads": reads,
        })
    };
    let _ = once(&FULL_SET);
    json!({
        "warmed_up": true,
        "note": "min is the least noise-sensitive estimator; the spread is reported so a claim can be checked against it",
        "base_set": phase(&BASE_SET),
        "core_set": phase(&CORE_SET),
        "core_plus_availability": phase(&CORE_PLUS_AVAILABILITY),
        "full_set": phase(&FULL_SET),
        "base_set_repeat": phase(&BASE_SET),
        "conditional_split": measure_conditional_split(automation, root, &walker),
    })
}

/// The conditional split, measured head to head against the flat set rather
/// than argued about: prefetch the core plus the availability flags, then
/// fetch the pattern-state group with one `BuildUpdatedCache` round trip per
/// node that actually advertises a pattern.
///
/// This is the design the plan pre-commits to if the flat set measures
/// materially slower, so whether it is actually cheaper is a number rather
/// than a preference.
fn measure_conditional_split(
    automation: &UIAutomation,
    root: &UIElement,
    walker: &UITreeWalker,
) -> Value {
    let gated: [(UIProperty, UIProperty); 5] = [
        (UIProperty::IsTogglePatternAvailable, UIProperty::ToggleToggleState),
        (
            UIProperty::IsExpandCollapsePatternAvailable,
            UIProperty::ExpandCollapseExpandCollapseState,
        ),
        (
            UIProperty::IsSelectionItemPatternAvailable,
            UIProperty::SelectionItemIsSelected,
        ),
        (UIProperty::IsValuePatternAvailable, UIProperty::ValueIsReadOnly),
        (
            UIProperty::IsSelectionPatternAvailable,
            UIProperty::SelectionCanSelectMultiple,
        ),
    ];
    let state_properties: Vec<UIProperty> = gated.iter().map(|(_, state)| *state).collect();
    let (Ok(first), Ok(second)) = (
        build_request(automation, &CORE_PLUS_AVAILABILITY),
        build_request(automation, &state_properties),
    ) else {
        return json!({ "failed": "the split cache requests could not be built" });
    };
    let mut totals = Vec::new();
    let mut updated = 0;
    for _ in 0..BATCH_REPEATS {
        let started = Instant::now();
        let nodes = cached_walk(walker, root, &first);
        updated = 0;
        for node in &nodes {
            let advertises = gated.iter().any(|(availability, _)| {
                node.get_cached_property_value(*availability)
                    .ok()
                    .and_then(|variant| boolean_of(&variant))
                    == Some(true)
            });
            if advertises && node.build_updated_cache(&second).is_ok() {
                updated += 1;
            }
        }
        totals.push(started.elapsed().as_micros());
    }
    totals.sort_unstable();
    json!({
        "nodes_needing_a_round_trip": updated,
        "total_min_micros": totals[0],
        "total_median_micros": totals[totals.len() / 2],
        "total_max_micros": totals[totals.len() - 1],
    })
}

/// Question 9: whether a cached element-returning property hands back an
/// element carrying the same request's properties, which is what makes
/// `LabeledBy` cost nothing beyond the payload it already rides in.
pub fn measure_cached_labeled_by(automation: &UIAutomation, root: &UIElement) -> Value {
    let walker = match automation.get_raw_view_walker() {
        Ok(walker) => walker,
        Err(error) => return json!({ "failed": failure_shape(&error) }),
    };
    let request = match build_request(automation, &FULL_SET) {
        Ok(request) => request,
        Err(error) => return json!({ "failed": failure_shape(&error) }),
    };
    let nodes = cached_walk(&walker, root, &request);
    let rows: Vec<Value> = nodes
        .iter()
        .filter_map(|node| {
            let variant = node.get_cached_property_value(UIProperty::LabeledBy).ok()?;
            let target = element_of(&variant)?;
            Some(json!({
                "control_type": control_type_of(node),
                "target_control_type": control_type_of(&target),
                "target_cached_name": match target.get_cached_property_value(UIProperty::Name) {
                    Ok(value) => variant_shape(&value),
                    Err(error) => json!({ "failed": failure_shape(&error) }),
                },
                "target_cached_is_password": match target
                    .get_cached_property_value(UIProperty::IsPassword)
                {
                    Ok(value) => variant_shape(&value),
                    Err(error) => json!({ "failed": failure_shape(&error) }),
                },
            }))
        })
        .collect();
    json!({ "cached_nodes": nodes.len(), "labelled_nodes": rows.len(), "rows": rows })
}

fn element_of(variant: &Variant) -> Option<UIElement> {
    use uiautomation::variants::Value as VariantValue;
    use windows::Win32::UI::Accessibility::IUIAutomationElement;
    use windows::core::Interface;
    let VariantValue::UNKNOWN(unknown) = variant.get_value().ok()? else {
        return None;
    };
    let element: IUIAutomationElement = unknown.cast().ok()?;
    Some(UIElement::from(&element))
}
