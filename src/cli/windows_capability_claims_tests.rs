use super::*;

const WINDOWS_SKILL_DOC: &str = include_str!("../../skills/agent-desktop-windows/SKILL.md");

const MUST_STAY_UNAVAILABLE_ON_WINDOWS: &[&str] = &[
    "list-surfaces",
    "list-notifications",
    "dismiss-notification",
    "dismiss-all-notifications",
    "notification-action",
    "cursor-overlay",
];

/// Commands the Windows table documents as unavailable that no adapter method
/// backs: core refuses them on every platform before dispatch reaches an
/// adapter, so the behavioural pin cannot cover them and must not pretend to.
/// They are listed rather than ignored so the closure assertion below stays
/// exhaustive.
const REFUSED_BY_CORE_ON_EVERY_PLATFORM: &[&str] =
    &["key-down", "key-up", "mouse-down", "mouse-up"];

/// The behavioural pin is one-directional: it proves the six named commands are
/// still documented as unavailable, and says nothing about a seventh row added
/// later. Such a row would satisfy every per-name assertion while never being
/// checked against the adapter at all, which is how a false claim ships. The
/// closing set assertion makes adding a row a decision: pin it behaviourally,
/// or declare it refused by core before any adapter is reached.
#[test]
fn windows_skill_capability_claims_resolve_against_dispatch() {
    let dispatchable: BTreeSet<String> = cli_command_names().into_iter().collect();
    let working = windows_skill_commands(true);
    let unavailable = windows_skill_commands(false);

    assert!(
        !working.is_empty(),
        "the Windows capability table must claim working commands"
    );
    for name in working.iter().chain(unavailable.iter()) {
        assert!(
            dispatchable.contains(name),
            "the Windows skill claims '{name}' but dispatch has no such command"
        );
    }
    for name in MUST_STAY_UNAVAILABLE_ON_WINDOWS {
        assert!(
            !working.contains(&(*name).to_owned()),
            "the Windows skill claims '{name}' works; the adapter does not implement it"
        );
        assert!(
            unavailable.contains(&(*name).to_owned()),
            "the Windows skill stopped documenting '{name}' as unavailable on Windows"
        );
    }

    let documented: BTreeSet<&str> = unavailable.iter().map(String::as_str).collect();
    let accounted: BTreeSet<&str> = MUST_STAY_UNAVAILABLE_ON_WINDOWS
        .iter()
        .copied()
        .chain(REFUSED_BY_CORE_ON_EVERY_PLATFORM.iter().copied())
        .collect();
    assert_eq!(
        documented, accounted,
        "every command the Windows table documents as unavailable must be either \
         behaviourally pinned in MUST_STAY_UNAVAILABLE_ON_WINDOWS or declared in \
         REFUSED_BY_CORE_ON_EVERY_PLATFORM; an unaccounted row is never verified"
    );
}

fn windows_skill_commands(claimed_working: bool) -> Vec<String> {
    let mut names = Vec::new();
    for row in WINDOWS_SKILL_DOC.lines() {
        if !row.trim_start().starts_with('|') || row.contains("---") {
            continue;
        }
        let cells: Vec<&str> = row.split('|').collect();
        if cells.len() < 4 || (cells[3].contains("Works")) != claimed_working {
            continue;
        }
        names.extend(
            cells[2]
                .split('`')
                .enumerate()
                .filter(|(index, _)| index % 2 == 1)
                .map(|(_, token)| token.trim().to_owned())
                .filter(|token| !token.is_empty()),
        );
    }
    names
}

#[test]
#[cfg(target_os = "windows")]
fn windows_adapter_still_refuses_what_the_skill_marks_unavailable() {
    use agent_desktop_core::{
        CursorOverlayControl, Deadline, DismissAllNotificationsRequest, DismissNotificationRequest,
        ErrorCode, InteractionLease, InteractionPolicy, NotificationActionRequest,
        NotificationFilter, NotificationIdentity, ObservationOps, ProcessIdentity, SystemOps,
    };

    let adapter = agent_desktop_windows::WindowsAdapter::new();
    let deadline = Deadline::after(5_000).expect("bounded deadline");
    let process = ProcessIdentity::new(std::process::id(), "skill-capability-probe");
    let identity = NotificationIdentity::default();
    let lease = InteractionLease::guarded(deadline, ()).expect("lease");

    let refusals = [
        (
            "list-surfaces",
            ObservationOps::list_surfaces(&adapter, process, deadline)
                .expect_err("list-surfaces must refuse")
                .code,
        ),
        (
            "list-notifications",
            SystemOps::list_notifications(
                &adapter,
                &NotificationFilter::default(),
                InteractionPolicy::headless(),
                deadline,
                None,
            )
            .expect_err("list-notifications must refuse")
            .code,
        ),
        (
            "dismiss-notification",
            SystemOps::dismiss_notification(
                &adapter,
                DismissNotificationRequest {
                    index: 0,
                    app_filter: None,
                    identity: &identity,
                    policy: InteractionPolicy::headless(),
                },
                &lease,
            )
            .expect_err("dismiss-notification must refuse")
            .code,
        ),
        (
            "dismiss-all-notifications",
            SystemOps::dismiss_all_notifications(
                &adapter,
                DismissAllNotificationsRequest {
                    app_filter: None,
                    policy: InteractionPolicy::headless(),
                },
                &lease,
            )
            .expect_err("dismiss-all-notifications must refuse")
            .code,
        ),
        (
            "notification-action",
            SystemOps::notification_action(
                &adapter,
                NotificationActionRequest {
                    index: 0,
                    identity: &identity,
                    action_name: "Reply",
                    policy: InteractionPolicy::headless(),
                },
                &lease,
            )
            .expect_err("notification-action must refuse")
            .code,
        ),
    ];

    for (name, code) in refusals {
        assert_eq!(
            code,
            ErrorCode::PlatformNotSupported,
            "{name} changed behaviour; update the Windows skill's capability table"
        );
    }

    SystemOps::update_cursor_overlay(
        &adapter,
        &CursorOverlayControl::disable("skill-capability-probe".into()),
    )
    .expect(
        "cursor-overlay must keep core's Ok(()) default on Windows: the skill \
         documents 'records its session setting, renders nothing', so an \
         override arriving here needs a capability-table update in the same PR",
    );
}
