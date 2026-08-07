//! Window-attribution corroboration and occluder evidence for hit testing.
//!
//! `InterceptedBy` requires two-opinion agreement: the UIA hit element's root
//! and `WindowFromPoint`→`GA_ROOT` must agree on the window question before an
//! unrelated hit is named. Transparent / disabled overlays that Win32
//! documented-skips toward the target's root against a differing hit root are
//! designed `Unknown` (A18-4). Same-root overlays are named on UIA's verdict
//! alone when both roots equal the target's (A18-4: same-root was not
//! live-proven on the probe desktop due to foreign-window contamination; the
//! designed rule and fixture live test still ship).
//!
//! A18-4's elevated High-over-Medium occluder leg was not measured on this
//! box (token privilege denied). Cross-integrity occluder reads therefore
//! rest on A9-2 / A16-12 observation-across-boundary evidence until an
//! elevated A18 capture lands — not a product-path blocker.

use agent_desktop_core::{LocatorField, Point, hit_test::HitTestResult};
use uiautomation::core::UITreeWalker;

use super::classify::ancestry_limit;
use crate::tree::element::UIAElement;
use crate::tree::element_properties::ElementProperties;
use crate::tree::name_evidence::{name_fields, read_label};
use crate::tree::properties::read_one;
use crate::tree::property_ids::TreeProperty;
use crate::tree::roles::resolve_role;

/// Properties the occluder evidence batch must read together. `IsPassword`
/// gates value-bearing text in [`ElementProperties::from_reads`]; omitting it
/// would leave the secure-field withhold open.
pub(crate) const OCCLUDER_EVIDENCE_PROPERTIES: &[TreeProperty] = &[
    TreeProperty::IsPassword,
    TreeProperty::Name,
    TreeProperty::HelpText,
    TreeProperty::FullDescription,
    TreeProperty::ControlType,
    TreeProperty::BoundingRectangle,
];

/// Pure window-attribution decision. `true` means the two opinions agree that
/// an interception is reportable; evidence is assembled separately.
pub(crate) fn interception_agreed(
    target_root: Option<isize>,
    hit_root: Option<isize>,
    win32_root: Option<isize>,
    target_pid: Option<u32>,
    hit_pid: Option<u32>,
    win32_owner_pid: Option<u32>,
) -> bool {
    let Some(target) = nonzero(target_root) else {
        return false;
    };
    let Some(win32) = nonzero(win32_root) else {
        return false;
    };
    match nonzero(hit_root) {
        Some(hit) if hit == target && win32 == target => true,
        Some(hit) if hit != target && win32 != target && win32 == hit => true,
        Some(_) => false,
        None => matches!(
            (target_pid, hit_pid, win32_owner_pid),
            (Some(target_pid), Some(hit_pid), Some(win32_pid))
                if win32 != target && hit_pid != target_pid && hit_pid == win32_pid
        ),
    }
}

fn nonzero(handle: Option<isize>) -> Option<isize> {
    handle.filter(|&value| value != 0)
}

/// Walks to the first non-zero `NativeWindowHandle`, then `GA_ROOT`.
pub(crate) fn element_root_hwnd(start: &UIAElement, walker: &UITreeWalker) -> Option<isize> {
    let hwnd = first_native_hwnd(start, walker)?;
    root_of_hwnd(hwnd)
}

pub(crate) fn first_native_hwnd(start: &UIAElement, walker: &UITreeWalker) -> Option<isize> {
    use windows::Win32::Foundation::HWND;

    let mut current = start.clone();
    for _ in 0..ancestry_limit() {
        if let Ok(handle) = current.0.get_native_window_handle() {
            let hwnd: HWND = handle.into();
            let value = hwnd.0 as isize;
            if value != 0 {
                return Some(value);
            }
        }
        match parent_step(walker, &current) {
            Ok(Some(parent)) => current = parent,
            Ok(None) | Err(()) => return None,
        }
    }
    None
}

fn parent_step(walker: &UITreeWalker, node: &UIAElement) -> Result<Option<UIAElement>, ()> {
    use crate::tree::automation::failure_of;

    match walker.get_parent(&node.0) {
        Ok(parent) => Ok(Some(UIAElement::from(parent))),
        Err(error) => {
            if failure_of(&error).is_exhaustion() {
                Ok(None)
            } else {
                Err(())
            }
        }
    }
}

