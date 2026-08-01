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

/// The 2.3 vocabulary captures, taken against real applications on four UI
/// stacks by `probes/windows/scratch/run-dogfood.ps1`.
const VOCABULARY_CAPTURE_FILES: [&str; 4] =
    ["notepad.json", "explorer.json", "winforms.json", "wpf.json"];

/// Every field in a capture that carries text read out of the target, and is
/// therefore recorded as presence and length rather than as content.
///
/// The rule previously named `Name` alone while the renderer emitted any other
/// text field verbatim, so a property added to the census without this
/// treatment would have put a real application's text into a committed file.
/// Asserting the shape on the committed artifacts is what catches that: a
/// field that renders as a bare string here has bypassed the rule.
const PRESENCE_ONLY_FIELDS: [&str; 6] = [
    "name",
    "description",
    "automation_id",
    "help_text",
    "full_description",
    "legacy_default_action",
];

#[cfg(test)]
#[path = "captures_tests.rs"]
mod tests;
