//! Corroboration matrix: one leg per KTD3 arm (U1 item 3).

use serde_json::{Value, json};
use uiautomation::UIAutomation;
use uiautomation::types::Point;

use crate::hit::{
    classify_hit, element_root_hwnd, ktd3_arm, native_hwnd, pid_of_hwnd, relation_name,
    root_from_hwnd, window_from_point_child, window_from_point_root,
};
use crate::util::{Bounds, element_shape, failure_shape, find_by_automation_id, walk_tree};
use crate::win;

fn probe_point(
    automation: &UIAutomation,
    target: &uiautomation::UIElement,
    x: i32,
    y: i32,
    label: &str,
    extra: Value,
) -> Value {
    let target_root = element_root_hwnd(automation, target);
    let hit = match automation.element_from_point(Point::new(x, y)) {
        Ok(element) => element,
        Err(error) => {
            return json!({
                "leg": label,
                "point": format!("{x},{y}"),
                "element_from_point": { "ok": false, "failure": failure_shape(&error) },
                "extra": extra,
            });
        }
    };
    let hit_root = element_root_hwnd(automation, &hit);
    let win32_root = window_from_point_root(x, y);
    let win32_child = window_from_point_child(x, y);
    let relation = classify_hit(automation, target, &hit);
    let arm = ktd3_arm(target_root, hit_root, win32_root);
    let occluder_evidence = json!({
        "role_control_type": crate::util::control_type_of(&hit),
        "name_withheld": true,
        "bounds": Bounds::from_element(&hit).as_ref().map(Bounds::as_csv),
        "evidence_read_ok": Bounds::from_element(&hit).is_some(),
    });
    json!({
        "leg": label,
        "point": format!("{x},{y}"),
        "target_root": target_root,
        "hit_root": hit_root,
        "win32_root": win32_root,
        "win32_child": win32_child,
        "hit_native_hwnd": native_hwnd(&hit),
        "target_pid": target_root.map(pid_of_hwnd),
        "hit_pid": hit_root.map(pid_of_hwnd),
        "win32_pid": pid_of_hwnd(win32_root),
        "relation": relation_name(relation),
        "ktd3_arm": arm,
        "hit": element_shape(&hit),
        "occluder_evidence": occluder_evidence,
        "extra": extra,
    })
}

fn same_root_overlay(automation: &UIAutomation, hwnd: isize, stack: &str) -> Value {
    win::minimize_restore(hwnd);
    let root = match root_from_hwnd(automation, hwnd) {
        Ok(root) => root,
        Err(error) => return json!({ "stack": stack, "error": error }),
    };
    let elements = match walk_tree(automation, &root) {
        Ok(elements) => elements,
        Err(error) => return json!({ "stack": stack, "error": error }),
    };
    let Some(covered) = find_by_automation_id(&elements, "btnCovered") else {
        return json!({ "stack": stack, "btnCovered_present": false });
    };
    let Some(overlay) = find_by_automation_id(&elements, "btnOverlay") else {
        return json!({ "stack": stack, "btnOverlay_present": false });
    };
    let Some(bounds) = Bounds::from_element(overlay) else {
        return json!({ "stack": stack, "overlay_bounds": null });
    };
    let (x, y) = bounds.center();
    let child_vs_native = {
        let win32_child = window_from_point_child(x, y);
        let hit = automation.element_from_point(Point::new(x, y)).ok();
        let hit_hwnd = hit.as_ref().map(native_hwnd).unwrap_or(0);
        json!({
            "win32_child_hwnd": win32_child,
            "hit_native_hwnd": hit_hwnd,
            "child_matches_hit_native": win32_child != 0 && win32_child == hit_hwnd,
            "informative_only": true,
        })
    };
    let mut row = probe_point(
        automation,
        covered,
        x,
        y,
        "same_root_overlay",
        json!({ "stack": stack, "child_hwnd_vs_native": child_vs_native }),
    );
    if let Some(object) = row.as_object_mut() {
        object.insert("stack".into(), json!(stack));
    }
    row
}

fn style_occluder_leg(
    automation: &UIAutomation,
    target_hwnd: isize,
    occluder: &win::HostedWindow,
    label: &str,
) -> Value {
    let root = match root_from_hwnd(automation, target_hwnd) {
        Ok(root) => root,
        Err(error) => return json!({ "leg": label, "error": error }),
    };
    let Some(bounds) = crate::util::Bounds::from_element(&root) else {
        return json!({ "leg": label, "target_bounds": null });
    };
    let (x, y) = bounds.center();
    win::minimize_restore(occluder.handle);
    probe_point(
        automation,
        &root,
        x,
        y,
        label,
        json!({
            "occluder_hwnd": occluder.handle,
            "occluder_rect": win::window_rect_csv(occluder.handle),
        }),
    )
}

