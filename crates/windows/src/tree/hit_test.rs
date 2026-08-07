//! Windows `ElementFromPoint` hit testing for the actionability occlusion gate.
//!
//! Coordinate space is shared by construction (A18-9): `ElementFromPoint` and
//! `BoundingRectangle` both speak physical screen pixels under the process
//! `PER_MONITOR_AWARE_V2` bootstrap; the only transform is a saturating
//! `f64`→`i32` narrowing into the crate `Point`. Mixed-DPI live verification
//! stays with the A16-4 deferral chain.
//!
//! Ancestor landings yield `Unknown` with no macOS-style application-scoped
//! retry — UIA has no scoped `ElementFromPoint`. That is the designed Chromium
//! outcome when a render-host pane answers for non-hit-addressable web content.
//!
//! Unrelated hits reach window-attribution corroboration (`hit_test_corroborate`):
//! `InterceptedBy` only on two-opinion agreement. Same-root demotion for points
//! outside `target ∩ scroll viewport` (A18-2 unclipped provider rects) is
//! applied before that seam.

use agent_desktop_core::{
    AdapterError, Deadline, ErrorCode, Point, Rect, hit_test::HitTestResult,
    native_handle::NativeHandle,
};

#[cfg(target_os = "windows")]
#[path = "hit_test_classify.rs"]
mod classify;

#[cfg(target_os = "windows")]
#[path = "hit_test_corroborate.rs"]
mod corroborate;

