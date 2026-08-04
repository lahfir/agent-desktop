//! Evidence reading and the ambiguity census for the 2.5 resolution probe.
//!
//! Reads the identity evidence the shipped resolver compares (AutomationId,
//! control type, name, bounding rectangle) off every walked element, then
//! answers two questions the plan refuses to infer:
//!
//! - the 0/1/N shape over a real tree: how many distinct `AutomationId`
//!   values resolve to exactly one candidate, how many to two or more
//!   (the live `AMBIGUOUS_TARGET` population), and how many ref-able
//!   elements carry no identifier at all;
//! - the ambiguity census item U1-7 prices: zero-extent bounds, duplicate
//!   bounds, and ref-able elements whose only stored corroborant would be a
//!   degenerate hash.
//!
//! Everything is shape and count: identifiers are bucketed, never echoed.

use std::collections::HashMap;

use serde_json::{Value, json};
use uiautomation::types::UIProperty;
use uiautomation::{UIElement, UITreeWalker};

pub const WALK_DEPTH_LIMIT: u32 = 50;

pub fn number_of(variant: &uiautomation::variants::Variant) -> Option<i32> {
    match variant.get_value() {
        Ok(uiautomation::variants::Value::I4(number) | uiautomation::variants::Value::INT(number)) => {
            Some(number)
        }
        _ => None,
    }
}

pub fn boolean_of(variant: &uiautomation::variants::Variant) -> Option<bool> {
    match variant.get_value() {
        Ok(uiautomation::variants::Value::BOOL(flag)) => Some(flag),
        _ => None,
    }
}

pub fn control_type_of(element: &UIElement) -> i32 {
    element
        .get_property_value(UIProperty::ControlType)
        .ok()
        .and_then(|variant| number_of(&variant))
        .unwrap_or(0)
}

/// A coarse approximation of the product's ref-able set (interactive roles
/// plus actionable elements), for census accounting only. The product's
/// INTERACTIVE_ROLES live in core; a probe may only approximate them.
pub fn is_ref_able_control_type(control_type: i32) -> bool {
    matches!(
        control_type,
        50000 // Button
        | 50002 // CheckBox
        | 50003 // ComboBox
        | 50004 // Edit
        | 50005 // Hyperlink
        | 50007 // ListItem
        | 50011 // MenuItem
        | 50013 // RadioButton
        | 50014 // ScrollBar
        | 50015 // Slider
        | 50016 // Spinner
        | 50018 // Tab
        | 50019 // TabItem
        | 50024 // TreeItem
        | 50029 // DataItem
        | 50031 // SplitButton
        | 50035 // HeaderItem
    )
}

