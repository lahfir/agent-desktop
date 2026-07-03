use super::*;
use agent_desktop_core::process_state::ProcessState;
use std::cell::Cell;

#[test]
fn dead_pid_classifies_exited_without_consulting_probe() {
    let probe_calls = Cell::new(0);
    let state = classify(false, || {
        probe_calls.set(probe_calls.get() + 1);
        AxProbeResult::Responsive
    });
    assert_eq!(state, ProcessState::Exited { code: None });
    assert_eq!(
        probe_calls.get(),
        0,
        "a dead pid must short-circuit before probing AX at all"
    );
}

#[test]
fn single_transient_cannot_complete_does_not_classify_unresponsive() {
    let calls = Cell::new(0);
    let state = classify(true, || {
        let n = calls.get() + 1;
        calls.set(n);
        if n == 1 {
            AxProbeResult::CannotComplete
        } else {
            AxProbeResult::Responsive
        }
    });
    assert_eq!(
        state,
        ProcessState::Running,
        "one transient AX blip on a healthy-but-busy app must not hard-fail as Unresponsive"
    );
    assert_eq!(calls.get(), 2, "a transient failure retries exactly once");
}

#[test]
fn two_consecutive_cannot_complete_classifies_unresponsive() {
    let calls = Cell::new(0);
    let state = classify(true, || {
        calls.set(calls.get() + 1);
        AxProbeResult::CannotComplete
    });
    assert_eq!(state, ProcessState::Unresponsive);
    assert_eq!(
        calls.get(),
        2,
        "classification requires exactly a second consecutive CannotComplete, not more"
    );
}

#[test]
fn immediately_responsive_pid_classifies_running_with_one_probe() {
    let calls = Cell::new(0);
    let state = classify(true, || {
        calls.set(calls.get() + 1);
        AxProbeResult::Responsive
    });
    assert_eq!(state, ProcessState::Running);
    assert_eq!(calls.get(), 1);
}

#[cfg(target_os = "macos")]
#[test]
fn exited_child_process_is_classified_exited_with_no_code() {
    let mut child = std::process::Command::new("/bin/echo")
        .arg("hi")
        .spawn()
        .expect("spawn /bin/echo");
    let pid = child.id() as i32;
    child.wait().expect("wait for exit");
    for _ in 0..50 {
        if !pid_is_alive(pid) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    let state = process_state_impl(pid).expect("process_state_impl should not error");
    assert_eq!(state, ProcessState::Exited { code: None });
}

#[cfg(target_os = "macos")]
#[test]
fn nonpositive_pid_is_never_alive() {
    assert!(!pid_is_alive(0));
    assert!(!pid_is_alive(-1));
}

#[cfg(target_os = "macos")]
#[test]
fn currently_running_pid_is_alive() {
    assert!(pid_is_alive(std::process::id() as i32));
}
