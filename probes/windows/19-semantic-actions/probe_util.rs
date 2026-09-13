//! Shared helpers for the semantic-action probe: product client, walk, digests.

use serde_json::{Value, json};
use uiautomation::types::{Handle, UIProperty};
use uiautomation::variants::Variant;
use uiautomation::{UIAutomation, UIElement, UITreeWalker};
use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

pub const WALK_DEPTH_LIMIT: u32 = 60;
pub const REPEATS: usize = 7;
pub const SECRET_MARKER: &str = "zza19secretzz";

pub struct Bounds {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Bounds {
    pub fn from_element(element: &UIElement) -> Option<Self> {
        let rect = element.get_bounding_rectangle().ok()?;
        Some(Self {
            left: rect.get_left(),
            top: rect.get_top(),
            right: rect.get_right(),
            bottom: rect.get_bottom(),
        })
    }

    pub fn width(&self) -> i32 {
        self.right - self.left
    }

    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }

    pub fn as_csv(&self) -> String {
        format!("{},{},{},{}", self.left, self.top, self.width(), self.height())
    }
}

pub fn failure_shape(error: &uiautomation::Error) -> Value {
    json!({
        "code": error.code(),
        "result_hex": error.result().map(|hresult| format!("0x{:08X}", hresult.0 as u32)),
    })
}

pub fn bootstrap_product_client() -> Result<UIAutomation, Value> {
    if let Err(error) = agent_desktop_windows::ensure_owned_process_mta_and_dpi() {
        return Err(json!({
            "bootstrap_failed": true,
            "message_digest": digest_of(&error.message),
            "code": format!("{:?}", error.code),
        }));
    }
    agent_desktop_windows::tree::automation::automation_client().map_err(|error| {
        json!({
            "client_failed": true,
            "message_digest": digest_of(&error.message),
            "code": format!("{:?}", error.code),
        })
    })
}

pub fn digest_of(text: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

pub fn redacted_text(text: &str) -> Value {
    json!({
        "len": text.len(),
        "digest": digest_of(text),
        "contains_marker": text.contains(SECRET_MARKER),
    })
}

pub fn number_of(variant: &Variant) -> Option<i32> {
    match variant.get_value() {
        Ok(uiautomation::variants::Value::I4(number) | uiautomation::variants::Value::INT(number)) => {
            Some(number)
        }
        _ => None,
    }
}

pub fn bool_of(variant: &Variant) -> Option<bool> {
    match variant.get_value() {
        Ok(uiautomation::variants::Value::BOOL(flag)) => Some(flag),
        _ => None,
    }
}

pub fn pattern_available(element: &UIElement, property: UIProperty) -> Option<bool> {
    element
        .get_property_value(property)
        .ok()
        .and_then(|variant| bool_of(&variant))
}

pub fn control_type_of(element: &UIElement) -> i32 {
    element
        .get_property_value(UIProperty::ControlType)
        .ok()
        .and_then(|variant| number_of(&variant))
        .unwrap_or(0)
}

pub fn automation_id_of(element: &UIElement) -> Option<String> {
    element.get_automation_id().ok().filter(|id| !id.is_empty())
}

pub fn root_from_hwnd(automation: &UIAutomation, hwnd: isize) -> Result<UIElement, Value> {
    automation
        .element_from_handle(Handle::from(hwnd))
        .map_err(|error| json!({ "root_failed": failure_shape(&error) }))
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

fn collect_descendants(
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
        out.push(child.clone());
        collect_descendants(walker, &child, depth + 1, limit, out);
    }
}

pub fn walk_tree(automation: &UIAutomation, root: &UIElement) -> Result<Vec<UIElement>, Value> {
    let walker = automation
        .get_raw_view_walker()
        .map_err(|error| json!({ "walker_failed": failure_shape(&error) }))?;
    let mut elements = Vec::new();
    elements.push(root.clone());
    collect_descendants(&walker, root, 0, WALK_DEPTH_LIMIT, &mut elements);
    Ok(elements)
}

pub fn find_by_automation_id<'a>(
    elements: &'a [UIElement],
    automation_id: &str,
) -> Option<&'a UIElement> {
    elements
        .iter()
        .find(|element| automation_id_of(element).as_deref() == Some(automation_id))
}

