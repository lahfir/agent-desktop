use super::*;

#[test]
fn accepted_stream_waits_for_a_delayed_control() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path =
        std::path::Path::new("/tmp").join(format!("cd-{}-{unique:x}.sock", std::process::id()));
    let result = std::panic::catch_unwind(|| {
        let listener = UnixListener::bind(&path).unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut client = UnixStream::connect(&path).unwrap();
        let (stream, _) = listener.accept().unwrap();
        prepare_stream(&stream).unwrap();
        let sender = thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            let control = CursorOverlayControl::hide("run-delayed".into());
            client
                .write_all(&serde_json::to_vec(&control).unwrap())
                .unwrap();
        });
        assert_eq!(read_control(stream).unwrap().session_id(), "run-delayed");
        sender.join().unwrap();
    });
    let _ = std::fs::remove_file(path);
    result.unwrap();
}

fn state(at: Option<Point>) -> OverlayState {
    OverlayState {
        at,
        ..OverlayState::default()
    }
}

fn screen() -> agent_desktop_core::Rect {
    agent_desktop_core::Rect {
        x: 0.0,
        y: 0.0,
        width: 1920.0,
        height: 1080.0,
    }
}

fn instruction(destination: Point, click: bool) -> CursorOverlayInstruction {
    let config = CursorOverlayConfig::enabled(None, 6).expect("valid config");
    CursorOverlayInstruction::new(destination, &config, click).expect("valid instruction")
}

#[test]
fn sampled_motion_ends_at_the_requested_destination() {
    let destination = Point { x: 900.0, y: 500.0 };
    let frames = motion_frames(
        &state(None),
        &instruction(destination.clone(), false),
        &screen(),
        120,
    );

    assert_eq!(frames.last().map(|pose| &pose.point), Some(&destination));
    let biggest_step = frames
        .windows(2)
        .map(|pair| (pair[1].point.x - pair[0].point.x).hypot(pair[1].point.y - pair[0].point.y))
        .fold(0.0_f64, f64::max);
    assert!(frames.len() >= 20, "the path is sampled per frame");
    assert!(
        biggest_step < 40.0,
        "no visible jump between frames: {biggest_step}"
    );
}

#[test]
fn subsequent_motion_starts_from_the_previous_destination() {
    let start = Point { x: 200.0, y: 300.0 };
    let destination = Point { x: 900.0, y: 500.0 };
    let frames = motion_frames(
        &state(Some(start.clone())),
        &instruction(destination.clone(), false),
        &screen(),
        120,
    );

    assert_eq!(frames.first().map(|pose| &pose.point), Some(&start));
    assert_eq!(frames.last().map(|pose| &pose.point), Some(&destination));
}

#[test]
fn a_click_instruction_adds_ripple_frames_at_the_destination() {
    let destination = Point { x: 900.0, y: 500.0 };
    let moved = motion_frames(
        &state(None),
        &instruction(destination.clone(), false),
        &screen(),
        120,
    );
    let clicked = motion_frames(
        &state(None),
        &instruction(destination.clone(), true),
        &screen(),
        120,
    );

    assert!(clicked.len() > moved.len());
    assert!(clicked.iter().any(|pose| pose.ripple > 0.0));
    assert_eq!(clicked.last().map(|pose| &pose.point), Some(&destination));
}

#[test]
fn click_effect_returns_immediately_to_the_control_loop() {
    let destination = Point { x: 900.0, y: 500.0 };
    let effect =
        instruction(destination.clone(), true).with_phase(agent_desktop_core::CursorPhase::Effect);

    let frames = motion_frames(&state(Some(destination.clone())), &effect, &screen(), 120);

    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0], CursorPose::still(destination.clone()));
    assert_eq!(frames[1].point, destination);
    assert_eq!(frames[1].ripple, 1.0);
}
