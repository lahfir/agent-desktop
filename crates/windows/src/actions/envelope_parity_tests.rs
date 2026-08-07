use crate::actions::chain::{DeliveryOutcome, build_step};
use crate::actions::mutation::classify_mutation;
use crate::system::hresult::{
    E_ACCESSDENIED, E_INVALIDARG, RPC_E_SERVERFAULT, UIA_E_ELEMENTNOTAVAILABLE,
    UIA_E_ELEMENTNOTENABLED, UIA_E_INVALIDOPERATION, UIA_E_TIMEOUT,
};
use crate::tree::automation::UiaFailure;
use agent_desktop_core::{
    Action, ActionResult, AppError, DeliverySemantics, Direction, ElementState, ErrorPayload,
};
use serde_json::Value;

fn state_bearing(value: &str) -> ElementState {
    ElementState {
        role: "control".into(),
        states: vec![],
        value: Some(value.into()),
        enabled: Some(true),
        hidden: Some(false),
        offscreen: Some(false),
    }
}

fn succeeded_step(label: &'static str, verified: bool) -> agent_desktop_core::ActionStep {
    build_step(
        label,
        if verified {
            DeliveryOutcome::DeliveredVerified
        } else {
            DeliveryOutcome::DeliveredUnverified
        },
    )
}

fn skipped_step(label: &'static str) -> agent_desktop_core::ActionStep {
    build_step(label, DeliveryOutcome::NotDelivered)
}

fn satisfied_step(label: &'static str) -> agent_desktop_core::ActionStep {
    build_step(label, DeliveryOutcome::SatisfiedNoDelivery)
}

fn assert_disposition_matches_projection(json: &Value, semantics: DeliverySemantics) {
    let projected = serde_json::to_value(semantics).expect("disposition serializes");
    assert_eq!(
        json["disposition"], projected,
        "wire disposition must equal DeliverySemantics projection"
    );
    assert_eq!(
        json["disposition"]["delivery"], projected["delivery"],
        "delivery wire string"
    );
    assert_eq!(
        json["disposition"]["retry"], projected["retry"],
        "retry wire string"
    );
}

fn serialize_result(
    action: &Action,
    steps: Vec<agent_desktop_core::ActionStep>,
    post_state: Option<ElementState>,
) -> (ActionResult, Value) {
    let result = ActionResult::from_execution(action, steps, post_state).expect("ActionResult");
    let json = serde_json::to_value(&result).expect("ActionResult serializes");
    (result, json)
}

#[test]
fn succeeded_step_wire_uses_semantic_api_and_succeeded() {
    let (result, json) = serialize_result(
        &Action::Click,
        vec![succeeded_step("InvokePattern.Invoke", false)],
        None,
    );

    assert_eq!(json["steps"][0]["outcome"], "succeeded");
    assert_eq!(json["steps"][0]["mechanism"], "semantic_api");
    assert_eq!(json["steps"][0]["verified"], false);
    assert_eq!(json["steps"][0]["label"], "InvokePattern.Invoke");
    assert_disposition_matches_projection(&json, result.disposition());
    assert_eq!(json["disposition"]["delivery"], "delivered_unverified");
    assert_eq!(json["disposition"]["retry"], "unsafe");
}

#[test]
fn skipped_step_wire_uses_semantic_api_and_skipped() {
    let (result, json) = serialize_result(
        &Action::Click,
        vec![
            skipped_step("InvokePattern.Invoke"),
            succeeded_step("LegacyIAccessible.DoDefaultAction", false),
        ],
        None,
    );

    assert_eq!(json["steps"][0]["outcome"], "skipped");
    assert_eq!(json["steps"][0]["mechanism"], "semantic_api");
    assert!(json["steps"][0].get("verified").is_none());
    assert_eq!(json["steps"][1]["outcome"], "succeeded");
    assert_eq!(json["steps"][1]["mechanism"], "semantic_api");
    assert_disposition_matches_projection(&json, result.disposition());
}

