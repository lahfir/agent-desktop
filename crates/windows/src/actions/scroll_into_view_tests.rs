use super::{
    VisibilitySample, finish_observation, rect_has_area, scroll_effect_observed,
    scroll_into_view_judged_for, unsupported_error, visibility_verified,
};
use crate::adapter::WindowsAdapter;
use crate::system::hresult::{
    UIA_E_ELEMENTNOTAVAILABLE, classify_read_hresult, com_hresult_detail, hresult_record,
};
use crate::tree::automation::automation_client;
use crate::tree::element::UIAElement;
use crate::tree::fixture::{LocalFixture, ensure_test_apartment};
use crate::tree::fixture_window;
use crate::tree::properties::read_one;
use crate::tree::property_ids::TreeProperty;
use crate::tree::property_outcome::{PropertyOutcome, PropertyValue};
use agent_desktop_core::{
    ActionOps, AdapterError, Deadline, DeliveryDisposition, DeliverySemantics, ErrorCode,
    InteractionLease, Rect,
};
use std::cell::Cell;
use std::ffi::c_void;
use std::time::Duration;
use uiautomation::types::Handle;

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

fn visible_in_viewport(bounds: Rect, viewport: Rect) -> VisibilitySample {
    VisibilitySample {
        bounds: Some(bounds),
        offscreen: Some(false),
        viewport: Some(viewport),
    }
}

fn hidden_sample(bounds: Rect) -> VisibilitySample {
    VisibilitySample {
        bounds: Some(bounds),
        offscreen: Some(true),
        viewport: Some(rect(0.0, 0.0, 100.0, 100.0)),
    }
}

fn short_deadline() -> Deadline {
    Deadline::after(2_000).expect("deadline")
}

#[test]
fn area_requires_finite_positive_dimensions() {
    assert!(rect_has_area(rect(0.0, 0.0, 10.0, 10.0)));
    assert!(!rect_has_area(rect(0.0, 0.0, 0.0, 10.0)));
    assert!(!rect_has_area(rect(f64::NAN, 0.0, 10.0, 10.0)));
}

#[test]
fn verified_requires_on_screen_area_and_viewport_intersection() {
    let bounds = rect(10.0, 10.0, 20.0, 20.0);
    let viewport = rect(0.0, 0.0, 100.0, 100.0);
    assert!(visibility_verified(&visible_in_viewport(bounds, viewport)));
    assert!(!visibility_verified(&VisibilitySample {
        bounds: Some(bounds),
        offscreen: Some(true),
        viewport: Some(viewport),
    }));
    assert!(!visibility_verified(&VisibilitySample {
        bounds: Some(rect(200.0, 200.0, 20.0, 20.0)),
        offscreen: Some(false),
        viewport: Some(viewport),
    }));
    assert!(!visibility_verified(&VisibilitySample {
        bounds: Some(bounds),
        offscreen: Some(false),
        viewport: None,
    }));
}

#[test]
fn unsupported_is_action_failed_not_platform_not_supported() {
    let error = unsupported_error();
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_ne!(error.code, ErrorCode::PlatformNotSupported);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_ne!(error.disposition, DeliverySemantics::delivered_unverified());
    let details = error.details.expect("unsupported carries details");
    assert_eq!(details["kind"], "scroll_into_view_unsupported");
    assert_eq!(details["complete"], serde_json::json!(true));
    assert_eq!(details["retryable"], serde_json::json!(false));
    let defaulted = AdapterError::not_supported("scroll_into_view");
    assert_eq!(defaulted.code, ErrorCode::PlatformNotSupported);
    assert_ne!(error.code, defaulted.code);
}

#[test]
fn verified_visible_after_invoke_is_ok() {
    let bounds = rect(10.0, 10.0, 40.0, 20.0);
    let viewport = rect(0.0, 0.0, 200.0, 200.0);
    let result = scroll_into_view_judged_for(
        short_deadline(),
        Some(bounds),
        None,
        Duration::from_millis(800),
        || Ok(visible_in_viewport(bounds, viewport)),
    );
    assert!(result.is_ok());
}

#[test]
fn unchanged_geometry_is_not_delivered_not_unverified() {
    let bounds = rect(50.0, 50.0, 30.0, 20.0);
    let error = finish_observation(Some(bounds), Some(bounds), None).expect_err("unchanged");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_ne!(error.disposition, DeliverySemantics::delivered_unverified());
}

