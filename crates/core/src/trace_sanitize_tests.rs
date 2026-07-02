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
