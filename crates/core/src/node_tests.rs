use super::*;

#[test]
fn test_rect_null_fields_deserialize() {
    let json = r#"{"x": null, "y": null, "width": 0.0, "height": 0.0}"#;
    let rect: Rect = serde_json::from_str(json).unwrap();
    assert_eq!(rect.x, 0.0);
    assert_eq!(rect.y, 0.0);
}

#[test]
fn test_rect_missing_fields_deserialize() {
    let json = r#"{"width": 100.0, "height": 50.0}"#;
    let rect: Rect = serde_json::from_str(json).unwrap();
    assert_eq!(rect.x, 0.0);
    assert_eq!(rect.y, 0.0);
    assert_eq!(rect.width, 100.0);
}

#[test]
fn test_children_count_omitted_when_none() {
    let node = AccessibilityNode {
        ref_id: None,
        role: "group".into(),
        name: Some("Sidebar".into()),
        value: None,
        description: None,
        hint: None,
        states: vec![],
        available_actions: vec![],
        bounds: None,
        children_count: None,
        children: vec![],
    };
    let json = serde_json::to_string(&node).unwrap();
    assert!(!json.contains("children_count"));
}

#[test]
fn test_children_count_present_when_set() {
    let node = AccessibilityNode {
        ref_id: None,
        role: "group".into(),
        name: Some("Sidebar".into()),
        value: None,
        description: None,
        hint: None,
        states: vec![],
        available_actions: vec![],
        bounds: None,
        children_count: Some(47),
        children: vec![],
    };
    let json = serde_json::to_string(&node).unwrap();
    assert!(json.contains("\"children_count\":47"));
}

#[test]
fn test_children_count_backward_compat() {
    let json = r#"{"role":"button","name":"OK"}"#;
    let node: AccessibilityNode = serde_json::from_str(json).unwrap();
    assert!(node.children_count.is_none());
}

#[test]
fn test_rect_normal_roundtrip() {
    let rect = Rect {
        x: 10.5,
        y: 20.3,
        width: 100.0,
        height: 50.0,
    };
    let json = serde_json::to_string(&rect).unwrap();
    let back: Rect = serde_json::from_str(&json).unwrap();
    assert_eq!(back.x, 10.5);
    assert_eq!(back.width, 100.0);
}

/// AppInfo.bundle_id is annotated skip_serializing_if = "Option::is_none".
/// When absent it must not appear in the JSON — agents must tolerate its
/// absence rather than fail on a missing key.
#[test]
fn app_info_bundle_id_none_omitted_from_json() {
    let info = AppInfo {
        name: "Finder".into(),
        pid: 42,
        bundle_id: None,
    };
    let json = serde_json::to_string(&info).unwrap();
    assert!(
        !json.contains("\"bundle_id\":"),
        "bundle_id must be absent when None, json={json}"
    );
}

/// When bundle_id is Some it must appear in the JSON with the correct value.
#[test]
fn app_info_bundle_id_some_present_in_json() {
    let info = AppInfo {
        name: "Safari".into(),
        pid: 7,
        bundle_id: Some("com.apple.Safari".into()),
    };
    let json = serde_json::to_string(&info).unwrap();
    assert!(
        json.contains("\"bundle_id\":\"com.apple.Safari\""),
        "bundle_id must be present when Some, json={json}"
    );
}

/// AppInfo round-trips through serde with all fields intact.
/// Uses field-by-field comparison because AppInfo does not derive PartialEq.
#[test]
fn app_info_roundtrip_preserves_all_fields() {
    let original = AppInfo {
        name: "TextEdit".into(),
        pid: 1234,
        bundle_id: Some("com.apple.TextEdit".into()),
    };
    let json = serde_json::to_string(&original).unwrap();
    let back: AppInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(back.name, original.name);
    assert_eq!(back.pid, original.pid);
    assert_eq!(back.bundle_id, original.bundle_id);
}

/// Agents may produce AppInfo JSON without a bundle_id key (e.g. non-macOS adapters
/// or older protocol versions). Deserialization must succeed and yield None.
/// This pins the JSON backward-compat contract: bundle_id is always optional on the wire.
#[test]
fn app_info_bundle_id_missing_key_deserializes_to_none() {
    let json = r#"{"name":"Finder","pid":42}"#;
    let info: AppInfo =
        serde_json::from_str(json).expect("deserialize AppInfo without bundle_id key");
    assert_eq!(info.bundle_id, None);
    assert_eq!(info.name, "Finder");
    assert_eq!(info.pid, 42);
}

/// AccessibilityNode.bounds is annotated skip_serializing_if = "Option::is_none".
/// When the adapter or ref-alloc pipeline strips bounds, the key must not
/// appear in the JSON, keeping token counts low.
#[test]
fn accessibility_node_bounds_none_omitted_from_json() {
    let node = AccessibilityNode {
        ref_id: None,
        role: "button".into(),
        name: Some("OK".into()),
        value: None,
        description: None,
        hint: None,
        states: vec![],
        available_actions: vec![],
        bounds: None,
        children_count: None,
        children: vec![],
    };
    let json = serde_json::to_string(&node).unwrap();
    assert!(
        !json.contains("\"bounds\":"),
        "bounds must be absent when None, json={json}"
    );
}

/// AccessibilityNode.hint is annotated skip_serializing_if = "Option::is_none".
/// When not provided by the platform adapter, it must not appear in the JSON.
#[test]
fn accessibility_node_hint_none_omitted_from_json() {
    let node = AccessibilityNode {
        ref_id: None,
        role: "textfield".into(),
        name: None,
        value: None,
        description: None,
        hint: None,
        states: vec![],
        available_actions: vec![],
        bounds: None,
        children_count: None,
        children: vec![],
    };
    let json = serde_json::to_string(&node).unwrap();
    assert!(
        !json.contains("\"hint\":"),
        "hint must be absent when None, json={json}"
    );
}
