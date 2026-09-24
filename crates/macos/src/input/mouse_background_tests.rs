use super::*;
use agent_desktop_core::{MouseButton, MouseEventKind, Point};

fn event(kind: MouseEventKind) -> MouseEvent {
    MouseEvent {
        kind,
        point: Point {
            x: -2850.0,
            y: 160.0,
        },
        button: MouseButton::Left,
        modifiers: Vec::new(),
    }
}

#[test]
fn failures_before_posting_are_reported_as_not_delivered() {
    let window = |id: &str| WindowInfo {
        id: id.to_string(),
        title: String::new(),
        app: "Code".to_string(),
        pid: agent_desktop_core::ProcessId::new(std::process::id()),
        process_instance: None,
        bounds: None,
        state: agent_desktop_core::WindowState::default(),
    };
    let cases = [
        (window("not-a-window"), MouseEventKind::Move),
        (window("w-99999999999"), MouseEventKind::Move),
        (
            window("w-9555"),
            MouseEventKind::Wheel {
                delta_x: 0.0,
                delta_y: 1.0,
            },
        ),
    ];

    for (target, kind) in cases {
        let deadline = Deadline::after(1_000).unwrap();
        let err = deliver(&target, event(kind), deadline).expect_err("must fail before posting");

        assert_eq!(
            err.disposition,
            agent_desktop_core::DeliverySemantics::not_delivered()
        );
    }
}
