use super::classify::{
    Ancestry, HitClassification, ancestry_with, classify_hit_with, classify_relation,
    remember_ancestor_key, should_demote_outside_viewport,
};
use super::hit_test_impl;
use super::imp::{
    guard_outside_virtual_screen, guard_point_outside_bounds, guard_zero_area, physical_point,
    pre_read_fate_for_test, resolve_classification, saturate_coord,
};
use crate::system::hresult::{E_ACCESSDENIED, E_FAIL, UIA_E_NOTSUPPORTED, UIA_E_TIMEOUT};
use crate::tree::automation::{ERR_INVALID_ARG, ERR_TIMEOUT, UiaFailure, root_from_hwnd};
use crate::tree::fixture::{LocalFixture, ensure_test_apartment};
use crate::tree::fixture_window;
use crate::tree::walker::NodeKey;
use crate::tree::walker_fake::deadline;
use agent_desktop_core::{AdapterError, ErrorCode, Point, Rect, hit_test::HitTestResult};
use std::cell::Cell;
use std::collections::HashMap;

#[test]
fn self_hit_reaches_target() {
    assert_eq!(
        classify_relation(true, false),
        HitClassification::ReachesTarget
    );
}

#[test]
fn descendant_relation_reaches_target() {
    assert_eq!(
        classify_relation(true, true),
        HitClassification::ReachesTarget
    );
}

#[test]
fn ancestor_hit_classifies_unknown_not_intercepted() {
    let classification = classify_relation(false, true);
    assert_eq!(classification, HitClassification::AncestorOfTarget);
    let result = resolve_classification(classification, false, intercepted_stub);
    assert_eq!(result, HitTestResult::Unknown);
    assert!(
        !matches!(result, HitTestResult::InterceptedBy { .. }),
        "ancestor arm must never invent InterceptedBy"
    );
}

#[test]
fn unrelated_hit_reaches_corroboration_seam() {
    let called = Cell::new(false);
    let result = resolve_classification(HitClassification::Unrelated, false, || {
        called.set(true);
        HitTestResult::Unknown
    });
    assert!(
        called.get(),
        "unrelated hits must invoke the corroboration seam"
    );
    assert_eq!(result, HitTestResult::Unknown);
}

#[test]
fn classify_hit_self_and_descendant_and_ancestor_arms() {
    let parents: HashMap<i32, i32> = [(2, 1), (3, 1)].into_iter().collect();
    let parent_of = |node: &i32| match parents.get(node) {
        Some(parent) => Ok(Some(*parent)),
        None => Ok(None),
    };
    let same = |left: &i32, right: &i32| left == right;
    let identity = |node: &i32| NodeKey::Runtime(vec![*node]);

    assert_eq!(
        classify_hit_with(&1, &1, &same, &identity, &parent_of),
        Some(HitClassification::ReachesTarget)
    );
    assert_eq!(
        classify_hit_with(&1, &2, &same, &identity, &parent_of),
        Some(HitClassification::ReachesTarget)
    );
    assert_eq!(
        classify_hit_with(&2, &1, &same, &identity, &parent_of),
        Some(HitClassification::AncestorOfTarget)
    );
    assert_eq!(
        classify_hit_with(&2, &3, &same, &identity, &parent_of),
        Some(HitClassification::Unrelated)
    );
}

#[test]
fn viewport_demotion_skips_corroboration_seam() {
    let called = Cell::new(false);
    let result = resolve_classification(HitClassification::Unrelated, true, || {
        called.set(true);
        intercepted_stub()
    });
    assert!(!called.get());
    assert_eq!(result, HitTestResult::Unknown);
}

#[test]
fn zero_area_guard_is_unknown_not_intercept_or_err() {
    let bounds = Rect {
        x: 10.0,
        y: 10.0,
        width: 0.0,
        height: 20.0,
    };
    assert!(guard_zero_area(&bounds));
    assert_ne!(
        HitTestResult::Unknown,
        HitTestResult::InterceptedBy {
            role: Some("desktop".into()),
            name: None,
            bounds: None,
        }
    );
}

#[test]
fn point_outside_bounds_guard_is_unknown() {
    let bounds = Rect {
        x: 100.0,
        y: 100.0,
        width: 40.0,
        height: 40.0,
    };
    let point = Point { x: 10.0, y: 10.0 };
    assert!(guard_point_outside_bounds(&point, &bounds));
}

#[test]
fn virtual_screen_guard_rejects_freed_coordinates() {
    let bounds = Rect {
        x: -32_000.0,
        y: -32_000.0,
        width: 100.0,
        height: 100.0,
    };
    let point = Point {
        x: -32_000.0,
        y: -32_000.0,
    };
    assert!(guard_outside_virtual_screen(&point, &bounds));
}

