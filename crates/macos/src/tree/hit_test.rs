#[cfg(target_os = "macos")]
mod imp {
    use crate::actions::ax_helpers;
    use crate::tree::{
        AXElement, capabilities::same_element, copy_string_attr, element::ABSOLUTE_MAX_DEPTH,
        read_bounds, resolve_element_name,
    };
    use accessibility_sys::{AXUIElementCopyElementAtPosition, kAXErrorSuccess, kAXRoleAttribute};
    use agent_desktop_core::{
        action::Point, error::AdapterError, hit_test::HitTestResult, native_handle::NativeHandle,
    };
    use std::mem::ManuallyDrop;

    /// Hit-tests `point` against `target`'s owning application, then
    /// classifies the result against `target`'s ancestor chain: a hit on
    /// `target` or a descendant reaches it, a hit outside that chain names a
    /// real occluder, and a hit on `target`'s own ancestor — like any probe
    /// failure — is `Unknown` rather than a false occlusion, since composited
    /// or custom-drawn views often expose no distinct child node to hit-test.
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
            return Ok(HitTestResult::Unknown);
        };
        if bounds.width <= 0.0 || bounds.height <= 0.0 {
            return Ok(HitTestResult::Unknown);
        }
        let Some(pid) = crate::system::app_ops::pid_from_element(target) else {
            return Ok(HitTestResult::Unknown);
        };
        let app = crate::tree::element_for_pid(pid);
        let mut hit_ref: accessibility_sys::AXUIElementRef = std::ptr::null_mut();
        let err = unsafe {
            AXUIElementCopyElementAtPosition(app.0, point.x as f32, point.y as f32, &mut hit_ref)
        };
        if err != kAXErrorSuccess || hit_ref.is_null() {
            return Ok(HitTestResult::Unknown);
        }
        let hit = AXElement(hit_ref);
        Ok(classify_hit(target, &hit))
    }

    fn classify_hit(target: &AXElement, hit: &AXElement) -> HitTestResult {
        let limit = ABSOLUTE_MAX_DEPTH as usize;
        let reaches_target = same_element(target, hit)
            || ax_helpers::try_each_ancestor(hit, |ancestor| same_element(ancestor, target), limit);
        let is_ancestor_of_target = !reaches_target
            && ax_helpers::try_each_ancestor(target, |ancestor| same_element(ancestor, hit), limit);
        match classify_relation(reaches_target, is_ancestor_of_target) {
            HitClassification::ReachesTarget => HitTestResult::ReachesTarget,
            HitClassification::AncestorOfTarget => HitTestResult::Unknown,
            HitClassification::Unrelated => intercepted_by(hit),
        }
    }

    fn intercepted_by(hit: &AXElement) -> HitTestResult {
        let ax_role = copy_string_attr(hit, kAXRoleAttribute);
        let (role, promoted_label) =
            crate::tree::roles::normalized_role_and_label(hit, ax_role.as_deref());
        HitTestResult::InterceptedBy {
            role: Some(role),
            name: promoted_label.or_else(|| resolve_element_name(hit)),
            bounds: read_bounds(hit),
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(super) enum HitClassification {
        ReachesTarget,
        AncestorOfTarget,
        Unrelated,
    }

    pub(super) fn classify_relation(
        reaches_target: bool,
        is_ancestor_of_target: bool,
    ) -> HitClassification {
        if reaches_target {
            HitClassification::ReachesTarget
        } else if is_ancestor_of_target {
            HitClassification::AncestorOfTarget
        } else {
            HitClassification::Unrelated
        }
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

#[cfg(all(test, target_os = "macos"))]
#[path = "hit_test_tests.rs"]
mod tests;
