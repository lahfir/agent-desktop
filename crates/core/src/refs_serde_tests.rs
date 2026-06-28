use super::*;
use crate::adapter::SnapshotSurface;

fn minimal_entry(role: &str) -> RefEntry {
    RefEntry {
        pid: 1,
        role: role.into(),
        name: None,
        value: None,
        description: None,
        states: vec![],
        bounds: None,
        bounds_hash: None,
        available_actions: vec![],
        source_app: None,
        source_window_id: None,
        source_window_title: None,
        source_surface: SnapshotSurface::Window,
        root_ref: None,
        path_is_absolute: false,
        path: smallvec::SmallVec::new(),
    }
}

/// Fields annotated with `skip_serializing_if` must be absent from the JSON
/// when they hold their zero/empty/default value. Agents parsing the wire
/// format must not break if any of these keys are missing.
#[test]
fn ref_entry_skip_fields_absent_when_none_empty_or_default() {
    let e = minimal_entry("button");
    let json = serde_json::to_string(&e).unwrap();

    assert!(
        !json.contains("\"value\":"),
        "value absent when None, json={json}"
    );
    assert!(
        !json.contains("\"description\":"),
        "description absent when None, json={json}"
    );
    assert!(
        !json.contains("\"states\":"),
        "states absent when empty, json={json}"
    );
    assert!(
        !json.contains("\"bounds\":"),
        "bounds absent when None, json={json}"
    );
    assert!(
        !json.contains("\"source_window_id\":"),
        "source_window_id absent when None, json={json}"
    );
    assert!(
        !json.contains("\"source_window_title\":"),
        "source_window_title absent when None, json={json}"
    );
    assert!(
        !json.contains("\"source_surface\":"),
        "source_surface absent for Window default, json={json}"
    );
    assert!(
        !json.contains("\"root_ref\":"),
        "root_ref absent when None, json={json}"
    );
    assert!(
        !json.contains("\"path_is_absolute\":"),
        "path_is_absolute absent when false, json={json}"
    );
    assert!(
        !json.contains("\"path\":"),
        "path absent when empty, json={json}"
    );
}

/// SnapshotSurface::Window is omitted from RefEntry JSON because it is the
/// default surface. A non-Window surface must appear as its snake_case string.
/// This pins the #[serde(skip_serializing_if = "SnapshotSurface::is_window")]
/// annotation on RefEntry.source_surface.
#[test]
fn ref_entry_source_surface_omitted_for_window_present_for_non_window() {
    let window_entry = minimal_entry("button");
    let window_json = serde_json::to_string(&window_entry).unwrap();
    assert!(
        !window_json.contains("\"source_surface\":"),
        "Window surface must be omitted as the default, json={window_json}"
    );

    let alert_entry = RefEntry {
        source_surface: SnapshotSurface::Alert,
        ..minimal_entry("button")
    };
    let alert_json = serde_json::to_string(&alert_entry).unwrap();
    assert!(
        alert_json.contains("\"source_surface\":\"alert\""),
        "Alert surface must serialize to 'alert', json={alert_json}"
    );

    let menu_entry = RefEntry {
        source_surface: SnapshotSurface::Menu,
        ..minimal_entry("button")
    };
    let menu_json = serde_json::to_string(&menu_entry).unwrap();
    assert!(
        menu_json.contains("\"source_surface\":\"menu\""),
        "Menu surface must serialize to 'menu', json={menu_json}"
    );
}

/// Every SnapshotSurface variant must serialize to its snake_case string
/// and round-trip through serde. This pins the wire format against accidental
/// rename and confirms #[non_exhaustive] has not changed existing variant names.
#[test]
fn snapshot_surface_serializes_to_snake_case_and_roundtrips() {
    let cases = [
        (SnapshotSurface::Window, "\"window\""),
        (SnapshotSurface::Focused, "\"focused\""),
        (SnapshotSurface::Menu, "\"menu\""),
        (SnapshotSurface::Menubar, "\"menubar\""),
        (SnapshotSurface::Sheet, "\"sheet\""),
        (SnapshotSurface::Popover, "\"popover\""),
        (SnapshotSurface::Alert, "\"alert\""),
    ];
    for (variant, expected_json) in cases {
        let serialized = serde_json::to_string(&variant).unwrap();
        assert_eq!(
            serialized, expected_json,
            "wrong wire string for {variant:?}"
        );
        let back: SnapshotSurface = serde_json::from_str(&serialized).unwrap();
        assert_eq!(back, variant, "round-trip failed for {variant:?}");
    }
}

/// RefEntry serializes and deserializes back to the same field values.
/// Uses field-by-field comparison because RefEntry does not derive PartialEq.
#[test]
fn ref_entry_full_roundtrip_preserves_all_fields() {
    let original = RefEntry {
        pid: 99,
        role: "textfield".into(),
        name: Some("Email".into()),
        value: Some("user@example.com".into()),
        description: Some("Enter email".into()),
        states: vec!["focused".into()],
        bounds: None,
        bounds_hash: Some(0xDEAD_BEEF),
        available_actions: vec!["SetValue".into(), "Click".into()],
        source_app: Some("Mail".into()),
        source_window_id: Some("w-7".into()),
        source_window_title: Some("Compose".into()),
        source_surface: SnapshotSurface::Sheet,
        root_ref: Some("@e5".into()),
        path_is_absolute: true,
        path: smallvec::SmallVec::from_slice(&[2, 0, 1]),
    };
    let json = serde_json::to_string(&original).unwrap();
    let back: RefEntry = serde_json::from_str(&json).unwrap();

    assert_eq!(back.pid, original.pid);
    assert_eq!(back.role, original.role);
    assert_eq!(back.name, original.name);
    assert_eq!(back.value, original.value);
    assert_eq!(back.description, original.description);
    assert_eq!(back.states, original.states);
    assert_eq!(back.bounds_hash, original.bounds_hash);
    assert_eq!(back.available_actions, original.available_actions);
    assert_eq!(back.source_app, original.source_app);
    assert_eq!(back.source_window_id, original.source_window_id);
    assert_eq!(back.source_window_title, original.source_window_title);
    assert_eq!(back.source_surface, original.source_surface);
    assert_eq!(back.root_ref, original.root_ref);
    assert_eq!(back.path_is_absolute, original.path_is_absolute);
    assert_eq!(back.path.as_slice(), original.path.as_slice());
}
