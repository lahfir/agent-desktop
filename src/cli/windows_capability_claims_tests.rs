use super::*;

const WINDOWS_SKILL_DOC: &str = include_str!("../../skills/agent-desktop-windows/SKILL.md");

/// The one command the Windows table still documents as unavailable that no
/// Windows adapter behaviour backs: core records the session setting and
/// nothing renders, so the pin below keeps the skill's row honest instead of
/// a refusal code.
const MUST_STAY_UNAVAILABLE_ON_WINDOWS: &[&str] = &["cursor-overlay"];

/// Commands the Windows table documents as unavailable that no adapter method
/// backs: core refuses them on every platform before dispatch reaches an
/// adapter, so the behavioural pin cannot cover them and must not pretend to.
/// They are listed rather than ignored so the closure assertion below stays
/// exhaustive.
const REFUSED_BY_CORE_ON_EVERY_PLATFORM: &[&str] =
    &["key-down", "key-up", "mouse-down", "mouse-up"];

/// Set assertion enforces that new unavailable commands are explicitly listed:
/// per-name checks alone miss rows that satisfy old assertions while never being verified.
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
        CursorOverlayControl, Deadline, ErrorCode, InteractionLease, InteractionPolicy,
        SnapshotSurface, SystemOps,
    };

    let adapter = agent_desktop_windows::WindowsAdapter::new();
    let deadline = Deadline::after(5_000).expect("bounded deadline");
    let lease = InteractionLease::guarded(deadline, ()).expect("lease");

    let quick_settings = SystemOps::open_system_surface(
        &adapter,
        SnapshotSurface::QuickSettings,
        InteractionPolicy::headed(),
        &lease,
    )
    .expect_err(
        "quick-settings must refuse on a build whose shell carries the \
         quick actions inside the Action Center",
    );
    assert_eq!(
        quick_settings.code,
        ErrorCode::PlatformNotSupported,
        "quick-settings changed behaviour; update the Windows skill's capability table"
    );
    let detail = quick_settings.platform_detail.expect(
        "the quick-settings refusal must carry the platform detail naming \
         the build and the surface that holds the capability",
    );
    assert!(
        detail.contains("action-center"),
        "the refusal must name 'action-center' as the surface carrying the \
         capability, got: {detail}"
    );

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
