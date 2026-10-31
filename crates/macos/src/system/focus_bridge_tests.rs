use super::*;

fn result(status: u8, started: bool) -> ActivationResult {
    ActivationResult {
        status,
        delivery_started: u8::from(started),
    }
}

#[test]
fn bridge_maps_native_status_and_delivery_evidence() {
    for started in [false, true] {
        assert_eq!(
            finish_activation(result(0, started), None).unwrap(),
            started
        );
        for (status, code) in [
            (1, ErrorCode::StaleRef),
            (5, ErrorCode::StaleRef),
            (2, ErrorCode::ActionFailed),
            (3, ErrorCode::AppUnresponsive),
            (4, ErrorCode::AppUnresponsive),
            (255, ErrorCode::AppUnresponsive),
        ] {
            let error = finish_activation(result(status, started), None).unwrap_err();
            assert_eq!(error.code, code);
            assert_eq!(
                error.disposition,
                if started && status != 2 {
                    DeliverySemantics::uncertain()
                } else {
                    DeliverySemantics::not_delivered()
                }
            );
            if code == ErrorCode::AppUnresponsive {
                assert_eq!(error.details.unwrap()["status"], status);
            }
        }
    }
}

#[test]
fn identity_callback_error_overrides_bridge_status_without_losing_details() {
    let failure = AdapterError::new(ErrorCode::PermDenied, "process identity unavailable")
        .with_details(serde_json::json!({ "identity": "unreadable" }));
    let error = finish_activation(result(5, false), Some(failure.clone())).unwrap_err();
    assert_eq!(error.code, failure.code);
    assert_eq!(error.details, failure.details);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}
