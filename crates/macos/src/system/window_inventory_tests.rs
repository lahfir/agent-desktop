use super::*;
use crate::system::cg_window::WindowRecord;
use agent_desktop_core::Rect;
use rustc_hash::FxHashMap;

fn ax_state(focused: FocusedWindowIdentity) -> crate::system::window_ax_state::WindowAxState {
    crate::system::window_ax_state::WindowAxState {
        focused,
        minimized_by_id: FxHashMap::default(),
    }
}

#[test]
fn apps_from_window_records_deduplicates_by_pid() {
    let apps = apps_from_window_records(&[
        record("Finder", 10, "Window 1", 1),
        record("Finder", 10, "Window 2", 2),
    ]);

    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].name, "Finder");
}

#[test]
fn apps_from_window_records_keeps_same_name_with_distinct_pids() {
    let apps = apps_from_window_records(&[
        record("Preview", 10, "Preview", 10),
        record("Preview", 11, "Preview", 11),
    ]);

    assert_eq!(apps.len(), 2);
}

#[test]
fn matches_app_filter_accepts_exact_case_insensitive_name() {
    assert!(matches_app_filter("Docker Desktop", "docker desktop"));
    assert!(!matches_app_filter("Finder", "docker"));
}

#[test]
fn matches_app_filter_rejects_substring_match() {
    assert!(!matches_app_filter("Mail Helper", "mail"));
}

#[test]
fn windows_from_records_marks_single_focused_window_once() {
    let windows = windows_from_records_with_focus(
        vec![
            record("Mail", 10, "Inbox", 1),
            record("Mail", 10, "Inbox", 2),
        ],
        false,
        |_| Ok(ax_state(Some((Some("Inbox".to_string()), Some(2))))),
        |_, _| Ok(true),
    )
    .unwrap();

    assert!(!windows[0].state.is_focused);
    assert!(windows[1].state.is_focused);
}

#[test]
fn windows_from_records_focus_only_filters_unfocused_windows() {
    let windows = windows_from_records_with_focus(
        vec![
            record("Mail", 10, "Inbox", 1),
            record("Mail", 10, "Sent", 2),
        ],
        true,
        |_| Ok(ax_state(Some((Some("Sent".to_string()), Some(2))))),
        |_, _| Ok(true),
    )
    .unwrap();

    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0].title, "Sent");
}

#[test]
fn windows_from_records_preserve_capture_bounds() {
    let expected = Rect {
        x: 12.0,
        y: 34.0,
        width: 800.0,
        height: 600.0,
    };
    let mut source = record("Preview", 10, "Document", 7);
    source.bounds = expected;

    let windows = windows_from_records_with_focus(
        vec![source],
        false,
        |_| Ok(ax_state(None)),
        |_, _| Ok(true),
    )
    .unwrap();

    assert_eq!(windows[0].bounds, Some(expected));
}

#[test]
fn windows_from_records_never_publish_zero_window_ids() {
    let windows = windows_from_records_with_focus(
        vec![record("Preview", 10, "Unverified", 0)],
        false,
        |_| Ok(ax_state(None)),
        |_, _| Ok(true),
    )
    .unwrap();

    assert!(windows.is_empty());
}

#[test]
fn window_inventory_rejects_owner_generation_change_around_focus_read() {
    let mut checks = 0;
    let error = windows_from_records_with_focus(
        vec![record("Preview", 10, "Document", 7)],
        false,
        |_| Ok(ax_state(None)),
        |_, _| {
            checks += 1;
            Ok(checks == 1)
        },
    )
    .expect_err("owner changed after AX join");

    assert_eq!(error.code, agent_desktop_core::ErrorCode::AppUnresponsive);
    assert_eq!(error.details.unwrap()["kind"], "window_identity_race");
}

#[test]
fn matches_focused_window_uses_window_number_when_available() {
    let identity = Some((Some("Other".to_string()), Some(7)));

    assert!(matches_focused_window("Inbox", 7, &identity, 3));
    assert!(!matches_focused_window("Inbox", 8, &identity, 1));
}

#[test]
fn matches_focused_window_uses_unique_title_without_window_number() {
    let identity = Some((Some("Inbox".to_string()), None));

    assert!(matches_focused_window("Inbox", 0, &identity, 1));
    assert!(!matches_focused_window("Inbox", 0, &identity, 2));
    assert!(!matches_focused_window("Sent", 0, &identity, 1));
}

#[test]
fn deadline_window_inventory_rejects_expiry_before_native_reads() {
    let filter = WindowFilter {
        app: None,
        focused_only: false,
    };

    let error = list_windows_until(&filter, Instant::now()).unwrap_err();

    assert_eq!(error.code.as_str(), "TIMEOUT");
}

fn record(app_name: &str, pid: i32, title: &str, window_number: i64) -> WindowRecord {
    WindowRecord {
        app_name: app_name.to_string(),
        pid,
        title: Some(title.to_string()),
        window_number,
        bounds: agent_desktop_core::Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        },
        visible: true,
        process_instance: Some(format!("instance-{pid}")),
    }
}
