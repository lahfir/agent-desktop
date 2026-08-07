//! Unknown-trigger coordinates and envelope / DPI / cost / chromium legs.

use serde_json::{Value, json};
use uiautomation::UIAutomation;
use uiautomation::types::Point;

use crate::hit::{
    candidate_points, classify_hit, element_from_point_shape, min_of_ms, relation_name,
    root_from_hwnd, virtual_screen, window_from_point_root,
};
use crate::util::{
    Bounds, control_type_of, digest_of, element_shape, failure_shape, find_by_automation_id,
    walk_tree,
};

pub fn measure_unknown(automation: &UIAutomation) -> Value {
    let screen = virtual_screen();
    let beyond_x = screen.right + 64;
    let beyond_y = screen.bottom + 64;
    let points = [
        ("neg_one", -1, -1),
        ("origin", 0, 0),
        ("beyond_virtual_screen", beyond_x, beyond_y),
        ("minimized_anchor_region", -32000, -32000),
    ];
    let mut rows = Vec::new();
    for (label, x, y) in points {
        let inside = screen.contains(x, y);
        let hit = element_from_point_shape(automation, x, y);
        rows.push(json!({
            "label": label,
            "point": format!("{x},{y}"),
            "inside_virtual_screen": inside,
            "element_from_point": hit,
            "win32_root": window_from_point_root(x, y),
            "guard_would_skip": !inside,
        }));
    }
    json!({
        "stack": "uia3-com-product-CUIAutomation8",
        "virtual_screen": screen.as_csv(),
        "points": rows,
        "ktd4_branch": "any_point_outside_virtual_screen_guarded_to_unknown_before_call",
        "guard_prevents": "desktop_answer_at_freed_or_offscreen_coordinates_reaching_corroboration_as_interception",
    })
}

pub fn measure_envelope(automation: &UIAutomation, wpf_hwnd: isize, winforms_hwnd: Option<isize>) -> Value {
    let wpf_root = match root_from_hwnd(automation, wpf_hwnd) {
        Ok(root) => root,
        Err(error) => return json!({ "wpf_error": error }),
    };
    let wpf_elements = match walk_tree(automation, &wpf_root) {
        Ok(elements) => elements,
        Err(error) => return json!({ "wpf_error": error }),
    };
    let zero = find_by_automation_id(&wpf_elements, "btnZeroSize");
    let disabled = find_by_automation_id(&wpf_elements, "btnDisabled");
    let wpf_zero = json!({
        "present_in_walk": zero.is_some(),
        "shape": zero.map(element_shape),
        "branch": if zero.is_some() {
            "wpf_exposes_zero_size_u5_live_case"
        } else {
            "wpf_hides_zero_size_fake_driven"
        },
    });
    let wpf_disabled = json!({
        "present_in_walk": disabled.is_some(),
        "shape": disabled.map(element_shape),
        "is_enabled": disabled.and_then(|e| e.is_enabled().ok()),
    });

    let mut winforms = json!({ "skipped": "no winforms hwnd" });
    if let Some(hwnd) = winforms_hwnd {
        if let Ok(root) = root_from_hwnd(automation, hwnd) {
            if let Ok(elements) = walk_tree(automation, &root) {
                let disabled = find_by_automation_id(&elements, "btnDisabled");
                let zero = find_by_automation_id(&elements, "btnZeroSize");
                winforms = json!({
                    "btnDisabled_present": disabled.is_some(),
                    "btnDisabled_enabled": disabled.and_then(|e| e.is_enabled().ok()),
                    "btnZeroSize_present": zero.is_some(),
                    "btnZeroSize_note": "A5-2: Win32 zero-size absent from walk",
                });
            }
        }
    }

    json!({
        "stack": "uia3-com-product-CUIAutomation8",
        "wpf_zero_size": wpf_zero,
        "wpf_disabled": wpf_disabled,
        "winforms": winforms,
    })
}

pub fn measure_dpi() -> Value {
    json!({
        "stack": "uia3-com-product-CUIAutomation8",
        "measurable": false,
        "branch": "precommitted_unmeasurable",
        "by_construction": "ElementFromPoint and BoundingRectangle share physical screen pixels under PER_MONITOR_AWARE_V2 (KTD7)",
        "deferral_citations": ["A10-3", "A16-4"],
        "reason": "this display offers zero scale steps; hosted runner reproduces single-96-DPI",
        "no_fake_measurement": true,
    })
}

