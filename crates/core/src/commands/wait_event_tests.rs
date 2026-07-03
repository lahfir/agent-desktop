use super::*;

#[test]
fn parse_app_activated_requires_app() {
    let err = parse_event_signal("app_activated", None, None, None).unwrap_err();
    assert!(err.to_string().contains("--app"));
}

#[test]
fn parse_window_focused_builds_signal() {
    let signal = parse_event_signal(
        "window_focused",
        None,
        Some("w-1".into()),
        Some("Docs".into()),
    )
    .unwrap();
    assert_eq!(
        signal,
        DesktopSignal::WindowFocused {
            window_id: "w-1".into(),
            title: "Docs".into(),
        }
    );
}
