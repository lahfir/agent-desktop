//! Shared helpers for the actionability probe: product client bootstrap,
//! rect formatting, and tree walk.

use serde_json::{Value, json};
use uiautomation::patterns::UIScrollItemPattern;
use uiautomation::types::UIProperty;
use uiautomation::{UIAutomation, UIElement, UITreeWalker};

pub const WALK_DEPTH_LIMIT: u32 = 50;
pub const ANCESTRY_DEPTH_CAP: u32 = 50;
pub const REPEATS: usize = 7;

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

    pub fn has_area(&self) -> bool {
        self.width() > 0 && self.height() > 0
    }

    pub fn as_csv(&self) -> String {
        format!("{},{},{},{}", self.left, self.top, self.width(), self.height())
    }

    pub fn center(&self) -> (i32, i32) {
        (self.left + self.width() / 2, self.top + self.height() / 2)
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }

    pub fn intersection(&self, other: &Self) -> Option<Self> {
        let left = self.left.max(other.left);
        let top = self.top.max(other.top);
        let right = self.right.min(other.right);
        let bottom = self.bottom.min(other.bottom);
        if right > left && bottom > top {
            Some(Self {
                left,
                top,
                right,
                bottom,
            })
        } else {
            None
        }
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

pub fn number_of(variant: &uiautomation::variants::Variant) -> Option<i32> {
    match variant.get_value() {
        Ok(uiautomation::variants::Value::I4(number) | uiautomation::variants::Value::INT(number)) => {
            Some(number)
        }
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

pub fn automation_id_of(element: &UIElement) -> Option<String> {
    element.get_automation_id().ok().filter(|id| !id.is_empty())
}

pub fn is_offscreen_of(element: &UIElement) -> Option<bool> {
    element.is_offscreen().ok()
}

pub fn scroll_item_available(element: &UIElement) -> Option<bool> {
    element
        .get_property_value(UIProperty::IsScrollItemPatternAvailable)
        .ok()
        .and_then(|variant| match variant.get_value() {
            Ok(uiautomation::variants::Value::BOOL(flag)) => Some(flag),
            _ => None,
        })
}

pub fn invoke_scroll_into_view(element: &UIElement) -> Result<(), uiautomation::Error> {
    let pattern: UIScrollItemPattern = element.get_pattern()?;
    pattern.scroll_into_view()
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

pub fn element_shape(element: &UIElement) -> Value {
    let bounds = Bounds::from_element(element);
    json!({
        "control_type": control_type_of(element),
        "automation_id_digest": automation_id_of(element).as_ref().map(|id| digest_of(id)),
        "automation_id_len": automation_id_of(element).as_ref().map(|id| id.len()),
        "bounds": bounds.as_ref().map(Bounds::as_csv),
        "has_area": bounds.as_ref().map(Bounds::has_area),
        "is_offscreen": is_offscreen_of(element),
        "scroll_item_available": scroll_item_available(element),
        "native_hwnd_nonzero": element
            .get_native_window_handle()
            .ok()
            .map(|handle| Into::<isize>::into(handle) != 0),
    })
}
