use super::{
    finish_observation, rect_has_area, scroll_effect_observed, scroll_into_view_judged_for,
    unsupported_error, visibility_verified, VisibilitySample,
};
use crate::system::hresult::{
    UIA_E_ELEMENTNOTAVAILABLE, classify_read_hresult, com_hresult_detail, hresult_record,
};
use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode, Rect};
use std::cell::Cell;
use std::time::Duration;

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
    assert!(visibility_verified(&VisibilitySample {
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
    assert_ne!(
        error.disposition,
        DeliverySemantics::delivered_unverified()
    );
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
    assert_ne!(
        error.disposition,
        DeliverySemantics::delivered_unverified()
    );
}

#[test]
fn moved_but_unproven_is_delivered_unverified_not_not_delivered() {
    let before = rect(50.0, 300.0, 30.0, 20.0);
    let after = rect(50.0, 40.0, 30.0, 20.0);
    assert!(scroll_effect_observed(before, after));
    let error = finish_observation(Some(before), Some(after), None).expect_err("unproven");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.disposition,
        DeliverySemantics::delivered_unverified()
    );
    assert_ne!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn observation_failure_is_delivered_unverified_never_bare_read_err() {
    let before = rect(10.0, 10.0, 20.0, 20.0);
    let calls = Cell::new(0);
    let error = scroll_into_view_judged_for(short_deadline(), Some(before), None, Duration::from_millis(800), || {
        calls.set(calls.get() + 1);
        Err(AdapterError::new(
            ErrorCode::StaleRef,
            "provider died during observation",
        )
        .with_details(serde_json::json!({ "complete": false, "retryable": true })))
    })
    .expect_err("observation failure");
    assert_eq!(calls.get(), 1);
    assert_eq!(
        error.disposition,
        DeliverySemantics::delivered_unverified()
    );
    assert_ne!(error.disposition, DeliverySemantics::not_delivered());
    assert_ne!(error.disposition, DeliverySemantics::unknown());
}

#[test]
fn degenerate_after_state_is_delivered_unverified_not_not_delivered() {
    let before = rect(10.0, 10.0, 20.0, 20.0);
    let error = finish_observation(Some(before), Some(rect(0.0, 0.0, 0.0, 0.0)), None)
        .expect_err("degenerate after");
    assert_eq!(
        error.disposition,
        DeliverySemantics::delivered_unverified()
    );
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
    assert_eq!(
        error.disposition,
        DeliverySemantics::delivered_unverified()
    );
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

#[test]
fn write_path_source_never_names_read_classifier() {
    let banned = concat!("classify_read_", "hresult");
    for line in include_str!("scroll_into_view.rs").lines() {
        let is_prose = line.trim_start().starts_with("///") || line.trim_start().starts_with("//!");
        assert!(
            is_prose || !line.contains(banned),
            "the write path must not consult {banned}: {line}"
        );
    }
    assert!(
        include_str!("scroll_into_view.rs").contains("com_hresult_detail"),
        "failed invokes format through com_hresult_detail only"
    );
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
