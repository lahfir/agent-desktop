use super::*;
use crate::commands::wait::{WaitModeArgs, WaitPredicateArgs};

fn args(mode: WaitModeArgs) -> WaitArgs {
    WaitArgs {
        mode,
        predicate: WaitPredicateArgs {
            snapshot_id: None,
            predicate: None,
            value: None,
            action: None,
            count: None,
        },
        timeout_ms: 1_000,
        app: None,
    }
}

fn mode() -> WaitModeArgs {
    WaitModeArgs {
        ms: None,
        element: None,
        window: None,
        text: None,
        menu: false,
        menu_closed: false,
        notification: false,
        event: None,
        window_id: None,
    }
}

#[test]
fn event_with_window_title_narrowing_is_a_single_mode() {
    let result = validate_wait_mode(&args(WaitModeArgs {
        event: Some("window-opened".into()),
        window: Some("Untitled".into()),
        ..mode()
    }));
    assert!(
        result.is_ok(),
        "--window alongside --event must narrow the event wait, not count as a second mode: {result:?}"
    );
}

#[test]
fn event_alone_is_still_a_single_mode() {
    let result = validate_wait_mode(&args(WaitModeArgs {
        event: Some("window-opened".into()),
        ..mode()
    }));
    assert!(result.is_ok());
}

#[test]
fn window_and_element_together_remain_ambiguous() {
    let result = validate_wait_mode(&args(WaitModeArgs {
        window: Some("Untitled".into()),
        element: Some("@e1".into()),
        ..mode()
    }));
    assert!(result.is_err());
}

#[test]
fn from_args_threads_window_title_into_event_mode() {
    let parsed = WaitMode::from_args(args(WaitModeArgs {
        event: Some("window-opened".into()),
        window: Some("Untitled".into()),
        ..mode()
    }))
    .unwrap();
    match parsed {
        WaitMode::Event {
            event,
            window_title,
            ..
        } => {
            assert_eq!(event, "window-opened");
            assert_eq!(window_title.as_deref(), Some("Untitled"));
        }
        _ => panic!("expected WaitMode::Event, got a different mode"),
    }
}
