use super::*;
use std::os::unix::net::UnixListener;

#[test]
fn travel_acknowledgement_obeys_remaining_budget() {
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
        .unwrap();
        let control = CursorOverlayControl::present("run-budget".into(), instruction);
        let started = Instant::now();
        assert!(send_until(&path, &control, started + Duration::from_millis(30)).unwrap());
        let elapsed = started.elapsed();
        receiver.join().unwrap();
        assert!(elapsed < Duration::from_millis(500), "{elapsed:?}");
    });
    let _ = std::fs::remove_file(path);
    result.unwrap();
}
