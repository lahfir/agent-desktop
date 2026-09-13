//! U1 item 7: combobox dance + nested scroll ladder geometry.

use serde_json::{Value, json};
use uiautomation::patterns::UIScrollPattern;
use uiautomation::types::ScrollAmount;
use uiautomation::UIAutomation;

use crate::ops::{
    bounds_of, expand_pattern, read_expand, read_selected, scroll_small, select_pattern,
};
use crate::util::{
    automation_id_of, digest_of, element_shape, find_by_automation_id, refind, root_from_hwnd,
    walk_tree, Bounds,
};

pub fn measure_combobox(automation: &UIAutomation, hwnd: isize) -> Value {
    if hwnd == 0 {
        return json!({ "skipped": "hwnd unavailable" });
    }
    let combo = match refind(automation, hwnd, "cboChoice") {
        Ok(element) => element,
        Err(error) => return error,
    };
    let _ = collapse_if_needed(automation, hwnd);
    let collapsed_walk = match root_from_hwnd(automation, hwnd).and_then(|root| walk_tree(automation, &root)) {
        Ok(elements) => elements,
        Err(error) => return error,
    };
    let children_collapsed: Vec<_> = collapsed_walk
        .iter()
        .filter_map(|element| {
            let id = automation_id_of(element)?;
            if id.starts_with("cboItem") {
                Some(json!({
                    "automation_id_digest": digest_of(&id),
                    "selected": read_selected(element),
                }))
            } else {
                None
            }
        })
        .collect();
    let expand = expand_pattern(&combo);
    std::thread::sleep(std::time::Duration::from_millis(150));
    let expanded_root = match root_from_hwnd(automation, hwnd) {
        Ok(root) => root,
        Err(error) => return error,
    };
    let expanded_walk = match walk_tree(automation, &expanded_root) {
        Ok(elements) => elements,
        Err(error) => return error,
    };
    let children_expanded: Vec<_> = expanded_walk
        .iter()
        .filter_map(|element| {
            let id = automation_id_of(element)?;
            if id.starts_with("cboItem") {
                Some(json!({
                    "automation_id_digest": digest_of(&id),
                    "bounds": bounds_of(element),
                    "selected": read_selected(element),
                }))
            } else {
                None
            }
        })
        .collect();
    let last_id = "cboItem7";
    let last_present_after_expand = find_by_automation_id(&expanded_walk, last_id).is_some();
    let select_target = find_by_automation_id(&expanded_walk, "cboItem2").cloned();
    let select = select_target
        .as_ref()
        .map(select_pattern)
        .unwrap_or(json!({ "skipped": "cboItem2 absent after expand" }));
    std::thread::sleep(std::time::Duration::from_millis(100));
    let verify = refind(automation, hwnd, "cboItem2")
        .map(|el| read_selected(&el))
        .unwrap_or(json!({ "refind_failed": true }));
    let branch = if children_collapsed.is_empty() && !children_expanded.is_empty() {
        if last_present_after_expand {
            "expansion_required_and_fully_realized"
        } else {
            "expansion_required_not_fully_realized_needs_scroll"
        }
    } else if !children_collapsed.is_empty() {
        "children_present_while_collapsed"
    } else {
        "no_selection_items_observed"
    };
    json!({
        "expand_state_before": read_expand(&combo),
        "expand_call": expand,
        "children_collapsed_count": children_collapsed.len(),
        "children_expanded_count": children_expanded.len(),
        "children_collapsed": children_collapsed,
        "children_expanded": children_expanded,
        "last_item_present_after_expand": last_present_after_expand,
        "select": select,
        "verify_selected": verify,
        "branch": branch,
    })
}

fn collapse_if_needed(automation: &UIAutomation, hwnd: isize) {
    if let Ok(combo) = refind(automation, hwnd, "cboChoice") {
        if let Ok(pattern) =
            combo.get_pattern::<uiautomation::patterns::UIExpandCollapsePattern>()
        {
            let _ = pattern.collapse();
            std::thread::sleep(std::time::Duration::from_millis(80));
        }
    }
}