#[test]
fn demotion_outside_target_viewport_intersection() {
    let target = Rect {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 200.0,
    };
    let viewport = Rect {
        x: 0.0,
        y: 50.0,
        width: 100.0,
        height: 100.0,
    };
    let outside = Point { x: 50.0, y: 10.0 };
    let inside = Point { x: 50.0, y: 80.0 };
    assert!(should_demote_outside_viewport(
        &outside,
        &target,
        Some(&viewport)
    ));
    assert!(!should_demote_outside_viewport(
        &inside,
        &target,
        Some(&viewport)
    ));
    assert!(!should_demote_outside_viewport(&outside, &target, None));
}

#[test]
fn ancestry_cycle_terminates_as_incomplete_never_hangs() {
    let parents: HashMap<i32, i32> = [(1, 2), (2, 1)].into_iter().collect();
    let steps = Cell::new(0);
    let result = ancestry_with(
        &1,
        &99,
        50,
        &|left, right| left == right,
        &|node| NodeKey::Runtime(vec![*node]),
        &|node| {
            steps.set(steps.get() + 1);
            assert!(steps.get() <= 60, "cycle walk must not hang");
            Ok(Some(parents[node]))
        },
    );
    assert_eq!(result, Ancestry::Incomplete);
}

#[test]
fn fake_probe_failure_shape_is_unknown_not_err() {
    let probe_failed = true;
    let outcome: Result<HitTestResult, AdapterError> = if probe_failed {
        Ok(HitTestResult::Unknown)
    } else {
        Err(AdapterError::new(
            ErrorCode::Timeout,
            "probe failures must not escape as Err",
        ))
    };
    assert_eq!(outcome.unwrap(), HitTestResult::Unknown);
}

#[test]
fn settled_absent_pre_read_demotes_to_unknown() {
    assert!(pre_read_fate_for_test(UiaFailure::Hresult(UIA_E_NOTSUPPORTED)).is_ok());
    assert!(pre_read_fate_for_test(UiaFailure::Sentinel(ERR_INVALID_ARG)).is_ok());
}

#[test]
fn terminal_unclassified_pre_read_demotes_to_unknown() {
    assert!(pre_read_fate_for_test(UiaFailure::Hresult(E_FAIL)).is_ok());
}

#[test]
fn transport_retryable_pre_read_escapes_as_err() {
    let error = pre_read_fate_for_test(UiaFailure::Hresult(UIA_E_TIMEOUT))
        .expect_err("retryable pre-read must escape");
    assert!(error.is_explicitly_retryable());
    let error = pre_read_fate_for_test(UiaFailure::Sentinel(ERR_TIMEOUT))
        .expect_err("timeout sentinel must escape");
    assert!(error.is_explicitly_retryable());
}

#[test]
fn permission_pre_read_escapes_as_err() {
    let error = pre_read_fate_for_test(UiaFailure::Hresult(E_ACCESSDENIED))
        .expect_err("permission denial must escape");
    assert_eq!(error.code, ErrorCode::PermDenied);
}

#[test]
fn dead_token_preamble_escapes_as_stale_reader_err() {
    ensure_test_apartment();
    let fixture = LocalFixture::create().expect("off-screen fixture starts");
    let root = root_from_hwnd(fixture.handle(), deadline()).expect("fixture root");
    let handle = root
        .with_verified_process(std::process::id(), "dead-token-for-hit-test".into())
        .into_native_handle();
    let error = hit_test_impl(
        &handle,
        Point {
            x: f64::from(fixture_window::OFFSCREEN_LEFT + 20),
            y: f64::from(fixture_window::OFFSCREEN_TOP + 20),
        },
        deadline(),
    )
    .expect_err("a dead verified token must escape");
    assert_eq!(error.code, ErrorCode::StaleRef);
}

#[test]
fn saturating_physical_point_never_panics() {
    assert_eq!(saturate_coord(f64::from(i32::MAX) + 10.0), i32::MAX);
    assert_eq!(saturate_coord(f64::from(i32::MIN) - 10.0), i32::MIN);
    let point = physical_point(&Point { x: 12.7, y: -3.2 });
    assert_eq!(point.get_x(), 12);
    assert_eq!(point.get_y(), -3);
}

#[test]
fn remember_ancestor_detects_runtime_id_cycles() {
    let mut keys = Vec::new();
    let mut unkeyed: Vec<i32> = Vec::new();
    let same = |left: &i32, right: &i32| left == right;
    assert!(remember_ancestor_key(
        &mut keys,
        &mut unkeyed,
        NodeKey::Runtime(vec![1, 2]),
        &1,
        &same
    ));
    assert!(!remember_ancestor_key(
        &mut keys,
        &mut unkeyed,
        NodeKey::Runtime(vec![1, 2]),
        &1,
        &same
    ));
}

fn intercepted_stub() -> HitTestResult {
    HitTestResult::InterceptedBy {
        role: Some("pane".into()),
        name: None,
        bounds: None,
    }
}
