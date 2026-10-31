use super::*;

#[test]
fn skewed_appkit_launch_date_passes_when_the_live_instance_matches() {
    let pid = i32::try_from(std::process::id()).expect("test pid fits macOS pid_t");
    let (process_instance, launch_time) = live_token_and_skewed_launch(pid, -9.0);
    let owners = Owners {
        launches: FxHashMap::from_iter([(pid, Some(launch_time))]),
        frontmost: None,
    };
    let mut current = record(pid, 1, "Current");
    current.process_instance = Some(process_instance);

    let windows = assemble_global_windows(vec![current], &owners, false, |_| {
        panic!("no AX probe without a frontmost owner")
    })
    .unwrap();

    assert_eq!(windows.len(), 1);
}

#[test]
fn skewed_appkit_launch_date_with_a_dead_instance_still_reports_churn() {
    let pid = i32::try_from(std::process::id()).expect("test pid fits macOS pid_t");
    let (process_instance, launch_time) = live_token_and_skewed_launch(pid, -9.0);
    let mut parts = process_instance.split(':');
    let prefix = parts.next().unwrap();
    let seconds = parts.next().unwrap().parse::<u64>().unwrap();
    let microseconds = parts.next().unwrap();
    let stale = format!("{prefix}:{}:{microseconds}", seconds + 1);
    let owners = Owners {
        launches: FxHashMap::from_iter([(pid, Some(launch_time))]),
        frontmost: None,
    };
    let mut stale_record = record(pid, 1, "Current");
    stale_record.process_instance = Some(stale);

    let error = assemble_global_windows(vec![stale_record], &owners, false, |_| {
        panic!("no AX probe without a frontmost owner")
    })
    .unwrap_err();

    assert_eq!(
        error.details.unwrap()["phase"],
        "process_generation_changed"
    );
}

fn live_token_and_skewed_launch(pid: i32, skew_seconds: f64) -> (String, f64) {
    let token = crate::system::process::process_identity::token_for_pid(pid)
        .unwrap()
        .expect("current process token");
    let start_seconds = token
        .split(':')
        .nth(1)
        .and_then(|value| value.parse::<u64>().ok())
        .expect("token carries start seconds");
    (token, start_seconds as f64 + skew_seconds)
}
