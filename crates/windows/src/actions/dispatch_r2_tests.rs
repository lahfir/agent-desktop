use super::click_chain_judged_for;
use crate::actions::chain::DeliveryOutcome;
use crate::actions::disclosure::{DisclosureInput, ExpandKind, disclosure_judged_for};
use crate::actions::focus::focus_from_delivery;
use crate::actions::scroll::{ScrollPlan, scroll_judged_for};
use crate::actions::select::{SelectOps, SelectPlan, select_judged_for};
use crate::actions::toggle_state::toggle_judged_for;
use crate::actions::value_write::set_value_judged_for;
use crate::tree::actions::resolve_actions;
use crate::tree::properties::ElementProperties;
use crate::tree::property_ids::TreeProperty;
use crate::tree::property_outcome::{PropertyOutcome, PropertyValue};
use agent_desktop_core::{ActionStepOutcome, Deadline, InteractionPolicy, capability};
use std::cell::Cell;

fn short_deadline() -> Deadline {
    Deadline::after(5_000).expect("deadline")
}

fn known_flag(value: bool) -> PropertyOutcome {
    PropertyOutcome::Known(PropertyValue::Flag(value))
}

fn known_text(value: &str) -> PropertyOutcome {
    PropertyOutcome::Known(PropertyValue::Text(value.to_string()))
}

fn inert_reads() -> Vec<(TreeProperty, PropertyOutcome)> {
    vec![
        (TreeProperty::InvokeAvailable, known_flag(false)),
        (TreeProperty::ToggleAvailable, known_flag(false)),
        (TreeProperty::ExpandCollapseAvailable, known_flag(false)),
        (TreeProperty::SelectionItemAvailable, known_flag(false)),
        (TreeProperty::ValueAvailable, known_flag(false)),
        (TreeProperty::RangeValueAvailable, known_flag(false)),
        (TreeProperty::ScrollAvailable, known_flag(false)),
        (TreeProperty::ScrollItemAvailable, known_flag(false)),
        (TreeProperty::IsKeyboardFocusable, known_flag(false)),
        (TreeProperty::LegacyDefaultAction, PropertyOutcome::Absent),
    ]
}

#[test]
fn r2_invoke_advertisement_reaches_click_rung() {
    let mut reads = inert_reads();
    reads.retain(|(property, _)| *property != TreeProperty::InvokeAvailable);
    reads.push((TreeProperty::InvokeAvailable, known_flag(true)));
    let actions = resolve_actions(&ElementProperties::from_reads(reads));
    let known = actions.known().expect("Known actions");
    assert!(known.iter().any(|action| action == capability::CLICK));

    let invoke = Cell::new(0u8);
    let legacy = Cell::new(0u8);
    let steps = click_chain_judged_for(
        short_deadline(),
        InteractionPolicy::headless(),
        true,
        false,
        || {
            invoke.set(invoke.get() + 1);
            Ok(DeliveryOutcome::DeliveredUnverified)
        },
        || {
            legacy.set(legacy.get() + 1);
            Ok(DeliveryOutcome::DeliveredUnverified)
        },
    )
    .expect("invoke rung");
    assert_eq!(invoke.get(), 1);
    assert_eq!(legacy.get(), 0);
    assert!(matches!(steps[0].outcome, ActionStepOutcome::Succeeded));
}

#[test]
fn r2_legacy_advertisement_reaches_legacy_rung() {
    let mut reads = inert_reads();
    reads.retain(|(property, _)| *property != TreeProperty::LegacyDefaultAction);
    reads.push((TreeProperty::LegacyDefaultAction, known_text("Press")));
    let actions = resolve_actions(&ElementProperties::from_reads(reads));
    let known = actions.known().expect("Known actions");
    assert!(known.iter().any(|action| action == capability::CLICK));

    let invoke = Cell::new(0u8);
    let legacy = Cell::new(0u8);
    let steps = click_chain_judged_for(
        short_deadline(),
        InteractionPolicy::headless(),
        false,
        true,
        || {
            invoke.set(invoke.get() + 1);
            Ok(DeliveryOutcome::DeliveredUnverified)
        },
        || {
            legacy.set(legacy.get() + 1);
            Ok(DeliveryOutcome::DeliveredUnverified)
        },
    )
    .expect("legacy rung");
    assert_eq!(invoke.get(), 0);
    assert_eq!(legacy.get(), 1);
    assert_eq!(steps[1].label(), "LegacyIAccessible.DoDefaultAction");
}

#[test]
fn r2_focusable_advertisement_reaches_set_focus_arm() {
    let mut reads = inert_reads();
    reads.retain(|(property, _)| *property != TreeProperty::IsKeyboardFocusable);
    reads.push((TreeProperty::IsKeyboardFocusable, known_flag(true)));
    let actions = resolve_actions(&ElementProperties::from_reads(reads));
    let known = actions.known().expect("Known actions");
    assert!(known.iter().any(|action| action == capability::SET_FOCUS));

    let result =
        focus_from_delivery(InteractionPolicy::headed(), Ok(true), true).expect("SetFocus arm");
    assert!(matches!(
        result.steps[0].outcome,
        ActionStepOutcome::Succeeded
    ));
}

