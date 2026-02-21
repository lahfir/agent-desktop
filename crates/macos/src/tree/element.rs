use agent_desktop_core::node::Rect;

pub const ABSOLUTE_MAX_DEPTH: u8 = 50;

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use accessibility_sys::{
        kAXChildrenAttribute, kAXDescriptionAttribute, kAXEnabledAttribute, kAXErrorSuccess,
        kAXFocusedAttribute, kAXRoleAttribute, kAXTitleAttribute, kAXValueAttribute,
        AXUIElementCopyAttributeValue, AXUIElementCopyMultipleAttributeValues,
        AXUIElementCreateApplication, AXUIElementRef, AXUIElementSetMessagingTimeout,
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

    pub fn fetch_node_attrs(
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
            crate::tree::builder::label_from_children(&children)
        })
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

    pub fn copy_bool_attr(el: &AXElement, attr: &str) -> Option<bool> {
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
    pub fn copy_ax_array(_el: &AXElement, _attr: &str) -> Option<Vec<AXElement>> {
        None
    }
    pub fn copy_string_attr(_el: &AXElement, _attr: &str) -> Option<String> {
        None
    }
    pub fn copy_bool_attr(_el: &AXElement, _attr: &str) -> Option<bool> {
        None
    }
    pub fn copy_element_attr(_el: &AXElement, _attr: &str) -> Option<AXElement> {
        None
    }
    pub fn read_bounds(_el: &AXElement) -> Option<Rect> {
        None
    }
    pub fn resolve_element_name(_el: &AXElement) -> Option<String> {
        None
    }
    pub fn fetch_node_attrs(
        _el: &AXElement,
    ) -> (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        bool,
        bool,
    ) {
        (None, None, None, None, true, false)
    }
}

pub use imp::{
    copy_ax_array, copy_bool_attr, copy_element_attr, copy_string_attr, element_for_pid,
    fetch_node_attrs, read_bounds, resolve_element_name, AXElement,
};
