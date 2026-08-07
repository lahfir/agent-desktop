//! ScrollIntoView failure surface and straddling-item geometry (U1 item 1).

use serde_json::{Value, json};
use uiautomation::UIAutomation;
use uiautomation::types::{Point, UIProperty};
use uiautomation::variants::Variant;

use crate::hit::{candidate_points, element_from_point_shape, root_from_hwnd};
use crate::util::{
    Bounds, automation_id_of, element_shape, failure_shape, find_by_automation_id,
    invoke_scroll_into_view, is_offscreen_of, scroll_item_available, walk_tree,
};
use crate::win;

fn item_id(index: u32) -> String {
    format!("lstItem-Item-{index:02}")
}

fn find_realized_or_by_id(
    automation: &UIAutomation,
    root: &uiautomation::UIElement,
    elements: &[uiautomation::UIElement],
    automation_id: &str,
) -> Option<uiautomation::UIElement> {
    if let Some(found) = find_by_automation_id(elements, automation_id) {
        return Some(found.clone());
    }
    let condition = automation
        .create_property_condition(UIProperty::AutomationId, Variant::from(automation_id), None)
        .ok()?;
    root.find_first(uiautomation::types::TreeScope::Descendants, &condition)
        .ok()
}

fn scroll_list_vertical(list: &uiautomation::UIElement, vertical_percent: f64) -> Value {
    match list.get_pattern::<uiautomation::patterns::UIScrollPattern>() {
        Ok(pattern) => match pattern.set_scroll_percent(-1.0, vertical_percent) {
            Ok(()) => json!({ "ok": true, "vertical_percent": vertical_percent }),
            Err(error) => json!({ "ok": false, "failure": failure_shape(&error) }),
        },
        Err(error) => json!({ "ok": false, "pattern": failure_shape(&error) }),
    }
}

