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
            attributes::{
                copy_ax_array, copy_bool_attr, copy_first_element_attr, copy_string_attr,
                copy_value_typed,
            },
            ax_element::AXElement,
            ax_value,
            element_bounds::{read_bounds, rect_from_parts},
            node_attrs::{NodeAttrStates, parse_bool_attr, parse_enabled},
        },
    };
    use accessibility_sys::{
        AXUIElementCopyMultipleAttributeValues, AXUIElementCreateApplication,
        AXUIElementGetAttributeValueCount, AXUIElementSetMessagingTimeout, AXValueGetValue,
        kAXDescriptionAttribute, kAXEnabledAttribute, kAXErrorSuccess, kAXPositionAttribute,
        kAXRoleAttribute, kAXSizeAttribute, kAXTitleAttribute, kAXValueAttribute,
        kAXValueTypeCGPoint, kAXValueTypeCGSize,
    };
    use core_foundation::{
        array::CFArray,
        base::{CFType, CFTypeRef, TCFType},
        boolean::CFBoolean,
        number::CFNumber,
        string::CFString,
    };
    use core_graphics::geometry::{CGPoint, CGSize};

    const SCROLLBAR_ATTRS: [&str; 2] = ["AXVerticalScrollBar", "AXHorizontalScrollBar"];

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
            kAXPositionAttribute,
            kAXSizeAttribute,
            SCROLLBAR_ATTRS[0],
            SCROLLBAR_ATTRS[1],
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

        let mut texts: [Option<String>; 8] = Default::default();
        let mut position: Option<CGPoint> = None;
        let mut size: Option<CGSize> = None;
        let mut has_scrollbars = false;
        for (idx, item) in arr.into_iter().enumerate() {
            match idx {
                0..=7 => texts[idx] = decode_text_attr(idx, &item),
                8 => position = decode_ax_point(&item),
                9 => size = decode_ax_size(&item),
                10 | 11 => {
                    has_scrollbars =
                        has_scrollbars || ax_value::retained_ax_element(&item).is_some();
                }
                _ => {}
            }
        }

        let get = |i: usize| texts.get(i).and_then(|v| v.clone());
        NodeAttrs {
            role: get(0),
            title: get(1),
            description: get(2),
            value: get(3),
            states: NodeAttrStates {
                enabled: parse_enabled(get(4)),
                focused: parse_bool_attr(get(5)),
                expanded: parse_bool_attr(get(6)),
                disclosing: parse_bool_attr(get(7)),
            },
            bounds: position.zip(size).and_then(|(p, s)| rect_from_parts(p, s)),
            has_scrollbars,
        }
    }

    fn decode_text_attr(idx: usize, item: &CFType) -> Option<String> {
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
    }

    fn decode_ax_point(item: &CFType) -> Option<CGPoint> {
        let mut point = CGPoint::new(0.0, 0.0);
        let decoded = unsafe {
            AXValueGetValue(
                item.as_CFTypeRef() as _,
                kAXValueTypeCGPoint,
                &mut point as *mut _ as *mut std::ffi::c_void,
            )
        };
        decoded.then_some(point)
    }

    fn decode_ax_size(item: &CFType) -> Option<CGSize> {
        let mut size = CGSize::new(0.0, 0.0);
        let decoded = unsafe {
            AXValueGetValue(
                item.as_CFTypeRef() as _,
                kAXValueTypeCGSize,
                &mut size as *mut _ as *mut std::ffi::c_void,
            )
        };
        decoded.then_some(size)
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
            states: NodeAttrStates {
                enabled,
                focused: copy_bool_attr(el, "AXFocused"),
                expanded: copy_bool_attr(el, "AXExpanded"),
                disclosing: copy_bool_attr(el, "AXDisclosing"),
            },
            bounds: read_bounds(el),
            has_scrollbars: copy_first_element_attr(el, &SCROLLBAR_ATTRS).is_some(),
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
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use crate::tree::{NodeAttrs, ax_element::AXElement};

    pub fn element_for_pid(_pid: i32) -> AXElement {
        AXElement(std::ptr::null())
    }

    pub fn count_children(_element: &AXElement, _ax_role: Option<&str>) -> u32 {
        0
    }

    pub fn resolve_element_name(_el: &AXElement) -> Option<String> {
        None
    }

    pub fn fetch_node_attrs(_el: &AXElement) -> NodeAttrs {
        NodeAttrs::default()
    }
}

pub use imp::{count_children, element_for_pid, fetch_node_attrs, resolve_element_name};
