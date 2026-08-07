use super::{MAX_ANCESTOR_SCROLLS, apply_ladder_seam, direction_for_visibility, ladder_judged_for};
use crate::actions::chain::DeliveryOutcome;
use crate::actions::scroll_into_view::{VisibilitySample, visibility_verified};
use agent_desktop_core::{
    AdapterError, Deadline, DeliveryDisposition, DeliverySemantics, Direction, ErrorCode, Rect,
};
use std::cell::Cell;

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

fn deadline() -> Deadline {
    Deadline::after(5_000).expect("deadline")
}

#[test]
fn direction_priority_matches_macos_vertical_before_horizontal() {
    let viewport = rect(100.0, 100.0, 200.0, 200.0);
    assert!(matches!(
        direction_for_visibility(rect(150.0, 50.0, 40.0, 20.0), viewport),
        Some(Direction::Up)
    ));
    assert!(matches!(
        direction_for_visibility(rect(150.0, 350.0, 40.0, 20.0), viewport),
        Some(Direction::Down)
    ));
    assert!(matches!(
        direction_for_visibility(rect(40.0, 150.0, 40.0, 20.0), viewport),
        Some(Direction::Left)
    ));
    assert!(matches!(
        direction_for_visibility(rect(350.0, 150.0, 40.0, 20.0), viewport),
        Some(Direction::Right)
    ));
    assert!(direction_for_visibility(rect(120.0, 120.0, 40.0, 20.0), viewport).is_none());
}

#[test]
fn vertical_edge_wins_when_both_axes_are_out() {
    let viewport = rect(0.0, 0.0, 100.0, 100.0);
    assert!(matches!(
        direction_for_visibility(rect(200.0, 200.0, 10.0, 10.0), viewport),
        Some(Direction::Down)
    ));
    assert!(matches!(
        direction_for_visibility(rect(-20.0, -20.0, 10.0, 10.0), viewport),
        Some(Direction::Up)
    ));
}

#[test]
fn visible_at_rung_zero_is_satisfied_no_delivery() {
    let scrolls = Cell::new(0u8);
    let mut next = || Ok(None);
    let mut scroll = |_: &Direction| {
        scrolls.set(scrolls.get() + 1);
        Ok(())
    };
    let outcome = ladder_judged_for(deadline(), &mut next, &mut scroll).expect("visible");
    assert_eq!(outcome, DeliveryOutcome::SatisfiedNoDelivery);
    assert_eq!(scrolls.get(), 0);
}

#[test]
fn visible_after_n_rungs_is_delivered_verified() {
    let scrolls = Cell::new(0u8);
    let mut next = || {
        if scrolls.get() < 3 {
            Ok(Some(Direction::Down))
        } else {
            Ok(None)
        }
    };
    let mut scroll = |_: &Direction| {
        scrolls.set(scrolls.get() + 1);
        Ok(())
    };
    let outcome = ladder_judged_for(deadline(), &mut next, &mut scroll).expect("ladder");
    assert_eq!(outcome, DeliveryOutcome::DeliveredVerified);
    assert_eq!(scrolls.get(), 3);
}

#[test]
fn unclipped_below_fold_does_not_verify_without_intersection() {
    let viewport = rect(0.0, 0.0, 100.0, 100.0);
    let below_fold = VisibilitySample {
        bounds: Some(rect(10.0, 150.0, 80.0, 200.0)),
        offscreen: Some(false),
        viewport: Some(viewport),
    };
    assert_eq!(below_fold.offscreen, Some(false));
    assert!(!visibility_verified(&below_fold));
    let seen = Cell::new(false);
    let mut next = || {
        if visibility_verified(&below_fold) {
            Ok(None)
        } else {
            seen.set(true);
            Ok(Some(Direction::Down))
        }
    };
    let calls = Cell::new(0u8);
    let mut scroll = |_: &Direction| {
        calls.set(calls.get() + 1);
        Ok(())
    };
    let error = ladder_judged_for(deadline(), &mut next, &mut scroll).expect_err("below-fold");
    assert!(seen.get());
    assert_eq!(calls.get(), MAX_ANCESTOR_SCROLLS as u8);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::DeliveredUnverified
    );
}