#[cfg(target_os = "windows")]
mod imp {
    use super::classify::{self, point_in_rect};
    use super::corroborate;
    use super::{AdapterError, Deadline, ErrorCode, HitTestResult, NativeHandle, Point, Rect};
    use crate::system::hresult::ReadDisposition;
    use crate::tree::automation::{
        UiaFailure, automation_client, failure_of, uia_failure_disposition, uia_failure_error,
    };
    use crate::tree::element::{UIAElement, uia_element};
    use crate::tree::live_read::corroborate_verified_process;
    use crate::tree::properties::read_one;
    use crate::tree::property_ids::TreeProperty;
    use crate::tree::walker::NodeKey;
    use uiautomation::UIAutomation;
    use uiautomation::core::UITreeWalker;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, IsIconic, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
        SM_YVIRTUALSCREEN,
    };

    /// Hit-tests `point` via bounded `ElementFromPoint`, then classifies the
    /// frontmost result against `target`. Every probe failure, guard trip, and
    /// ancestor landing is `Unknown`; `Err` is reserved for invalid handles,
    /// crossed deadlines, and the target pre-read's dead-token / permission /
    /// transport-retryable escapes.
    pub fn hit_test_impl(
        handle: &NativeHandle,
        point: Point,
        deadline: Deadline,
    ) -> Result<HitTestResult, AdapterError> {
        let target = uia_element(handle)?;
        crate::system::permissions::ensure_budget(deadline)?;
        let client = automation_client()?;
        hit_test_element(target, point, deadline, &client)
    }

    fn hit_test_element(
        target: &UIAElement,
        point: Point,
        deadline: Deadline,
        client: &UIAutomation,
    ) -> Result<HitTestResult, AdapterError> {
        corroborate_verified_process(target)?;
        let bounds = match read_target_bounds(target)? {
            Some(bounds) => bounds,
            None => return Ok(HitTestResult::Unknown),
        };
        corroborate_verified_process(target)?;
        if let Some(unknown) = pre_probe_guard(&bounds, &point, target, client) {
            return Ok(unknown);
        }
        crate::system::permissions::ensure_budget(deadline)?;
        let hit = match probe_element_from_point(client, &point) {
            Some(hit) => hit,
            None => return Ok(HitTestResult::Unknown),
        };
        let walker = match client.get_raw_view_walker() {
            Ok(walker) => walker,
            Err(_) => return Ok(HitTestResult::Unknown),
        };
        let same = |left: &UIAElement, right: &UIAElement| same_element(client, left, right);
        let identity = |node: &UIAElement| element_identity(node);
        let parent_of = |node: &UIAElement| parent_step(&walker, node);
        let Some(classification) =
            classify::classify_hit_with(target, &hit, &same, &identity, &parent_of)
        else {
            return Ok(HitTestResult::Unknown);
        };
        let viewport = nearest_scroll_viewport_bounds(target, &walker);
        let demote = classify::should_demote_outside_viewport(&point, &bounds, viewport.as_ref());
        Ok(finish_classification(
            classification,
            demote,
            target,
            &hit,
            point,
            &walker,
        ))
    }

    fn finish_classification(
        classification: classify::HitClassification,
        demote_for_viewport: bool,
        target: &UIAElement,
        hit: &UIAElement,
        point: Point,
        walker: &UITreeWalker,
    ) -> HitTestResult {
        resolve_classification(classification, demote_for_viewport, || {
            corroborate::corroborate_interception(target, hit, point, walker)
        })
    }

    pub(super) fn resolve_classification(
        classification: classify::HitClassification,
        demote_for_viewport: bool,
        corroborate: impl FnOnce() -> HitTestResult,
    ) -> HitTestResult {
        match classification {
            classify::HitClassification::ReachesTarget => HitTestResult::ReachesTarget,
            classify::HitClassification::AncestorOfTarget => HitTestResult::Unknown,
            classify::HitClassification::Unrelated => {
                if demote_for_viewport {
                    HitTestResult::Unknown
                } else {
                    corroborate()
                }
            }
        }
    }

    fn read_target_bounds(target: &UIAElement) -> Result<Option<Rect>, AdapterError> {
        match target.0.get_bounding_rectangle() {
            Ok(rectangle) => Ok(Some(Rect {
                x: f64::from(rectangle.get_left()),
                y: f64::from(rectangle.get_top()),
                width: crate::tree::properties::extent(rectangle.get_left(), rectangle.get_right()),
                height: crate::tree::properties::extent(
                    rectangle.get_top(),
                    rectangle.get_bottom(),
                ),
            })),
            Err(error) => match pre_read_fate(failure_of(&error)) {
                PreReadFate::Unknown => Ok(None),
                PreReadFate::Escape(error) => Err(error),
            },
        }
    }

    #[derive(Debug)]
    enum PreReadFate {
        Unknown,
        Escape(AdapterError),
    }

    fn pre_read_fate(failure: UiaFailure) -> PreReadFate {
        let error = uia_failure_error(failure, "read hit-test target bounds");
        match uia_failure_disposition(failure) {
            ReadDisposition::SettledAbsence => PreReadFate::Unknown,
            ReadDisposition::Retryable | ReadDisposition::Unavailable => PreReadFate::Escape(error),
            ReadDisposition::Terminal if error.code == ErrorCode::PermDenied => {
                PreReadFate::Escape(error)
            }
            ReadDisposition::Terminal => PreReadFate::Unknown,
        }
    }

    fn pre_probe_guard(
        bounds: &Rect,
        point: &Point,
        target: &UIAElement,
        client: &UIAutomation,
    ) -> Option<HitTestResult> {
        if bounds.width <= 0.0 || bounds.height <= 0.0 {
            return Some(HitTestResult::Unknown);
        }
        if root_is_iconic(target, client).unwrap_or(true) {
            return Some(HitTestResult::Unknown);
        }
        let screen = virtual_screen_rect();
        if !point_in_rect(point, &screen) || !rect_intersects_screen(bounds, &screen) {
            return Some(HitTestResult::Unknown);
        }
        if !point_in_rect(point, bounds) {
            return Some(HitTestResult::Unknown);
        }
        None
    }

    fn probe_element_from_point(client: &UIAutomation, point: &Point) -> Option<UIAElement> {
        let physical = physical_point(point);
        client
            .element_from_point(physical)
            .ok()
            .map(UIAElement::from)
    }

    pub(super) fn physical_point(point: &Point) -> uiautomation::types::Point {
        uiautomation::types::Point::new(saturate_coord(point.x), saturate_coord(point.y))
    }

    pub(super) fn saturate_coord(value: f64) -> i32 {
        if !value.is_finite() {
            return 0;
        }
        if value >= f64::from(i32::MAX) {
            i32::MAX
        } else if value <= f64::from(i32::MIN) {
            i32::MIN
        } else {
            value as i32
        }
    }

    fn virtual_screen_rect() -> Rect {
        let left = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
        let top = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
        let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
        let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
        Rect {
            x: f64::from(left),
            y: f64::from(top),
            width: f64::from(width.max(0)),
            height: f64::from(height.max(0)),
        }
    }

    fn rect_intersects_screen(bounds: &Rect, screen: &Rect) -> bool {
        intersect_screen(bounds, screen).is_some()
    }

    fn intersect_screen(bounds: &Rect, screen: &Rect) -> Option<Rect> {
        super::classify::intersect_rects(*bounds, *screen)
    }

    fn root_is_iconic(target: &UIAElement, client: &UIAutomation) -> Option<bool> {
        let walker = client.get_raw_view_walker().ok()?;
        let root = corroborate::element_root_hwnd(target, &walker)?;
        Some(unsafe { IsIconic(root as *mut std::ffi::c_void) != 0 })
    }

    fn same_element(client: &UIAutomation, left: &UIAElement, right: &UIAElement) -> bool {
        client.compare_elements(&left.0, &right.0).unwrap_or(false)
    }

    fn element_identity(node: &UIAElement) -> NodeKey {
        match node.0.get_runtime_id() {
            Ok(runtime_id) if !runtime_id.is_empty() => NodeKey::Runtime(runtime_id),
            _ => NodeKey::Unavailable,
        }
    }

    fn parent_step(walker: &UITreeWalker, node: &UIAElement) -> Result<Option<UIAElement>, ()> {
        match walker.get_parent(&node.0) {
            Ok(parent) => Ok(Some(UIAElement::from(parent))),
            Err(error) => {
                if failure_of(&error).is_exhaustion() {
                    Ok(None)
                } else {
                    Err(())
                }
            }
        }
    }

    fn nearest_scroll_viewport_bounds(target: &UIAElement, walker: &UITreeWalker) -> Option<Rect> {
        let mut current = target.clone();
        for _ in 0..super::classify::ancestry_limit() {
            let parent = match parent_step(walker, &current) {
                Ok(Some(parent)) => parent,
                Ok(None) | Err(()) => return None,
            };
            if read_one(&parent, TreeProperty::ScrollAvailable).flag() == Some(true) {
                return match read_one(&parent, TreeProperty::BoundingRectangle).bounds() {
                    agent_desktop_core::LocatorField::Known(bounds)
                        if bounds.width > 0.0 && bounds.height > 0.0 =>
                    {
                        Some(bounds)
                    }
                    _ => None,
                };
            }
            current = parent;
        }
        None
    }

    #[cfg(test)]
    pub(super) fn pre_read_fate_for_test(failure: UiaFailure) -> Result<(), AdapterError> {
        match pre_read_fate(failure) {
            PreReadFate::Unknown => Ok(()),
            PreReadFate::Escape(error) => Err(error),
        }
    }

    #[cfg(test)]
    pub(super) fn guard_zero_area(bounds: &Rect) -> bool {
        bounds.width <= 0.0 || bounds.height <= 0.0
    }

    #[cfg(test)]
    pub(super) fn guard_point_outside_bounds(point: &Point, bounds: &Rect) -> bool {
        !point_in_rect(point, bounds)
    }

    #[cfg(test)]
    pub(super) fn guard_outside_virtual_screen(point: &Point, bounds: &Rect) -> bool {
        let screen = virtual_screen_rect();
        !point_in_rect(point, &screen) || !rect_intersects_screen(bounds, &screen)
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use super::{AdapterError, Deadline, HitTestResult, NativeHandle, Point};

    pub fn hit_test_impl(
        _handle: &NativeHandle,
        _point: Point,
        _deadline: Deadline,
    ) -> Result<HitTestResult, AdapterError> {
        Err(AdapterError::not_supported("hit_test"))
    }
}

#[cfg(target_os = "windows")]
pub(crate) use imp::hit_test_impl;

#[cfg(not(target_os = "windows"))]
pub(crate) use imp::hit_test_impl;

#[cfg(all(test, target_os = "windows"))]
#[path = "hit_test_tests.rs"]
mod tests;

#[cfg(all(test, target_os = "windows"))]
#[path = "hit_test_live_tests.rs"]
mod live_tests;

#[cfg(all(test, target_os = "windows"))]
#[path = "envelope_tests.rs"]
mod envelope_tests;

#[cfg(all(test, target_os = "windows"))]
#[path = "envelope_live_tests.rs"]
mod envelope_live_tests;
