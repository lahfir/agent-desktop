//! The A7-3 fixture-controlled reproduction (U1-2).
//!
//! A7-3 measured Explorer keying its list rows by index, with 5 of 29
//! `AutomationId`-keyed refs re-resolving to a different element after
//! content mutation. This arm builds the same shape on a repo-controlled
//! fixture: the WinForms scratch fixture's plain `ListBox`, and answers
//! whether the fixture's items are reachable by the product's own walk at
//! all (they are index-keyed by the default provider when exposed).
//!
//! Marker names only: the fixture is probe-authored synthetic content.

use std::collections::HashMap;
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};
use uiautomation::UIAutomation;
use windows_sys::Win32::Foundation::HWND;

use crate::measure::{collect_descendants, read_evidence, Evidence};

const LIST_ITEM_NATIVE: i32 = 50007;
const ITEM_PREFIX: &str = "Item-";
const WM_APP_MUTATE: u32 = 0x8000 + 5;

pub fn measure_list_swap(
    automation: &UIAutomation,
    root: &uiautomation::UIElement,
    handle: isize,
) -> Value {
    let walker = match automation.get_raw_view_walker() {
        Ok(walker) => walker,
        Err(error) => return json!({ "walker_failed": format!("{error}") }),
    };

    let before_elements = collect_descendants(&walker, root, crate::measure::WALK_DEPTH_LIMIT);
    let before_evidence: Vec<Evidence> = before_elements.iter().map(read_evidence).collect();
    let before_items = list_items(&before_evidence);
    let shape = tree_shape(&before_evidence);
    let walker_list_item_count = before_items.len();
    let index_id_ratio = if before_items.is_empty() {
        0.0
    } else {
        (before_items
            .iter()
            .filter(|item| is_index_id(&item.native_id))
            .count() as f64
            / before_items.len() as f64 * 100.0)
            .round() / 100.0
    };

    let send_result = unsafe {
        let ok = windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(
            handle as HWND,
            WM_APP_MUTATE,
            0,
            0,
        );
        ok != 0
    };

    thread::sleep(Duration::from_millis(900));
    let after_elements = collect_descendants(&walker, root, crate::measure::WALK_DEPTH_LIMIT);
    let after_evidence: Vec<Evidence> = after_elements.iter().map(read_evidence).collect();
    let after_items = list_items(&after_evidence);

    let mutants_created = ensure_mutation_applied(&before_items, &after_items);

    let mut landed_correct = 0usize;
    let mut caught_refuted = 0usize;
    let mut id_only_would_misresolve = 0usize;
    let mut gone = 0usize;
    let mut ambiguous = 0usize;
    for before in &before_items {
        let exact = after_items
            .iter()
            .filter(|after| {
                same_id(&before.native_id, &after.native_id)
                    && after.control_type == before.control_type
                    && before.name.is_some()
                    && after.name == before.name
            })
            .count();
        let id_only = after_items
            .iter()
            .filter(|after| same_id(&before.native_id, &after.native_id))
            .count();
        if exact == 1 {
            landed_correct += 1;
        } else if exact == 0 && id_only == 1 {
            caught_refuted += 1;
            if before.native_id.is_some() {
                id_only_would_misresolve += 1;
            }
        } else if exact == 0 && id_only == 0 {
            gone += 1;
        } else {
            ambiguous += 1;
        }
    }

    json!({
        "before_items": before_items.len(),
        "after_items": after_items.len(),
        "items_carrying_index_id": index_id_ratio,
        "index_id_ratio_percent": index_id_ratio,
        "mutation_trigger": { "postmessage_wm_app": send_result },
        "mutation_applied": mutants_created,
        "tree_shape": shape,
        "list_item_reachability": measure_list_item_reachability(automation, root, walker_list_item_count),
        "classification": {
            "landed_correct_exact": landed_correct,
            "caught_by_name_corroboration": caught_refuted,
            "id_only_would_misresolve": id_only_would_misresolve,
            "gone": gone,
            "ambiguous": ambiguous,
        },
    })
}