pub(crate) fn root_of_hwnd(hwnd: isize) -> Option<isize> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GA_ROOT, GetAncestor};

    if hwnd == 0 {
        return None;
    }
    let root = unsafe { GetAncestor(hwnd as *mut std::ffi::c_void, GA_ROOT) };
    if root.is_null() {
        None
    } else {
        Some(root as isize)
    }
}

pub(crate) fn window_from_point_root(point: &Point) -> Option<isize> {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::UI::WindowsAndMessaging::WindowFromPoint;

    let physical = POINT {
        x: saturate_coord(point.x),
        y: saturate_coord(point.y),
    };
    let hwnd = unsafe { WindowFromPoint(physical) };
    if hwnd.is_null() {
        return None;
    }
    root_of_hwnd(hwnd as isize)
}

pub(crate) fn pid_of_hwnd(hwnd: isize) -> Option<u32> {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

    if hwnd == 0 {
        return None;
    }
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd as *mut std::ffi::c_void, &mut pid) };
    (pid != 0).then_some(pid)
}

fn process_id_of(element: &UIAElement) -> Option<u32> {
    element.0.get_process_id().ok().filter(|&pid| pid != 0)
}

fn saturate_coord(value: f64) -> i32 {
    if !value.is_finite() {
        return 0;
    }
    if value >= f64::from(i32::MAX) {
        i32::MAX
    } else if value <= f64::from(i32::MIN) {
        i32::MIN
    } else {
        value as i32
    }
}

/// Assembles occluder evidence from the hit element. Returns `None` when a
/// required evidence read failed (bounds unread), so the caller demotes to
/// `Unknown`.
pub(crate) fn occluder_evidence(hit: &UIAElement) -> Option<HitTestResult> {
    let properties = read_occluder_properties(hit);
    let role = occluder_role(&properties);
    let label = read_label(hit, false);
    let (name_field, _) = name_fields(&properties, &label);
    let name = name_field.known().cloned();
    let bounds = match properties.get(TreeProperty::BoundingRectangle).bounds() {
        LocatorField::Known(bounds) => Some(bounds),
        LocatorField::Absent => None,
        LocatorField::Unknown => return None,
    };
    Some(HitTestResult::InterceptedBy {
        role: Some(role),
        name,
        bounds,
    })
}

/// Builds `InterceptedBy` from a prepared property set (tests pin withholding
/// and the `"unknown"` role fallback without a live probe).
#[cfg(test)]
pub(crate) fn occluder_from_properties(
    properties: &ElementProperties,
    label: crate::tree::name_evidence::LabelOutcome,
) -> Option<HitTestResult> {
    let role = occluder_role(properties);
    let (name_field, _) = name_fields(properties, &label);
    let name = name_field.known().cloned();
    let bounds = match properties.get(TreeProperty::BoundingRectangle).bounds() {
        LocatorField::Known(bounds) => Some(bounds),
        LocatorField::Absent => None,
        LocatorField::Unknown => return None,
    };
    Some(HitTestResult::InterceptedBy {
        role: Some(role),
        name,
        bounds,
    })
}

fn read_occluder_properties(hit: &UIAElement) -> ElementProperties {
    let reads = OCCLUDER_EVIDENCE_PROPERTIES
        .iter()
        .copied()
        .map(|property| (property, read_one(hit, property)))
        .collect();
    ElementProperties::from_reads(reads)
}

fn occluder_role(properties: &ElementProperties) -> String {
    match resolve_role(properties) {
        LocatorField::Known(role) => role,
        LocatorField::Absent | LocatorField::Unknown => "unknown".to_string(),
    }
}

pub(crate) fn corroborate_interception(
    target: &UIAElement,
    hit: &UIAElement,
    point: Point,
    walker: &UITreeWalker,
) -> HitTestResult {
    let target_root = element_root_hwnd(target, walker);
    let hit_root = element_root_hwnd(hit, walker);
    let win32_root = window_from_point_root(&point);
    let win32_owner_pid = win32_root.and_then(pid_of_hwnd);
    let agreed = interception_agreed(
        target_root,
        hit_root,
        win32_root,
        process_id_of(target),
        process_id_of(hit),
        win32_owner_pid,
    );
    if !agreed {
        return HitTestResult::Unknown;
    }
    occluder_evidence(hit).unwrap_or(HitTestResult::Unknown)
}

#[cfg(test)]
#[path = "hit_test_corroborate_tests.rs"]
mod tests;