#[test]
fn satisfied_without_delivery_wire_is_not_delivered_safe() {
    let (result, json) = serialize_result(
        &Action::Check,
        vec![satisfied_step("AlreadyInState")],
        Some(state_bearing("1")),
    );

    assert_eq!(json["steps"][0]["outcome"], "skipped");
    assert_eq!(json["steps"][0]["mechanism"], "semantic_api");
    assert_eq!(json["steps"][0]["verified"], true);
    assert!(
        json.get("post_state").is_none(),
        "no-op success omits post_state"
    );
    assert_eq!(result.disposition(), DeliverySemantics::not_delivered());
    assert_disposition_matches_projection(&json, DeliverySemantics::not_delivered());
    assert_eq!(json["disposition"]["delivery"], "not_delivered");
    assert_eq!(json["disposition"]["retry"], "safe");
}

#[test]
fn verified_delivery_wire_is_delivered_verified_unsafe() {
    let (result, json) = serialize_result(
        &Action::SetValue("done".into()),
        vec![succeeded_step("ValuePattern.SetValue", true)],
        Some(state_bearing("done")),
    );

    assert_eq!(
        result.disposition(),
        DeliverySemantics::delivered_verified()
    );
    assert_disposition_matches_projection(&json, DeliverySemantics::delivered_verified());
    assert_eq!(json["disposition"]["delivery"], "delivered_verified");
    assert_eq!(json["disposition"]["retry"], "unsafe");
}

#[test]
fn post_state_present_for_state_bearing_actions_when_delivered() {
    let cases = [
        (Action::SetValue("x".into()), "observed"),
        (Action::Clear, ""),
        (Action::Toggle, "1"),
        (Action::Check, "1"),
        (Action::Uncheck, "0"),
        (Action::Expand, "expanded"),
        (Action::Collapse, "collapsed"),
    ];
    for (action, value) in cases {
        let (_result, json) = serialize_result(
            &action,
            vec![succeeded_step("Pattern.Write", true)],
            Some(state_bearing(value)),
        );
        assert!(
            json.get("post_state").is_some(),
            "{} must serialize post_state when delivered with state",
            action.name()
        );
        assert_eq!(json["post_state"]["role"], "control");
        assert_eq!(json["post_state"]["value"], value);
    }
}

#[test]
fn post_state_absent_for_click_scroll_and_focus() {
    let cases = [
        (
            Action::Click,
            vec![succeeded_step("InvokePattern.Invoke", false)],
        ),
        (
            Action::Scroll(Direction::Down, 1),
            vec![succeeded_step("ScrollPattern.Scroll", false)],
        ),
        (
            Action::SetFocus,
            vec![succeeded_step("Element.SetFocus", true)],
        ),
    ];
    for (action, steps) in cases {
        let (_result, json) = serialize_result(&action, steps, None);
        assert!(
            json.get("post_state").is_none(),
            "{} must omit post_state",
            action.name()
        );
    }
}

fn classify_hresult(code: i32) -> agent_desktop_core::AdapterError {
    classify_mutation("Pattern.Write", "Pattern.Write", &UiaFailure::Hresult(code))
        .expect_err("classifier Err arm")
}

fn error_disposition_json(error: agent_desktop_core::AdapterError) -> Value {
    let payload = ErrorPayload::from_app_error(&AppError::from(error));
    serde_json::to_value(&payload).expect("ErrorPayload serializes")["disposition"].clone()
}

#[test]
fn classifier_error_disposition_wire_matches_projection() {
    let cases: &[(i32, DeliverySemantics)] = &[
        (E_ACCESSDENIED, DeliverySemantics::not_delivered()),
        (
            UIA_E_ELEMENTNOTAVAILABLE,
            DeliverySemantics::not_delivered(),
        ),
        (E_INVALIDARG, DeliverySemantics::not_delivered()),
        (UIA_E_ELEMENTNOTENABLED, DeliverySemantics::not_delivered()),
        (RPC_E_SERVERFAULT, DeliverySemantics::uncertain()),
        (UIA_E_TIMEOUT, DeliverySemantics::uncertain()),
        (UIA_E_INVALIDOPERATION, DeliverySemantics::uncertain()),
    ];

    for &(hresult, expected) in cases {
        let error = classify_hresult(hresult);
        assert_eq!(error.disposition, expected);
        let wire = error_disposition_json(error);
        let projected = serde_json::to_value(expected).expect("projection");
        assert_eq!(wire, projected, "HRESULT {hresult:#x} disposition wire");
    }
}