pub fn measure_cost(
    automation: &UIAutomation,
    own_hwnd: Option<isize>,
    wpf_hwnd: Option<isize>,
    chromium_hwnd: Option<isize>,
) -> Value {
    let screen = virtual_screen();
    let desktop_point = (screen.left + 8, screen.top + 8);

    let efp_desktop = min_of_ms(|| {
        automation
            .element_from_point(Point::new(desktop_point.0, desktop_point.1))
            .map(|_| ())
            .map_err(|_| ())
    });

    let mut efp_own = json!({ "skipped": "no own hwnd" });
    let mut pre_read_own = json!({ "skipped": "no own hwnd" });
    let mut five_point = json!({ "skipped": "no own hwnd" });
    if let Some(hwnd) = own_hwnd {
        if let Ok(root) = root_from_hwnd(automation, hwnd) {
            if let Some(bounds) = Bounds::from_element(&root) {
                let (x, y) = bounds.center();
                efp_own = min_of_ms(|| {
                    automation
                        .element_from_point(Point::new(x, y))
                        .map(|_| ())
                        .map_err(|_| ())
                });
                pre_read_own = min_of_ms(|| {
                    Bounds::from_element(&root)
                        .filter(|b| b.has_area())
                        .map(|_| ())
                        .ok_or(())
                });
                let points = candidate_points(&bounds);
                five_point = min_of_ms(|| {
                    for (px, py) in points {
                        let _ = automation.element_from_point(Point::new(px, py));
                    }
                    Ok(())
                });
            }
        }
    }

    let mut efp_wpf = json!({ "skipped": "no wpf hwnd" });
    if let Some(hwnd) = wpf_hwnd {
        if let Ok(root) = root_from_hwnd(automation, hwnd) {
            if let Some(bounds) = Bounds::from_element(&root) {
                let (x, y) = bounds.center();
                efp_wpf = min_of_ms(|| {
                    automation
                        .element_from_point(Point::new(x, y))
                        .map(|_| ())
                        .map_err(|_| ())
                });
            }
        }
    }

    let mut efp_chromium = json!({ "skipped": "no chromium hwnd" });
    if let Some(hwnd) = chromium_hwnd {
        if let Ok(root) = root_from_hwnd(automation, hwnd) {
            if let Some(bounds) = Bounds::from_element(&root) {
                let (x, y) = bounds.center();
                efp_chromium = min_of_ms(|| {
                    automation
                        .element_from_point(Point::new(x, y))
                        .map(|_| ())
                        .map_err(|_| ())
                });
            }
        }
    }

    json!({
        "stack": "uia3-com-product-CUIAutomation8",
        "methodology": "min-of-seven discard warm-up (A15-13)",
        "element_from_point_desktop": efp_desktop,
        "element_from_point_own": efp_own,
        "element_from_point_wpf": efp_wpf,
        "element_from_point_chromium": efp_chromium,
        "target_pre_read": pre_read_own,
        "five_point_worst_case": five_point,
    })
}

pub fn measure_chromium(automation: &UIAutomation, hwnd: isize) -> Value {
    let root = match root_from_hwnd(automation, hwnd) {
        Ok(root) => root,
        Err(error) => return json!({ "error": error }),
    };
    // Fresh client protocol (A16-11): rebuild client after settle is orchestrator-owned;
    // this arm still uses the product bounded client for the probes themselves.
    let elements = match walk_tree(automation, &root) {
        Ok(elements) => elements,
        Err(error) => return json!({ "error": error }),
    };
    let leaf = elements.iter().rev().find(|element| {
        let ct = control_type_of(element);
        let Some(bounds) = Bounds::from_element(element) else {
            return false;
        };
        bounds.has_area()
            && bounds.width() > 24
            && bounds.height() > 16
            && bounds.width() < 800
            && bounds.height() < 400
            && !matches!(ct, 50032 | 50033 | 50030 | 50026) // Window/Pane/Document/Group hosts
    });
    let Some(target) = leaf else {
        return json!({
            "leaf_present": false,
            "walk_element_count": elements.len(),
            "branch": "target_absent_or_shell_bound",
        });
    };
    let Some(bounds) = Bounds::from_element(target) else {
        return json!({ "leaf_bounds": null });
    };
    let mut hits = Vec::new();
    for (idx, (x, y)) in candidate_points(&bounds).into_iter().enumerate() {
        let hit = match automation.element_from_point(Point::new(x, y)) {
            Ok(element) => {
                let relation = classify_hit(automation, target, &element);
                json!({
                    "candidate_index": idx,
                    "point": format!("{x},{y}"),
                    "ok": true,
                    "relation": relation_name(relation),
                    "hit": element_shape(&element),
                    "win32_root": window_from_point_root(x, y),
                })
            }
            Err(error) => json!({
                "candidate_index": idx,
                "point": format!("{x},{y}"),
                "ok": false,
                "failure": failure_shape(&error),
                "win32_root": window_from_point_root(x, y),
            }),
        };
        hits.push(hit);
    }
    let relations: Vec<&str> = hits
        .iter()
        .filter_map(|h| h.get("relation").and_then(|v| v.as_str()))
        .collect();
    let branch = if relations.iter().any(|r| *r == "ancestor") {
        "pane_ancestor_unknown"
    } else if relations
        .iter()
        .any(|r| *r == "self" || *r == "descendant")
    {
        "web_element_or_descendant_standard_rule"
    } else if relations.iter().any(|r| *r == "unrelated_same_root") {
        "unrelated_same_root_intercepted_or_fallback"
    } else {
        "inconclusive"
    };
    json!({
        "stack": "uia3-com-product-CUIAutomation8",
        "leaf_present": true,
        "target": element_shape(target),
        "target_id_digest": automation_id_of_digest(target),
        "five_candidate_hits": hits,
        "chromium_branch": branch,
        "walk_element_count": elements.len(),
    })
}

fn automation_id_of_digest(element: &uiautomation::UIElement) -> Option<String> {
    crate::util::automation_id_of(element).map(|id| digest_of(&id))
}
