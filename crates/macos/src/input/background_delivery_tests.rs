use super::*;
use agent_desktop_core::{DeliverySemantics, ErrorCode, ProcessId, WindowState};
use core_graphics::event::EventField;
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

const USER: i32 = 100;
const TARGET: i32 = 200;
const WINDOW: u32 = 9555;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sent {
    FocusRecord,
    SkyLight(i64),
    PostToPid(i64),
}

/// Scripted delivery environment. The clock only moves when the delivery
/// sleeps or when `jump_after_post` says a post took that long, so deadline
/// expiry lands on an exact event.
struct FakeIo {
    now: Duration,
    skylight_available: bool,
    focus_record: Result<(), String>,
    jump_after_post: Option<(usize, Duration)>,
    sent: Vec<Sent>,
}

impl FakeIo {
    fn new() -> Self {
        Self {
            now: Duration::ZERO,
            skylight_available: true,
            focus_record: Ok(()),
            jump_after_post: None,
            sent: Vec::new(),
        }
    }

    fn posts(&self) -> usize {
        self.sent
            .iter()
            .filter(|sent| **sent != Sent::FocusRecord)
            .count()
    }

    fn record_post(&mut self, sent: Sent) {
        self.sent.push(sent);
        if let Some((index, jump)) = self.jump_after_post
            && self.posts() == index + 1
        {
            self.now += jump;
        }
    }
}

impl GuardIo for FakeIo {
    fn now(&mut self) -> Duration {
        self.now
    }

    fn frontmost(&mut self) -> Option<i32> {
        Some(USER)
    }

    fn restore(&mut self, _pid: i32) -> bool {
        true
    }

    fn sleep(&mut self, duration: Duration) {
        self.now += duration;
    }
}

impl DeliveryIo for FakeIo {
    fn focus_target_window(&mut self, pid: libc::pid_t, window_number: u32) -> Result<(), String> {
        assert_eq!((pid, window_number), (TARGET, WINDOW));
        self.sent.push(Sent::FocusRecord);
        self.focus_record.clone()
    }

    fn post_skylight(&mut self, pid: libc::pid_t, event: &CGEvent) -> bool {
        assert_eq!(pid, TARGET);
        if !self.skylight_available {
            return false;
        }
        self.record_post(Sent::SkyLight(marker_of(event)));
        true
    }

    fn post_to_pid(&mut self, pid: libc::pid_t, event: &CGEvent) {
        assert_eq!(pid, TARGET);
        self.record_post(Sent::PostToPid(marker_of(event)));
    }
}

fn marker_of(event: &CGEvent) -> i64 {
    event.get_integer_value_field(EventField::EVENT_SOURCE_USER_DATA)
}

/// An event tagged with `marker` so the fake can tell events apart.
fn event(marker: i64, completes_press: bool, pause_after_ms: u64) -> PreparedEvent {
    let source = CGEventSource::new(CGEventSourceStateID::Private).unwrap();
    let event = CGEvent::new(source).unwrap();
    event.set_integer_value_field(EventField::EVENT_SOURCE_USER_DATA, marker);
    PreparedEvent {
        event,
        pause_after: Duration::from_millis(pause_after_ms),
        completes_press,
    }
}

/// A routed double click: move, then two down/up pairs.
fn double_click() -> Vec<PreparedEvent> {
    vec![
        event(1, false, 15),
        event(2, false, 10),
        event(3, true, 30),
        event(4, false, 10),
        event(5, true, 0),
    ]
}

fn prepared(layers: BackgroundLayers, events: Vec<PreparedEvent>) -> Prepared {
    Prepared {
        pid: TARGET,
        window_number: WINDOW,
        layers,
        events,
        degradations: Vec::new(),
    }
}

fn skylight_only() -> BackgroundLayers {
    BackgroundLayers {
        skylight: true,
        ..BackgroundLayers::default()
    }
}

fn deadline(ms: u64) -> Deadline {
    Deadline::after(ms).unwrap()
}

#[test]
fn a_missing_skylight_symbol_falls_back_to_post_to_pid_for_every_event() {
    let mut io = FakeIo::new();
    io.skylight_available = false;

    let report = run(
        prepared(skylight_only(), double_click()),
        deadline(5_000),
        &mut io,
    )
    .unwrap();

    assert_eq!(
        io.sent,
        (1..=5).map(Sent::PostToPid).collect::<Vec<_>>(),
        "each event is posted once, in order, through the fallback"
    );
    assert_eq!(
        report.degradations,
        ["skylight:SLEventPostToPid_unavailable"]
    );
}

