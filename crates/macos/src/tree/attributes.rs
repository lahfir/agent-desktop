#[cfg(target_os = "macos")]
mod imp {
    use crate::{
        cf_type::created_cf_array,
        tree::{ax_element::AXElement, ax_value},
    };

    const MALFORMED_AX_VALUE: i32 = i32::MIN;
    use accessibility_sys::kAXErrorSuccess;
    use core_foundation::{
        array::CFArray,
        base::{CFType, CFTypeRef, TCFType},
        boolean::CFBoolean,
        string::CFString,
    };

    pub fn set_messaging_timeout(
        el: &AXElement,
        deadline: impl crate::tree::ax_ipc::AxDeadline,
    ) -> Result<(), agent_desktop_core::AdapterError> {
        crate::tree::ax_ipc::prepare(el, deadline).map(|_| ())
    }

    pub fn copy_bool_attr(
        el: &AXElement,
        attr: &str,
        deadline: impl crate::tree::ax_ipc::AxDeadline,
    ) -> Option<bool> {
        copy_bool_attr_result(el, attr, deadline).ok().flatten()
    }

    pub(crate) fn copy_bool_attr_result(
        el: &AXElement,
        attr: &str,
        deadline: impl crate::tree::ax_ipc::AxDeadline,
    ) -> Result<Option<bool>, i32> {
        let cf_attr = CFString::new(attr);
        let (err, value) =
            crate::tree::ax_ipc::copy_attribute_value(el, cf_attr.as_concrete_TypeRef(), deadline);
        if err != kAXErrorSuccess {
            if !value.is_null() {
                unsafe { core_foundation::base::CFRelease(value) };
            }
            return if is_absent_error(err) {
                Ok(None)
            } else {
                Err(err)
            };
        }
        if value.is_null() {
            return Ok(None);
        }
        let cf_type = unsafe { CFType::wrap_under_create_rule(value) };
        cf_type
            .downcast::<CFBoolean>()
            .map(|value| Some(value.into()))
            .ok_or(MALFORMED_AX_VALUE)
    }

    pub(crate) fn copy_ax_array_prefix_result(
        el: &AXElement,
        attr: &str,
        max_values: usize,
        deadline: impl crate::tree::ax_ipc::AxDeadline,
    ) -> Result<Option<Vec<AXElement>>, i32> {
        if max_values == 0 {
            return Ok(Some(Vec::new()));
        }
        let cf_attr = CFString::new(attr);
        let (err, value) = crate::tree::ax_ipc::copy_attribute_values(
            el,
            cf_attr.as_concrete_TypeRef(),
            0,
            max_values as core_foundation_sys::base::CFIndex,
            deadline,
        );
        if err != kAXErrorSuccess {
            if !value.is_null() {
                drop(created_cf_array(value as CFTypeRef));
            }
            return if is_absent_error(err) {
                Ok(None)
            } else {
                Err(err)
            };
        }
        if value.is_null() {
            return Ok(None);
        }
        let Some(arr) = created_cf_array(value as CFTypeRef) else {
            return Err(MALFORMED_AX_VALUE);
        };
        let expected = arr.len() as usize;
        let elements = ax_array_items(arr);
        if elements.len() != expected {
            return Err(MALFORMED_AX_VALUE);
        }
        Ok(Some(elements))
    }

    pub(crate) fn copy_element_attr_result(
        el: &AXElement,
        attr: &str,
        deadline: impl crate::tree::ax_ipc::AxDeadline,
    ) -> Result<Option<AXElement>, i32> {
        let cf_attr = CFString::new(attr);
        let (err, value) =
            crate::tree::ax_ipc::copy_attribute_value(el, cf_attr.as_concrete_TypeRef(), deadline);
        if err != kAXErrorSuccess {
            if !value.is_null() {
                unsafe { core_foundation::base::CFRelease(value) };
            }
            return if is_absent_error(err) {
                Ok(None)
            } else {
                Err(err)
            };
        }
        if value.is_null() {
            return Ok(None);
        }
        ax_value::created_ax_element(value)
            .map(Some)
            .ok_or(MALFORMED_AX_VALUE)
    }

    fn ax_array_items(arr: CFArray<CFType>) -> Vec<AXElement> {
        arr.into_iter()
            .filter_map(|item| ax_value::retained_ax_element(&item))
            .collect()
    }

    fn is_absent_error(error: i32) -> bool {
        crate::tree::ax_absence::is_absent_attribute_error(error)
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use crate::tree::ax_element::AXElement;

    pub(crate) fn copy_ax_array_result(
        _el: &AXElement,
        _attr: &str,
    ) -> Result<Option<Vec<AXElement>>, i32> {
        Ok(None)
    }

    pub fn copy_bool_attr(
        _el: &AXElement,
        _attr: &str,
        _deadline: impl crate::tree::ax_ipc::AxDeadline,
    ) -> Option<bool> {
        None
    }

    pub(crate) fn copy_bool_attr_result(
        _el: &AXElement,
        _attr: &str,
        _deadline: impl crate::tree::ax_ipc::AxDeadline,
    ) -> Result<Option<bool>, i32> {
        Ok(None)
    }

    pub(crate) fn copy_element_attr_result(
        _el: &AXElement,
        _attr: &str,
        _deadline: impl crate::tree::ax_ipc::AxDeadline,
    ) -> Result<Option<AXElement>, i32> {
        Ok(None)
    }

    pub(crate) fn copy_ax_array_prefix_result(
        _el: &AXElement,
        _attr: &str,
        _max_values: usize,
        _deadline: impl crate::tree::ax_ipc::AxDeadline,
    ) -> Result<Option<Vec<AXElement>>, i32> {
        Ok(None)
    }

    pub fn set_messaging_timeout(
        _el: &AXElement,
        _deadline: impl crate::tree::ax_ipc::AxDeadline,
    ) -> Result<(), agent_desktop_core::AdapterError> {
        Ok(())
    }
}

pub(crate) use super::text_attributes::{
    copy_string_attr_bounded_result, copy_string_attr_result, copy_value_typed,
    copy_value_typed_bounded_result, copy_value_typed_result,
};
pub(crate) use imp::{
    copy_ax_array_prefix_result, copy_bool_attr, copy_bool_attr_result, copy_element_attr_result,
    set_messaging_timeout,
};

#[cfg(not(target_os = "macos"))]
pub(crate) use imp::copy_ax_array_result;
