use super::{
    CHILD_ARGV_FLAG, PROTOCOL_GENERATION, child_arguments, parse_child_arguments, pipe_name,
    pipe_name_for_generation,
};
use std::path::Path;

fn root() -> &'static Path {
    Path::new(r"C:\Users\someone\.agent-desktop")
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
