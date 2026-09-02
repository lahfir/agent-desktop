use super::{
    CHILD_ARGV_FLAG, PROTOCOL_GENERATION, PROTOCOL_GENERATIONS, child_arguments,
    parse_child_arguments, pipe_name, pipe_name_for_generation, retired_generations,
};
use std::path::Path;

/// Deliberately not shaped like a home directory. The privacy scan treats
/// `C:\Users\<name>` as an operator identity wherever it appears, and it is
/// right to: a rule that made an exception for invented names could not tell
/// one from a real account captured by a probe. The pipe name hashes whatever
/// root it is given, so the fixture only has to be a path.
fn root() -> &'static Path {
    Path::new(r"C:\ProgramData\agent-desktop-fixture")
}

#[test]
fn the_same_root_and_session_always_resolve_to_the_same_name() {
    assert_eq!(pipe_name(root(), "s0000001"), pipe_name(root(), "s0000001"));
}

#[test]
fn a_different_session_resolves_to_a_different_name() {
    assert_ne!(pipe_name(root(), "s0000001"), pipe_name(root(), "s0000002"));
}

#[test]
fn a_different_state_root_resolves_to_a_different_name() {
    assert_ne!(
        pipe_name(root(), "s0000001"),
        pipe_name(Path::new(r"D:\elsewhere\.agent-desktop"), "s0000001")
    );
}

/// The generation is why a renderer left by an earlier build cannot keep
/// serving a rebuilt binary's controls: it answers on a name the new build
/// never asks for.
#[test]
fn two_generations_resolve_to_different_names() {
    assert_ne!(
        pipe_name_for_generation(root(), "s0000001", "w1"),
        pipe_name_for_generation(root(), "s0000001", "w2")
    );
}

#[test]
fn the_name_is_a_local_pipe_path() {
    let name = pipe_name(root(), "s0000001");

    assert!(
        name.starts_with(r"\\.\pipe\"),
        "the name must be a local named pipe, got {name}"
    );
    assert!(
        !name.contains("s0000001"),
        "the session id is hashed rather than embedded, so the name carries no identity"
    );
}

/// The environment block of another process cannot be read from outside, so
/// the session and generation ride in argv where a command-line enumeration
/// can find them.
#[test]
fn the_child_argv_names_its_session_and_generation() {
    let arguments = child_arguments("s0000001");

    assert_eq!(arguments[0], CHILD_ARGV_FLAG);
    assert_eq!(
        parse_child_arguments(&arguments),
        Some(("s0000001".to_owned(), PROTOCOL_GENERATION.to_owned()))
    );
}

#[test]
fn a_command_line_that_is_not_ours_parses_to_nothing() {
    assert_eq!(parse_child_arguments(&["snapshot".to_owned()]), None);
    assert_eq!(
        parse_child_arguments(&[CHILD_ARGV_FLAG.to_owned()]),
        None,
        "the flag alone names no session, so it is not a child of ours"
    );
    assert_eq!(
        parse_child_arguments(&[CHILD_ARGV_FLAG.to_owned(), String::new(), "w1".to_owned()]),
        None,
        "an empty session id is not a session"
    );
}

/// The shipped ledger has never been appended to, so nothing is retired and
/// the sweep has nothing to do. Asserted rather than assumed, because a sweep
/// that quietly targeted the live generation would disable the renderer it
/// just started.
#[test]
fn a_ledger_of_one_generation_retires_nothing() {
    assert!(retired_generations(&PROTOCOL_GENERATIONS).is_empty());
}

/// The rule the ledger exists to enforce, proved on a ledger that has been
/// appended to. Bumping the generation is an append, and this is what an
/// append does to the entry it displaces.
#[test]
fn appending_a_generation_moves_the_previous_one_into_the_retired_set() {
    let before: [&'static str; 2] = ["w1", "w2"];
    let after: [&'static str; 3] = ["w1", "w2", "w3"];

    assert_eq!(retired_generations(&before), ["w1"]);
    assert_eq!(
        retired_generations(&after),
        ["w1", "w2"],
        "an append retires the generation it displaced, and keeps every earlier one retired"
    );
}

/// The live generation is never a retirement target. A sweep that included it
/// would derive the name this build is about to use and disable whatever
/// answers there.
#[test]
fn the_generation_this_build_speaks_is_never_a_retirement_target() {
    assert_eq!(PROTOCOL_GENERATION, "w1");
    assert!(
        !retired_generations(&PROTOCOL_GENERATIONS).contains(&PROTOCOL_GENERATION),
        "the current generation must never be swept"
    );
    assert!(!retired_generations(&["w1", "w2", "w3"]).contains(&"w3"));
}

/// An empty ledger is not a state the constant can be in, but the function
/// takes any ledger and must not panic indexing one.
#[test]
fn an_empty_ledger_retires_nothing_rather_than_panicking() {
    assert!(retired_generations(&[]).is_empty());
}
