use super::*;

fn deadline() -> Instant {
    Instant::now() + std::time::Duration::from_secs(1)
}

fn snapshot_with(application_pid: i32, frontmost_pid: i32) -> String {
    format!(
        r#"{{
            "applications":[
                {{"name":"Mail","pid":{application_pid},"launch_time":100.25,"activation_policy":"regular"}}
            ],
            "frontmost_pid":{frontmost_pid},
            "frontmost_launch_time":null
        }}"#
    )
}

#[test]
fn owner_snapshot_rejects_non_positive_application_pid() {
    for pid in [0, -1] {
        let bytes = snapshot_with(pid, 0);
        let error = window_owner_snapshot_from_json(bytes.as_bytes(), deadline())
            .err()
            .unwrap();

        assert_eq!(error.code, ErrorCode::AppUnresponsive);
    }
}

#[test]
fn owner_snapshot_rejects_negative_frontmost_pid() {
    let bytes = snapshot_with(10, -1);
    let error = window_owner_snapshot_from_json(bytes.as_bytes(), deadline())
        .err()
        .unwrap();

    assert_eq!(error.code, ErrorCode::AppUnresponsive);
}

#[test]
fn owner_snapshot_treats_zero_frontmost_pid_without_launch_time_as_none() {
    let bytes = snapshot_with(10, 0);
    let snapshot = window_owner_snapshot_from_json(bytes.as_bytes(), deadline()).unwrap();

    assert!(snapshot.frontmost().is_none());
}

#[test]
fn app_listing_rejects_non_positive_application_pid() {
    for pid in [0, -1] {
        let bytes = snapshot_with(pid, 0);
        let error = apps_from_json(bytes.as_bytes(), deadline()).err().unwrap();

        assert_eq!(error.code, ErrorCode::AppUnresponsive);
    }
}
