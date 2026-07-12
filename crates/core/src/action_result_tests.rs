use super::*;

#[test]
fn delivered_unverified_constructor_is_explicit() {
    assert_eq!(
        ActionResult::delivered_unverified("click").disposition(),
        DeliverySemantics::delivered_unverified()
    );
}

#[test]
fn satisfied_without_delivery_is_successful_and_safe_to_repeat() {
    let result = ActionResult::satisfied_without_delivery("check").with_steps(vec![
        ActionStep::skipped("AlreadyInState").with_verified(true),
    ]);

    assert_eq!(result.disposition(), DeliverySemantics::not_delivered());
    let json = serde_json::to_value(result).unwrap();
    assert_eq!(json["disposition"]["delivery"], "not_delivered");
    assert_eq!(json["disposition"]["retry"], "safe");
}

#[test]
fn verification_cannot_turn_a_no_op_into_claimed_delivery() {
    let result = ActionResult::satisfied_without_delivery("check").with_verified_delivery();

    assert_eq!(result.disposition(), DeliverySemantics::not_delivered());
}

#[test]
fn verified_action_uses_verified_delivery() {
    assert_eq!(
        ActionResult::delivered_unverified("click")
            .with_verified_delivery()
            .disposition(),
        DeliverySemantics::delivered_verified()
    );
}

#[test]
fn legacy_json_defaults_to_delivered_unverified() {
    let result: ActionResult = serde_json::from_value(serde_json::json!({
        "action": "click"
    }))
    .expect("legacy action result");

    assert_eq!(
        result.disposition(),
        DeliverySemantics::delivered_unverified()
    );
}

#[test]
fn successful_action_rejects_unknown_and_uncertain_dispositions() {
    for delivery in ["unknown", "delivery_uncertain"] {
        let retry = if delivery == "unknown" {
            "unknown"
        } else {
            "unsafe"
        };
        let value = serde_json::json!({
            "action": "click",
            "disposition": {
                "delivery": delivery,
                "retry": retry
            }
        });

        assert!(serde_json::from_value::<ActionResult>(value).is_err());
    }
}

#[test]
fn successful_action_deserializes_satisfied_without_delivery() {
    let value = serde_json::json!({
        "action": "check",
        "disposition": {
            "delivery": "not_delivered",
            "retry": "safe"
        }
    });

    let result: ActionResult = serde_json::from_value(value).unwrap();
    assert_eq!(result.disposition(), DeliverySemantics::not_delivered());
}