#[test]
fn an_available_skylight_symbol_is_the_only_posting_path() {
    let mut io = FakeIo::new();

    let report = run(
        prepared(skylight_only(), double_click()),
        deadline(5_000),
        &mut io,
    )
    .unwrap();

    assert_eq!(io.sent, (1..=5).map(Sent::SkyLight).collect::<Vec<_>>());
    assert!(report.degradations.is_empty());
}

#[test]
fn without_the_skylight_layer_events_go_straight_to_post_to_pid() {
    let mut io = FakeIo::new();

    run(
        prepared(BackgroundLayers::default(), double_click()),
        deadline(5_000),
        &mut io,
    )
    .unwrap();

    assert_eq!(io.sent, (1..=5).map(Sent::PostToPid).collect::<Vec<_>>());
}

#[test]
fn the_focus_record_precedes_the_events_and_its_failure_only_degrades() {
    let layers = BackgroundLayers {
        activate: true,
        ..skylight_only()
    };
    let mut io = FakeIo::new();
    run(prepared(layers, double_click()), deadline(5_000), &mut io).unwrap();
    assert_eq!(io.sent[0], Sent::FocusRecord);
    assert_eq!(io.posts(), 5);

    let mut io = FakeIo::new();
    io.focus_record = Err("activate:SLPSPostEventRecordTo_unavailable".into());
    let report = run(prepared(layers, double_click()), deadline(5_000), &mut io).unwrap();
    assert_eq!(io.posts(), 5);
    assert_eq!(
        report.degradations,
        ["activate:SLPSPostEventRecordTo_unavailable"]
    );
}

#[test]
fn a_deadline_that_expires_during_the_activation_settle_posts_nothing() {
    let layers = BackgroundLayers {
        activate: true,
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

/// A wheel chunk starts nothing that needs finishing, so a budget that runs
/// out mid-scroll stops before the next chunk and reports what was posted.
#[test]
fn a_deadline_that_expires_between_wheel_chunks_stops_the_scroll() {
    let mut io = FakeIo::new();
    io.jump_after_post = Some((1, Duration::from_secs(10)));
    let wheel = vec![
        event(1, false, 0),
        event(2, false, 10),
        event(3, false, 10),
        event(4, false, 0),
    ];

    let error = run(prepared(skylight_only(), wheel), deadline(1_000), &mut io).unwrap_err();

    assert_eq!(io.sent, vec![Sent::SkyLight(1), Sent::SkyLight(2)]);
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let details = error.details.expect("timeout details");
    assert_eq!(details["delivered_events"], 2);
    assert_eq!(details["planned_events"], 4);
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

#[test]
fn repeated_degradations_are_reported_once() {
    let mut degradations = Vec::new();
    note_once(&mut degradations, "skylight:SLEventPostToPid_unavailable");
    note_once(&mut degradations, "skylight:SLEventPostToPid_unavailable");
    assert_eq!(degradations, ["skylight:SLEventPostToPid_unavailable"]);
}

#[test]
fn guard_needs_a_known_frontmost_app() {
    let mut degradations = Vec::new();
    let layers = BackgroundLayers::recommended();

    assert!(start_guard(layers, None, TARGET, &mut degradations).is_none());
    assert_eq!(degradations, ["guard:frontmost_unknown"]);
    assert!(start_guard(layers, Some(7), TARGET, &mut degradations).is_some());
    assert!(
        start_guard(
            BackgroundLayers::default(),
            Some(7),
            TARGET,
            &mut degradations
        )
        .is_none()
    );
}

#[test]
fn window_numbers_must_fit_a_cg_window_id() {
    let window = |id: &str| WindowInfo {
        id: id.to_string(),
        title: String::new(),
        app: "Code".to_string(),
        pid: ProcessId::new(1),
        process_instance: None,
        bounds: None,
        state: WindowState::default(),
    };

    assert_eq!(window_number(&window("w-15592")).unwrap(), 15592);
    for id in ["not-a-window", "w-99999999999"] {
        assert_eq!(
            window_number(&window(id)).unwrap_err().code,
            ErrorCode::InvalidArgs
        );
    }
}
