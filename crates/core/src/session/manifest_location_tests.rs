use super::*;
use crate::refs_test_support::HomeGuard;

/// `SESSION_MANIFEST_FILE` is public so a platform crate can read the
/// manifest as bytes without restating its name, which pins the constant to
/// wherever the writer actually puts the file. One that restated it read a
/// path that never existed, could not tell that from a deleted session, and
/// tore a live overlay down on a fixed timer.
#[test]
fn a_started_session_writes_its_manifest_where_the_public_constant_says() {
    let _guard = HomeGuard::new();
    let manifest = start_session(StartSessionOptions::default()).expect("a session starts");

    let expected = session_dir(&manifest.id)
        .expect("the session directory resolves")
        .join(SESSION_MANIFEST_FILE);

    let body = std::fs::read_to_string(&expected).unwrap_or_else(|error| {
        panic!(
            "the manifest must be readable at {}: {error}",
            expected.display()
        )
    });
    let parsed: serde_json::Value =
        serde_json::from_str(&body).expect("the manifest is the JSON a byte reader will parse");

    assert_eq!(
        parsed["id"], manifest.id,
        "the file at the published path is this session's manifest, not some other artifact"
    );
}

/// The other direction: a reader that composes the published path sees an
/// ended session as ended, so the constant carries the end signal too.
#[test]
fn ending_a_session_is_visible_to_a_reader_that_composes_the_published_path() {
    let _guard = HomeGuard::new();
    let manifest = start_session(StartSessionOptions::default()).expect("a session starts");
    let path = session_dir(&manifest.id)
        .expect("the session directory resolves")
        .join(SESSION_MANIFEST_FILE);

    let before: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("manifest readable"))
            .expect("manifest parses");
    assert!(
        before
            .get("ended_at")
            .is_none_or(serde_json::Value::is_null),
        "a live session must not already look ended, or the end signal proves nothing"
    );

    end_session(&manifest.id).expect("the session ends");

    let after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("manifest readable"))
            .expect("manifest parses");
    assert!(
        !after["ended_at"].is_null() && after.get("ended_at").is_some(),
        "the end must be visible in the file a byte reader opens"
    );
}
