pub(crate) struct NativeRead<T> {
    pub(crate) value: Option<T>,
    pub(crate) error: Option<i32>,
}

impl<T> NativeRead<T> {
    fn success(value: T) -> Self {
        Self {
            value: Some(value),
            error: None,
        }
    }

    fn failure(error: i32) -> Self {
        Self {
            value: None,
            error: Some(error),
        }
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use super::NativeRead;
    use crate::{cf_type::created_cf_array, tree::AXElement};
    use accessibility_sys::kAXErrorSuccess;
    use core_foundation::{
        base::{CFEqual, CFTypeRef, TCFType},
        string::CFString,
    };

    pub(crate) fn is_attr_settable_with_status(
        el: &AXElement,
        attr: &str,
        deadline: std::time::Instant,
    ) -> NativeRead<bool> {
        let cf_attr = CFString::new(attr);
        let (err, settable) =
            crate::tree::ax_ipc::is_attribute_settable(el, cf_attr.as_concrete_TypeRef(), deadline);
        if err == kAXErrorSuccess {
            NativeRead::success(settable)
        } else {
            NativeRead::failure(err)
        }
    }

    pub(crate) fn copy_action_names_with_status(
        el: &AXElement,
        deadline: std::time::Instant,
        usage: &mut crate::tree::observation_usage::ObservationUsage,
    ) -> NativeRead<Vec<String>> {
        let (err, actions_ref) = crate::tree::ax_ipc::copy_action_names(el, deadline);
        if err != kAXErrorSuccess {
            if !actions_ref.is_null() {
                drop(created_cf_array(actions_ref as _));
            }
            return NativeRead::failure(err);
        }
        if actions_ref.is_null() {
            return NativeRead::failure(err);
        }

        let Some(actions) = created_cf_array(actions_ref as _) else {
            return NativeRead::failure(i32::MIN);
        };
        if actions.len() > 256 {
            return NativeRead::failure(i32::MIN + 1);
        }
        let mut result = Vec::with_capacity(actions.len() as usize);
        for i in 0..actions.len() {
            if let Some(name) = actions.get(i).and_then(|v| v.downcast::<CFString>()) {
                match crate::tree::bounded_string::BoundedString::from_cf(&name, usage) {
                    Ok(name) if name.complete => result.push(name.value),
                    _ => return NativeRead::failure(i32::MIN + 2),
                }
            } else {
                return NativeRead::failure(i32::MIN);
            }
        }
        NativeRead::success(result)
    }

    pub fn same_element(a: &AXElement, b: &AXElement) -> bool {
        unsafe { CFEqual(a.0 as CFTypeRef, b.0 as CFTypeRef) != 0 }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::NativeRead;
    use crate::tree::AXElement;

    pub(crate) fn is_attr_settable_with_status(
        _el: &AXElement,
        _attr: &str,
        _deadline: std::time::Instant,
    ) -> NativeRead<bool> {
        NativeRead::failure(i32::MIN)
    }

    pub(crate) fn copy_action_names_with_status(
        _el: &AXElement,
        _deadline: std::time::Instant,
        _usage: &mut crate::tree::observation_usage::ObservationUsage,
    ) -> NativeRead<Vec<String>> {
        NativeRead::failure(i32::MIN)
    }

    pub fn same_element(_a: &AXElement, _b: &AXElement) -> bool {
        false
    }
}

pub(crate) use imp::same_element;
pub(crate) use imp::{copy_action_names_with_status, is_attr_settable_with_status};

#[cfg(test)]
mod tests {
    use super::NativeRead;

    #[test]
    fn failed_native_read_never_fabricates_an_empty_value() {
        let strings = NativeRead::<Vec<String>>::failure(-1);
        let flag = NativeRead::<bool>::failure(-1);

        assert!(strings.value.is_none());
        assert!(flag.value.is_none());
        assert_eq!(strings.error, Some(-1));
        assert_eq!(flag.error, Some(-1));
    }
}
