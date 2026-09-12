use super::*;
use std::cell::Cell;

#[test]
fn select_traversal_limits_are_bounded() {
    assert_eq!(MAX_SELECT_NODES, 2_048);
    assert_eq!(MAX_SELECT_DEPTH, 8);
}

#[test]
fn menu_items_are_pressed_without_setting_selected_first() {
    let selected_calls = Cell::new(0);
    let press_calls = Cell::new(0);

    let verified = deliver_candidate(
        false,
        || {
            selected_calls.set(selected_calls.get() + 1);
            Ok(Some(true))
        },
        || {
            press_calls.set(press_calls.get() + 1);
            Ok(true)
        },
    )
    .expect("menu candidate delivery");

    assert!(!verified);
    assert_eq!(selected_calls.get(), 0);
    assert_eq!(press_calls.get(), 1);
}

#[test]
fn collection_selection_falls_back_to_press_when_selected_write_fails() {
    let press_calls = Cell::new(0);
    let verified = deliver_candidate(
        true,
        || {
            Err(
                AdapterError::new(ErrorCode::ActionFailed, "AXSelected is unavailable")
                    .with_disposition(DeliverySemantics::not_delivered()),
            )
        },
        || {
            press_calls.set(press_calls.get() + 1);
            Ok(true)
        },
    )
    .expect("press fallback");

    assert!(!verified);
    assert_eq!(press_calls.get(), 1);
}

#[test]
fn uncertain_collection_selection_never_falls_back_to_press() {
    for disposition in [
        DeliverySemantics::uncertain(),
        DeliverySemantics::delivered_unverified(),
    ] {
        let presses = Cell::new(0);
        let result = deliver_candidate(
            true,
            || {
                Err(
                    AdapterError::new(ErrorCode::ActionFailed, "write uncertainty")
                        .with_disposition(disposition),
                )
            },
            || {
                presses.set(presses.get() + 1);
                Ok(true)
            },
        );
        assert_eq!(result.unwrap_err().disposition, disposition);
        assert_eq!(presses.get(), 0);
    }
}

#[test]
fn selected_readback_distinguishes_contradiction_from_missing_evidence() {
    assert_eq!(
        selected_readback(Some(false)).unwrap_err().disposition,
        DeliverySemantics::delivered_unverified()
    );
    assert!(selected_readback(Some(true)).unwrap());
    assert!(!selected_readback(None).unwrap());
}