#[test]
fn r2_set_value_advertisement_reaches_value_rung() {
    let mut reads = inert_reads();
    reads.retain(|(property, _)| {
        *property != TreeProperty::ValueAvailable && *property != TreeProperty::ValueIsReadOnly
    });
    reads.push((TreeProperty::ValueAvailable, known_flag(true)));
    reads.push((TreeProperty::ValueIsReadOnly, known_flag(false)));
    let actions = resolve_actions(&ElementProperties::from_reads(reads));
    let known = actions.known().expect("Known actions");
    assert!(known.iter().any(|action| action == capability::SET_VALUE));

    let value = Cell::new(0u8);
    let steps = set_value_judged_for(
        short_deadline(),
        InteractionPolicy::headless(),
        "x",
        true,
        false,
        || {
            value.set(value.get() + 1);
            Ok(DeliveryOutcome::DeliveredVerified)
        },
        || Ok(DeliveryOutcome::NotDelivered),
    )
    .expect("value rung");
    assert_eq!(value.get(), 1);
    assert!(matches!(steps[0].outcome, ActionStepOutcome::Succeeded));
}

#[test]
fn r2_toggle_advertisement_reaches_toggle_rung() {
    let mut reads = inert_reads();
    reads.retain(|(property, _)| *property != TreeProperty::ToggleAvailable);
    reads.push((TreeProperty::ToggleAvailable, known_flag(true)));
    let actions = resolve_actions(&ElementProperties::from_reads(reads));
    let known = actions.known().expect("Known actions");
    assert!(known.iter().any(|action| action == capability::TOGGLE));

    let toggle = Cell::new(0u8);
    let steps = toggle_judged_for(
        short_deadline(),
        InteractionPolicy::headless(),
        true,
        false,
        || {
            toggle.set(toggle.get() + 1);
            Ok(DeliveryOutcome::DeliveredVerified)
        },
        || Ok(DeliveryOutcome::NotDelivered),
    )
    .expect("toggle rung");
    assert_eq!(toggle.get(), 1);
    assert!(matches!(steps[0].outcome, ActionStepOutcome::Succeeded));
}

#[test]
fn r2_expand_collapse_advertisement_reaches_disclosure_rung() {
    let mut reads = inert_reads();
    reads.retain(|(property, _)| *property != TreeProperty::ExpandCollapseAvailable);
    reads.push((TreeProperty::ExpandCollapseAvailable, known_flag(true)));
    let actions = resolve_actions(&ElementProperties::from_reads(reads));
    let known = actions.known().expect("Known actions");
    assert!(known.iter().any(|action| action == capability::EXPAND));
    assert!(known.iter().any(|action| action == capability::COLLAPSE));

    let expand = Cell::new(0u8);
    let steps = disclosure_judged_for(
        short_deadline(),
        InteractionPolicy::headless(),
        DisclosureInput {
            want_expanded: true,
            current: Some(ExpandKind::Collapsed),
            pattern_ok: true,
            invoke_ok: false,
        },
        || {
            expand.set(expand.get() + 1);
            Ok(DeliveryOutcome::DeliveredVerified)
        },
        || Ok(DeliveryOutcome::NotDelivered),
    )
    .expect("expand rung");
    assert_eq!(expand.get(), 1);
    assert_eq!(steps[0].label(), "ExpandCollapsePattern.Expand");
}

#[test]
fn r2_selection_item_advertisement_reaches_select_arm() {
    let mut reads = inert_reads();
    reads.retain(|(property, _)| *property != TreeProperty::SelectionItemAvailable);
    reads.push((TreeProperty::SelectionItemAvailable, known_flag(true)));
    let actions = resolve_actions(&ElementProperties::from_reads(reads));
    let known = actions.known().expect("Known actions");
    assert!(known.iter().any(|action| action == capability::SELECT));

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
        short_deadline(),
        SelectPlan {
            self_match: true,
            needs_expand: false,
            value_chars: 1,
        },
        SelectOps {
            expand: &mut expand,
            collapse: &mut collapse,
            find: &mut find,
            realize: &mut realize,
            select_item: &mut select_item,
        },
    )
    .expect("select arm");
    assert_eq!(select.get(), 1);
    assert_eq!(steps[0].label(), "SelectionItemPattern.Select");
}

#[test]
fn r2_scroll_advertisement_reaches_scroll_arm() {
    let mut reads = inert_reads();
    reads.retain(|(property, _)| *property != TreeProperty::ScrollAvailable);
    reads.push((TreeProperty::ScrollAvailable, known_flag(true)));
    let actions = resolve_actions(&ElementProperties::from_reads(reads));
    let known = actions.known().expect("Known actions");
    assert!(known.iter().any(|action| action == capability::SCROLL));

    let scrolls = Cell::new(0u8);
    let mut scroll_once = || {
        scrolls.set(scrolls.get() + 1);
        Ok(())
    };
    let mut observe = || true;
    let steps = scroll_judged_for(
        short_deadline(),
        ScrollPlan {
            scroll_available: true,
            axis_scrollable: true,
            axis_name: "vertical",
            amount: 1,
        },
        &mut scroll_once,
        &mut observe,
    )
    .expect("scroll arm");
    assert_eq!(scrolls.get(), 1);
    assert_eq!(steps[0].label(), "ScrollPattern.Scroll");
}
