use super::*;

const NOTEPAD: &str =
    include_str!("../../../../docs/plans/2026-07-27-002-captures/notepad-com.json");
const EXPLORER: &str =
    include_str!("../../../../docs/plans/2026-07-27-002-captures/explorer-com.json");

fn captures() -> [(&'static str, &'static str); 2] {
    [(CAPTURE_FILES[0], NOTEPAD), (CAPTURE_FILES[1], EXPLORER)]
}

/// The redaction rule, not the contents. A capture that still carries a raw
/// process id, provider id, window handle or user path publishes a fingerprint
/// of the machine that produced it, and a partial substitution is worse than
/// none because it reads as redacted.
#[test]
fn no_committed_capture_leaks_a_host_identifier() {
    for (name, body) in captures() {
        for (key, placeholder) in [("pid:", "<pid>"), ("providerId:", "<providerid>")] {
            for occurrence in body.split(key).skip(1) {
                assert!(
                    occurrence.starts_with(placeholder),
                    "{name} leaks a value after {key}: {}",
                    &occurrence[..occurrence.len().min(24)]
                );
            }
        }
        assert!(
            !body.to_ascii_lowercase().contains(r"\users\"),
            "{name} carries a user-profile path"
        );
        assert!(
            !body.to_ascii_lowercase().contains("hwnd:0x"),
            "{name} carries a raw window handle"
        );
    }
}

/// The metadata whose absence made 2.0's dumps unusable as COM expectations.
#[test]
fn every_committed_capture_records_its_variant_build_and_stack() {
    for (name, body) in captures() {
        let document: serde_json::Value =
            serde_json::from_str(body).unwrap_or_else(|_| panic!("{name} is valid JSON"));
        for field in ["target_variant", "os_build", "client_stack", "crate"] {
            let value = document
                .get(field)
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            assert!(!value.is_empty(), "{name} does not record {field}");
        }
        assert_eq!(document["client_stack"], "uia3-com");
    }
}

/// A capture that resolved nothing is not evidence. The tool reports an
/// unresolvable target as a structured skip with a non-zero exit instead, so a
/// committed capture always holds a tree the walker actually reached.
#[test]
fn no_committed_capture_is_an_empty_tree() {
    for (name, body) in captures() {
        let document: serde_json::Value =
            serde_json::from_str(body).unwrap_or_else(|_| panic!("{name} is valid JSON"));
        assert!(
            document["node_count"].as_u64().unwrap_or_default() > 0,
            "{name} records no nodes, which reads as evidence that the tree was empty"
        );
        assert!(
            document.get("walk").is_some(),
            "{name} records no walk verdict"
        );
        for census in ["control_types", "providers", "sample"] {
            assert!(
                document[census]
                    .as_array()
                    .is_some_and(|rows| !rows.is_empty()),
                "{name} records an empty {census} census"
            );
        }
    }
}

const VOCAB_NOTEPAD: &str =
    include_str!("../../../../docs/plans/2026-07-31-001-captures/notepad.json");
const VOCAB_EXPLORER: &str =
    include_str!("../../../../docs/plans/2026-07-31-001-captures/explorer.json");
const VOCAB_WINFORMS: &str =
    include_str!("../../../../docs/plans/2026-07-31-001-captures/winforms.json");
const VOCAB_WPF: &str = include_str!("../../../../docs/plans/2026-07-31-001-captures/wpf.json");

fn vocabulary_captures() -> [(&'static str, &'static str); 4] {
    [
        (VOCABULARY_CAPTURE_FILES[0], VOCAB_NOTEPAD),
        (VOCABULARY_CAPTURE_FILES[1], VOCAB_EXPLORER),
        (VOCABULARY_CAPTURE_FILES[2], VOCAB_WINFORMS),
        (VOCABULARY_CAPTURE_FILES[3], VOCAB_WPF),
    ]
}

/// The redaction rule for the properties this sub-phase added, asserted on the
/// artifacts that actually get committed.
///
/// U9 is the only unit that reads real, uncontrolled third-party applications,
/// and a folder window's contents are somebody's file names. Every text field
/// must render as a presence object; a bare string means the property reached
/// the census without the presence-only treatment.
#[test]
fn no_vocabulary_capture_records_text_read_from_a_real_application() {
    for (name, body) in vocabulary_captures() {
        let document: serde_json::Value = serde_json::from_str(body)
            .unwrap_or_else(|error| panic!("{name} is not JSON: {error}"));
        let mut checked = 0;
        assert_presence_only(&document, name, &mut checked);
        assert!(
            checked > 0,
            "{name} carries none of the fields the rule covers, so this test proved nothing"
        );
    }
}

fn assert_presence_only(value: &serde_json::Value, name: &str, checked: &mut usize) {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                assert_presence_only(item, name, checked);
            }
        }
        serde_json::Value::Object(fields) => {
            for (key, field) in fields {
                if PRESENCE_ONLY_FIELDS.contains(&key.as_str()) {
                    *checked += 1;
                    assert!(
                        field.is_object(),
                        "{name} records {key} as a bare value, so its content reached the capture"
                    );
                    assert!(
                        field.get("present").is_some(),
                        "{name} records {key} without the presence shape"
                    );
                }
                assert_presence_only(field, name, checked);
            }
        }
        _ => {}
    }
}

/// The same host-identifier rule the 2.2 captures are held to, applied to the
/// four captures this sub-phase commits.
#[test]
fn no_vocabulary_capture_leaks_a_host_identifier() {
    for (name, body) in vocabulary_captures() {
        for (key, placeholder) in [("pid:", "<pid>"), ("providerId:", "<providerid>")] {
            for occurrence in body.split(key).skip(1) {
                assert!(
                    occurrence.starts_with(placeholder),
                    "{name} leaks a value after {key}"
                );
            }
        }
        assert!(
            !body.to_ascii_lowercase().contains(r"\users\"),
            "{name} carries a user-profile path"
        );
        assert!(
            !body.to_ascii_lowercase().contains("hwnd:0x"),
            "{name} carries a raw window handle"
        );
    }
}