pub fn measure_nested_scroll(automation: &UIAutomation, hwnd: isize) -> Value {
    if hwnd == 0 {
        return json!({ "skipped": "hwnd unavailable" });
    }
    let root = match root_from_hwnd(automation, hwnd) {
        Ok(root) => root,
        Err(error) => return error,
    };
    let elements = match walk_tree(automation, &root) {
        Ok(elements) => elements,
        Err(error) => return error,
    };
    let outer = find_by_automation_id(&elements, "svOuter")
        .or_else(|| find_by_automation_id(&elements, "pnlScrollOuter"));
    let inner = find_by_automation_id(&elements, "svInner")
        .or_else(|| find_by_automation_id(&elements, "pnlScrollInner"));
    let target = find_by_automation_id(&elements, "btnNestedDeep");
    let (Some(outer), Some(inner), Some(target)) = (outer, inner, target) else {
        return json!({
            "skipped": "nested scroll controls absent",
            "outer": outer.map(element_shape),
            "inner": inner.map(element_shape),
            "target": target.map(element_shape),
        });
    };
    let outer_bounds = Bounds::from_element(outer).map(|b| b.as_csv());
    let inner_bounds = Bounds::from_element(inner).map(|b| b.as_csv());
    let target_before = Bounds::from_element(target).map(|b| b.as_csv());
    let target_offscreen_before = target.is_offscreen().ok();
    let mut rungs = Vec::new();
    for step in 0..10 {
        let fresh_root = match root_from_hwnd(automation, hwnd) {
            Ok(root) => root,
            Err(_) => break,
        };
        let fresh = match walk_tree(automation, &fresh_root) {
            Ok(elements) => elements,
            Err(_) => break,
        };
        let Some(target_now) = find_by_automation_id(&fresh, "btnNestedDeep").cloned() else {
            rungs.push(json!({ "step": step, "target_absent": true }));
            break;
        };
        let Some(outer_now) = find_by_automation_id(&fresh, "svOuter")
            .or_else(|| find_by_automation_id(&fresh, "pnlScrollOuter"))
            .cloned()
        else {
            break;
        };
        let Some(inner_now) = find_by_automation_id(&fresh, "svInner")
            .or_else(|| find_by_automation_id(&fresh, "pnlScrollInner"))
            .cloned()
        else {
            break;
        };
        let target_bounds = Bounds::from_element(&target_now);
        let outer_b = Bounds::from_element(&outer_now);
        let inner_b = Bounds::from_element(&inner_now);
        let visible_in_outer = match (&target_bounds, &outer_b) {
            (Some(t), Some(o)) => t.top >= o.top && t.bottom <= o.bottom,
            _ => false,
        };
        let visible_in_inner = match (&target_bounds, &inner_b) {
            (Some(t), Some(i)) => t.top >= i.top && t.bottom <= i.bottom,
            _ => false,
        };
        if visible_in_outer && visible_in_inner {
            rungs.push(json!({
                "step": step,
                "direction": "none",
                "target_bounds": target_bounds.as_ref().map(Bounds::as_csv),
                "outer_bounds": outer_b.as_ref().map(Bounds::as_csv),
                "inner_bounds": inner_b.as_ref().map(Bounds::as_csv),
                "visible": true,
            }));
            break;
        }
        let scroll_inner_first = !visible_in_inner;
        let (ancestor, label) = if scroll_inner_first {
            (inner_now, "inner")
        } else {
            (outer_now, "outer")
        };
        let direction = match (&target_bounds, Bounds::from_element(&ancestor)) {
            (Some(t), Some(a)) if t.top < a.top => "before_vertical",
            (Some(t), Some(a)) if t.bottom > a.bottom => "after_vertical",
            _ => "none",
        };
        let call = if direction == "after_vertical" {
            scroll_small(&ancestor, ScrollAmount::NoAmount, ScrollAmount::SmallIncrement)
        } else if direction == "before_vertical" {
            scroll_small(&ancestor, ScrollAmount::NoAmount, ScrollAmount::SmallDecrement)
        } else {
            json!({ "ok": false, "note": "no direction" })
        };
        rungs.push(json!({
            "step": step,
            "ancestor": label,
            "direction": direction,
            "call": call,
            "target_bounds": target_bounds.as_ref().map(Bounds::as_csv),
            "outer_bounds": outer_b.as_ref().map(Bounds::as_csv),
            "inner_bounds": inner_b.as_ref().map(Bounds::as_csv),
            "visible_in_outer": visible_in_outer,
            "visible_in_inner": visible_in_inner,
        }));
        std::thread::sleep(std::time::Duration::from_millis(80));
        if direction == "none" {
            break;
        }
    }
    let realized = rungs.iter().any(|row| {
        row.get("visible").and_then(|v| v.as_bool()) == Some(true)
    });
    json!({
        "outer_bounds_initial": outer_bounds,
        "inner_bounds_initial": inner_bounds,
        "target_bounds_initial": target_before,
        "target_offscreen_initial": target_offscreen_before,
        "outer_scroll_available": outer.get_pattern::<UIScrollPattern>().is_ok(),
        "inner_scroll_available": inner.get_pattern::<UIScrollPattern>().is_ok(),
        "rungs": rungs,
        "target_visible_after_ladder": realized,
        "branch": if realized {
            "ladder_geometry_measured_both_ancestors"
        } else {
            "ladder_exhausted_or_unscrollable"
        },
    })
}

pub fn measure(automation: &UIAutomation, wpf: Option<isize>) -> Value {
    json!({
        "combobox": wpf.map(|hwnd| measure_combobox(automation, hwnd)),
        "nested_scroll": wpf.map(|hwnd| measure_nested_scroll(automation, hwnd)),
    })
}
