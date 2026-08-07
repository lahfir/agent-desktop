use super::{click_chain_judged_for, execute_action_impl};
use crate::actions::chain::DeliveryOutcome;
use crate::actions::focus::focus_from_delivery;
use crate::tree::actions::resolve_actions;
use crate::tree::automation::automation_client;
use crate::tree::element::UIAElement;
use crate::tree::fixture::{LocalFixture, ensure_test_apartment};
use crate::tree::fixture_window;
use crate::tree::properties::ElementProperties;
use crate::tree::property_ids::TreeProperty;
use crate::tree::property_outcome::{PropertyOutcome, PropertyValue};
use agent_desktop_core::{
    Action, ActionRequest, ActionStepOutcome, AdapterError, Deadline, DeliveryDisposition,
    Direction, DragParams, ErrorCode, InteractionLease, InteractionPolicy, KeyCombo, NativeHandle,
    Point, capability,
};
use std::cell::Cell;
use uiautomation::types::Handle;

fn lease() -> InteractionLease {
    InteractionLease::guarded(Deadline::after(5_000).expect("deadline"), ()).expect("lease")
}

fn short_deadline() -> Deadline {
    Deadline::after(5_000).expect("deadline")
}

fn dummy_key() -> KeyCombo {
    KeyCombo {
        key: "a".into(),
        modifiers: vec![],
    }
}

fn dummy_drag() -> DragParams {
    DragParams {
        from: Point { x: 0.0, y: 0.0 },
        to: Point { x: 1.0, y: 1.0 },
        duration_ms: None,
        drop_delay_ms: None,
    }
}

fn all_actions() -> [Action; 21] {
    [
        Action::Click,
        Action::DoubleClick,
        Action::RightClick,
        Action::TripleClick,
        Action::SetValue("x".into()),
        Action::SetFocus,
        Action::Expand,
        Action::Collapse,
        Action::Select("x".into()),
        Action::Toggle,
        Action::Check,
        Action::Uncheck,
        Action::Scroll(Direction::Down, 1),
        Action::ScrollTo,
        Action::PressKey(dummy_key()),
        Action::KeyDown(dummy_key()),
        Action::KeyUp(dummy_key()),
        Action::TypeText("x".into()),
        Action::Clear,
        Action::Hover,
        Action::Drag(dummy_drag()),
    ]
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

fn control_handle(button: *mut std::ffi::c_void) -> Result<NativeHandle, AdapterError> {
    let client = automation_client()?;
    let element = client
        .element_from_handle(Handle::from(button as isize))
        .map_err(|error| {
            crate::tree::automation::uia_error(&error, "resolve the fixture button")
        })?;
    Ok(UIAElement::from(element).into_native_handle())
}

fn code_lines(source: &str) -> impl Iterator<Item = (usize, &str)> {
    source
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            let trimmed = line.trim_start();
            !(trimmed.starts_with("///") || trimmed.starts_with("//!"))
        })
        .map(|(index, line)| (index + 1, line))
}

#[test]
fn null_handle_press_key_names_key_synthesis() {
    let error = execute_action_impl(
        &NativeHandle::null(),
        ActionRequest::headless(Action::PressKey(dummy_key())),
        &lease(),
    )
    .expect_err("null PressKey");
    assert_eq!(error.code, ErrorCode::PlatformNotSupported);
    assert!(error.message.contains("key synthesis"));
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
}

#[test]
fn null_handle_click_is_invalid_native_handle() {
    let error = execute_action_impl(
        &NativeHandle::null(),
        ActionRequest::headless(Action::Click),
        &lease(),
    )
    .expect_err("null Click");
    assert_eq!(error.code, ErrorCode::InvalidArgs);
    assert_eq!(
        error.details.as_ref().and_then(|d| d.get("kind")),
        Some(&serde_json::json!("invalid_native_handle"))
    );
}

#[test]
fn honest_arms_name_missing_capabilities() {
    ensure_test_apartment();
    let fixture = LocalFixture::create().expect("fixture");
    let handle = control_handle(fixture_window::find_button(fixture.handle())).expect("handle");
    let cases = [
        (Action::TypeText("x".into()), "key synthesis"),
        (Action::PressKey(dummy_key()), "key synthesis"),
        (Action::DoubleClick, "multi-click"),
        (Action::TripleClick, "multi-click"),
        (Action::RightClick, "physical context-menu click"),
    ];
    for (action, needle) in cases {
        let error = execute_action_impl(&handle, ActionRequest::headless(action), &lease())
            .expect_err("honest arm");
        assert_eq!(error.code, ErrorCode::PlatformNotSupported);
        assert!(
            error.message.contains(needle),
            "expected {needle:?} in {}",
            error.message
        );
        assert_eq!(
            error.disposition.delivery(),
            DeliveryDisposition::NotDelivered
        );
    }
}

