use super::{sanitize_trace_value, trace_key_tokens};
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

#[test]
fn trace_redacts_acronym_prefixed_camelcase_keys() {
    let cases = [
        "APIToken",
        "URLValue",
        "HTMLLabel",
        "XMLName",
        "JSONText",
        "CSSSelector",
        "HTTPSecret",
        "URLSecret",
        "HTMLText",
        "GUILabel",
    ];
    for key in cases {
        let value = sanitize_trace_value(json!({ key: "leak" }));
        assert_eq!(
            value[key]["redacted"], true,
            "key `{key}` should be redacted - got {value}",
        );
    }
}

#[test]
fn trace_key_tokens_splits_acronym_boundaries() {
    assert_eq!(trace_key_tokens("APIToken"), ["api", "token"]);
    assert_eq!(trace_key_tokens("URLValue"), ["url", "value"]);
    assert_eq!(trace_key_tokens("HTMLLabel"), ["html", "label"]);
    assert_eq!(trace_key_tokens("CSSSelector"), ["css", "selector"]);
    assert_eq!(trace_key_tokens("HTTPSecret"), ["http", "secret"]);
    assert_eq!(trace_key_tokens("XMLName"), ["xml", "name"]);
    assert_eq!(trace_key_tokens("JSONText"), ["json", "text"]);
}

#[test]
fn trace_key_tokens_preserves_single_uppercase_prefix() {
    assert_eq!(trace_key_tokens("Url"), ["url"]);
    assert_eq!(trace_key_tokens("X"), ["x"]);
    assert_eq!(trace_key_tokens("URL"), ["url"]);
    assert_eq!(trace_key_tokens("AB"), ["ab"]);
    assert_eq!(trace_key_tokens("Name"), ["name"]);
}

#[test]
fn trace_key_tokens_does_not_redact_non_sensitive_acronym_keys() {
    let value = sanitize_trace_value(json!({
        "APIVersion": "1.2.3",
        "HTTPStatus": 200,
        "JSONCount": 3,
        "XMLFlag": true,
        "CSSRule": "display:block",
    }));
    assert_eq!(value["APIVersion"], "1.2.3");
    assert_eq!(value["HTTPStatus"], 200);
    assert_eq!(value["JSONCount"], 3);
    assert_eq!(value["XMLFlag"], true);
    assert_eq!(value["CSSRule"], "display:block");
}
