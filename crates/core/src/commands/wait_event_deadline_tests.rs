use super::*;

#[test]
fn late_matching_observation_is_discarded() {
    let current = baseline_with_windows(vec![window("w-late", "Too late")]);
    let adapter = SequenceAdapter::new(vec![current]).with_delay(Duration::from_millis(30));
    let mut request = input("window-opened", None);
    request.timeout_ms = 10;

    let error = wait_for_event(request, &adapter, Some(Ok(empty_baseline())))
        .expect_err("an observation returned after the deadline must not match");

    assert_eq!(error.code(), "TIMEOUT");
    assert_eq!(*adapter.calls.lock().unwrap(), 1);
    let remaining = adapter.remaining_at_call.lock().unwrap()[0];
    assert!(remaining > Duration::ZERO);
    assert!(remaining <= Duration::from_millis(10));
}

#[test]
fn zero_timeout_does_not_start_an_observation() {
    let adapter = SequenceAdapter::new(vec![empty_baseline()]);
    let mut request = input("window-opened", None);
    request.timeout_ms = 0;

    let error = wait_for_event(request, &adapter, None)
        .expect_err("a zero timeout must expire before baseline capture");

    assert_eq!(error.code(), "TIMEOUT");
    assert_eq!(*adapter.calls.lock().unwrap(), 0);
}

#[test]
fn oversized_timeout_cannot_overflow_the_deadline() {
    let start = Instant::now();

    let error = crate::Deadline::at(start, u64::MAX).unwrap_err();
    assert_eq!(error.code, ErrorCode::InvalidArgs);
}

#[test]
fn parse_event_kind_accepts_every_documented_token() {
    for token in EventKind::all_tokens() {
        assert!(
            parse_event_kind(token).is_ok(),
            "token '{token}' from EventKind::all_tokens() must parse"
        );
    }
}
