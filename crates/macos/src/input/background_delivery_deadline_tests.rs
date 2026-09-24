use super::*;

/// The focus record's settle can use up the whole budget. The key-window
/// pair is not sent after that, and neither is any input event.
#[test]
fn a_deadline_that_expires_during_the_activation_settle_sends_nothing_more() {
    let layers = BackgroundLayers {
        activate: true,
        key_window: true,
        ..skylight_only()
    };
    let mut io = FakeIo::new();

    let error = run(prepared(layers, double_click()), deadline(40), &mut io).unwrap_err();

    assert_eq!(io.sent, [Sent::FocusRecord]);
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    let details = error.details.expect("timeout details");
    assert_eq!(details["delivered_events"], 0);
    assert_eq!(details["planned_events"], 5);
}

#[test]
fn a_deadline_that_expires_between_click_pairs_stops_before_the_next_press() {
    let mut io = FakeIo::new();
    io.jump_after_post = Some((2, Duration::from_secs(10)));

    let error = run(
        prepared(skylight_only(), double_click()),
        deadline(1_000),
        &mut io,
    )
    .unwrap_err();

    assert_eq!(io.sent, (1..=3).map(Sent::SkyLight).collect::<Vec<_>>());
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let details = error.details.expect("timeout details");
    assert_eq!(details["delivered_events"], 3);
    assert_eq!(details["planned_events"], 5);
    assert_eq!(
        details["kind"], "deadline",
        "the deadline's own details survive"
    );
    assert!(details["timeout_ms"].is_u64());
}

#[test]
fn a_press_already_posted_is_released_even_after_the_deadline() {
    let mut io = FakeIo::new();
    io.jump_after_post = Some((1, Duration::from_secs(10)));

    let error = run(
        prepared(skylight_only(), double_click()),
        deadline(1_000),
        &mut io,
    )
    .unwrap_err();

    assert_eq!(
        io.sent,
        (1..=3).map(Sent::SkyLight).collect::<Vec<_>>(),
        "the button-up after the expired down is still posted"
    );
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
}

/// The final button-down can itself overrun the budget. Its release still
/// goes out so nothing stays held, but the delivery ran past its deadline
/// and must not report success.
#[test]
fn a_deadline_that_expires_after_the_final_press_still_releases_and_times_out() {
    let mut io = FakeIo::new();
    io.jump_after_post = Some((3, Duration::from_secs(10)));

    let error = run(
        prepared(skylight_only(), double_click()),
        deadline(1_000),
        &mut io,
    )
    .unwrap_err();

    assert_eq!(io.sent, (1..=5).map(Sent::SkyLight).collect::<Vec<_>>());
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let details = error.details.expect("timeout details");
    assert_eq!(details["delivered_events"], 5);
    assert_eq!(details["planned_events"], 5);
}

#[test]
fn a_move_that_overruns_the_deadline_times_out_as_delivered() {
    let mut io = FakeIo::new();
    io.jump_after_post = Some((0, Duration::from_secs(10)));

    let error = run(
        prepared(skylight_only(), vec![event(1, false, 0)]),
        deadline(1_000),
        &mut io,
    )
    .unwrap_err();

    assert_eq!(io.sent, [Sent::SkyLight(1)]);
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let details = error.details.expect("timeout details");
    assert_eq!(details["delivered_events"], 1);
    assert_eq!(details["planned_events"], 1);
}

/// Typed text is a run of key down/up pairs. When the budget runs out after
/// a key down, its key up still goes out, and the next key down does not.
#[test]
fn typed_text_releases_the_held_key_and_stops_before_the_next_key_down() {
    let typed = vec![
        event(1, false, 8),
        event(2, true, 16),
        event(3, false, 8),
        event(4, true, 16),
        event(5, false, 8),
        event(6, true, 16),
    ];
    let mut io = FakeIo::new();
    io.jump_after_post = Some((2, Duration::from_millis(500)));

    let error = run(prepared(skylight_only(), typed), deadline(100), &mut io).unwrap_err();

    assert_eq!(io.sent, (1..=4).map(Sent::SkyLight).collect::<Vec<_>>());
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let details = error.details.expect("partial delivery details");
    assert_eq!(details["delivered_events"], 4);
    assert_eq!(details["planned_events"], 6);
}
