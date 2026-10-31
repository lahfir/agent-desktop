use agent_desktop_core::{ErrorCode, WindowInfo, WindowState};

use super::{Liveness, RootRead, verify};

fn window(id: &str, pid: u32, instance: Option<&str>, title: &str) -> WindowInfo {
    WindowInfo {
        id: id.to_owned(),
        title: title.to_owned(),
        app: "a".to_owned(),
        pid: pid.into(),
        process_instance: instance.map(str::to_owned),
        bounds: None,
        state: WindowState::default(),
    }
}

#[test]
fn a_live_window_with_the_same_process_resolves_even_when_retitled() {
    let stored = window("w-1.2-a", 5, Some("b:1"), "Old title");
    let live = vec![window("w-1.2-a", 5, Some("b:1"), "New title")];
    assert_eq!(
        verify(&stored, (live, RootRead::Read), Liveness::Alive)
            .expect("resolves")
            .title,
        "New title"
    );
}

#[test]
fn a_different_pid_is_window_not_found() {
    let stored = window("w-1.2-a", 5, Some("b:1"), "t");
    let live = vec![window("w-1.2-a", 6, Some("b:1"), "t")];
    assert_eq!(
        verify(&stored, (live, RootRead::Read), Liveness::Alive)
            .expect_err("refused")
            .code,
        ErrorCode::WindowNotFound
    );
}

#[test]
fn a_stored_window_without_a_process_instance_is_refused() {
    let stored = window("w-1.2-a", 5, None, "t");
    let live = vec![window("w-1.2-a", 5, Some("b:1"), "t")];
    assert_eq!(
        verify(&stored, (live, RootRead::Read), Liveness::Alive)
            .expect_err("refused")
            .code,
        ErrorCode::StaleRef
    );
}

#[test]
fn a_stopped_process_is_unresponsive_with_or_without_a_listed_window() {
    let stored = window("w-1.2-a", 5, Some("b:1"), "t");
    for live in [Vec::new(), vec![stored.clone()]] {
        let error =
            verify(&stored, (live, RootRead::Read), Liveness::Stopped).expect_err("stopped");
        assert_eq!(error.code, ErrorCode::AppUnresponsive);
        assert!(error.message.contains("'a'"));
    }
}

#[test]
fn an_absent_window_of_a_gone_process_is_window_not_found() {
    let stored = window("w-1.2-a", 5, Some("b:1"), "t");
    let error = verify(&stored, (Vec::new(), RootRead::Read), Liveness::Gone).expect_err("gone");
    assert_eq!(error.code, ErrorCode::WindowNotFound);
    assert!(error.message.contains("no longer open"));
}

#[test]
fn a_reused_process_id_keeps_the_different_process_refusal_while_the_process_answers() {
    let stored = window("w-1.2-a", 5, Some("b:1"), "t");
    for process in [Liveness::Alive, Liveness::Gone] {
        let live = vec![window("w-1.2-a", 5, Some("b:2"), "t")];
        let error = verify(&stored, (live, RootRead::Read), process).expect_err("refused");
        assert_eq!(error.code, ErrorCode::WindowNotFound);
        assert!(error.message.contains("different process"));
    }
}

#[test]
fn an_unanswered_sibling_does_not_hide_a_verified_target_or_prove_its_absence() {
    let stored = window("target", 5, Some("b:1"), "target");
    let sibling = window("sibling", 5, Some("b:1"), "sibling");
    let found = verify(
        &stored,
        (vec![stored.clone(), sibling.clone()], RootRead::TimedOut),
        Liveness::Alive,
    )
    .expect("the observed target remains usable");
    assert_eq!(found.id, stored.id);
    for live in [Vec::new(), vec![sibling]] {
        let unknown = verify(&stored, (live.clone(), RootRead::TimedOut), Liveness::Alive)
            .expect_err("an unread frame cannot prove absence");
        assert_eq!(unknown.code, ErrorCode::Timeout);
        assert_eq!(unknown.details.unwrap()["kind"], "frames_unanswered");
        assert_eq!(
            unknown.disposition,
            agent_desktop_core::DeliverySemantics::not_delivered()
        );
        let absent = verify(&stored, (live, RootRead::Read), Liveness::Alive)
            .expect_err("the complete read proves absence");
        assert_eq!(absent.code, ErrorCode::WindowNotFound);
    }
}
