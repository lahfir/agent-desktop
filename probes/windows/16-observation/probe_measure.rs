//! The UIA-side observation measurements sub-phase 2.4's plan refuses to infer.
//!
//! Reports shape, never content that could be application text: variant type,
//! length, boolean or integer content, HRESULT - and the distinct string values
//! only for the properties whose content is provider vocabulary rather than
//! target text (`LocalizedControlType`, and `AriaRole` only where it is
//! provider-produced ARIA vocabulary rather than author-derived text).

use serde_json::{Value, json};
use uiautomation::types::{Handle, UIProperty};
use uiautomation::variants::Variant;
use uiautomation::{Error as UiaError, UIAutomation, UIElement, UITreeWalker};

/// An author-defined, non-ARIA-spec role token written into the probe's own
/// fixture, so a capture can answer whether the stack passes arbitrary role
/// tokens through `AriaRole` verbatim without disclosing application text.
/// The token is the probe's, so reporting whether it appears is evidence about
/// the stack, not about the target.
const AUTHOR_ROLE_MARKER: &str = "zzcustomproberole";

const WALK_DEPTH_LIMIT: u32 = 24;

pub fn failure_shape(error: &UiaError) -> Value {
    json!({
        "code": error.code(),
        "result_is_none": error.result().is_none(),
        "result_hex": error.result().map(|hresult| format!("0x{:08X}", hresult.0 as u32)),
    })
}

pub fn variant_shape(variant: &Variant) -> Value {
    let rendered = variant.get_string().unwrap_or_default();
    let mut shape = serde_json::Map::new();
    shape.insert(
        "variant_type".into(),
        json!(format!("{:?}", variant.get_type())),
    );
    if variant.is_null() {
        shape.insert("is_null".into(), json!(true));
    }
    if !rendered.is_empty() {
        shape.insert("length".into(), json!(rendered.chars().count()));
        shape.insert("non_empty".into(), json!(!rendered.trim().is_empty()));
    }
    Value::Object(shape)
}

