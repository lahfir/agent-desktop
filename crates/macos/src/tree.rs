use agent_desktop_core::node::{AccessibilityNode, Rect};
use rustc_hash::FxHashSet;

pub const ABSOLUTE_MAX_DEPTH: u8 = 50;

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use accessibility_sys::{
        kAXChildrenAttribute, kAXContentsAttribute, kAXDescriptionAttribute, kAXEnabledAttribute,
        kAXErrorSuccess, kAXFocusedAttribute, kAXRoleAttribute, kAXTitleAttribute,
        kAXValueAttribute, kAXWindowsAttribute, AXUIElementCopyAttributeValue,
        AXUIElementCopyMultipleAttributeValues, AXUIElementCreateApplication, AXUIElementRef,
        AXUIElementSetMessagingTimeout,
    };
    use core_foundation::{
        array::CFArray,
        base::{CFRelease, CFRetain, CFType, CFTypeRef, TCFType},
        boolean::CFBoolean,
        number::CFNumber,
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
        let el = AXElement(unsafe { AXUIElementCreateApplication(pid) });
        if !el.0.is_null() {
            unsafe { AXUIElementSetMessagingTimeout(el.0, 2.0) };
        }
        el
    }

    /// Find the AXWindow element whose title matches `win_title`.
    /// Falls back to the first window, then to the app element if no windows.
    pub fn window_element_for(pid: i32, win_title: &str) -> AXElement {
        let app = element_for_pid(pid);

        if let Some(windows) = copy_ax_array(&app, kAXWindowsAttribute) {
            for win in &windows {
                let title = copy_string_attr(win, kAXTitleAttribute);
                if title.as_deref() == Some(win_title) {
                    return win.clone();
                }
            }
            for win in &windows {
                let title = copy_string_attr(win, kAXTitleAttribute);
                if title
                    .as_deref()
                    .is_some_and(|t| t.contains(win_title) || win_title.contains(t))
                {
                    return win.clone();
                }
            }
            if let Some(first) = windows.into_iter().next() {
                return first;
            }
        }

        app
    }

    /// Batch-fetch six most-used attributes. Returns (role, title, desc, value, enabled, focused).
    /// Value handles CFString, CFBoolean, and CFNumber types.
    fn fetch_node_attrs(
        el: &AXElement,
    ) -> (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        bool,
        bool,
    ) {
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
            let role = copy_string_attr(el, kAXRoleAttribute);
            let title = copy_string_attr(el, kAXTitleAttribute);
            let desc = copy_string_attr(el, kAXDescriptionAttribute);
            let val = copy_value_typed(el);
            let enabled = copy_bool_attr(el, kAXEnabledAttribute).unwrap_or(true);
            let focused = copy_bool_attr(el, kAXFocusedAttribute).unwrap_or(false);
            return (role, title, desc, val, enabled, focused);
        }

        let arr = unsafe { CFArray::<CFType>::wrap_under_create_rule(result_ref as _) };
        let items: Vec<Option<String>> = arr
            .into_iter()
            .enumerate()
            .map(|(idx, item)| {
                if let Some(s) = item.downcast::<CFString>() {
                    return Some(s.to_string());
                }
                match idx {
                    3 => {
                        if let Some(b) = item.downcast::<CFBoolean>() {
                            return Some(bool::from(b).to_string());
                        }
                        if let Some(n) = item.downcast::<CFNumber>() {
                            if let Some(i) = n.to_i64() {
                                return Some(i.to_string());
                            }
                            if let Some(f) = n.to_f64() {
                                return Some(format!("{:.2}", f));
                            }
                        }
                        None
                    }
                    4 | 5 => item
                        .downcast::<CFBoolean>()
                        .map(|b| bool::from(b).to_string()),
                    _ => None,
                }
            })
            .collect();

        let get = |i: usize| items.get(i).and_then(|v| v.clone());
        let role = get(0);
        let title = get(1);
        let desc = get(2);
        let val = get(3);
        let enabled = get(4).map(|s| s == "true").unwrap_or(true);
        let focused = get(5).map(|s| s == "true").unwrap_or(false);

        (role, title, desc, val, enabled, focused)
    }

    /// Compute the effective display name for any element, mirroring `build_subtree` name resolution.
    pub fn resolve_element_name(el: &AXElement) -> Option<String> {
        let ax_role = copy_string_attr(el, kAXRoleAttribute);
        let title = copy_string_attr(el, kAXTitleAttribute);
        let desc = copy_string_attr(el, kAXDescriptionAttribute);

        let name = title.or(desc);
        let name = if name.is_none() && ax_role.as_deref() == Some("AXStaticText") {
            copy_string_attr(el, kAXValueAttribute).or(name)
        } else {
            name
        };

        name.or_else(|| {
            let children = copy_ax_array(el, kAXChildrenAttribute).unwrap_or_default();
            label_from_children(&children)
        })
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

        let name = if name.is_none() && ax_role.as_deref() == Some("AXStaticText") {
            value.clone().or(name)
        } else {
            name
        };

        let mut states = Vec::new();
        if focused {
            states.push("focused".into());
        }
        if !enabled {
            states.push("disabled".into());
        }

        let bounds = if include_bounds {
            read_bounds(el)
        } else {
            None
        };

        let children_raw = copy_children(el, ax_role.as_deref()).unwrap_or_default();
        let name = name.or_else(|| label_from_children(&children_raw));

        let children = children_raw
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

    /// Scan immediate children (and one level deeper through AXCell) to find a text label.
    /// Resolves the common macOS pattern: AXRow → AXCell → AXStaticText.kAXValue = "label".
    fn label_from_children(children: &[AXElement]) -> Option<String> {
        fn text_of(el: &AXElement) -> Option<String> {
            copy_string_attr(el, kAXValueAttribute)
                .or_else(|| copy_string_attr(el, kAXTitleAttribute))
                .filter(|s| !s.is_empty())
        }

        for child in children.iter().take(5) {
            match copy_string_attr(child, kAXRoleAttribute).as_deref() {
                Some("AXStaticText") => {
                    if let Some(s) = text_of(child) {
                        return Some(s);
                    }
                }
                Some("AXCell") | Some("AXGroup") => {
                    for gc in copy_ax_array(child, kAXChildrenAttribute).unwrap_or_default() {
                        if copy_string_attr(&gc, kAXRoleAttribute).as_deref()
                            == Some("AXStaticText")
                        {
                            if let Some(s) = text_of(&gc) {
                                return Some(s);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// Read children using the appropriate attribute for this element's role.
    /// AXBrowser exposes its content via AXColumns, not AXChildren.
    fn copy_children(el: &AXElement, ax_role: Option<&str>) -> Option<Vec<AXElement>> {
        if ax_role == Some("AXBrowser") {
            return copy_ax_array(el, "AXColumns");
        }
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

    pub fn copy_string_attr(el: &AXElement, attr: &str) -> Option<String> {
        let cf_attr = CFString::new(attr);
        let mut value: CFTypeRef = std::ptr::null_mut();
        let err = unsafe {
            AXUIElementCopyAttributeValue(el.0, cf_attr.as_concrete_TypeRef(), &mut value)
        };
        if err != kAXErrorSuccess || value.is_null() {
            return None;
        }
        let cf_type = unsafe { CFType::wrap_under_create_rule(value) };
        cf_type.downcast::<CFString>().map(|s| s.to_string())
    }

    /// Read kAXValueAttribute handling CFString, CFBoolean, and CFNumber.
    fn copy_value_typed(el: &AXElement) -> Option<String> {
        let cf_attr = CFString::new(kAXValueAttribute);
        let mut val_ref: CFTypeRef = std::ptr::null_mut();
        let err = unsafe {
            AXUIElementCopyAttributeValue(el.0, cf_attr.as_concrete_TypeRef(), &mut val_ref)
        };
        if err != kAXErrorSuccess || val_ref.is_null() {
            return None;
        }
        let cf = unsafe { CFType::wrap_under_create_rule(val_ref) };
        if let Some(s) = cf.downcast::<CFString>() {
            return Some(s.to_string());
        }
        if let Some(b) = cf.downcast::<CFBoolean>() {
            return Some(bool::from(b).to_string());
        }
        if let Some(n) = cf.downcast::<CFNumber>() {
            if let Some(i) = n.to_i64() {
                return Some(i.to_string());
            }
            if let Some(f) = n.to_f64() {
                return Some(format!("{:.2}", f));
            }
        }
        None
    }

    fn copy_bool_attr(el: &AXElement, attr: &str) -> Option<bool> {
        let cf_attr = CFString::new(attr);
        let mut value: CFTypeRef = std::ptr::null_mut();
        let err = unsafe {
            AXUIElementCopyAttributeValue(el.0, cf_attr.as_concrete_TypeRef(), &mut value)
        };
        if err != kAXErrorSuccess || value.is_null() {
            return None;
        }
        let cf_type = unsafe { CFType::wrap_under_create_rule(value) };
        cf_type.downcast::<CFBoolean>().map(|b| b.into())
    }

    /// Read an array-typed AX attribute, retaining each AXUIElement.
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
                    return None;
                }
                unsafe { CFRetain(ptr as CFTypeRef) };
                Some(AXElement(ptr))
            })
            .collect();
        Some(children)
    }

    pub fn copy_element_attr(el: &AXElement, attr: &str) -> Option<AXElement> {
        let cf_attr = CFString::new(attr);
        let mut value: CFTypeRef = std::ptr::null_mut();
        let err = unsafe {
            AXUIElementCopyAttributeValue(el.0, cf_attr.as_concrete_TypeRef(), &mut value)
        };
        if err != kAXErrorSuccess || value.is_null() {
            return None;
        }
        let ptr = value as AXUIElementRef;
        Some(AXElement(ptr))
    }

    pub fn read_bounds(el: &AXElement) -> Option<Rect> {
        use accessibility_sys::{
            kAXPositionAttribute, kAXSizeAttribute, kAXValueTypeCGPoint, kAXValueTypeCGSize,
            AXValueGetValue,
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
            AXValueGetValue(
                pos_ref as _,
                kAXValueTypeCGPoint,
                &mut point as *mut _ as *mut c_void,
            )
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
            AXValueGetValue(
                size_ref as _,
                kAXValueTypeCGSize,
                &mut size as *mut _ as *mut c_void,
            )
        };
        unsafe { CFRelease(size_ref) };
        if !got_size {
            return None;
        }

        Some(Rect {
            x: point.x,
            y: point.y,
            width: size.width,
            height: size.height,
        })
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
    pub fn copy_string_attr(_el: &AXElement, _attr: &str) -> Option<String> {
        None
    }
    pub fn copy_element_attr(_el: &AXElement, _attr: &str) -> Option<AXElement> {
        None
    }
    pub fn read_bounds(_el: &AXElement) -> Option<agent_desktop_core::node::Rect> {
        None
    }
    pub fn resolve_element_name(_el: &AXElement) -> Option<String> {
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
}

pub use imp::{
    build_subtree, copy_ax_array, copy_element_attr, copy_string_attr, element_for_pid,
    read_bounds, resolve_element_name, window_element_for, AXElement,
};
