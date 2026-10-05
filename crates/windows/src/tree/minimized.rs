use agent_desktop_core::Deadline;

use super::hit_test::corroborate::element_root_hwnd;
use crate::tree::automation::automation_client;
use crate::tree::element::UIAElement;
use crate::tree::element_properties::ElementProperties;
use crate::tree::property_ids::TreeProperty;
use crate::tree::property_outcome::{PropertyOutcome, PropertyValue};

/// Whether the element's owning top-level window is minimized right now.
///
/// `None` answers nothing: the budget expired, the client or walker would
/// not build, or the climb never reached a window. Callers treat `None` as
/// unknown and change nothing, because a failed window question must never
/// invent an offscreen state.
pub(crate) fn owning_window_minimized(element: &UIAElement, deadline: Deadline) -> Option<bool> {
    crate::system::permissions::ensure_budget(deadline).ok()?;
    let client = automation_client().ok()?;
    let walker = client.get_raw_view_walker().ok()?;
    let root = element_root_hwnd(element, &walker, deadline).ok()??;
    Some(unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::IsIconic(root as *mut std::ffi::c_void) != 0
    })
}

/// Reports a minimized owner's whole subtree offscreen in the read set.
///
/// A minimized top-level window keeps reporting usable rectangles and a
/// clear `IsOffscreen` on its descendants (A14-8 measured the descendants
/// false throughout on the COM stack), so the visibility gate and `is
/// visible` both read the element as shown. Overwriting the one flag carries
/// the window truth into every projection the shared read serves, without
/// touching what the walk or the resolver collect on their own paths.
pub(crate) fn stamp_minimized_offscreen(properties: &mut ElementProperties) {
    properties.set(
        TreeProperty::IsOffscreen,
        PropertyOutcome::Known(PropertyValue::Flag(true)),
    );
}

#[cfg(test)]
#[path = "minimized_tests.rs"]
mod tests;