pub fn read_shape(element: &UIElement, property: UIProperty) -> Value {
    match element.get_property_value(property) {
        Ok(variant) => variant_shape(&variant),
        Err(error) => json!({ "failed": failure_shape(&error) }),
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

/// Question 11: connection-to-settled latency on the Chromium target.
///
/// A1-5 measured first contact at 13 nodes against a settled 172 - a 13.2x
/// understatement - but never timed the settle itself. The mechanism is
/// connection-triggered: Chromium 138+ builds its UIA tree asynchronously once
/// a client connects, and the connecting client's own root serves the shell
/// while the build completes, so **the poll uses a fresh client per walk** -
/// the same shape as A1-5's per-cell fresh instance. A stable count is not
/// taken as settled on the first pair of agreeing walks either: a covered
/// Chromium window holds the shell at first contact (A1-6), so 12,12,12 and
/// 165,165,165 are both stable but only one is settled. The scan runs the whole
/// window and reports settled only when stability survives to the end after
/// growth was observed.
pub fn settle_scan(
    handle: isize,
    max_walks: u32,
    interval: std::time::Duration,
    minimum_stable_walks: u32,
) -> Value {
    let mut counts: Vec<usize> = Vec::new();
    let mut previous: Option<usize> = None;
    let mut stable_tail = 0u32;
    let mut grew = false;
    for _walk_index in 0..max_walks {
        let count = match UIAutomation::new_direct() {
            Ok(automation) => match automation.get_raw_view_walker().and_then(|walker| {
                automation
                    .element_from_handle(Handle::from(handle))
                    .map(|root| collect_descendants(&walker, &root, WALK_DEPTH_LIMIT).len())
            }) {
                Ok(count) => count,
                Err(_) => 0,
            },
            Err(_) => 0,
        };
        if previous.is_some_and(|prev| prev != count) {
            grew = true;
        }
        if previous.is_some_and(|prev| prev == count) {
            stable_tail += 1;
        } else {
            stable_tail = 0;
        }
        counts.push(count);
        previous = Some(count);
        std::thread::sleep(interval);
    }
    let min = counts.iter().copied().min().unwrap_or(0);
    let max = counts.iter().copied().max().unwrap_or(0);
    let final_count = counts.last().copied().unwrap_or(0);
    json!({
        "walks": counts.len(),
        "series": counts,
        "first_contact": counts.first().copied().unwrap_or(0),
        "final": final_count,
        "min": min,
        "max": max,
        "stable_for": stable_tail,
        "grew_after_first_contact": grew,
        "settled": grew && stable_tail >= minimum_stable_walks,
        "understatement_ratio": if max > 0 {
            (max as f64 / counts.first().copied().unwrap_or(1).max(1) as f64 * 100.0).round() / 100.0
        } else { 0.0 },
    })
}

fn boolean_of(variant: &Variant) -> Option<bool> {
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

fn by_control_type(elements: &[UIElement]) -> Vec<(i32, Vec<&UIElement>)> {
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

/// Question 5: what `LocalizedControlType` carries per control type.
///
/// The content is **provider vocabulary** - UIA's own control-type description
/// for the client's UI language - not application text, so its distinct strings
/// are reportable. The row records, per control type: the count, the distinct
/// strings observed, and whether any read failed (an `Unknown` outcome that
/// would mean the producer contributes nothing).
pub fn measure_localized_control_type(elements: &[UIElement]) -> Value {
    let rows: Vec<Value> = by_control_type(elements)
        .into_iter()
        .map(|(control_type, bucket)| {
            let mut distinct: Vec<String> = Vec::new();
            let mut failed = 0usize;
            for element in &bucket {
                match element.get_property_value(UIProperty::LocalizedControlType) {
                    Ok(variant) => {
                        let text = variant.get_string().unwrap_or_default().trim().to_string();
                        if !text.is_empty() && !distinct.contains(&text) {
                            distinct.push(text);
                        }
                    }
                    Err(_) => failed += 1,
                }
            }
            distinct.sort_unstable();
            json!({
                "control_type": control_type,
                "count": bucket.len(),
                "distinct_localized": distinct,
                "failed_reads": failed,
            })
        })
        .collect();
    json!({ "rows": rows })
}

/// Questions 6 (AriaRole) and the `dom_classes` branch (AriaProperties).
///
/// `AriaRole` and `AriaProperties` are **author-derived** on web content - ARIA
/// roles and properties are authored by the page, so their strings can carry
/// target text and are withheld to shape-and-count on that stack. On native
/// fixtures they should be empty; the shape row records that. `dom_classes`'s
/// KTD5 branch is answered by whether `AriaProperties` carries class tokens as
/// a computed UI Automation value here.
pub fn measure_aria(elements: &[UIElement]) -> Value {
    let mut role_shapes: Vec<Value> = Vec::new();
    let mut props_shapes: Vec<Value> = Vec::new();
    let mut role_non_empty = 0usize;
    let mut props_non_empty = 0usize;
    let mut author_marker_in_role = 0usize;
    let mut class_token_in_props = 0usize;
    for element in elements {
        let role = match element.get_property_value(UIProperty::AriaRole) {
            Ok(variant) => {
                let text = variant.get_string().unwrap_or_default();
                if text.contains(AUTHOR_ROLE_MARKER) {
                    author_marker_in_role += 1;
                }
                variant_shape(&variant)
            }
            Err(error) => json!({ "failed": failure_shape(&error) }),
        };
        let props = match element.get_property_value(UIProperty::AriaProperties) {
            Ok(variant) => {
                let text = variant.get_string().unwrap_or_default();
                if text.contains("class") {
                    class_token_in_props += 1;
                }
                variant_shape(&variant)
            }
            Err(error) => json!({ "failed": failure_shape(&error) }),
        };
        if role.get("length").is_some() {
            role_non_empty += 1;
        }
        if props.get("length").is_some() {
            props_non_empty += 1;
        }
        role_shapes.push(role);
        props_shapes.push(props);
    }
    role_shapes.dedup();
    props_shapes.dedup();
    json!({
        "elements": elements.len(),
        "aria_role": {
            "non_empty_count": role_non_empty,
            "distinct_shapes": role_shapes,
            "author_defined_token_passed_verbatim": author_marker_in_role,
        },
        "aria_properties": {
            "non_empty_count": props_non_empty,
            "distinct_shapes": props_shapes,
            "carries_class_token": class_token_in_props,
        },
    })
}

/// Question 8: the transparent-wrapper density and depth census on a Chromium
/// tree.
///
/// Counts the `Group` (50026) and `Custom` (50025) control-type nodes UIA
/// produces for web layout containers. The wrapper skip (KTD6) is gated on
/// empty name/value and no action, so the census reports how many such nodes
/// carry an empty name **and** empty value - the population a gated skip would
/// pass through - versus how many carry a name or value (which would consume
/// depth). It also re-derives the logical depth to the deepest non-wrapper
/// node, so the value of the skip is stated per-tree rather than assumed.
pub fn measure_wrapper_census(elements: &[UIElement]) -> Value {
    let mut group_count = 0usize;
    let mut empty_name_and_value = 0usize;
    let mut named = 0usize;
    for element in elements {
        let control_type = control_type_of(element);
        if control_type == 50026 || control_type == 50025 {
            group_count += 1;
            let name = element
                .get_property_value(UIProperty::Name)
                .ok()
                .and_then(|variant| variant.get_string().ok())
                .unwrap_or_default();
            let value = element
                .get_property_value(UIProperty::ValueValue)
                .ok()
                .and_then(|variant| variant.get_string().ok())
                .unwrap_or_default();
            let wrapper = name.trim().is_empty() && value.trim().is_empty();
            if wrapper {
                empty_name_and_value += 1;
            } else if !name.trim().is_empty() {
                named += 1;
            }
        }
    }
    json!({
        "group_or_custom_nodes": group_count,
        "empty_name_and_value": empty_name_and_value,
        "named_groups": named,
        "total_elements": elements.len(),
        "depth_note": "depth arithmetic is a walk-tree measurement; see settle scan for raw depth to web content",
    })
}

/// The raw depth reached in a raw-view walk, reported as a depth-aware
/// counter: max_raw_depth advances on every node, max_logical_depth only on
/// nodes that are not transparent wrappers - the two-counter pattern the
/// walker already carries (KTD6).
pub fn measure_depth_to_content(
    walker: &UITreeWalker,
    root: &UIElement,
    depth: u32,
    limit: u32,
    raw: &mut usize,
    logical: &mut usize,
) {
    if depth >= limit {
        return;
    }
    for child in enumerate_children(walker, root) {
        let control_type = control_type_of(&child);
        let is_wrapper = control_type == 50026 || control_type == 50025;
        let wrapper = if is_wrapper {
            let name = child
                .get_property_value(UIProperty::Name)
                .ok()
                .and_then(|variant| variant.get_string().ok())
                .unwrap_or_default();
            let value = child
                .get_property_value(UIProperty::ValueValue)
                .ok()
                .and_then(|variant| variant.get_string().ok())
                .unwrap_or_default();
            name.trim().is_empty() && value.trim().is_empty()
        } else {
            false
        };
        *raw += 1;
        if !wrapper {
            *logical += 1;
        }
        measure_depth_to_content(
            walker,
            &child,
            depth + 1,
            limit,
            raw,
            logical,
        );
    }
}

/// Question 10's evidence: the two never-exercised ref-able arms.
///
/// `cell` (DataItem+TableItem/GridItem) and `switch` (Button+Toggle) are the
/// two role arms 2.3's dogfood could not observe. The fixture extension exists
/// so they can be, and this census records whether the extension actually
/// advertises the patterns: a `Button`-typed element with
/// `IsTogglePatternAvailable` true, and a `DataItem`-typed element with
/// `IsTableItemPatternAvailable` or `IsGridItemPatternAvailable` true.
pub fn measure_pattern_arms(elements: &[UIElement]) -> Value {
    let mut toggle_buttons = 0usize;
    let mut grid_item_cells = 0usize;
    let mut table_item_cells = 0usize;
    let mut grid_or_table_items = 0usize;
    let mut carrying_control_types: Vec<i32> = Vec::new();
    for element in elements {
        let control_type = control_type_of(element);
        let toggle_available = element
            .get_property_value(UIProperty::IsTogglePatternAvailable)
            .ok()
            .and_then(|variant| boolean_of(&variant))
            .unwrap_or(false);
        let grid_available = element
            .get_property_value(UIProperty::IsGridItemPatternAvailable)
            .ok()
            .and_then(|variant| boolean_of(&variant))
            .unwrap_or(false);
        let table_available = element
            .get_property_value(UIProperty::IsTableItemPatternAvailable)
            .ok()
            .and_then(|variant| boolean_of(&variant))
            .unwrap_or(false);
        if control_type == 50000 && toggle_available {
            toggle_buttons += 1;
        }
        if grid_available {
            grid_item_cells += 1;
        }
        if table_available {
            table_item_cells += 1;
        }
        if grid_available || table_available {
            if !carrying_control_types.contains(&control_type) {
                carrying_control_types.push(control_type);
            }
            if control_type != 50029 {
                grid_or_table_items += 1;
            }
        }
    }
    carrying_control_types.sort_unstable();
    json!({
        "total_elements": elements.len(),
        "button_with_toggle": toggle_buttons,
        "any_element_with_griditem": grid_item_cells,
        "any_element_with_tableitem": table_item_cells,
        "non_dataitem_carrying_grid_or_tableitem": grid_or_table_items,
        "control_types_carrying_grid_or_tableitem": carrying_control_types,
    })
}

/// Reads the process-generation token of the window's owning process: the same
/// creation-time-derived identity KTD3 defines. Returns shape only - whether a
/// token could be read at all is the split-integrity evidence.
pub fn process_token_of_window(_handle: isize) -> Value {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{
            GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

        let mut pid: u32 = 0;
        unsafe {
            GetWindowThreadProcessId(_handle as *mut core::ffi::c_void, &mut pid);
        }
        if pid == 0 {
            return json!({ "pid_zero": true });
        }
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if process.is_null() {
            return json!({ "open_failed": true });
        }
        let mut created = windows_sys::Win32::Foundation::FILETIME::default();
        let mut exit = windows_sys::Win32::Foundation::FILETIME::default();
        let mut kernel = windows_sys::Win32::Foundation::FILETIME::default();
        let mut user = windows_sys::Win32::Foundation::FILETIME::default();
        unsafe {
            GetProcessTimes(
                process,
                &mut created,
                &mut exit,
                &mut kernel,
                &mut user,
            );
            CloseHandle(process);
        }
        let created_ticks = ((created.dwHighDateTime as u64) << 32) | created.dwLowDateTime as u64;
        if created_ticks == 0 {
            return json!({ "creation_time_unavailable": true });
        }
        let seconds = created_ticks / 10_000_000;
        let micros = created_ticks % 10_000_000;
        json!({
            "creation_time_seconds": seconds,
            "creation_time_subtick": micros,
        })
    }
    #[cfg(not(target_os = "windows"))]
    {
        json!({ "skipped": true })
    }
}
