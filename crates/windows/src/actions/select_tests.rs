use super::{
    SELECT_LABEL, SelectOps, SelectPlan, resolve_select_verification, select_judged_for,
};
use crate::actions::chain::DeliveryOutcome;
use crate::actions::value_write::gated_value_compare;
use crate::tree::property_outcome::{PropertyOutcome, PropertyValue};
use agent_desktop_core::{
    ActionStepOutcome, AdapterError, Deadline, DeliveryDisposition, ErrorCode,
};
use std::cell::Cell;

fn deadline() -> Deadline {
    Deadline::after(5_000).expect("deadline")
}

fn known_flag(value: bool) -> PropertyOutcome {
    PropertyOutcome::Known(PropertyValue::Flag(value))
}

fn plan(self_match: bool, needs_expand: bool, value_chars: usize) -> SelectPlan {
    SelectPlan {
        self_match,
        needs_expand,
        value_chars,
    }
}

#[test]
fn self_match_selects_and_verifies_is_selected() {
    let select = Cell::new(0u8);
    let mut expand = || Ok(());
    let mut collapse = || {};
    let mut find = || Ok(false);
    let mut realize = || Ok(());
    let mut select_item = || {
        select.set(select.get() + 1);
        Ok(DeliveryOutcome::DeliveredVerified)
    };
    let steps = select_judged_for(
        deadline(),
        plan(true, false, 3),
        SelectOps {
            expand: &mut expand,
            collapse: &mut collapse,
            find: &mut find,
            realize: &mut realize,
            select_item: &mut select_item,
        },
    )
    .expect("self select");
    assert_eq!(select.get(), 1);
    assert_eq!(steps[0].label(), SELECT_LABEL);
    assert_eq!(steps[0].verified(), Some(true));
    assert!(matches!(steps[0].outcome, ActionStepOutcome::Succeeded));
}

#[test]
fn value_mismatch_is_element_not_found_with_char_count_not_text() {
    let marker = "secret-select-marker-zz";
    let mut expand = || Ok(());
    let mut collapse = || {};
    let mut find = || Ok(false);
    let mut realize = || Ok(());
    let mut select_item = || Ok(DeliveryOutcome::DeliveredVerified);
    let error = select_judged_for(
        deadline(),
        plan(false, false, marker.chars().count()),
        SelectOps {
            expand: &mut expand,
            collapse: &mut collapse,
            find: &mut find,
            realize: &mut realize,
            select_item: &mut select_item,
        },
    )
    .expect_err("miss");
    assert_eq!(error.code, ErrorCode::ElementNotFound);
    assert!(error.message.contains(&format!("{} chars", marker.chars().count())));
    assert!(
        !error.message.contains(marker),
        "must never echo the requested value text"
    );
}

#[test]
fn container_search_selects_when_find_hits() {
    let select = Cell::new(0u8);
    let find_calls = Cell::new(0u8);
    let mut expand = || Ok(());
    let mut collapse = || {};
    let mut find = || {
        find_calls.set(find_calls.get() + 1);
        Ok(true)
    };
    let mut realize = || Ok(());
    let mut select_item = || {
        select.set(select.get() + 1);
        Ok(DeliveryOutcome::DeliveredVerified)
    };
    let steps = select_judged_for(
        deadline(),
        plan(false, false, 4),
        SelectOps {
            expand: &mut expand,
            collapse: &mut collapse,
            find: &mut find,
            realize: &mut realize,
            select_item: &mut select_item,
        },
    )
    .expect("found");
    assert_eq!(find_calls.get(), 1);
    assert_eq!(select.get(), 1);
    assert_eq!(steps[0].label(), SELECT_LABEL);
}

#[test]
fn budget_exhaustion_surfaces_honest_error() {
    let mut expand = || Ok(());
    let mut collapse = || {};
    let mut find = || {
        Err(AdapterError::new(
            ErrorCode::AppUnresponsive,
            "Select search exceeded its accessibility-node budget",
        )
        .with_details(serde_json::json!({
            "kind": "select_node_limit",
            "limit": 2048,
            "complete": false,
        })))
    };
    let mut realize = || Ok(());
    let mut select_item = || Ok(DeliveryOutcome::DeliveredVerified);
    let error = select_judged_for(
        deadline(),
        plan(false, false, 1),
        SelectOps {
            expand: &mut expand,
            collapse: &mut collapse,
            find: &mut find,
            realize: &mut realize,
            select_item: &mut select_item,
        },
    )
    .expect_err("budget");
    assert_eq!(error.code, ErrorCode::AppUnresponsive);
    assert_eq!(
        error.details.as_ref().and_then(|d| d.get("kind")),
        Some(&serde_json::json!("select_node_limit"))
    );
}

#[test]
fn collapsed_combobox_expands_first_and_collapses_on_failure() {
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
    let mut find = || {
        order.set({
            let mut v = order.take();
            v.push("find");
            v
        });
        Ok(true)
    };
    let mut realize = || Ok(());
    let mut select_item = || {
        order.set({
            let mut v = order.take();
            v.push("select");
            v
        });
        Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "select failed after expand",
        )
        .with_disposition(agent_desktop_core::DeliverySemantics::delivered_unverified()))
    };
    let error = select_judged_for(
        deadline(),
        plan(false, true, 2),
        SelectOps {
            expand: &mut expand,
            collapse: &mut collapse,
            find: &mut find,
            realize: &mut realize,
            select_item: &mut select_item,
        },
    )
    .expect_err("select failed");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(order.take(), vec!["expand", "find", "select", "collapse"]);
}

#[test]
fn container_value_verification_outranks_is_selected() {
    assert_eq!(
        resolve_select_verification(Some(Some(true)), Some(false)),
        Some(true)
    );
    assert_eq!(
        resolve_select_verification(Some(Some(false)), Some(true)),
        Some(false)
    );
}

#[test]
fn is_password_skips_value_read_and_falls_back_to_is_selected() {
    let reads = Cell::new(0u8);
    let gated = gated_value_compare(known_flag(true), "marker-value", || {
        reads.set(reads.get() + 1);
        Ok("marker-value".into())
    })
    .expect("gate");
    assert_eq!(gated, None);
    assert_eq!(reads.get(), 0);
    assert_eq!(
        resolve_select_verification(Some(None), Some(true)),
        Some(true)
    );

    let unknown_reads = Cell::new(0u8);
    let unknown = gated_value_compare(PropertyOutcome::Unknown, "marker", || {
        unknown_reads.set(unknown_reads.get() + 1);
        Ok("marker".into())
    })
    .expect("unknown");
    assert_eq!(unknown, None);
    assert_eq!(unknown_reads.get(), 0);
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