#[derive(Clone, Debug, Default)]
pub struct Evidence {
    pub native_id: Option<String>,
    pub name: Option<String>,
    pub control_type: i32,
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Evidence {
    pub fn zero_extent(&self) -> bool {
        self.right <= self.left || self.bottom <= self.top
    }
    pub fn fnv_hash(&self) -> u64 {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for part in [self.left, self.top, self.right, self.bottom] {
            let bytes = (part as u32).to_le_bytes();
            for byte in bytes {
                hash ^= byte as u64;
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        hash
    }
}

pub fn read_evidence(element: &UIElement) -> Evidence {
    let native_id = element
        .get_property_value(UIProperty::AutomationId)
        .ok()
        .and_then(|variant| variant.get_string().ok())
        .filter(|value| !value.trim().is_empty());
    let name = element
        .get_property_value(UIProperty::Name)
        .ok()
        .and_then(|variant| variant.get_string().ok())
        .filter(|value| !value.trim().is_empty());
    let control_type = control_type_of(element);
    let rectangle = element.get_bounding_rectangle().ok();
    let (left, top, right, bottom) = rectangle
        .map(|rect| (rect.get_left(), rect.get_top(), rect.get_right(), rect.get_bottom()))
        .unwrap_or((0, 0, 0, 0));
    Evidence {
        native_id,
        name,
        control_type,
        left,
        top,
        right,
        bottom,
    }
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

fn descendants(
    walker: &UITreeWalker,
    root: &UIElement,
    depth: u32,
    limit: u32,
    out: &mut Vec<UIElement>,
) {
    if depth >= limit {
        return;
    }
    for child in enumerate_children(walker, root) {
        descendants(walker, &child, depth + 1, limit, out);
        out.push(child);
    }
}

pub fn collect_descendants(walker: &UITreeWalker, root: &UIElement, depth: u32) -> Vec<UIElement> {
    let mut out = Vec::new();
    descendants(walker, root, 0, depth, &mut out);
    out
}

fn bucket<T: std::hash::Hash + Eq>(groups: &mut HashMap<T, usize>, key: T) {
    *groups.entry(key).or_insert(0) += 1;
}

/// U1-7's numbers over one walked tree.
pub fn measure_census(elements: &[UIElement]) -> Value {
    let evidence: Vec<Evidence> = elements.iter().map(read_evidence).collect();

    let ref_able = evidence
        .iter()
        .filter(|item| is_ref_able_control_type(item.control_type))
        .count();

    let mut id_groups: HashMap<(Option<String>, i32), usize> = HashMap::new();
    for item in &evidence {
        bucket(&mut id_groups, (item.native_id.clone(), item.control_type));
    }
    let mut id_resolve_1 = 0usize;
    let mut id_resolve_2plus = 0usize;
    let mut id_collision_groups: Vec<Value> = Vec::new();
    let mut id_values = 0usize;
    let mut id_elements = 0usize;
    for ((id, control_type), count) in &id_groups {
        if id.is_some() {
            id_values += 1;
            id_elements += count;
            if *count == 1 {
                id_resolve_1 += 1;
            } else {
                id_resolve_2plus += 1;
                id_collision_groups.push(json!({
                    "candidates": count,
                    "control_type": control_type,
                }));
            }
        }
    }

    let mut id_less_ref_able: HashMap<(i32, Option<String>), usize> = HashMap::new();
    for item in evidence
        .iter()
        .filter(|item| item.native_id.is_none() && is_ref_able_control_type(item.control_type))
    {
        bucket(&mut id_less_ref_able, (item.control_type, item.name.clone()));
    }
    let id_less_count = id_less_ref_able.len();
    let name_collision_groups = id_less_ref_able.values().filter(|count| **count > 1).count();

    let mut zero_extent_ref_able = 0usize;
    let mut degenerate_hash_only = 0usize;
    let mut positive_area_bounds = 0usize;
    let mut ref_able_with_id_positive_area = 0usize;
    for item in evidence
        .iter()
        .filter(|item| is_ref_able_control_type(item.control_type))
    {
        if item.zero_extent() {
            zero_extent_ref_able += 1;
            if item.native_id.is_none() {
                degenerate_hash_only += 1;
            }
        } else {
            positive_area_bounds += 1;
            if item.native_id.is_some() {
                ref_able_with_id_positive_area += 1;
            }
        }
    }

    let mut hash_counts: HashMap<u64, usize> = HashMap::new();
    for item in evidence
        .iter()
        .filter(|item| !item.zero_extent() && is_ref_able_control_type(item.control_type))
    {
        bucket(&mut hash_counts, item.fnv_hash());
    }
    let mut duplicate_bounds_groups = 0usize;
    let mut duplicate_bounds_elements = 0usize;
    for (_, count) in &hash_counts {
        if *count > 1 {
            duplicate_bounds_groups += 1;
            duplicate_bounds_elements += count;
        }
    }

    json!({
        "total_elements": elements.len(),
        "ref_able_approx": ref_able,
        "id_census": {
            "elements_carrying_id": id_elements,
            "distinct_id_values": id_values,
            "ids_resolving_to_exactly_one": id_resolve_1,
            "ids_resolving_to_two_or_more": id_resolve_2plus,
            "collision_groups": id_collision_groups,
        },
        "id_less_ref_able": {
            "distinct_role_and_name_keys": id_less_count,
            "role_and_name_collision_groups": name_collision_groups,
        },
        "bounds_census": {
            "ref_able_positive_area": positive_area_bounds,
            "ref_able_zero_extent": zero_extent_ref_able,
            "ref_able_zero_extent_id_less": degenerate_hash_only,
            "ref_able_with_id_positive_area": ref_able_with_id_positive_area,
            "duplicate_bounds_groups": duplicate_bounds_groups,
            "duplicate_bounds_elements": duplicate_bounds_elements,
        },
    })
}