#[test]
fn perm_denied_error_disposition_is_not_delivered_safe() {
    let wire = error_disposition_json(classify_hresult(E_ACCESSDENIED));
    assert_eq!(wire["delivery"], "not_delivered");
    assert_eq!(wire["retry"], "safe");
}

#[test]
fn app_unresponsive_error_disposition_is_uncertain_unsafe() {
    let wire = error_disposition_json(classify_hresult(RPC_E_SERVERFAULT));
    assert_eq!(wire["delivery"], "delivery_uncertain");
    assert_eq!(wire["retry"], "unsafe");
}

const COST_ARMS: &[&str] = &[
    "Invoke",
    "Toggle",
    "SetValue",
    "Select",
    "Scroll",
    "click_chain_worst_case",
];

fn assert_cost_capture_spread(label: &str, raw: &str) {
    let value: Value = serde_json::from_str(raw).unwrap_or_else(|err| {
        panic!("{label} must parse as JSON: {err}");
    });
    for arm in COST_ARMS {
        let entry = value
            .get(*arm)
            .unwrap_or_else(|| panic!("{label} missing arm {arm}"));
        let min = entry["min_ms"]
            .as_f64()
            .unwrap_or_else(|| panic!("{label}/{arm} missing min_ms"));
        let median = entry["median_ms"]
            .as_f64()
            .unwrap_or_else(|| panic!("{label}/{arm} missing median_ms"));
        let max = entry["max_ms"]
            .as_f64()
            .unwrap_or_else(|| panic!("{label}/{arm} missing max_ms"));
        assert!(
            min <= median && median <= max,
            "{label}/{arm}: min<=median<=max ({min}, {median}, {max})"
        );
        assert_eq!(entry["n"], 7, "{label}/{arm} n");
        assert_eq!(
            entry["warmup_discarded"], true,
            "{label}/{arm} warmup_discarded"
        );
    }
    assert_eq!(
        value["methodology"], "min-of-seven discard warm-up (A15-13)",
        "{label} methodology"
    );
}

#[test]
fn a19_8_semantic_cost_captures_carry_min_median_max_both_environments() {
    // A19-8: probes/windows/19-semantic-actions/captures/semantic-cost-{devbox,ci}.json
    assert_cost_capture_spread(
        "semantic-cost-devbox",
        include_str!(
            "../../../../probes/windows/19-semantic-actions/captures/semantic-cost-devbox.json"
        ),
    );
    assert_cost_capture_spread(
        "semantic-cost-ci",
        include_str!(
            "../../../../probes/windows/19-semantic-actions/captures/semantic-cost-ci.json"
        ),
    );
}

#[test]
fn a18_7_target_pre_read_baseline_still_present_for_shared_primitive() {
    // Shared pre-read primitive compared by A19-8 DoD; numbers live in A18-7 captures.
    for (label, raw) in [
        (
            "actionability-cost-devbox",
            include_str!(
                "../../../../probes/windows/18-actionability/captures/actionability-cost-devbox.json"
            ),
        ),
        (
            "actionability-cost-ci",
            include_str!(
                "../../../../probes/windows/18-actionability/captures/actionability-cost-ci.json"
            ),
        ),
    ] {
        let value: Value = serde_json::from_str(raw).unwrap_or_else(|err| {
            panic!("{label} must parse as JSON: {err}");
        });
        let entry = value
            .get("target_pre_read")
            .unwrap_or_else(|| panic!("{label} missing target_pre_read"));
        assert!(entry["min_ms"].as_f64().is_some(), "{label} min_ms");
        assert!(entry["median_ms"].as_f64().is_some(), "{label} median_ms");
        assert!(entry["max_ms"].as_f64().is_some(), "{label} max_ms");
    }
}
