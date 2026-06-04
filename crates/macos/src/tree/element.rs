pub const ABSOLUTE_MAX_DEPTH: u8 = 50;

pub(crate) fn child_attributes(ax_role: Option<&str>) -> &'static [&'static str] {
    if ax_role == Some("AXBrowser") {
        &["AXColumns", "AXContents"]
    } else if ax_role == Some("AXApplication") {
        &["AXWindows", "AXFocusedWindow", "AXMainWindow", "AXChildren"]
    } else {
        &["AXChildren", "AXContents", "AXChildrenInNavigationOrder"]
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use crate::{
        cf_type::created_cf_array,
        tree::{
            NodeAttrs,
            ax_element::AXElement,
            ax_value,
            node_attrs::{parse_bool_attr, parse_enabled},
        },
    };
    use accessibility_sys::{
        AXUIElementCopyAttributeValue, AXUIElementCopyAttributeValues,
        AXUIElementCopyMultipleAttributeValues, AXUIElementCreateApplication,
        AXUIElementGetAttributeValueCount, AXUIElementSetMessagingTimeout, kAXDescriptionAttribute,
        kAXEnabledAttribute, kAXErrorSuccess, kAXRoleAttribute, kAXTitleAttribute,
        kAXValueAttribute,
    };
    use core_foundation::{
        array::CFArray,
        base::{CFType, CFTypeRef, TCFType},
        boolean::CFBoolean,
        number::CFNumber,
        string::CFString,
    };

    pub fn element_for_pid(pid: i32) -> AXElement {
        let el = AXElement(unsafe { AXUIElementCreateApplication(pid) });
        if !el.0.is_null() {
            unsafe { AXUIElementSetMessagingTimeout(el.0, 2.0) };
        }
        el
    }

    pub fn fetch_node_attrs(el: &AXElement) -> NodeAttrs {
        let attr_names = [
            kAXRoleAttribute,
            kAXTitleAttribute,
            kAXDescriptionAttribute,
            kAXValueAttribute,
            kAXEnabledAttribute,
            "AXFocused",
            "AXExpanded",
            "AXDisclosing",
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
            return fetch_node_attrs_slow(el);
        }

        let Some(arr) = created_cf_array(result_ref) else {
            return fetch_node_attrs_slow(el);
        };
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
                    4..=7 => item
                        .downcast::<CFBoolean>()
                        .map(|b| bool::from(b).to_string()),
                    _ => None,
                }
            })
            .collect();

        let get = |i: usize| items.get(i).and_then(|v| v.clone());
        NodeAttrs {
            role: get(0),
            title: get(1),
            description: get(2),
            value: get(3),
            enabled: parse_enabled(get(4)),
            focused: parse_bool_attr(get(5)),
            expanded: parse_bool_attr(get(6)),
            disclosing: parse_bool_attr(get(7)),
        }
    }

    fn fetch_node_attrs_slow(el: &AXElement) -> NodeAttrs {
        let role = copy_string_attr(el, kAXRoleAttribute);
        let title = copy_string_attr(el, kAXTitleAttribute);
        let desc = copy_string_attr(el, kAXDescriptionAttribute);
        let val = copy_value_typed(el);
        let enabled = copy_bool_attr(el, kAXEnabledAttribute).unwrap_or(true);
        NodeAttrs {
            role,
            title,
            description: desc,
            value: val,
            enabled,
            focused: copy_bool_attr(el, "AXFocused"),
            expanded: copy_bool_attr(el, "AXExpanded"),
            disclosing: copy_bool_attr(el, "AXDisclosing"),
        }
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
            let children = super::child_attributes(ax_role.as_deref())
                .iter()
                .find_map(|attr| copy_ax_array(el, attr).filter(|v| !v.is_empty()))
                .unwrap_or_default();
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

    pub fn count_children(element: &AXElement, ax_role: Option<&str>) -> u32 {
        unsafe {
            for attr_name in child_attributes(ax_role) {
                let mut count: core_foundation_sys::base::CFIndex = 0;
                let attr = CFString::from_static_string(attr_name);
                let err = AXUIElementGetAttributeValueCount(
                    element.0,
                    attr.as_concrete_TypeRef(),
                    &mut count,
                );
                if err != kAXErrorSuccess {
                    continue;
                }
                if count > 0 {
                    return count as u32;
                }
            }
            0
        }
    }

    fn ax_array_items(arr: CFArray<CFType>) -> Vec<AXElement> {
        arr.into_iter()
            .filter_map(|item| ax_value::retained_ax_element(&item))
            .collect()
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use crate::tree::{NodeAttrs, ax_element::AXElement};

    pub fn element_for_pid(_pid: i32) -> AXElement {
        AXElement(std::ptr::null())
    }

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

    pub fn count_children(_element: &AXElement, _ax_role: Option<&str>) -> u32 {
        0
    }

    pub fn resolve_element_name(_el: &AXElement) -> Option<String> {
        None
    }

    pub fn copy_value_typed(_el: &AXElement) -> Option<String> {
        None
    }

    pub fn fetch_node_attrs(_el: &AXElement) -> NodeAttrs {
        NodeAttrs {
            enabled: true,
            ..NodeAttrs::default()
        }
    }
}

pub use imp::{
    copy_ax_array, copy_ax_array_prefix, copy_bool_attr, copy_element_attr,
    copy_first_element_attr, copy_i64_attr, copy_string_attr, copy_value_typed, count_children,
    element_for_pid, fetch_node_attrs, resolve_element_name,
};