#[test]
fn delivered_verified_requires_intersection_signal() {
    let viewport = rect(0.0, 0.0, 100.0, 100.0);
    let hidden = VisibilitySample {
        bounds: Some(rect(10.0, 200.0, 20.0, 20.0)),
        offscreen: Some(false),
        viewport: Some(viewport),
    };
    let shown = VisibilitySample {
        bounds: Some(rect(10.0, 40.0, 20.0, 20.0)),
        offscreen: Some(false),
        viewport: Some(viewport),
    };
    assert!(!visibility_verified(&hidden));
    assert!(visibility_verified(&shown));
    let scrolls = Cell::new(0u8);
    let mut next = || {
        if scrolls.get() < 2 {
            assert!(!visibility_verified(&hidden));
            Ok(Some(Direction::Down))
        } else {
            assert!(visibility_verified(&shown));
            Ok(None)
        }
    };
    let mut scroll = |_: &Direction| {
        scrolls.set(scrolls.get() + 1);
        Ok(())
    };
    let outcome = ladder_judged_for(deadline(), &mut next, &mut scroll).expect("verified");
    assert_eq!(outcome, DeliveryOutcome::DeliveredVerified);
}

#[test]
fn ten_rungs_exhausted_is_delivered_unverified() {
    let calls = Cell::new(0u8);
    let mut next = || Ok(Some(Direction::Down));
    let mut scroll = |_: &Direction| {
        calls.set(calls.get() + 1);
        Ok(())
    };
    let error = ladder_judged_for(deadline(), &mut next, &mut scroll).expect_err("exhausted");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert_eq!(calls.get(), MAX_ANCESTOR_SCROLLS as u8);
}

#[test]
fn absence_ok_false_does_not_count_as_scroll_delivery() {
    let error = super::require_scroll_delivery(Ok(false)).expect_err("absence");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
    assert!(super::require_scroll_delivery(Ok(true)).is_ok());
}

#[test]
fn classified_write_error_aborts_without_further_scrolls() {
    let calls = Cell::new(0u8);
    let mut next = || Ok(Some(Direction::Down));
    let mut scroll = |_: &Direction| {
        calls.set(calls.get() + 1);
        Err(AdapterError::new(ErrorCode::PermDenied, "denied write")
            .with_disposition(DeliverySemantics::not_delivered()))
    };
    let error = ladder_judged_for(deadline(), &mut next, &mut scroll).expect_err("denied");
    assert_eq!(error.code, ErrorCode::PermDenied);
    assert_eq!(calls.get(), 1);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
}

#[test]
fn deadline_at_rung_zero_stays_not_delivered() {
    let expired = Deadline::after(1).expect("deadline");
    std::thread::sleep(std::time::Duration::from_millis(5));
    let calls = Cell::new(0u8);
    let mut next = || Ok(Some(Direction::Down));
    let mut scroll = |_: &Direction| {
        calls.set(calls.get() + 1);
        Ok(())
    };
    let error = ladder_judged_for(expired, &mut next, &mut scroll).expect_err("timeout");
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(calls.get(), 0);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
}

#[test]
fn deadline_after_scrolls_is_delivered_unverified() {
    let calls = Cell::new(0u8);
    let short = Deadline::after(30).expect("deadline");
    let mut next = || Ok(Some(Direction::Down));
    let mut scroll = |_: &Direction| {
        calls.set(calls.get() + 1);
        std::thread::sleep(std::time::Duration::from_millis(20));
        Ok(())
    };
    let error = ladder_judged_for(short, &mut next, &mut scroll).expect_err("mid-ladder");
    assert!(calls.get() >= 1);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::DeliveredUnverified
    );
}

#[test]
fn seam_no_ancestor_keeps_unsupported_terminal() {
    let fallback = crate::actions::scroll_into_view::unsupported_error();
    let resolved = apply_ladder_seam(fallback.clone(), Ok(None)).expect_err("no ancestor");
    assert_eq!(resolved.code, fallback.code);
    assert_eq!(resolved.disposition, fallback.disposition);
    assert_eq!(
        resolved.details.as_ref().and_then(|d| d.get("kind")),
        Some(&serde_json::json!("scroll_into_view_unsupported"))
    );
}

#[test]
fn seam_with_ancestor_ladders_instead_of_unsupported() {
    let fallback = crate::actions::scroll_into_view::unsupported_error();
    let outcome =
        apply_ladder_seam(fallback, Ok(Some(DeliveryOutcome::DeliveredVerified))).expect("ladder");
    assert_eq!(outcome, DeliveryOutcome::DeliveredVerified);
}
