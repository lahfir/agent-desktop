use super::sanitize_trace_value;
use serde_json::json;

#[test]
fn trace_redacts_sensitive_fields_but_preserves_messages() {
    let value = sanitize_trace_value(json!({
        "text": "secret",
        "message": "Target is not actionable: supported_action failed",
        "details": { "name": "Private Button" },
        "title": "Window"
    }));

    assert_eq!(value["text"]["redacted"], true);
    assert_eq!(value["details"]["name"]["redacted"], true);
    assert_eq!(value["title"]["redacted"], true);
    assert_eq!(
        value["message"],
        "Target is not actionable: supported_action failed"
    );
}

#[test]
fn trace_redacts_selector_keyed_values_including_in_nested_details() {
    let value = sanitize_trace_value(json!({
        "selector": "button:Submit password",
        "details": { "selector": "text:my secret" }
    }));

    assert_eq!(value["selector"]["redacted"], true);
    assert_eq!(value["details"]["selector"]["redacted"], true);
}

#[test]
fn trace_redaction_covers_nested_shapes_and_substring_keys() {
    let value = sanitize_trace_value(json!({
        "action": {
            "typed_text": ["secret", "another"],
            "api_token": {"kind": "bearer"},
            "typedText": "secret",
            "apiToken": "secret",
            "targetLabel": "secret",
            "userName": "secret",
            "filename": "report.txt",
            "password": null,
            "counter": 3
        }
    }));

    assert_eq!(value["action"]["typed_text"]["redacted"], true);
    assert_eq!(value["action"]["api_token"]["redacted"], true);
    assert_eq!(value["action"]["typedText"]["redacted"], true);
    assert_eq!(value["action"]["apiToken"]["redacted"], true);
    assert_eq!(value["action"]["targetLabel"]["redacted"], true);
    assert_eq!(value["action"]["userName"]["redacted"], true);
    assert_eq!(value["action"]["filename"], "report.txt");
    assert!(value["action"]["password"].is_null());
    assert_eq!(value["action"]["counter"], 3);
}

#[test]
fn trace_keeps_actionability_check_identifier_but_redacts_occluder_name() {
    let value = sanitize_trace_value(json!({
        "checks": [
            { "check": "supported_action", "status": "fail", "reason": "Click is not available" },
            {
                "check": "receives_events",
                "status": "fail",
                "occluder": { "role": "AXSheet", "name": "Save changes?" }
            }
        ]
    }));

    assert_eq!(value["checks"][0]["check"], "supported_action");
    assert_eq!(value["checks"][0]["reason"], "Click is not available");
    assert_eq!(value["checks"][1]["check"], "receives_events");
    assert_eq!(value["checks"][1]["occluder"]["role"], "AXSheet");
    assert_eq!(value["checks"][1]["occluder"]["name"]["redacted"], true);
}

/// The P2-O8 descriptor group rides evidence into trace sinks, and page-authored
/// tokens must be masked wherever it does — the same rule that masks a
/// placeholder. `subrole` and `dom_classes` tokenize to fragments this key list
/// names; `placeholder` and `role_description` (the `description` token) were
/// already covered.
#[test]
fn trace_redacts_descriptor_fields_alongside_placeholder() {
    let value = sanitize_trace_value(json!({
        "presentation": {
            "subrole": "custom-role",
            "role_description": "A button",
            "placeholder": "Type here",
            "dom_classes": ["panel", "pane"],
        }
    }));

    assert_eq!(value["presentation"]["subrole"]["redacted"], true);
    assert_eq!(value["presentation"]["role_description"]["redacted"], true);
    assert_eq!(value["presentation"]["placeholder"]["redacted"], true);
    assert_eq!(value["presentation"]["dom_classes"]["redacted"], true);
}

#[test]
fn trace_redacts_notification_body() {
    let value = sanitize_trace_value(json!({
        "body": "Your package has shipped"
    }));

    assert_eq!(value["body"]["redacted"], true);
}

#[test]
fn trace_redacts_notification_actions_as_a_whole_array_not_element_wise() {
    let value = sanitize_trace_value(json!({
        "actions": ["Snooze", "Dismiss"]
    }));

    assert_eq!(value["actions"]["redacted"], true);
    assert!(value["actions"].get(0).is_none());
}

#[test]
fn trace_redacts_notification_app_name_regression_pin() {
    let value = sanitize_trace_value(json!({
        "app_name": "Mail"
    }));

    assert_eq!(value["app_name"]["redacted"], true);
}

#[test]
fn trace_redacts_notification_title_regression_pin() {
    let value = sanitize_trace_value(json!({
        "title": "New Message"
    }));

    assert_eq!(value["title"]["redacted"], true);
}

#[test]
fn trace_does_not_redact_notification_index() {
    let value = sanitize_trace_value(json!({
        "index": 3
    }));

    assert_eq!(value["index"], 3);
}