pub fn measure_corroborate(
    automation: &UIAutomation,
    wpf_hwnd: Option<isize>,
    winforms_hwnd: Option<isize>,
    foreign_hwnd: Option<isize>,
) -> Value {
    let mut legs = Vec::new();

    match win::spawn_overlap_pair() {
        Ok((under, over)) => {
            if let Ok(root) = root_from_hwnd(automation, under.handle) {
                if let Some(bounds) = Bounds::from_element(&root) {
                    let ox = bounds.left + 80;
                    let oy = bounds.top + 60;
                    legs.push(probe_point(
                        automation,
                        &root,
                        ox,
                        oy,
                        "same_process_overlap",
                        json!({
                            "under": under.handle,
                            "over": over.handle,
                        }),
                    ));
                } else {
                    legs.push(json!({ "leg": "same_process_overlap", "error": "no bounds" }));
                }
                let mut foreign_host = None;
                let foreign = if let Some(hwnd) = foreign_hwnd {
                    Some(hwnd)
                } else if let Ok(hosted) = win::spawn_plain_window(
                    "a18-foreign-fallback",
                    140,
                    140,
                    400,
                    280,
                    0,
                    windows_sys::Win32::UI::WindowsAndMessaging::WS_OVERLAPPEDWINDOW,
                ) {
                    let handle = hosted.handle;
                    foreign_host = Some(hosted);
                    Some(handle)
                } else {
                    None
                };
                if let Some(foreign) = foreign {
                    win::minimize_restore(foreign);
                    if let Some(bounds) = Bounds::from_element(&root) {
                        let (x, y) = bounds.center();
                        legs.push(probe_point(
                            automation,
                            &root,
                            x,
                            y,
                            "foreign_process_occluder",
                            json!({
                                "foreign": foreign,
                                "foreign_pid": crate::hit::pid_of_hwnd(foreign),
                                "target_pid": crate::hit::pid_of_hwnd(under.handle),
                                "pids_differ": crate::hit::pid_of_hwnd(foreign)
                                    != crate::hit::pid_of_hwnd(under.handle),
                                "same_process_fallback": foreign_host.is_some(),
                            }),
                        ));
                    }
                    let third = win::spawn_plain_window(
                        "a18-third",
                        180,
                        180,
                        360,
                        240,
                        0,
                        windows_sys::Win32::UI::WindowsAndMessaging::WS_OVERLAPPEDWINDOW,
                    );
                    if let Ok(third) = third {
                        win::minimize_restore(third.handle);
                        if let Some(bounds) = Bounds::from_element(&root) {
                            let (x, y) = bounds.center();
                            legs.push(probe_point(
                                automation,
                                &root,
                                x,
                                y,
                                "three_window_stack",
                                json!({
                                    "under": under.handle,
                                    "foreign": foreign,
                                    "third": third.handle,
                                    "raised_nothing": true,
                                }),
                            ));
                        }
                    }
                }
                drop(foreign_host);
            } else {
                legs.push(json!({ "leg": "same_process_overlap", "error": "root unresolved" }));
            }
            // Hosts drop here after legs finish.
            drop(over);
            drop(under);
        }
        Err(error) => legs.push(json!({ "leg": "same_process_overlap", "error": error })),
    }

    if let Some(hwnd) = wpf_hwnd {
        legs.push(same_root_overlay(automation, hwnd, "wpf"));
    }
    if let Some(hwnd) = winforms_hwnd {
        legs.push(same_root_overlay(automation, hwnd, "winforms"));
    }

    if let Ok(target) = win::spawn_plain_window(
        "a18-style-target",
        100,
        100,
        320,
        200,
        0,
        windows_sys::Win32::UI::WindowsAndMessaging::WS_OVERLAPPEDWINDOW,
    ) {
        if let Ok(layered) = win::spawn_layered_occluder(120, 110) {
            legs.push(style_occluder_leg(automation, target.handle, &layered, "ws_ex_layered"));
        }
        if let Ok(transparent) = win::spawn_transparent_occluder(120, 110) {
            legs.push(style_occluder_leg(
                automation,
                target.handle,
                &transparent,
                "ws_ex_transparent",
            ));
        }
        if let Ok(disabled) = win::spawn_disabled_occluder(120, 110) {
            legs.push(style_occluder_leg(
                automation,
                target.handle,
                &disabled,
                "ws_disabled",
            ));
        }
    }

    json!({
        "stack": "uia3-com-product-CUIAutomation8",
        "legs": legs,
        "elevated_occluder": {
            "staged_here": false,
            "note": "elevated High-over-Medium staged by orchestrator",
        },
    })
}

pub fn measure_elevated_pair(
    automation: &UIAutomation,
    medium_hwnd: isize,
    high_hwnd: isize,
) -> Value {
    let root = match root_from_hwnd(automation, medium_hwnd) {
        Ok(root) => root,
        Err(error) => return json!({ "error": error }),
    };
    win::minimize_restore(high_hwnd);
    let Some(bounds) = Bounds::from_element(&root) else {
        return json!({ "target_bounds": null });
    };
    let (x, y) = bounds.center();
    probe_point(
        automation,
        &root,
        x,
        y,
        "elevated_high_over_medium",
        json!({
            "medium_hwnd": medium_hwnd,
            "high_hwnd": high_hwnd,
        }),
    )
}