pub fn refind(
    automation: &UIAutomation,
    hwnd: isize,
    automation_id: &str,
) -> Result<UIElement, Value> {
    let root = root_from_hwnd(automation, hwnd)?;
    let elements = walk_tree(automation, &root)?;
    find_by_automation_id(&elements, automation_id)
        .cloned()
        .ok_or_else(|| json!({ "refind_failed": automation_id_of_digest(automation_id) }))
}

fn automation_id_of_digest(automation_id: &str) -> Value {
    json!({
        "automation_id_len": automation_id.len(),
        "automation_id_digest": digest_of(automation_id),
    })
}

pub fn foreground_hwnd() -> isize {
    unsafe { GetForegroundWindow() as isize }
}

pub fn window_is_foreground(hwnd: isize) -> bool {
    hwnd != 0 && foreground_hwnd() == hwnd
}

pub fn map_ktd2_arm(result_hex: Option<&str>, code: Option<i32>) -> &'static str {
    match result_hex {
        Some("0x80040204") => "absent_UIA_E_NOTSUPPORTED",
        Some("0x80070005") => "denied_E_ACCESSDENIED",
        Some("0x80040201") => "stale_UIA_E_ELEMENTNOTAVAILABLE",
        Some("0x80070057") => "invalid_E_INVALIDARG",
        Some("0x80040200") => "not_enabled_UIA_E_ELEMENTNOTENABLED",
        Some("0x80010105") | Some("0x80010108") | Some("0x800706BA") | Some("0x800706BE") => {
            "transport_uncertain"
        }
        Some("0x80131505") => "timeout_uncertain",
        Some("0x80004002") => "absent_E_NOINTERFACE_get_pattern",
        Some(_) => "unclassified_uncertain",
        None => {
            if code.is_some() {
                "sentinel_or_non_hresult_absent"
            } else {
                "clean_ok"
            }
        }
    }
}

pub fn outcome_of(result: Result<(), uiautomation::Error>) -> Value {
    match result {
        Ok(()) => json!({
            "ok": true,
            "ktd2_arm": "clean_ok",
        }),
        Err(error) => {
            let shape = failure_shape(&error);
            let hex = shape
                .get("result_hex")
                .and_then(|value| value.as_str())
                .map(str::to_string);
            let code = shape.get("code").and_then(|value| value.as_i64()).map(|v| v as i32);
            json!({
                "ok": false,
                "failure": shape,
                "ktd2_arm": map_ktd2_arm(hex.as_deref(), code),
            })
        }
    }
}

pub fn min_of_ms(mut operation: impl FnMut() -> Result<(), ()>) -> Value {
    let mut samples = Vec::with_capacity(REPEATS);
    let _ = operation();
    for _ in 0..REPEATS {
        let started = std::time::Instant::now();
        let _ = operation();
        samples.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    json!({
        "min_ms": samples[0],
        "median_ms": samples[samples.len() / 2],
        "max_ms": samples[samples.len() - 1],
        "n": samples.len(),
        "warmup_discarded": true,
    })
}

pub fn element_shape(element: &UIElement) -> Value {
    let bounds = Bounds::from_element(element);
    json!({
        "control_type": control_type_of(element),
        "automation_id_digest": automation_id_of(element).as_ref().map(|id| digest_of(id)),
        "automation_id_len": automation_id_of(element).as_ref().map(|id| id.len()),
        "bounds": bounds.as_ref().map(Bounds::as_csv),
        "is_enabled": element.is_enabled().ok(),
        "is_password": element
            .get_property_value(UIProperty::IsPassword)
            .ok()
            .and_then(|variant| bool_of(&variant)),
        "invoke_available": pattern_available(element, UIProperty::IsInvokePatternAvailable),
        "toggle_available": pattern_available(element, UIProperty::IsTogglePatternAvailable),
        "value_available": pattern_available(element, UIProperty::IsValuePatternAvailable),
        "expand_available": pattern_available(element, UIProperty::IsExpandCollapsePatternAvailable),
        "selection_item_available": pattern_available(element, UIProperty::IsSelectionItemPatternAvailable),
        "range_available": pattern_available(element, UIProperty::IsRangeValuePatternAvailable),
        "scroll_available": pattern_available(element, UIProperty::IsScrollPatternAvailable),
        "legacy_available": pattern_available(element, UIProperty::IsLegacyIAccessiblePatternAvailable),
    })
}
