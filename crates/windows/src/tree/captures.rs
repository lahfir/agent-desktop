//! Where sub-phase 2.2's committed COM evidence lives, and the normalisation
//! rules it is held to.
//!
//! The captures under `docs/plans/2026-07-27-002-captures/` are produced by
//! `crates/windows/examples/uia_tree_dump.rs` on a developer machine, in the
//! UIA3 COM stack the adapter ships. Sub-phase 2.0's dumps were taken on the
//! managed stack, which A2-4 measured reporting the identical Notepad window
//! as 3 nodes against 26 on COM, so they cannot serve as COM expectations.
//!
//! Nothing in CI asserts what the captures contain - that would be an
//! `app/provider` assertion R9 forbids. What is asserted here is the
//! **redaction rule**: a capture that leaks a process id, a provider id, a
//! window handle or a user path is a defect regardless of what tree it holds.

/// The committed dev-box captures, by the target variant each records.
const CAPTURE_FILES: [&str; 2] = ["notepad-com.json", "explorer-com.json"];

#[cfg(test)]
#[path = "captures_tests.rs"]
mod tests;
