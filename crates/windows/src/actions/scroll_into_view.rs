//! `ScrollItemPattern.ScrollIntoView` with delivery judged by observation.
//!
//! Census reality (A18-1): `ScrollItem` is rare — mostly WPF `ListItem` /
//! `TreeItem` — so the unsupported arm is the common case and must stay an
//! honest `ACTION_FAILED` `not_delivered` rather than `PLATFORM_NOT_SUPPORTED`.
//! Provider rects are unclipped against their scroll viewport (A18-2), so
//! verified visibility requires viewport intersection, not `IsOffscreen` alone.
//!
//! The write's HRESULT is diagnostic `platform_detail` only: the read-path
//! classifier is never consulted here.

use agent_desktop_core::{
    AdapterError, Deadline, DeliverySemantics, ErrorCode, InteractionLease, Rect,
};

/// Reports whether a provider rectangle is measurable and encloses real area.
///
/// Finiteness is part of the test rather than decoration. A provider that
/// answers `NaN` or an infinity satisfies a bare positive-dimension comparison,
/// and accepting that rectangle turns an unmeasurable answer into a verified
/// one — the exact false positive the post-invoke re-read exists to prevent.
///
/// Crate-visible because the scroll-viewport ancestor walk is not the only
/// reader that must decide whether a rectangle can be trusted; a second copy of
/// this predicate is a second chance for the two to disagree.
#[cfg(target_os = "windows")]
pub(crate) fn rect_has_area(rect: Rect) -> bool {
    rect.x.is_finite()
        && rect.y.is_finite()
        && rect.width.is_finite()
        && rect.height.is_finite()
        && rect.width > 0.0
        && rect.height > 0.0
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{
        AdapterError, Deadline, DeliverySemantics, ErrorCode, InteractionLease, Rect, rect_has_area,
    };
    use crate::system::hresult::com_hresult_detail;
    use crate::system::permissions::ensure_budget;
    use crate::tree::automation::{ERR_NONE, UiaFailure, automation_client, failure_of};
    use crate::tree::element::{UIAElement, uia_element};
    use crate::tree::live_read::corroborate_verified_process;
    use crate::tree::properties::read_one;
    use crate::tree::property_ids::TreeProperty;
    use crate::tree::property_outcome::{PropertyOutcome, PropertyValue};
    use crate::tree::walker::DEFAULT_MAX_RAW_DEPTH;
    use agent_desktop_core::LocatorField;
    use agent_desktop_core::native_handle::NativeHandle;
    use std::time::{Duration, Instant};
    use uiautomation::core::UITreeWalker;
    use uiautomation::patterns::UIScrollItemPattern;

    const VERIFY_WINDOW: Duration = Duration::from_millis(800);
    const POLL_SLICE: Duration = Duration::from_millis(20);

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum InvokeOutcome {
        Succeeded,
        Failed(i32),
        EmptyPattern,
    }

    #[derive(Debug, Clone, Copy)]
    pub(crate) struct VisibilitySample {
        pub(crate) bounds: Option<Rect>,
        pub(crate) offscreen: Option<bool>,
        pub(crate) viewport: Option<Rect>,
    }

    pub fn scroll_into_view_impl(
        handle: &NativeHandle,
        lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        let element = uia_element(handle)?;
        let deadline = lease.deadline();
        ensure_budget(deadline)?;
        corroborate_verified_process(element)?;
        scroll_into_view_element(element, deadline)
    }

    fn scroll_into_view_element(
        element: &UIAElement,
        deadline: Deadline,
    ) -> Result<(), AdapterError> {
        if !scroll_item_available(element) {
            return Err(unsupported_error());
        }
        let client = automation_client()?;
        let walker = client.get_raw_view_walker().ok();
        let before = read_bounds_opt(element);
        let invoke_hr = match invoke_scroll_into_view(element) {
            InvokeOutcome::EmptyPattern => return Err(unsupported_error()),
            InvokeOutcome::Failed(hresult) => Some(hresult),
            InvokeOutcome::Succeeded => None,
        };
        scroll_into_view_judged_for(deadline, before, invoke_hr, VERIFY_WINDOW, || {
            observe_visibility(element, deadline, walker.as_ref())
        })
    }

    pub(crate) fn scroll_into_view_judged_for(
        deadline: Deadline,
        before: Option<Rect>,
        invoke_hr: Option<i32>,
        verify_window: Duration,
        mut observe: impl FnMut() -> Result<VisibilitySample, AdapterError>,
    ) -> Result<(), AdapterError> {
        let local_end = Instant::now() + verify_window;
        let mut last_bounds: Option<Rect>;
        loop {
            match observe() {
                Ok(sample) => {
                    last_bounds = sample.bounds;
                    if visibility_verified(&sample) {
                        return Ok(());
                    }
                }
                Err(error) => return Err(after_delivery(error)),
            }
            if deadline.is_expired() {
                return Err(after_delivery(deadline.timeout_error().with_details(
                    serde_json::json!({
                        "verification": "scroll_visibility_not_observed",
                    }),
                )));
            }
            if Instant::now() >= local_end {
                return finish_observation(before, last_bounds, invoke_hr);
            }
            let pause = deadline
                .remaining_slice(POLL_SLICE)
                .map_err(after_delivery)?;
            std::thread::sleep(pause.min(POLL_SLICE));
        }
    }

    pub(crate) fn finish_observation(
        before: Option<Rect>,
        after: Option<Rect>,
        invoke_hr: Option<i32>,
    ) -> Result<(), AdapterError> {
        let Some(after) = completed_bounds(after) else {
            return Err(unverified_error(
                "ScrollIntoView completed without an observable after-state",
                invoke_hr,
            ));
        };
        let Some(before) = completed_bounds(before) else {
            return Err(unverified_error(
                "ScrollIntoView completed without a comparable before-state",
                invoke_hr,
            ));
        };
        if !scroll_effect_observed(before, after) {
            return Err(not_delivered_error(invoke_hr));
        }
        Err(unverified_error(
            "ScrollIntoView moved the target but visibility was not verified",
            invoke_hr,
        ))
    }

    pub(crate) fn visibility_verified(sample: &VisibilitySample) -> bool {
        let Some(bounds) = sample.bounds else {
            return false;
        };
        if !rect_has_area(bounds) {
            return false;
        }
        if sample.offscreen != Some(false) {
            return false;
        }
        match sample.viewport {
            Some(viewport) => intersects(bounds, viewport),
            None => false,
        }
    }

    pub(crate) fn intersects(left: Rect, right: Rect) -> bool {
        left.x < right.x + right.width
            && left.x + left.width > right.x
            && left.y < right.y + right.height
            && left.y + left.height > right.y
    }

    pub(crate) fn scroll_effect_observed(before: Rect, after: Rect) -> bool {
        before.bounds_hash() != after.bounds_hash()
    }

    pub(crate) fn unsupported_error() -> AdapterError {
        AdapterError::new(
            ErrorCode::ActionFailed,
            "ScrollIntoView is not available on this element",
        )
        .with_suggestion(
            "The element does not expose ScrollItemPattern; scroll a containing viewport first, or target a scroll-item control",
        )
        .with_details(serde_json::json!({
            "kind": "scroll_into_view_unsupported",
            "complete": true,
            "retryable": false,
        }))
        .with_disposition(DeliverySemantics::not_delivered())
    }

    fn not_delivered_error(invoke_hr: Option<i32>) -> AdapterError {
        attach_invoke_detail(
            AdapterError::new(
                ErrorCode::ActionFailed,
                "ScrollIntoView did not change target geometry",
            )
            .with_disposition(DeliverySemantics::not_delivered()),
            invoke_hr,
        )
    }

    fn unverified_error(message: &str, invoke_hr: Option<i32>) -> AdapterError {
        attach_invoke_detail(
            AdapterError::new(ErrorCode::ActionFailed, message)
                .with_disposition(DeliverySemantics::delivered_unverified()),
            invoke_hr,
        )
    }

    fn after_delivery(error: AdapterError) -> AdapterError {
        error.with_disposition(DeliverySemantics::delivered_unverified())
    }

    /// Formats a failed write's HRESULT for `platform_detail` only.
    ///
    /// Deliberately does not call the read-path HRESULT classifier: that table
    /// is forbidden on the write path, and delivery stays observation-derived.
    fn attach_invoke_detail(error: AdapterError, invoke_hr: Option<i32>) -> AdapterError {
        match invoke_hr {
            Some(hresult) => error.with_platform_detail(com_hresult_detail(hresult)),
            None => error,
        }
    }

    fn completed_bounds(bounds: Option<Rect>) -> Option<Rect> {
        bounds.filter(|rect| rect_has_area(*rect))
    }

    fn scroll_item_available(element: &UIAElement) -> bool {
        read_one(element, TreeProperty::ScrollItemAvailable).flag() == Some(true)
    }

    fn invoke_scroll_into_view(element: &UIAElement) -> InvokeOutcome {
        match element.0.get_pattern::<UIScrollItemPattern>() {
            Ok(pattern) => match pattern.scroll_into_view() {
                Ok(()) => InvokeOutcome::Succeeded,
                Err(error) => match failure_of(&error) {
                    UiaFailure::Hresult(hresult) => InvokeOutcome::Failed(hresult),
                    UiaFailure::Sentinel(ERR_NONE) => InvokeOutcome::EmptyPattern,
                    UiaFailure::Sentinel(code) => InvokeOutcome::Failed(code),
                },
            },
            Err(error) => match failure_of(&error) {
                UiaFailure::Sentinel(ERR_NONE) => InvokeOutcome::EmptyPattern,
                other if other.is_exhaustion() => InvokeOutcome::EmptyPattern,
                UiaFailure::Hresult(hresult) => InvokeOutcome::Failed(hresult),
                UiaFailure::Sentinel(code) => InvokeOutcome::Failed(code),
            },
        }
    }

    fn observe_visibility(
        element: &UIAElement,
        deadline: Deadline,
        walker: Option<&UITreeWalker>,
    ) -> Result<VisibilitySample, AdapterError> {
        ensure_budget(deadline)?;
        corroborate_verified_process(element)?;
        let bounds = match read_one(element, TreeProperty::BoundingRectangle).bounds() {
            LocatorField::Known(bounds) => Some(bounds),
            LocatorField::Absent => None,
            LocatorField::Unknown => {
                return Err(AdapterError::new(
                    ErrorCode::ActionFailed,
                    "Could not re-read target bounds after ScrollIntoView",
                ));
            }
        };
        let offscreen = match read_one(element, TreeProperty::IsOffscreen) {
            PropertyOutcome::Known(PropertyValue::Flag(flag)) => Some(flag),
            PropertyOutcome::Absent => None,
            PropertyOutcome::Unknown => {
                return Err(AdapterError::new(
                    ErrorCode::ActionFailed,
                    "Could not re-read IsOffscreen after ScrollIntoView",
                ));
            }
            PropertyOutcome::Known(_) => None,
        };
        let viewport = walker.and_then(|walker| nearest_scroll_viewport_bounds(element, walker));
        Ok(VisibilitySample {
            bounds,
            offscreen,
            viewport,
        })
    }

    fn read_bounds_opt(element: &UIAElement) -> Option<Rect> {
        match read_one(element, TreeProperty::BoundingRectangle).bounds() {
            LocatorField::Known(bounds) => Some(bounds),
            _ => None,
        }
    }

    fn nearest_scroll_viewport_bounds(target: &UIAElement, walker: &UITreeWalker) -> Option<Rect> {
        let mut current = target.clone();
        for _ in 0..DEFAULT_MAX_RAW_DEPTH as usize {
            let parent = match walker.get_parent(&current.0) {
                Ok(parent) => UIAElement::from(parent),
                Err(_) => return None,
            };
            if read_one(&parent, TreeProperty::ScrollAvailable).flag() == Some(true) {
                return match read_one(&parent, TreeProperty::BoundingRectangle).bounds() {
                    LocatorField::Known(bounds) if rect_has_area(bounds) => Some(bounds),
                    _ => None,
                };
            }
            current = parent;
        }
        None
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use super::{AdapterError, InteractionLease};
    use agent_desktop_core::native_handle::NativeHandle;

    pub fn scroll_into_view_impl(
        _handle: &NativeHandle,
        _lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("scroll_into_view"))
    }
}

pub(crate) use imp::scroll_into_view_impl;

#[cfg(all(test, target_os = "windows"))]
pub(crate) use imp::{
    VisibilitySample, finish_observation, scroll_effect_observed, scroll_into_view_judged_for,
    unsupported_error, visibility_verified,
};

#[cfg(all(test, target_os = "windows"))]
#[path = "scroll_into_view_tests.rs"]
mod tests;
