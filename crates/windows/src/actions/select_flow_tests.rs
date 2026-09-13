use super::{SelectOps, SelectPlan, select_judged_for};
use crate::actions::chain::DeliveryOutcome;
use agent_desktop_core::{AdapterError, Deadline, DeliveryDisposition, ErrorCode};
use std::cell::Cell;

fn deadline() -> Deadline {
    Deadline::after(5_000).expect("deadline")
}

fn plan(self_match: bool, needs_expand: bool, value_chars: usize) -> SelectPlan {
    SelectPlan {
        self_match,
        needs_expand,
        value_chars,
    }
}

#[test]
fn first_match_still_realizes_before_select() {
    let finds = Cell::new(0u8);
    let realizes = Cell::new(0u8);
    let mut expand = || Ok(());
    let mut collapse = || {};
    let mut find = || {
        finds.set(finds.get() + 1);
        Ok(true)
    };
    let mut realize = || {
        realizes.set(realizes.get() + 1);
        Ok(())
    };
    let mut select_item = || Ok(DeliveryOutcome::DeliveredVerified);
    select_judged_for(
        deadline(),
        plan(false, false, 3),
        SelectOps {
            expand: &mut expand,
            collapse: &mut collapse,
            find: &mut find,
            realize: &mut realize,
            select_item: &mut select_item,
        },
    )
    .expect("select");
    assert_eq!(finds.get(), 2);
    assert_eq!(realizes.get(), 1);
}

#[test]
fn mid_realize_search_ambiguity_aborts() {
    let mut expand = || Ok(());
    let mut collapse = || {};
    let mut find = || Ok(true);
    let mut realize = || {
        Err(AdapterError::ambiguous_target(
            "Multiple SelectionItem elements share the requested accessible name",
        )
        .with_details(serde_json::json!({
            "kind": "ambiguous_select_value",
        })))
    };
    let mut select_item = || Ok(DeliveryOutcome::DeliveredVerified);
    let error = select_judged_for(
        deadline(),
        plan(false, false, 3),
        SelectOps {
            expand: &mut expand,
            collapse: &mut collapse,
            find: &mut find,
            realize: &mut realize,
            select_item: &mut select_item,
        },
    )
    .expect_err("ambiguous mid-realize");
    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
}

#[test]
fn post_realize_duplicate_is_ambiguous() {
    let finds = Cell::new(0u8);
    let mut expand = || Ok(());
    let mut collapse = || {};
    let mut find = || {
        finds.set(finds.get() + 1);
        if finds.get() == 1 {
            Ok(true)
        } else {
            Err(AdapterError::ambiguous_target(
                "Multiple SelectionItem elements share the requested accessible name",
            )
            .with_details(serde_json::json!({
                "kind": "ambiguous_select_value",
            })))
        }
    };
    let mut realize = || Ok(());
    let mut select_item = || Ok(DeliveryOutcome::DeliveredVerified);
    let error = select_judged_for(
        deadline(),
        plan(false, false, 3),
        SelectOps {
            expand: &mut expand,
            collapse: &mut collapse,
            find: &mut find,
            realize: &mut realize,
            select_item: &mut select_item,
        },
    )
    .expect_err("ambiguous after realize");
    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
    assert_eq!(finds.get(), 2);
}

#[test]
fn miss_after_realize_still_collapses_when_expanded() {
    let order = Cell::new(Vec::<&'static str>::new());
    let mut expand = || {
        order.set({
            let mut v = order.take();
            v.push("expand");
            v
        });
        Ok(())
    };
    let mut collapse = || {
        order.set({
            let mut v = order.take();
            v.push("collapse");
            v
        });
    };
    let finds = Cell::new(0u8);
    let mut find = || {
        finds.set(finds.get() + 1);
        order.set({
            let mut v = order.take();
            v.push("find");
            v
        });
        Ok(false)
    };
    let mut realize = || {
        order.set({
            let mut v = order.take();
            v.push("realize");
            v
        });
        Ok(())
    };
    let mut select_item = || Ok(DeliveryOutcome::DeliveredVerified);
    let error = select_judged_for(
        deadline(),
        plan(false, true, 7),
        SelectOps {
            expand: &mut expand,
            collapse: &mut collapse,
            find: &mut find,
            realize: &mut realize,
            select_item: &mut select_item,
        },
    )
    .expect_err("still missing");
    assert_eq!(error.code, ErrorCode::ElementNotFound);
    assert_eq!(finds.get(), 2);
    assert_eq!(
        order.take(),
        vec!["expand", "find", "realize", "find", "collapse"]
    );
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
}
