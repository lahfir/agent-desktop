use agent_desktop_core::node::{AccessibilityNode, Rect};
use rustc_hash::FxHashSet;

pub const ABSOLUTE_MAX_DEPTH: u8 = 50;

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use accessibility_sys::{
        kAXChildrenAttribute, kAXContentsAttribute, kAXDescriptionAttribute,
        kAXEnabledAttribute, kAXErrorSuccess, kAXFocusedAttribute, kAXRoleAttribute,
        kAXTitleAttribute, kAXValueAttribute, kAXWindowsAttribute,
        AXUIElementCopyAttributeValue, AXUIElementCopyMultipleAttributeValues,
        AXUIElementCreateApplication, AXUIElementRef,
    };
    use core_foundation::{
        array::CFArray,
        base::{CFRelease, CFRetain, CFType, CFTypeRef, TCFType},
        boolean::CFBoolean,
        string::CFString,
    };

    pub struct AXElement(pub(crate) AXUIElementRef);

    impl Drop for AXElement {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { CFRelease(self.0 as CFTypeRef) }
            }
        }
    }

    impl Clone for AXElement {
        fn clone(&self) -> Self {
            if !self.0.is_null() {
                unsafe { CFRetain(self.0 as CFTypeRef) };
            }
            AXElement(self.0)
        }
    }

    pub fn element_for_pid(pid: i32) -> AXElement {
        AXElement(unsafe { AXUIElementCreateApplication(pid) })
    }

    /// Find the AXWindow element whose title matches `win_title`.
    /// Falls back to the first window, then to the app element if no windows.
    pub fn window_element_for(pid: i32, win_title: &str) -> AXElement {
        let app = element_for_pid(pid);

        // Try kAXWindowsAttribute
        if let Some(windows) = copy_ax_array(&app, kAXWindowsAttribute) {
            // Exact title match
            for win in &windows {
                let title = copy_string_attr(win, kAXTitleAttribute);
                if title.as_deref() == Some(win_title) {
                    let matched = win.clone();
                    return matched;
                }
            }
            // Partial match
            for win in &windows {
                let title = copy_string_attr(win, kAXTitleAttribute);
                if title.as_deref().is_some_and(|t| t.contains(win_title) || win_title.contains(t)) {
                    let matched = win.clone();
                    return matched;
                }
            }
            // First available window
            if let Some(first) = windows.into_iter().next() {
                return first;
            }
        }

        // Fallback: app root
        app
    }

    /// Batch-fetch the six most-used attributes in a single AX API call.
    /// Returns (role, title, description, value, enabled, focused).
    fn fetch_node_attrs(el: &AXElement) -> (Option<String>, Option<String>, Option<String>, Option<String>, bool, bool) {
        let attr_names = [
            kAXRoleAttribute,
            kAXTitleAttribute,
            kAXDescriptionAttribute,
            kAXValueAttribute,
            kAXEnabledAttribute,
            kAXFocusedAttribute,
        ];
        let cf_names: Vec<CFString> = attr_names.iter().map(|a| CFString::new(a)).collect();
        let cf_refs: Vec<_> = cf_names.iter().map(|s| s.as_concrete_TypeRef()).collect();
        let names_arr = CFArray::from_copyable(&cf_refs);

        let mut result_ref: CFTypeRef = std::ptr::null_mut();
        let err = unsafe {
            AXUIElementCopyMultipleAttributeValues(
                el.0,
                names_arr.as_concrete_TypeRef(),
                0,
                &mut result_ref as *mut _ as *mut _,
            )
        };

        if err != kAXErrorSuccess || result_ref.is_null() {
            // Fallback to individual calls
            let role  = copy_string_attr(el, kAXRoleAttribute);
            let title = copy_string_attr(el, kAXTitleAttribute);
            let desc  = copy_string_attr(el, kAXDescriptionAttribute);
            let val   = copy_string_attr(el, kAXValueAttribute);
            let enabled = copy_bool_attr(el, kAXEnabledAttribute).unwrap_or(true);
            let focused = copy_bool_attr(el, kAXFocusedAttribute).unwrap_or(false);
            return (role, title, desc, val, enabled, focused);
        }

        let arr = unsafe { CFArray::<CFType>::wrap_under_create_rule(result_ref as _) };
        let items: Vec<Option<String>> = arr.into_iter().map(|item| {
            item.downcast::<CFString>().map(|s| s.to_string())
        }).collect();

        let get = |i: usize| items.get(i).and_then(|v| v.clone());
        let role    = get(0);
        let title   = get(1);
        let desc    = get(2);
        let val     = get(3);
        // enabled/focused are CFBoolean not CFString, so they'll be None from downcast
        // re-read them individually (cheap since it's only 2 attrs)
        let enabled = copy_bool_attr(el, kAXEnabledAttribute).unwrap_or(true);
        let focused = copy_bool_attr(el, kAXFocusedAttribute).unwrap_or(false);

        (role, title, desc, val, enabled, focused)
    }

    pub fn build_subtree(
        el: &AXElement,
        depth: u8,
        max_depth: u8,
        include_bounds: bool,
        visited: &mut FxHashSet<usize>,
    ) -> Option<AccessibilityNode> {
        if depth > max_depth || depth >= ABSOLUTE_MAX_DEPTH {
            return None;
        }
        if !visited.insert(el.0 as usize) {
            return None;
        }

        let (ax_role, title, ax_desc, value, enabled, focused) = fetch_node_attrs(el);

        let role = ax_role
            .as_deref()
            .map(crate::roles::ax_role_to_str)
            .unwrap_or("unknown")
            .to_string();

        let name = title.clone().or_else(|| ax_desc.clone());
        let description = if title.is_some() { ax_desc } else { None };

        let mut states = Vec::new();
        if focused {
            states.push("focused".into());
        }
        if !enabled {
            states.push("disabled".into());
        }

        let bounds = if include_bounds { read_bounds(el) } else { None };

        let children = copy_children(el)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|child| {
                build_subtree(&child, depth + 1, max_depth, include_bounds, visited)
            })
            .collect();

        Some(AccessibilityNode {
            ref_id: None,
            role,
            name,
            value,
            description,
            states,
            bounds,
            children,
        })
    }

    pub fn copy_string_attr(el: &AXElement, attr: &str) -> Option<String> {
        let cf_attr = CFString::new(attr);
        let mut value: CFTypeRef = std::ptr::null_mut();
        let err = unsafe {
            AXUIElementCopyAttributeValue(
                el.0,
                cf_attr.as_concrete_TypeRef(),
                &mut value,
            )
        };
        if err != kAXErrorSuccess || value.is_null() {
            return None;
        }
        let cf_type = unsafe { CFType::wrap_under_create_rule(value) };
        cf_type.downcast::<CFString>().map(|s| s.to_string())
    }

    fn copy_bool_attr(el: &AXElement, attr: &str) -> Option<bool> {
        let cf_attr = CFString::new(attr);
        let mut value: CFTypeRef = std::ptr::null_mut();
        let err = unsafe {
            AXUIElementCopyAttributeValue(
                el.0,
                cf_attr.as_concrete_TypeRef(),
                &mut value,
            )
        };
        if err != kAXErrorSuccess || value.is_null() {
            return None;
        }
        let cf_type = unsafe { CFType::wrap_under_create_rule(value) };
        cf_type.downcast::<CFBoolean>().map(|b| b.into())
    }

    /// Read an array-typed AX attribute, retaining each AXUIElement so they
    /// stay alive past CFArray deallocation.
    pub fn copy_ax_array(el: &AXElement, attr: &str) -> Option<Vec<AXElement>> {
        let cf_attr = CFString::new(attr);
        let mut value: CFTypeRef = std::ptr::null_mut();
        let err = unsafe {
            AXUIElementCopyAttributeValue(el.0, cf_attr.as_concrete_TypeRef(), &mut value)
        };
        if err != kAXErrorSuccess || value.is_null() {
            return None;
        }
        let arr = unsafe { CFArray::<CFType>::wrap_under_create_rule(value as _) };
        let children: Vec<AXElement> = arr
            .into_iter()
            .filter_map(|item| {
                let ptr = item.as_concrete_TypeRef() as AXUIElementRef;
                if ptr.is_null() {
                    None
                } else {
                    unsafe { CFRetain(ptr as CFTypeRef) };
                    Some(AXElement(ptr))
                }
            })
            .collect();
        Some(children)
    }

    fn copy_children(el: &AXElement) -> Option<Vec<AXElement>> {
        for attr in &[
            kAXChildrenAttribute,
            kAXContentsAttribute,
            "AXChildrenInNavigationOrder",
        ] {
            if let Some(v) = copy_ax_array(el, attr) {
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }
        None
    }

    pub fn read_bounds(el: &AXElement) -> Option<Rect> {
        use accessibility_sys::{
            kAXPositionAttribute, kAXSizeAttribute, AXValueGetValue,
            kAXValueTypeCGPoint, kAXValueTypeCGSize,
        };
        use core_graphics::geometry::{CGPoint, CGSize};
        use std::ffi::c_void;

        let pos_cf = CFString::new(kAXPositionAttribute);
        let mut pos_ref: CFTypeRef = std::ptr::null_mut();
        let pos_ok = unsafe {
            AXUIElementCopyAttributeValue(el.0, pos_cf.as_concrete_TypeRef(), &mut pos_ref)
        };
        if pos_ok != kAXErrorSuccess || pos_ref.is_null() {
            return None;
        }
        let mut point = CGPoint::new(0.0, 0.0);
        let got_pos = unsafe {
            AXValueGetValue(pos_ref as _, kAXValueTypeCGPoint, &mut point as *mut _ as *mut c_void)
        };
        unsafe { CFRelease(pos_ref) };
        if !got_pos {
            return None;
        }

        let size_cf = CFString::new(kAXSizeAttribute);
        let mut size_ref: CFTypeRef = std::ptr::null_mut();
        let size_ok = unsafe {
            AXUIElementCopyAttributeValue(el.0, size_cf.as_concrete_TypeRef(), &mut size_ref)
        };
        if size_ok != kAXErrorSuccess || size_ref.is_null() {
            return None;
        }
        let mut size = CGSize::new(0.0, 0.0);
        let got_size = unsafe {
            AXValueGetValue(size_ref as _, kAXValueTypeCGSize, &mut size as *mut _ as *mut c_void)
        };
        unsafe { CFRelease(size_ref) };
        if !got_size {
            return None;
        }

        Some(Rect { x: point.x, y: point.y, width: size.width, height: size.height })
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;

    pub struct AXElement(pub(crate) *const std::ffi::c_void);

    impl Drop for AXElement {
        fn drop(&mut self) {}
    }

    impl Clone for AXElement {
        fn clone(&self) -> Self {
            AXElement(self.0)
        }
    }

    pub fn element_for_pid(_pid: i32) -> AXElement {
        AXElement(std::ptr::null())
    }

    pub fn window_element_for(_pid: i32, _win_title: &str) -> AXElement {
        AXElement(std::ptr::null())
    }

    pub fn copy_ax_array(_el: &AXElement, _attr: &str) -> Option<Vec<AXElement>> {
        None
    }

    pub fn build_subtree(
        _el: &AXElement,
        _depth: u8,
        _max_depth: u8,
        _include_bounds: bool,
        _visited: &mut FxHashSet<usize>,
    ) -> Option<AccessibilityNode> {
        None
    }

    pub fn copy_string_attr(_el: &AXElement, _attr: &str) -> Option<String> {
        None
    }

    pub fn read_bounds(_el: &AXElement) -> Option<agent_desktop_core::node::Rect> {
        None
    }
}

pub use imp::{build_subtree, copy_ax_array, copy_string_attr, element_for_pid, read_bounds, window_element_for, AXElement};
