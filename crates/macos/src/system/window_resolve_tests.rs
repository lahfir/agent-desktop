use super::*;
use crate::system::cg_window::WindowRecord;

fn record(app_name: &str, pid: i32, title: &str, window_number: i64) -> WindowRecord {
    WindowRecord {
        app_name: app_name.into(),
        pid,
        title: Some(title.into()),
        window_number,
        area: 100.0,
    }
}

fn win(id: &str, pid: i32, title: &str) -> WindowInfo {
    WindowInfo {
        id: id.into(),
        title: title.into(),
        app: "TextEdit".into(),
        pid,
        bounds: None,
        is_focused: false,
    }
}

#[test]
fn parse_window_number_accepts_w_prefix() {
    assert_eq!(parse_window_number("w-42"), Some(42));
    assert_eq!(parse_window_number("42"), None);
    assert_eq!(parse_window_number("w-bad"), None);
}

#[test]
fn verify_rejects_recycled_id_with_wrong_pid() {
    let requested = win("w-100", 10, "Untitled");
    let live = record("TextEdit", 99, "Untitled", 100);
    assert!(verify_window_record(&requested, &live).is_err());
}

#[test]
fn verify_rejects_title_mismatch_when_title_provided() {
    let requested = win("w-100", 10, "Doc A");
    let live = record("TextEdit", 10, "Doc B", 100);
    assert!(verify_window_record(&requested, &live).is_err());
}

#[test]
fn verify_accepts_matching_pid_and_title() {
    let requested = win("w-100", 10, "Untitled");
    let live = record("TextEdit", 10, "Untitled", 100);
    assert!(verify_window_record(&requested, &live).is_ok());
}

#[test]
fn verify_skips_title_when_request_title_empty() {
    let requested = WindowInfo {
        id: "w-100".into(),
        title: String::new(),
        app: "TextEdit".into(),
        pid: 10,
        bounds: None,
        is_focused: false,
    };
    let live = record("TextEdit", 10, "Any Title", 100);
    assert!(verify_window_record(&requested, &live).is_ok());
}
