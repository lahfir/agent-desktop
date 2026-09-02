use super::{EndWatch, SessionReading, classify};

fn unreadable() -> Option<std::io::Result<String>> {
    Some(Err(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "the manifest was being replaced",
    )))
}

fn missing() -> Option<std::io::Result<String>> {
    Some(Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "no manifest",
    )))
}

#[test]
fn a_live_session_reads_live() {
    let body = r#"{"id":"s0000001","cursor_overlay":{"enabled":true}}"#;

    assert_eq!(classify(Some(Ok(body.to_owned()))), SessionReading::Live);
}

#[test]
fn an_ended_session_reads_finished() {
    let body = r#"{"id":"s0000001","ended_at":"2026-09-01T00:00:00Z"}"#;

    assert_eq!(
        classify(Some(Ok(body.to_owned()))),
        SessionReading::Finished
    );
}

#[test]
fn a_missing_manifest_reads_finished() {
    assert_eq!(classify(missing()), SessionReading::Finished);
    assert_eq!(classify(None), SessionReading::Finished);
}

/// A `Present` that raced an acknowledged `Disable` can bring a fresh
/// renderer up against a session whose overlay is switched off. Without this
/// condition it would rest on screen until the session ended.
#[test]
fn a_session_whose_overlay_is_switched_off_reads_finished() {
    let body = r#"{"id":"s0000001","cursor_overlay":{"enabled":false}}"#;

    assert_eq!(
        classify(Some(Ok(body.to_owned()))),
        SessionReading::Finished
    );
}

/// The distinction this module exists for. Core's reader folds both of these
/// into the same value a deleted session produces, so polling it would end a
/// live overlay on a transient file error - a fault read as a fact, which is
/// the defect this separation exists to prevent, reintroduced by the very
/// mechanism added to close a different hole.
#[test]
fn an_unreadable_or_unparsable_manifest_reads_unknown_not_finished() {
    assert_eq!(classify(unreadable()), SessionReading::Unknown);
    assert_eq!(
        classify(Some(Ok("this is not json".to_owned()))),
        SessionReading::Unknown
    );
}

#[test]
fn one_unreadable_tick_never_ends_a_live_overlay() {
    let mut watch = EndWatch::default();

    assert!(!watch.observe(SessionReading::Live));
    assert!(!watch.observe(SessionReading::Unknown));
    assert!(!watch.observe(SessionReading::Unknown));
    assert!(!watch.observe(SessionReading::Live));
}

/// Two consecutive readings, so the manifest being rewritten mid-session
/// cannot be mistaken for the session ending.
#[test]
fn teardown_needs_two_consecutive_finished_readings() {
    let mut watch = EndWatch::default();

    assert!(!watch.observe(SessionReading::Finished));
    assert!(watch.observe(SessionReading::Finished));
}

#[test]
fn a_live_reading_between_two_finished_ones_resets_the_count() {
    let mut watch = EndWatch::default();

    assert!(!watch.observe(SessionReading::Finished));
    assert!(!watch.observe(SessionReading::Live));
    assert!(!watch.observe(SessionReading::Finished));
    assert!(watch.observe(SessionReading::Finished));
}