#[test]
fn moved_but_unproven_is_delivered_unverified_not_not_delivered() {
    let before = rect(50.0, 300.0, 30.0, 20.0);
    let after = rect(50.0, 40.0, 30.0, 20.0);
    assert!(scroll_effect_observed(before, after));
    let error = finish_observation(Some(before), Some(after), None).expect_err("unproven");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert_ne!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn observation_failure_is_delivered_unverified_never_bare_read_err() {
    let before = rect(10.0, 10.0, 20.0, 20.0);
    let calls = Cell::new(0);
    let error = scroll_into_view_judged_for(
        short_deadline(),
        Some(before),
        None,
        Duration::from_millis(800),
        || {
            calls.set(calls.get() + 1);
            Err(
                AdapterError::new(ErrorCode::StaleRef, "provider died during observation")
                    .with_details(serde_json::json!({ "complete": false, "retryable": true })),
            )
        },
    )
    .expect_err("observation failure");
    assert_eq!(calls.get(), 1);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert_ne!(error.disposition, DeliverySemantics::not_delivered());
    assert_ne!(error.disposition, DeliverySemantics::unknown());
}

#[test]
fn degenerate_after_state_is_delivered_unverified_not_not_delivered() {
    let before = rect(10.0, 10.0, 20.0, 20.0);
    let error = finish_observation(Some(before), Some(rect(0.0, 0.0, 0.0, 0.0)), None)
        .expect_err("degenerate after");
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert_ne!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn degenerate_before_and_after_is_delivered_unverified() {
    let error = finish_observation(
        Some(rect(0.0, 0.0, 0.0, 0.0)),
        Some(rect(0.0, 0.0, 0.0, 0.0)),
        None,
    )
    .expect_err("degenerate both");
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert_ne!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn failed_invoke_unchanged_geometry_keeps_action_failed_and_platform_detail() {
    let bounds = rect(12.0, 12.0, 40.0, 18.0);
    let hresult = UIA_E_ELEMENTNOTAVAILABLE;
    let classified = hresult_record(hresult);
    assert_eq!(
        classified.code,
        ErrorCode::StaleRef,
        "precondition: the read classifier maps this HRESULT to StaleRef"
    );
    let _ = classify_read_hresult(hresult);
    let error =
        finish_observation(Some(bounds), Some(bounds), Some(hresult)).expect_err("not delivered");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_ne!(error.code, classified.code);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    let expected = com_hresult_detail(hresult);
    assert_eq!(error.platform_detail.as_deref(), Some(expected.as_str()));
}

/// Bans every name that reaches the read-path classification table, not one
/// projection of it.
///
/// `classify_read_hresult` is a one-line projection of `hresult_record`, and
/// `uia_failure_disposition` is that same table reached through the failure
/// enum, so pinning only the projection leaves two unguarded doors into the
/// table the write path is forbidden to consult. The formatting helpers stay
/// allowed on purpose: rendering an HRESULT symbol into `platform_detail` is
/// what the write path is supposed to do with a failed invoke.
///
/// Each name is spelled in halves so this file's own text cannot be mistaken
/// for a violation by a source scan reading either file.
#[test]
fn write_path_source_never_reaches_the_read_classification_table() {
    let banned = [
        concat!("classify_read_", "hresult"),
        concat!("hresult_", "record"),
        concat!("uia_failure_", "disposition"),
    ];
    let source = include_str!("scroll_into_view.rs");
    for line in source.lines() {
        let is_prose = line.trim_start().starts_with("///") || line.trim_start().starts_with("//!");
        for name in banned {
            assert!(
                is_prose || !line.contains(name),
                "the write path must not consult {name}: {line}"
            );
        }
    }
    assert!(
        source.contains("com_hresult_detail"),
        "failed invokes format through com_hresult_detail only"
    );
}

/// Drives the shipped gate against a real provider instead of a fake.
///
/// Every other pin here injects an observation closure or calls the error
/// constructor directly, so the production chain — the gated
/// `ScrollItemAvailable` read, the invoke, and the observation loop behind it —
/// was never executed by the suite at all. A Win32 `BUTTON` exposes no
/// `ScrollItemPattern`, which is the arm the census says is the common case, so
/// it proves the gate reads the property and refuses before invoking, and that
/// the refusal is the honest not-delivered `ACTION_FAILED` rather than the
/// trait default's `PLATFORM_NOT_SUPPORTED`.
#[test]
fn live_element_without_scroll_item_is_unsupported_through_the_adapter() {
    ensure_test_apartment();
    let fixture = LocalFixture::create().expect("a fixture window");
    let button = fixture_window::find_button(fixture.handle());
    assert!(!button.is_null(), "the fixture exposes its BUTTON control");
    let element = fixture_element(button).expect("a UI Automation element for the button");
    assert_eq!(
        read_one(&element, TreeProperty::ScrollItemAvailable),
        PropertyOutcome::Known(PropertyValue::Flag(false)),
        "the gate is a gated property read that genuinely answered false"
    );
    let handle = element.into_native_handle();
    let lease = InteractionLease::guarded(short_deadline(), ()).expect("an interaction lease");
    let error = WindowsAdapter::new()
        .scroll_into_view(&handle, &lease)
        .expect_err("a BUTTON carries no ScrollItemPattern");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_ne!(error.code, ErrorCode::PlatformNotSupported);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
    let details = error.details.expect("the unsupported arm carries details");
    assert_eq!(details["kind"], "scroll_into_view_unsupported");
}

fn fixture_element(control: *mut c_void) -> Option<UIAElement> {
    let client = automation_client().ok()?;
    let element = client
        .element_from_handle(Handle::from(control as isize))
        .ok()?;
    Some(UIAElement::from(element))
}

#[test]
fn zero_verify_window_unchanged_reaches_not_delivered() {
    let bounds = rect(8.0, 8.0, 25.0, 15.0);
    let error = scroll_into_view_judged_for(
        short_deadline(),
        Some(bounds),
        None,
        Duration::from_millis(0),
        || Ok(hidden_sample(bounds)),
    )
    .expect_err("unchanged after zero window");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}