fn measure_scroll_case(
    automation: &UIAutomation,
    hwnd: isize,
    automation_id: &str,
    label: &str,
) -> Value {
    let root = match root_from_hwnd(automation, hwnd) {
        Ok(root) => root,
        Err(error) => return json!({ "case": label, "error": error }),
    };
    let elements = match walk_tree(automation, &root) {
        Ok(elements) => elements,
        Err(error) => return json!({ "case": label, "error": error }),
    };
    if label == "below_fold" {
        if let Some(list) = find_by_automation_id(&elements, "lstItems") {
            let _ = scroll_list_vertical(list, 0.0);
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
    }
    let elements = match walk_tree(automation, &root) {
        Ok(elements) => elements,
        Err(error) => return json!({ "case": label, "error": error }),
    };
    let present_before_scroll = find_by_automation_id(&elements, automation_id).is_some()
        || find_realized_or_by_id(automation, &root, &elements, automation_id).is_some();
    // For below-fold: first record whether virtualization hid the item at the top,
    // then scroll the list far enough to realize it without ScrollIntoView yet.
    let mut realization_note = json!(null);
    let elements = if label == "below_fold" {
        let top_present = present_before_scroll;
        if let Some(list) = find_by_automation_id(&elements, "lstItems") {
            realization_note = json!({
                "at_scroll_top_present": top_present,
                "scroll_to_realize": scroll_list_vertical(list, 70.0),
            });
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        match walk_tree(automation, &root) {
            Ok(elements) => elements,
            Err(error) => return json!({ "case": label, "error": error }),
        }
    } else {
        elements
    };
    let Some(target) = find_realized_or_by_id(automation, &root, &elements, automation_id) else {
        return json!({
            "case": label,
            "present_in_walk": false,
            "present_via_findall": false,
            "automation_id_digest": crate::util::digest_of(automation_id),
            "walk_element_count": elements.len(),
            "list_item_count": elements.iter().filter(|e| {
                automation_id_of(e).as_deref().is_some_and(|id| id.starts_with("lstItem-"))
            }).count(),
        });
    };
    let present_in_walk = find_by_automation_id(&elements, automation_id).is_some();
    let before = Bounds::from_element(&target);
    let offscreen_before = is_offscreen_of(&target);
    let available = scroll_item_available(&target);
    let invoke = match invoke_scroll_into_view(&target) {
        Ok(()) => json!({ "ok": true }),
        Err(error) => json!({ "ok": false, "failure": failure_shape(&error) }),
    };
    std::thread::sleep(std::time::Duration::from_millis(250));
    let after = Bounds::from_element(&target);
    let offscreen_after = is_offscreen_of(&target);
    let geometry_moved = match (&before, &after) {
        (Some(a), Some(b)) => a.as_csv() != b.as_csv(),
        _ => false,
    };
    json!({
        "case": label,
        "present_in_walk": present_in_walk,
        "present_via_findall": true,
        "scroll_item_available": available,
        "bounds_before": before.as_ref().map(Bounds::as_csv),
        "bounds_after": after.as_ref().map(Bounds::as_csv),
        "is_offscreen_before": offscreen_before,
        "is_offscreen_after": offscreen_after,
        "geometry_moved": geometry_moved,
        "invoke": invoke,
        "target": element_shape(&target),
        "virtualization_note": "WPF ListBox default VirtualizingStackPanel Recycling",
        "realization": realization_note,
    })
}

fn measure_straddling(automation: &UIAutomation, hwnd: isize) -> Value {
    let root = match root_from_hwnd(automation, hwnd) {
        Ok(root) => root,
        Err(error) => return json!({ "error": error }),
    };
    let elements = match walk_tree(automation, &root) {
        Ok(elements) => elements,
        Err(error) => return json!({ "error": error }),
    };
    let Some(list) = find_by_automation_id(&elements, "lstItems") else {
        return json!({ "list_present": false });
    };
    let Some(list_bounds) = Bounds::from_element(list) else {
        return json!({ "list_bounds": null });
    };
    let scroll = scroll_list_vertical(list, 40.0);
    std::thread::sleep(std::time::Duration::from_millis(250));
    let elements = match walk_tree(automation, &root) {
        Ok(elements) => elements,
        Err(error) => return json!({ "error": error, "scroll": scroll }),
    };
    let below_id = item_id(12);
    let Some(below) = find_realized_or_by_id(automation, &root, &elements, &below_id) else {
        return json!({
            "list_bounds": list_bounds.as_csv(),
            "below_fold_present": false,
            "scroll": scroll,
            "note": "item still absent after ScrollPattern realize",
        });
    };
    let _ = invoke_scroll_into_view(&below);
    std::thread::sleep(std::time::Duration::from_millis(150));
    let near_id = item_id(14);
    if let Some(near) = find_realized_or_by_id(automation, &root, &elements, &near_id) {
        let _ = invoke_scroll_into_view(&near);
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    // Nudge the list so an item straddles the viewport edge.
    if let Ok(tree) = walk_tree(automation, &root) {
        if let Some(list) = find_by_automation_id(&tree, "lstItems") {
            let _ = scroll_list_vertical(list, 45.0);
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
    }
    let refreshed = match walk_tree(automation, &root) {
        Ok(elements) => elements,
        Err(error) => return json!({ "refresh_error": error }),
    };
    let list = find_by_automation_id(&refreshed, "lstItems");
    let list_bounds = list.and_then(Bounds::from_element);
    let mut straddler = None;
    for element in &refreshed {
        let Some(id) = automation_id_of(element) else {
            continue;
        };
        if !id.starts_with("lstItem-") {
            continue;
        }
        let Some(bounds) = Bounds::from_element(element) else {
            continue;
        };
        let Some(list_b) = list_bounds.as_ref() else {
            continue;
        };
        let Some(intersection) = bounds.intersection(list_b) else {
            continue;
        };
        if intersection.height() > 0
            && intersection.height() < bounds.height()
            && bounds.has_area()
        {
            straddler = Some((id, bounds, intersection));
            break;
        }
    }
    let Some((id, bounds, intersection)) = straddler else {
        return json!({
            "list_bounds": list_bounds.as_ref().map(Bounds::as_csv),
            "straddler_found": false,
            "branch": "no_straddler_staged",
        });
    };
    let clipped = bounds.as_csv() == intersection.as_csv()
        || (bounds.top >= list_bounds.as_ref().map(|b| b.top).unwrap_or(0)
            && bounds.bottom <= list_bounds.as_ref().map(|b| b.bottom).unwrap_or(0));
    let provider_clipped = bounds.height() == intersection.height()
        && (bounds.top < list_bounds.as_ref().map(|b| b.top).unwrap_or(i32::MIN)
            || bounds.bottom > list_bounds.as_ref().map(|b| b.bottom).unwrap_or(i32::MAX));
    let out_of_viewport_points: Vec<Value> = candidate_points(&bounds)
        .into_iter()
        .filter(|(x, y)| {
            list_bounds
                .as_ref()
                .is_some_and(|lb| !lb.contains(*x, *y))
        })
        .map(|(x, y)| {
            json!({
                "point": format!("{x},{y}"),
                "hit": element_from_point_shape(automation, x, y),
            })
        })
        .collect();
    let viewport_clipped = bounds.height() <= intersection.height()
        && list_bounds
            .as_ref()
            .is_some_and(|lb| bounds.top >= lb.top && bounds.bottom <= lb.bottom);
    let measured_clipped = !out_of_viewport_points.is_empty()
        && bounds.intersection(list_bounds.as_ref().unwrap()).is_some()
        && (bounds.top < list_bounds.as_ref().unwrap().top
            || bounds.bottom > list_bounds.as_ref().unwrap().bottom);
    let branch = if measured_clipped && viewport_clipped {
        "clipped_bounds"
    } else if measured_clipped {
        "provider_rect_extends_outside_viewport"
    } else if bounds.intersection(list_bounds.as_ref().unwrap()).is_some()
        && (bounds.top < list_bounds.as_ref().unwrap().top
            || bounds.bottom > list_bounds.as_ref().unwrap().bottom)
    {
        "unclipped_full_rect"
    } else {
        "fully_inside_or_outside"
    };
    let _ = (clipped, provider_clipped);
    json!({
        "straddler_found": true,
        "automation_id_digest": crate::util::digest_of(&id),
        "item_bounds": bounds.as_csv(),
        "list_bounds": list_bounds.as_ref().map(Bounds::as_csv),
        "intersection": intersection.as_csv(),
        "bounding_rectangle_equals_intersection": bounds.as_csv() == intersection.as_csv(),
        "provider_rect_extends_outside_viewport": bounds.top < list_bounds.as_ref().unwrap().top
            || bounds.bottom > list_bounds.as_ref().unwrap().bottom,
        "out_of_viewport_candidate_hits": out_of_viewport_points,
        "ktd4_branch": branch,
    })
}

pub fn measure_scroll(automation: &UIAutomation, hwnd: isize) -> Value {
    let visible = measure_scroll_case(automation, hwnd, &item_id(0), "already_visible");
    let below = measure_scroll_case(automation, hwnd, &item_id(30), "below_fold");
    let straddling = measure_straddling(automation, hwnd);

    win::minimize_only(hwnd);
    std::thread::sleep(std::time::Duration::from_millis(300));
    let minimized = measure_scroll_case(automation, hwnd, &item_id(1), "minimized_window");
    win::restore_only(hwnd);
    std::thread::sleep(std::time::Duration::from_millis(300));
    win::minimize_restore(hwnd);

    json!({
        "stack": "uia3-com-product-CUIAutomation8",
        "fixture": "ScratchWpf tall ListBox",
        "already_visible": visible,
        "below_fold": below,
        "minimized_window": minimized,
        "straddling": straddling,
        "killed_provider": {
            "staged_here": false,
            "note": "killed-provider staged by orchestrator after this capture",
        },
    })
}

pub fn measure_killed_provider(automation: &UIAutomation, hwnd: isize, kill: impl FnOnce()) -> Value {
    let root = match root_from_hwnd(automation, hwnd) {
        Ok(root) => root,
        Err(error) => return json!({ "error": error }),
    };
    let elements = match walk_tree(automation, &root) {
        Ok(elements) => elements,
        Err(error) => return json!({ "error": error }),
    };
    let Some(target) = find_realized_or_by_id(automation, &root, &elements, &item_id(5)) else {
        return json!({ "present_in_walk": false });
    };
    let before = Bounds::from_element(&target);
    let available = scroll_item_available(&target);
    kill();
    std::thread::sleep(std::time::Duration::from_millis(400));
    let invoke = match invoke_scroll_into_view(&target) {
        Ok(()) => json!({ "ok": true }),
        Err(error) => json!({ "ok": false, "failure": failure_shape(&error) }),
    };
    let after_read = match Bounds::from_element(&target) {
        Some(bounds) => json!({
            "ok": true,
            "bounds": bounds.as_csv(),
            "has_area": bounds.has_area(),
            "empty_values": !bounds.has_area(),
        }),
        None => json!({ "ok": false, "read_failed": true }),
    };
    let offscreen_after = is_offscreen_of(&target);
    let point_probe = before.as_ref().map(|b| {
        let (x, y) = b.center();
        match automation.element_from_point(Point::new(x, y)) {
            Ok(_) => json!({ "ok": true }),
            Err(error) => json!({ "ok": false, "failure": failure_shape(&error) }),
        }
    });
    let branch = if after_read.get("ok") == Some(&json!(true))
        && after_read.get("empty_values") == Some(&json!(true))
    {
        "succeeding_empty_reads_a14_9"
    } else if after_read.get("ok") == Some(&json!(false)) {
        "errored_observation"
    } else {
        "observation_returned_geometry"
    };
    json!({
        "present_in_walk": true,
        "scroll_item_available_before_kill": available,
        "bounds_before": before.as_ref().map(Bounds::as_csv),
        "invoke_after_kill": invoke,
        "observation_after_kill": after_read,
        "is_offscreen_after_kill": offscreen_after,
        "element_from_point_after_kill": point_probe,
        "ktd5_arm": "delivered_unverified",
        "observation_branch": branch,
    })
}
