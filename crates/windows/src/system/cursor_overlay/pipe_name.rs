//! The name the overlay's control pipe is reached by.
//!
//! Pure: a deterministic function of the state root, the session id and the
//! protocol generation, so any process that knows those three computes the
//! same name without a lookup file, a registry or a pid.
//!
//! The generation is not decoration. A renderer left running by an earlier
//! build keeps serving a rebuilt binary's controls if the name it answers on
//! does not change when the protocol does — which happens on any rebuild
//! during a live session, this branch's own end-to-end runs included. macOS
//! hashes its protocol version into the socket path for the same reason.

use std::path::Path;

/// Every generation this control protocol has ever had, oldest first, with the
/// one this build speaks last.
///
/// Changing the wire format or the acknowledgement contract means appending
/// here, and an append moves the outgoing generation into the retired set in
/// the same edit. That is the point of the ledger: a maintainer cannot bump
/// the generation without also handing the previous one to the retirement
/// sweep, so a renderer still drawing under it cannot be forgotten.
///
/// One element today, so the retired set is empty and the sweep is a no-op.
/// That is the correct state for a protocol that has never changed, and it is
/// said out loud rather than left to be discovered: the sweep costs nothing
/// until the first append and does its work from the moment there is one.
pub(crate) const PROTOCOL_GENERATIONS: [&str; 1] = ["w1"];

/// The generation this build speaks. A child whose marker names a different
/// one refuses to serve, and its pipe name differs, so a stale renderer is
/// unreachable rather than subtly wrong.
pub(crate) const PROTOCOL_GENERATION: &str = PROTOCOL_GENERATIONS[PROTOCOL_GENERATIONS.len() - 1];

/// The environment marker that turns an ordinary invocation of this binary
/// into the overlay child. Read before argument parsing, so the child never
/// reaches the CLI at all.
pub(crate) const CHILD_MARKER: &str = "AGENT_DESKTOP_CURSOR_OVERLAY_CHILD";

/// The argv token the child carries beside the marker. The environment block
/// of another process is not readable from outside, so anything that has to
/// recognise this child by inspection — the end-to-end suite's reaper — matches
/// on the command line instead.
///
/// Retiring a stale generation is not such a consumer: it derives that
/// generation's pipe name and speaks to whoever answers it, which needs no
/// command line at all.
pub const CHILD_ARGV_FLAG: &str = "--cursor-overlay-child";

pub(crate) fn pipe_name(root: &Path, session_id: &str) -> String {
    pipe_name_for_generation(root, session_id, PROTOCOL_GENERATION)
}

pub(crate) fn pipe_name_for_generation(root: &Path, session_id: &str, generation: &str) -> String {
    format!(
        r"\\.\pipe\agent-desktop-cursor-{:016x}",
        endpoint_hash(root, session_id, generation)
    )
}

/// The generations a renderer may still be drawing under that this build can
/// no longer talk to: every entry of the ledger but the last.
///
/// Takes the ledger rather than reading the shipped one, so the promotion rule
/// — appending retires the entry it displaces — is provable against a ledger
/// with more than one generation in it, which the shipped one does not yet
/// have.
pub(crate) fn retired_generations<'a>(ledger: &'a [&'static str]) -> &'a [&'static str] {
    ledger.split_last().map_or(&[], |(_, earlier)| earlier)
}

/// The argv the child is spawned with: the flag, its session, and its
/// generation. Carried as arguments rather than only in the environment so a
/// process that cannot read the child's environment can still recognise it.
pub(crate) fn child_arguments(session_id: &str) -> Vec<String> {
    vec![
        CHILD_ARGV_FLAG.to_owned(),
        session_id.to_owned(),
        PROTOCOL_GENERATION.to_owned(),
    ]
}

/// The session and generation a command line names, when it is one of ours.
pub(crate) fn parse_child_arguments(arguments: &[String]) -> Option<(String, String)> {
    let flag = arguments
        .iter()
        .position(|value| value == CHILD_ARGV_FLAG)?;
    let session = arguments.get(flag + 1)?;
    let generation = arguments.get(flag + 2)?;
    if session.is_empty() || generation.is_empty() {
        return None;
    }
    Some((session.clone(), generation.clone()))
}

fn endpoint_hash(root: &Path, session_id: &str, generation: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let root = root.to_string_lossy();
    for byte in root
        .as_bytes()
        .iter()
        .chain(session_id.as_bytes())
        .chain(generation.as_bytes())
    {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
#[path = "pipe_name_tests.rs"]
mod tests;