/// Whether the WinForms `ListBox` items are reachable at all, and through
/// which mechanism: the raw walker, a `FindAll` over the pane's children, or
/// a control-type condition over the descendants. A7-3's index-keyed wrong-
/// target shape is only reproducible on a fixture whose resolver can see the
/// items - this decides which branch U1-2 takes.
fn measure_list_item_reachability(
    automation: &UIAutomation,
    root: &uiautomation::UIElement,
    walker_list_item_count: usize,
) -> Value {
    use uiautomation::types::{PropertyConditionFlags, TreeScope, UIProperty};

    let mut pane_found = false;
    let mut children_findall_count: Option<usize> = None;
    let mut descendant_list_item_count: Option<usize> = None;

    if let Ok(condition) = automation.create_property_condition(
        UIProperty::AutomationId,
        "lstItems".to_string().into(),
        Some(PropertyConditionFlags::None),
    ) {
        if let Ok(pane) = root.find_first(TreeScope::Descendants, &condition) {
            pane_found = true;
            if let Ok(true_condition) = automation.create_true_condition() {
                children_findall_count = pane
                    .find_all(TreeScope::Children, &true_condition)
                    .ok()
                    .map(|list| list.len());
            }
        }
    }
    if let Ok(condition) = automation.create_property_condition(
        UIProperty::ControlType,
        50007i32.into(),
        None,
    ) {
        descendant_list_item_count = root
            .find_all(TreeScope::Descendants, &condition)
            .ok()
            .map(|list| list.len());
    }

    json!({
        "raw_walker_listitem_count": walker_list_item_count,
        "listbox_pane_found_by_id": pane_found,
        "pane_child_findall_count": children_findall_count,
        "descendant_listitem_findall_count": descendant_list_item_count,
    })
}

fn list_items(evidence: &[Evidence]) -> Vec<Evidence> {
    evidence
        .iter()
        .filter(|item| {
            item.control_type == LIST_ITEM_NATIVE
                && item
                    .name
                    .as_deref()
                    .is_some_and(|name| name.starts_with(ITEM_PREFIX))
        })
        .cloned()
        .collect()
}

fn is_index_id(id: &Option<String>) -> bool {
    id.as_deref()
        .is_some_and(|value| value.parse::<u32>().is_ok())
}

fn same_id(a: &Option<String>, b: &Option<String>) -> bool {
    match (a, b) {
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

fn ensure_mutation_applied(before: &[Evidence], after: &[Evidence]) -> bool {
    let before_names: Vec<&str> = before
        .iter()
        .filter_map(|item| item.name.as_deref())
        .collect();
    let name_sets_differ = after.iter().any(|item| {
        item.name
            .as_deref()
            .is_some_and(|name| !before_names.contains(&name))
    });
    let count_changed = before.len() != after.len();
    name_sets_differ || count_changed
}

/// Per-control-type histogram over the walked tree, plus how many `ListItem`
/// elements carry a name by prefix bucket. Diagnostic: explains a swap arm
/// that finds no list items on a target the probe had not walked before.
fn tree_shape(evidence: &[Evidence]) -> Value {
    let mut by_type: HashMap<i32, usize> = HashMap::new();
    let mut list_item_prefixes: HashMap<&str, usize> = HashMap::new();
    for item in evidence {
        *by_type.entry(item.control_type).or_insert(0) += 1;
        if item.control_type == LIST_ITEM_NATIVE {
            let bucket = item
                .name
                .as_deref()
                .map(|name| {
                    if name.starts_with(ITEM_PREFIX) {
                        "Item-"
                    } else if name.starts_with("Node-") {
                        "Node-"
                    } else if name.starts_with("Row-") {
                        "Row-"
                    } else {
                        "other"
                    }
                })
                .unwrap_or("<nameless>");
            *list_item_prefixes.entry(bucket).or_insert(0) += 1;
        }
    }
    let mut types: Vec<Value> = by_type
        .iter()
        .map(|(control_type, count)| json!({ "control_type": control_type, "count": count }))
        .collect();
    types.sort_by(|a, b| {
        a["control_type"]
            .as_i64()
            .unwrap_or(0)
            .cmp(&b["control_type"].as_i64().unwrap_or(0))
    });
    json!({
        "elements": evidence.len(),
        "control_types": types,
        "list_items_by_name_prefix": list_item_prefixes,
    })
}