#[test]
fn adapter_level_rejection_mirrors_macos_message() {
    ensure_test_apartment();
    let fixture = LocalFixture::create().expect("fixture");
    let handle = control_handle(fixture_window::find_button(fixture.handle())).expect("handle");
    for action in [
        Action::KeyDown(dummy_key()),
        Action::Hover,
        Action::Drag(dummy_drag()),
    ] {
        let label = action.name();
        let error = execute_action_impl(&handle, ActionRequest::headless(action), &lease())
            .expect_err("adapter-level");
        assert_eq!(error.code, ErrorCode::ActionNotSupported);
        assert_eq!(
            error.message,
            format!("{label} requires adapter-level handling, not element action")
        );
    }
}

#[test]
fn every_action_variant_returns_a_deliberate_outcome() {
    ensure_test_apartment();
    let fixture = LocalFixture::create().expect("fixture");
    let handle = control_handle(fixture_window::find_button(fixture.handle())).expect("handle");
    for action in all_actions() {
        let result = execute_action_impl(&handle, ActionRequest::headless(action.clone()), &lease());
        match result {
            Ok(ok) => {
                assert!(
                    matches!(action, Action::Click | Action::ScrollTo)
                        || ok.steps.iter().any(|step| {
                            matches!(
                                step.outcome,
                                ActionStepOutcome::Succeeded | ActionStepOutcome::Skipped
                            )
                        }),
                    "unexpected success for {}",
                    action.name()
                );
            }
            Err(error) => {
                assert!(
                    !error.message.contains("execute_action"),
                    "{} fell through to the trait default: {}",
                    action.name(),
                    error.message
                );
            }
        }
    }
}

#[test]
fn set_focus_call_site_lives_only_in_focus_rs() {
    let sources = [
        ("actions/mutation.rs", include_str!("mutation.rs")),
        (
            "actions/scroll_into_view.rs",
            include_str!("scroll_into_view.rs"),
        ),
        ("actions/chain.rs", include_str!("chain.rs")),
        ("actions/dispatch.rs", include_str!("dispatch.rs")),
        ("actions/focus.rs", include_str!("focus.rs")),
    ];
    let banned = concat!(".", "set_focus(");
    for (name, source) in sources {
        for (number, line) in code_lines(source) {
            if name.ends_with("focus.rs") {
                continue;
            }
            assert!(
                !line.contains(banned),
                "{name}:{number} must not call set_focus: {line}"
            );
        }
    }
    assert!(
        include_str!("focus.rs").contains(banned),
        "focus.rs must own the set_focus call site"
    );
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

    let result = focus_from_delivery(InteractionPolicy::headed(), Ok(true), true)
        .expect("SetFocus arm");
    assert!(matches!(result.steps[0].outcome, ActionStepOutcome::Succeeded));
}

#[test]
fn no_affordance_click_chain_exhausts_not_delivered() {
    let error = click_chain_judged_for(
        short_deadline(),
        InteractionPolicy::headless(),
        false,
        false,
        || Ok(DeliveryOutcome::DeliveredUnverified),
        || Ok(DeliveryOutcome::DeliveredUnverified),
    )
    .expect_err("exhausted");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
}

#[test]
fn live_fixture_click_delivers_invoke_or_legacy_unverified() {
    ensure_test_apartment();
    let fixture = LocalFixture::create().expect("fixture");
    let handle = control_handle(fixture_window::find_button(fixture.handle())).expect("handle");
    let result = execute_action_impl(&handle, ActionRequest::headless(Action::Click), &lease())
        .expect("click");
    assert_eq!(
        result.disposition().delivery(),
        DeliveryDisposition::DeliveredUnverified
    );
    let succeeded = result
        .steps
        .iter()
        .find(|step| matches!(step.outcome, ActionStepOutcome::Succeeded))
        .expect("a delivered step");
    assert!(
        succeeded.label() == "InvokePattern.Invoke"
            || succeeded.label() == "LegacyIAccessible.DoDefaultAction",
        "unexpected label {}",
        succeeded.label()
    );
    assert_eq!(succeeded.verified(), Some(false));
    assert_eq!(
        succeeded.mechanism(),
        Some(agent_desktop_core::StepMechanism::SemanticApi)
    );
}

#[test]
fn headless_set_focus_on_live_element_is_policy_denied() {
    ensure_test_apartment();
    let fixture = LocalFixture::create().expect("fixture");
    let handle = control_handle(fixture_window::find_button(fixture.handle())).expect("handle");
    let error = execute_action_impl(
        &handle,
        ActionRequest::headless(Action::SetFocus),
        &lease(),
    )
    .expect_err("headless SetFocus");
    assert_eq!(error.code, ErrorCode::PolicyDenied);
    assert!(
        error
            .suggestion
            .as_deref()
            .unwrap_or("")
            .contains("--headed")
    );
}
