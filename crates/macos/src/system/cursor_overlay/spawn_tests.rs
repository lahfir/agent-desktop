use super::*;
use std::os::unix::net::UnixListener;

#[test]
fn travel_acknowledgement_obeys_remaining_budget() {
    check_acknowledgement(agent_desktop_core::CursorPhase::Travel, false);
}

#[test]
fn drag_requires_acknowledgement_before_tracking_is_considered_ready() {
    check_acknowledgement(agent_desktop_core::CursorPhase::Drag, false);
}

fn check_acknowledgement(phase: agent_desktop_core::CursorPhase, accepted: bool) {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::path::PathBuf::from(format!("/tmp/ca-{}-{unique:x}.sock", std::process::id()));
    let result = std::panic::catch_unwind(|| {
        let listener = UnixListener::bind(&path).unwrap();
        let receiver = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut payload = Vec::new();
            stream.read_to_end(&mut payload).unwrap();
            thread::sleep(Duration::from_millis(700));
        });
        let instruction = agent_desktop_core::CursorOverlayInstruction::new(
            agent_desktop_core::Point { x: 10.0, y: 20.0 },
            &agent_desktop_core::CursorOverlayConfig::enabled(None, 8).unwrap(),
            false,
        )
        .unwrap()
        .with_phase(phase);
        let control = CursorOverlayControl::present("run-budget".into(), instruction);
        let started = Instant::now();
        assert_eq!(
            send_until(&path, &control, started + Duration::from_millis(30)).is_ok(),
            accepted
        );
        let elapsed = started.elapsed();
        receiver.join().unwrap();
        assert!(elapsed < Duration::from_millis(500), "{elapsed:?}");
    });
    let _ = std::fs::remove_file(path);
    result.unwrap();
}
