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
                copy_bool_attr, copy_first_element_attr, copy_string_attr, copy_value_typed,
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
        string::{CFString, CFStringRef},
    };
    use core_graphics::geometry::{CGPoint, CGSize};

    const SCROLLBAR_ATTRS: [&str; 2] = ["AXVerticalScrollBar", "AXHorizontalScrollBar"];

    const FETCH_ATTR_NAMES: [&str; 18] = [
        kAXRoleAttribute,
        kAXTitleAttribute,
        kAXDescriptionAttribute,
        kAXValueAttribute,
        kAXEnabledAttribute,
        "AXFocused",
        "AXExpanded",
        "AXDisclosing",
        "AXSelected",
        "AXHidden",
        "AXElementBusy",
        "AXModal",
        "AXRequired",
        "AXIdentifier",
        kAXPositionAttribute,
        kAXSizeAttribute,
        SCROLLBAR_ATTRS[0],
        SCROLLBAR_ATTRS[1],
    ];

    /// Owns the CFStrings backing [`FETCH_ATTR_NAMES`] alongside the
    /// non-retaining `CFArray` view over them, so the array stays valid for
    /// the lifetime of this thread-local cache.
    struct AttrNamesCache {
        _names: Vec<CFString>,
        array: CFArray<CFStringRef>,
    }

    impl AttrNamesCache {
        fn build() -> Self {
            let names: Vec<CFString> = FETCH_ATTR_NAMES.iter().map(|a| CFString::new(a)).collect();
            let refs: Vec<CFStringRef> = names.iter().map(|s| s.as_concrete_TypeRef()).collect();
            let array = CFArray::from_copyable(&refs);
            Self {
                _names: names,
                array,
            }
        }
    }

    thread_local! {
        static ATTR_NAMES_CACHE: AttrNamesCache = AttrNamesCache::build();
    }

    pub fn element_for_pid(pid: i32) -> AXElement {
        let el = AXElement(unsafe { AXUIElementCreateApplication(pid) });
        if !el.0.is_null() {
            unsafe { AXUIElementSetMessagingTimeout(el.0, 2.0) };
        }
        el
    }

    pub fn fetch_node_attrs(el: &AXElement) -> NodeAttrs {
        let mut result_ref: CFTypeRef = std::ptr::null_mut();
        let err = ATTR_NAMES_CACHE.with(|cache| unsafe {
            AXUIElementCopyMultipleAttributeValues(
                el.0,
                cache.array.as_concrete_TypeRef(),
                0,
                &mut result_ref as *mut _ as *mut _,
            )
        });

        if err != kAXErrorSuccess || result_ref.is_null() {
            return fetch_node_attrs_slow(el);
        }

        let Some(arr) = created_cf_array(result_ref) else {
            return fetch_node_attrs_slow(el);
        };

        let mut texts: [Option<String>; 14] = Default::default();
        let mut position: Option<CGPoint> = None;
        let mut size: Option<CGSize> = None;
        let mut has_scrollbars = false;
        for (idx, item) in arr.into_iter().enumerate() {
            match idx {
                0..=13 => texts[idx] = decode_text_attr(idx, &item),
                14 => position = decode_ax_point(&item),
                15 => size = decode_ax_size(&item),
                16 | 17 => {
                    has_scrollbars =
                        has_scrollbars || ax_value::retained_ax_element(&item).is_some();
                }
                _ => {}
            }
        }

        let get = |i: usize| texts.get(i).and_then(|v| v.clone());
        let role = get(0);
        let readonly = compute_readonly(el, role.as_deref());
        NodeAttrs {
            role,
            title: get(1),
            description: get(2),
            value: get(3),
            native_id: crate::tree::native_id::meaningful_native_id(get(13)),
            states: NodeAttrStates {
                enabled: parse_enabled(get(4)),
                focused: parse_bool_attr(get(5)),
                expanded: parse_bool_attr(get(6)),
                disclosing: parse_bool_attr(get(7)),
                selected: parse_bool_attr(get(8)),
                hidden: parse_bool_attr(get(9)),
                busy: parse_bool_attr(get(10)),
                modal: parse_bool_attr(get(11)),
                required: parse_bool_attr(get(12)),
                readonly,
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
            4..=12 => item
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
        let readonly = compute_readonly(el, role.as_deref());
        let native_id =
            crate::tree::native_id::meaningful_native_id(copy_string_attr(el, "AXIdentifier"));
        NodeAttrs {
            role,
            title,
            description: desc,
            value: val,
            native_id,
            states: NodeAttrStates {
                enabled,
                focused: copy_bool_attr(el, "AXFocused"),
                expanded: copy_bool_attr(el, "AXExpanded"),
                disclosing: copy_bool_attr(el, "AXDisclosing"),
                selected: copy_bool_attr(el, "AXSelected"),
                hidden: copy_bool_attr(el, "AXHidden"),
                busy: copy_bool_attr(el, "AXElementBusy"),
                modal: copy_bool_attr(el, "AXModal"),
                required: copy_bool_attr(el, "AXRequired"),
                readonly,
            },
            bounds: read_bounds(el),
            has_scrollbars: copy_first_element_attr(el, &SCROLLBAR_ATTRS).is_some(),
        }
    }

    /// Single owner of the readonly derivation shared by the fast
    /// (`AXUIElementCopyMultipleAttributeValues`) and slow (per-attribute)
    /// read paths: an element only carries a readonly flag when its role is
    /// editable, and the flag is the negation of whether `AXValue` is
    /// currently settable.
    fn compute_readonly(el: &AXElement, role: Option<&str>) -> Option<bool> {
        editable_ax_role(role)
            .then(|| !crate::tree::capabilities::is_attr_settable(el, kAXValueAttribute))
    }

    fn editable_ax_role(role: Option<&str>) -> bool {
        matches!(
            role,
            Some(
                "AXTextField"
                    | "AXTextArea"
                    | "AXSearchField"
                    | "AXComboBox"
                    | "AXPopUpButton"
                    | "AXIncrementor"
                    | "AXStepper"
                    | "AXSlider"
                    | "AXValueIndicator"
            )
        )
    }

    pub fn count_children(element: &AXElement, ax_role: Option<&str>) -> u32 {
        for attr_name in child_attributes(ax_role) {
            let mut count: core_foundation_sys::base::CFIndex = 0;
            let attr = CFString::from_static_string(attr_name);
            let err = unsafe {
                AXUIElementGetAttributeValueCount(element.0, attr.as_concrete_TypeRef(), &mut count)
            };
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

#[cfg(not(target_os = "macos"))]
mod imp {
    use crate::tree::{NodeAttrs, ax_element::AXElement};

    pub fn element_for_pid(_pid: i32) -> AXElement {
        AXElement(std::ptr::null())
    }

    pub fn count_children(_element: &AXElement, _ax_role: Option<&str>) -> u32 {
        0
    }

    pub fn fetch_node_attrs(_el: &AXElement) -> NodeAttrs {
        NodeAttrs::default()
    }
}

pub use imp::{count_children, element_for_pid, fetch_node_attrs};

/// The element's accessible name, computed by the one shared reducer
/// [`super::builder::accessible_name`] (title -> description -> static-text
/// value -> aggregated child label, each trimmed and blank-as-absent). The
/// snapshot builder stores a ref's name through the same reducer, so strict ref
/// re-resolution here always recomputes exactly what was stored — the single
/// source of truth every name consumer shares (builder, strict resolver,
/// hit-test occluder naming, ambiguity classification).
#[cfg(target_os = "macos")]
pub fn resolve_element_name(el: &super::ax_element::AXElement) -> Option<String> {
    super::builder::accessible_name(el, &fetch_node_attrs(el))
}

#[cfg(not(target_os = "macos"))]
pub fn resolve_element_name(_el: &super::ax_element::AXElement) -> Option<String> {
    None
}

#[cfg(test)]
#[path = "element_tests.rs"]
mod tests;
