use super::{
    SCROLL_LABEL, ScrollPlan, axis_name, scroll_effect_verified, scroll_judged_for,
};
use agent_desktop_core::{
    ActionStepOutcome, AdapterError, Deadline, DeliveryDisposition, Direction, ErrorCode, Rect,
};
use std::cell::Cell;

fn deadline() -> Deadline {
    Deadline::after(5_000).expect("deadline")
}

fn plan(amount: u32, axis_scrollable: bool) -> ScrollPlan {
    ScrollPlan {
        scroll_available: true,
        axis_scrollable,
        axis_name: "vertical",
        amount,
    }
}

#[test]
fn down_times_three_issues_three_vertical_small_increments() {
    let calls = Cell::new(0u8);
    let mut scroll_once = || {
        calls.set(calls.get() + 1);
        Ok(())
    };
    let mut observe = || true;
    let steps = scroll_judged_for(deadline(), plan(3, true), &mut scroll_once, &mut observe)
        .expect("scroll");
    assert_eq!(calls.get(), 3);
    assert_eq!(steps[0].label(), SCROLL_LABEL);
    assert_eq!(steps[0].verified(), Some(true));
    assert!(matches!(steps[0].outcome, ActionStepOutcome::Succeeded));
    assert_eq!(axis_name(&Direction::Down), "vertical");
}

#[test]
fn unscrollable_axis_is_not_delivered_and_names_the_axis() {
    let calls = Cell::new(0u8);
    let mut scroll_once = || {
        calls.set(calls.get() + 1);
        Ok(())
    };
    let mut observe = || false;
    let error = scroll_judged_for(
        deadline(),
        ScrollPlan {
            scroll_available: true,
            axis_scrollable: false,
            axis_name: "horizontal",
            amount: 1,
        },
        &mut scroll_once,
        &mut observe,
    )
    .expect_err("axis");
    assert_eq!(calls.get(), 0);
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert!(error.message.contains("horizontal"));
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
    assert_eq!(
        error.details.as_ref().and_then(|d| d.get("axis")),
        Some(&serde_json::json!("horizontal"))
    );
}

#[test]
fn percent_unavailable_falls_back_to_bounds_change() {
    let before = Rect {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let after = Rect {
        x: 0.0,
        y: 4.0,
        width: 10.0,
        height: 10.0,
    };
    assert!(scroll_effect_verified(None, None, Some(before), Some(after)));
    assert!(!scroll_effect_verified(None, None, Some(before), Some(before)));
    assert!(scroll_effect_verified(Some(0.0), Some(10.0), None, None));
    assert!(!scroll_effect_verified(Some(5.0), Some(5.0), Some(before), Some(before)));
}

#[test]
fn scroll_unavailable_is_not_delivered() {
    let mut scroll_once = || Ok(());
    let mut observe = || true;
    let error = scroll_judged_for(
        deadline(),
        ScrollPlan {
            scroll_available: false,
            axis_scrollable: true,
            axis_name: "vertical",
            amount: 1,
        },
        &mut scroll_once,
        &mut observe,
    )
    .expect_err("unavailable");
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
}

#[test]
fn mid_scroll_failure_is_delivered_unverified() {
    let calls = Cell::new(0u8);
    let mut scroll_once = || {
        let next = calls.get() + 1;
        calls.set(next);
        if next >= 2 {
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                "scroll invoke failed",
            ));
        }
        Ok(())
    };
    let mut observe = || false;
    let error = scroll_judged_for(deadline(), plan(3, true), &mut scroll_once, &mut observe)
        .expect_err("partial");
    assert_eq!(calls.get(), 2);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::DeliveredUnverified
    );
}
