use agent_desktop_core::node::{AccessibilityNode, Rect};
use rustc_hash::FxHashSet;

pub const ABSOLUTE_MAX_DEPTH: u8 = 50;

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use accessibility_sys::{
        kAXChildrenAttribute, kAXDescriptionAttribute, kAXEnabledAttribute,
        kAXErrorSuccess, kAXFocusedAttribute, kAXRoleAttribute,
        kAXTitleAttribute, kAXValueAttribute,
        AXUIElementCopyAttributeValue, AXUIElementCreateApplication, AXUIElementRef,
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

        let role = copy_string_attr(el, kAXRoleAttribute)?;
        let normalized_role = crate::roles::ax_role_to_str(&role).to_string();

        let title = copy_string_attr(el, kAXTitleAttribute);
        let ax_desc = copy_string_attr(el, kAXDescriptionAttribute);
        let name = title.clone().or_else(|| ax_desc.clone());
        let description = if title.is_some() { ax_desc } else { None };

        let value = copy_string_attr(el, kAXValueAttribute);

        let mut states = Vec::new();
        if copy_bool_attr(el, kAXFocusedAttribute) == Some(true) {
            states.push("focused".into());
        }
        if copy_bool_attr(el, kAXEnabledAttribute) == Some(false) {
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
            role: normalized_role,
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

    fn copy_children(el: &AXElement) -> Option<Vec<AXElement>> {
        let cf_attr = CFString::new(kAXChildrenAttribute);
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
        let arr = unsafe { CFArray::<CFType>::wrap_under_create_rule(value as _) };
        let children = arr
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

    fn read_bounds(el: &AXElement) -> Option<Rect> {
        use accessibility_sys::{
            kAXPositionAttribute, kAXSizeAttribute,
            AXValueGetValue,
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
}

pub use imp::{build_subtree, copy_string_attr, element_for_pid, AXElement};
