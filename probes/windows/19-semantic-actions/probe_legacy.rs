//! U1 item 6: LegacyIAccessible.DoDefaultAction on legacy-only scratch + Notepad.

use serde_json::{Value, json};
use uiautomation::types::{Handle, UIProperty};
use uiautomation::variants::Variant;
use uiautomation::UIAutomation;

use crate::ops::{invoke_pattern, legacy_default_action, read_status_sink};
use crate::util::{
    digest_of, element_shape, pattern_available, refind, root_from_hwnd, walk_tree,
    window_is_foreground,
};

pub fn measure_scratch(automation: &UIAutomation, hwnd: isize) -> Value {
    if hwnd == 0 {
        return json!({ "skipped": "hwnd unavailable" });
    }
    let targets = ["btnLegacyOnly", "btnAction"];
    let mut rows = Vec::new();
    for automation_id in targets {
        let element = match refind(automation, hwnd, automation_id) {
            Ok(element) => element,
            Err(error) => {
                rows.push(json!({
                    "automation_id_digest": digest_of(automation_id),
                    "error": error,
                }));
                continue;
            }
        };
        let shape = element_shape(&element);
        let invoke_avail = pattern_available(&element, UIProperty::IsInvokePatternAvailable);
        let invoke_try = invoke_pattern(&element);
        let before_sink = read_status_sink(automation, hwnd);
        let fg = window_is_foreground(hwnd);
        let call = legacy_default_action(&element);
        std::thread::sleep(std::time::Duration::from_millis(120));
        let after_sink = read_status_sink(automation, hwnd);
        let functional = call.get("ok").and_then(|v| v.as_bool()) == Some(true)
            && before_sink != after_sink;
        rows.push(json!({
            "automation_id_digest": digest_of(automation_id),
            "shape": shape,
            "invoke_available": invoke_avail,
            "invoke_attempt": invoke_try,
            "foreground_at_call": fg,
            "legacy_call": call,
            "status_before": before_sink,
            "status_after": after_sink,
            "functional_effect": functional,
            "legacy_only_surface": invoke_avail == Some(false)
                || invoke_try.get("ok").and_then(|v| v.as_bool()) == Some(false),
        }));
    }
    let any_functional = rows.iter().any(|row| {
        row.get("functional_effect")
            .and_then(|v| v.as_bool())
            == Some(true)
    });
    json!({
        "rows": rows,
        "branch": if any_functional {
            "functional_rung_ships"
        } else {
            "non_functional_disabled_by_measurement"
        },
    })
}

pub fn measure_notepad(automation: &UIAutomation) -> Value {
    let notepad = std::process::Command::new("notepad.exe")
        .spawn()
        .map_err(|error| error.to_string());
    let mut child = match notepad {
        Ok(child) => child,
        Err(error) => return json!({ "skipped": error }),
    };
    std::thread::sleep(std::time::Duration::from_millis(800));
    let pid = child.id();
    let hwnd = find_top_level_for_pid(automation, pid);
    let Some(hwnd) = hwnd else {
        let _ = child.kill();
        return json!({ "skipped": "notepad hwnd not found" });
    };
    let root = match root_from_hwnd(automation, hwnd) {
        Ok(root) => root,
        Err(error) => {
            let _ = child.kill();
            return error;
        }
    };
    let elements = match walk_tree(automation, &root) {
        Ok(elements) => elements,
        Err(error) => {
            let _ = child.kill();
            return error;
        }
    };
    let document = elements.iter().find(|element| {
        pattern_available(element, UIProperty::IsLegacyIAccessiblePatternAvailable) == Some(true)
            && element
                .get_property_value(UIProperty::ControlType)
                .ok()
                .and_then(|variant| crate::util::number_of(&variant))
                == Some(50030)
    });
    let result = if let Some(element) = document {
        let shape = element_shape(element);
        let call = legacy_default_action(element);
        json!({
            "target": "Document",
            "shape": shape,
            "legacy_call": call,
            "branch": if call.get("ok").and_then(|v| v.as_bool()) == Some(true) {
                "functional_on_notepad_document"
            } else {
                "non_functional_on_notepad_document"
            },
        })
    } else {
        let edit = automation
            .create_property_condition(UIProperty::ControlType, Variant::from(50004i32), None)
            .ok()
            .and_then(|condition| {
                root.find_first(uiautomation::types::TreeScope::Descendants, &condition)
                    .ok()
            });
        match edit {
            Some(element) => {
                let shape = element_shape(&element);
                let call = legacy_default_action(&element);
                json!({
                    "target": "Edit_fallback",
                    "shape": shape,
                    "legacy_call": call,
                    "note": "Document control type not present; measured Edit",
                })
            }
            None => json!({ "skipped": "no Document or Edit under notepad" }),
        }
    };
    let _ = child.kill();
    let _ = child.wait();
    let _ = Handle::from(hwnd);
    result
}

fn find_top_level_for_pid(automation: &UIAutomation, pid: u32) -> Option<isize> {
    let root = automation.get_root_element().ok()?;
    let walker = automation.get_control_view_walker().ok()?;
    let mut current = walker.get_first_child(&root).ok()?;
    for _ in 0..64 {
        let element_pid = current.get_process_id().ok().unwrap_or(0) as u32;
        if element_pid == pid {
            if let Ok(handle) = current.get_native_window_handle() {
                let hwnd: isize = handle.into();
                if hwnd != 0 {
                    return Some(hwnd);
                }
            }
        }
        current = walker.get_next_sibling(&current).ok()?;
    }
    None
}

pub fn measure(automation: &UIAutomation, winforms_legacy: Option<isize>) -> Value {
    json!({
        "scratch": winforms_legacy.map(|hwnd| measure_scratch(automation, hwnd)),
        "notepad": measure_notepad(automation),
    })
}
