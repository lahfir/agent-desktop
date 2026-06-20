#[cfg(target_os = "macos")]
mod imp {
    use crate::{
        cf_type::created_cf_array,
        tree::{ax_element::AXElement, ax_value},
    };
    use accessibility_sys::{
        AXUIElementCopyAttributeValue, AXUIElementCopyAttributeValues,
        AXUIElementCopyMultipleAttributeValues, AXUIElementSetMessagingTimeout, kAXErrorSuccess,
        kAXValueAttribute,
    };
    use core_foundation::{
        array::CFArray,
        base::{CFType, CFTypeRef, TCFType},
        boolean::CFBoolean,
        number::CFNumber,
        string::CFString,
    };

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

    pub fn set_messaging_timeout(el: &AXElement, timeout: std::time::Duration) {
        if el.0.is_null() {
            return;
        }
        let seconds = timeout.as_secs_f32().clamp(0.001, 2.0);
        unsafe { AXUIElementSetMessagingTimeout(el.0, seconds) };
    }

    pub fn copy_value_typed(el: &AXElement) -> Option<String> {
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

    pub fn copy_i64_attr(el: &AXElement, attr: &str) -> Option<i64> {
        let cf_attr = CFString::new(attr);
        let mut value: CFTypeRef = std::ptr::null_mut();
        let err = unsafe {
            AXUIElementCopyAttributeValue(el.0, cf_attr.as_concrete_TypeRef(), &mut value)
        };
        if err != kAXErrorSuccess || value.is_null() {
            return None;
        }
        let cf_type = unsafe { CFType::wrap_under_create_rule(value) };
        cf_type.downcast::<CFNumber>().and_then(|n| n.to_i64())
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
        let arr = created_cf_array(value)?;
        Some(ax_array_items(arr))
    }

    pub fn copy_ax_array_prefix(
        el: &AXElement,
        attr: &str,
        max_values: usize,
    ) -> Option<Vec<AXElement>> {
        if max_values == 0 {
            return Some(Vec::new());
        }
        let cf_attr = CFString::new(attr);
        let mut value: core_foundation_sys::array::CFArrayRef = std::ptr::null();
        let err = unsafe {
            AXUIElementCopyAttributeValues(
                el.0,
                cf_attr.as_concrete_TypeRef(),
                0,
                max_values as core_foundation_sys::base::CFIndex,
                &mut value,
            )
        };
        if err != kAXErrorSuccess || value.is_null() {
            return None;
        }
        let arr = created_cf_array(value as CFTypeRef)?;
        Some(ax_array_items(arr))
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
        ax_value::created_ax_element(value)
    }

    pub fn copy_first_element_attr(el: &AXElement, attrs: &[&str]) -> Option<AXElement> {
        if attrs.is_empty() {
            return None;
        }
        let cf_names: Vec<CFString> = attrs.iter().map(|attr| CFString::new(attr)).collect();
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
        if !result_ref.is_null() {
            let arr = created_cf_array(result_ref);
            if err == kAXErrorSuccess
                && let Some(arr) = arr
            {
                return arr
                    .into_iter()
                    .find_map(|item| ax_value::retained_ax_element(&item));
            }
        }
        attrs.iter().find_map(|attr| copy_element_attr(el, attr))
    }

    fn ax_array_items(arr: CFArray<CFType>) -> Vec<AXElement> {
        arr.into_iter()
            .filter_map(|item| ax_value::retained_ax_element(&item))
            .collect()
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use crate::tree::ax_element::AXElement;

    pub fn copy_ax_array(_el: &AXElement, _attr: &str) -> Option<Vec<AXElement>> {
        None
    }

    pub fn copy_ax_array_prefix(
        _el: &AXElement,
        _attr: &str,
        _max_values: usize,
    ) -> Option<Vec<AXElement>> {
        None
    }

    pub fn copy_string_attr(_el: &AXElement, _attr: &str) -> Option<String> {
        None
    }

    pub fn copy_bool_attr(_el: &AXElement, _attr: &str) -> Option<bool> {
        None
    }

    pub fn copy_i64_attr(_el: &AXElement, _attr: &str) -> Option<i64> {
        None
    }

    pub fn copy_element_attr(_el: &AXElement, _attr: &str) -> Option<AXElement> {
        None
    }

    pub fn copy_first_element_attr(_el: &AXElement, _attrs: &[&str]) -> Option<AXElement> {
        None
    }

    pub fn copy_value_typed(_el: &AXElement) -> Option<String> {
        None
    }

    pub fn set_messaging_timeout(_el: &AXElement, _timeout: std::time::Duration) {}
}

pub(crate) use imp::{
    copy_ax_array, copy_ax_array_prefix, copy_bool_attr, copy_element_attr,
    copy_first_element_attr, copy_i64_attr, copy_string_attr, copy_value_typed,
    set_messaging_timeout,
};
