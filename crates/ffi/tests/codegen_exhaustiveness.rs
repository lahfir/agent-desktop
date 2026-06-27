/// Codegen exhaustiveness + per-command policy pin tests.
///
/// Independently verifies that:
/// 1. Every expected Family-B command has a generated `ad_<name>` wrapper in
///    `src/commands/generated.rs` (the committed output).
/// 2. Each command's interaction-policy pin is preserved across refactors.
///
/// Adding a new Family-B command requires updating EXPECTED_COMMANDS here
/// and adding a template to build.rs. This test fails the build if either
/// side is out of sync with the generated file.
mod common;

use agent_desktop_core::action::Action;
use agent_desktop_core::interaction_policy::InteractionPolicy;

/// Known Family-B commands — the exhaustive set of command-backed JSON
/// wrappers that must appear in `src/commands/generated.rs`.
const EXPECTED_COMMANDS: &[&str] = &["execute_by_ref", "snapshot", "status", "version", "wait"];

#[test]
fn generated_file_contains_all_expected_wrappers() {
    let src = include_str!("../src/commands/generated.rs");

    for name in EXPECTED_COMMANDS {
        let fn_sig = format!("pub unsafe extern \"C\" fn ad_{name}(");
        assert!(
            src.contains(&fn_sig),
            "generated src/commands/generated.rs is missing `ad_{name}` — \
             check templates in build.rs and run cargo build to regenerate"
        );
    }
}

#[test]
fn expected_command_count_matches_generated_wrapper_count() {
    let src = include_str!("../src/commands/generated.rs");
    let actual_count = src
        .lines()
        .filter(|l| l.contains("pub unsafe extern \"C\" fn ad_"))
        .count();
    assert_eq!(
        actual_count,
        EXPECTED_COMMANDS.len(),
        "generated file has {actual_count} wrappers but EXPECTED_COMMANDS has {} — \
         update EXPECTED_COMMANDS to match the templates in build.rs",
        EXPECTED_COMMANDS.len()
    );
}

#[test]
fn generated_wrappers_are_in_alphabetical_order() {
    let src = include_str!("../src/commands/generated.rs");
    let fn_names: Vec<&str> = src
        .lines()
        .filter(|l| l.contains("pub unsafe extern \"C\" fn ad_"))
        .filter_map(|l| {
            l.split("fn ad_")
                .nth(1)
                .and_then(|rest| rest.split('(').next())
        })
        .collect();
    let sorted = {
        let mut s = fn_names.clone();
        s.sort_unstable();
        s
    };
    assert_eq!(
        fn_names, sorted,
        "generated wrappers must appear in alphabetical order"
    );
}

#[test]
fn policy_type_text_base_is_focus_fallback() {
    let base = Action::TypeText("hi".into()).base_interaction_policy();
    assert_eq!(
        base,
        InteractionPolicy::focus_fallback(),
        "TypeText base policy must be focus_fallback (KTD6)"
    );
}

#[test]
fn policy_click_base_is_headless() {
    let base = Action::Click.base_interaction_policy();
    assert_eq!(
        base,
        InteractionPolicy::headless(),
        "Click base policy must be headless (KTD6)"
    );
}

#[test]
fn policy_headless_caller_cannot_downgrade_type_text() {
    let base = Action::TypeText("x".into()).base_interaction_policy();
    let effective = base.join(InteractionPolicy::headless());
    assert_eq!(
        effective,
        InteractionPolicy::focus_fallback(),
        "headless caller must not downgrade TypeText below focus_fallback"
    );
}

#[test]
fn policy_headed_caller_elevates_click_to_headed() {
    let base = Action::Click.base_interaction_policy();
    let effective = base.join(InteractionPolicy::headed());
    assert_eq!(
        effective,
        InteractionPolicy::headed(),
        "headed caller must elevate Click to headed"
    );
}

#[test]
fn click_base_plus_focus_fallback_caller_gives_focus_fallback() {
    let base = Action::Click.base_interaction_policy();
    let effective = base.join(InteractionPolicy::focus_fallback());
    assert_eq!(effective, InteractionPolicy::focus_fallback());
}

#[test]
fn type_text_base_plus_headed_caller_becomes_headed() {
    let base = Action::TypeText("x".into()).base_interaction_policy();
    let effective = base.join(InteractionPolicy::headed());
    assert_eq!(effective, InteractionPolicy::headed());
}
