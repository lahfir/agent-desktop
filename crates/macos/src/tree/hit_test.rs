#[cfg(target_os = "macos")]
mod imp {
    use crate::tree::{
        AXElement, capabilities::same_element, copy_element_attr, copy_string_attr, read_bounds,
    };
    use accessibility_sys::{AXUIElementCopyElementAtPosition, kAXErrorSuccess, kAXRoleAttribute};
    use agent_desktop_core::{
        action::Point, error::AdapterError, hit_test::HitTestResult, native_handle::NativeHandle,
    };
    use std::mem::ManuallyDrop;

    pub fn hit_test_impl(
        handle: &NativeHandle,
        point: Point,
    ) -> Result<HitTestResult, AdapterError> {
        with_ax(handle, |target| hit_test_element(target, point))
    }

    fn with_ax<T>(
        handle: &NativeHandle,
        f: impl FnOnce(&AXElement) -> Result<T, AdapterError>,
    ) -> Result<T, AdapterError> {
        let el = ManuallyDrop::new(AXElement(
            handle.as_raw() as accessibility_sys::AXUIElementRef
        ));
        f(&el)
    }

    fn hit_test_element(target: &AXElement, point: Point) -> Result<HitTestResult, AdapterError> {
        let Some(bounds) = read_bounds(target) else {
            return Err(AdapterError::new(
                agent_desktop_core::error::ErrorCode::ActionFailed,
                "Element bounds unavailable for hit test",
            ));
        };
        if bounds.width <= 0.0 || bounds.height <= 0.0 {
            return Ok(HitTestResult::blocked(None));
        }
        let pid = crate::system::app_ops::pid_from_element(target)
            .ok_or_else(|| AdapterError::internal("Could not read pid for hit test"))?;
        let app = crate::tree::element_for_pid(pid);
        let mut hit_ref: accessibility_sys::AXUIElementRef = std::ptr::null_mut();
        let err = unsafe {
            AXUIElementCopyElementAtPosition(app.0, point.x as f32, point.y as f32, &mut hit_ref)
        };
        if err != kAXErrorSuccess || hit_ref.is_null() {
            return Ok(HitTestResult::blocked(None));
        }
        let hit = AXElement(hit_ref);
        let topmost_role = copy_string_attr(&hit, kAXRoleAttribute);
        let receives = element_contains(target, &hit) || element_contains(&hit, target);
        Ok(if receives {
            HitTestResult::receives_events(topmost_role)
        } else {
            HitTestResult::blocked(topmost_role)
        })
    }

    fn element_contains(ancestor: &AXElement, candidate: &AXElement) -> bool {
        if same_element(ancestor, candidate) {
            return true;
        }
        let mut current = copy_element_attr(candidate, "AXParent");
        while let Some(parent) = current {
            if same_element(ancestor, &parent) {
                return true;
            }
            current = copy_element_attr(&parent, "AXParent");
        }
        false
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use agent_desktop_core::{
        action::Point, error::AdapterError, hit_test::HitTestResult, native_handle::NativeHandle,
    };

    pub fn hit_test_impl(
        _handle: &NativeHandle,
        _point: Point,
    ) -> Result<HitTestResult, AdapterError> {
        Err(AdapterError::not_supported("hit_test"))
    }
}

pub use imp::hit_test_impl;
