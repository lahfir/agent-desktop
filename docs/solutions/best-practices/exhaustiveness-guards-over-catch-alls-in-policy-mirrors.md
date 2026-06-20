---
title: Use named arms and exhaustiveness guard tests instead of catch-alls in policy and dispatch mirrors
date: 2026-06-10
category: best-practices
module: crates/core, src
problem_type: best_practice
component: command-policy
severity: high
applies_when:
  - A helper mirrors the per-case behavior of real command dispatch (a preflight reproducing each command's policy)
  - A new command or action name is added and a parallel mapping must stay in sync
  - A match over named semantic cases is tempted to use a catch-all arm
  - A registry or test list must provably cover everything the codebase actually contains
tags:
  - exhaustiveness
  - command-policy
  - dispatch
  - wait-predicate
  - regression-prevention
  - contract-tests
  - actionability
  - rust-patterns
---

# Use named arms and exhaustiveness guard tests instead of catch-alls in policy and dispatch mirrors

## Context

`wait --predicate actionable --action <name>` answers "would this action succeed" by running the same actionability preflight the real command runs — which means it must mirror each command's interaction policy exactly. The first implementation centralized that mirror behind a catch-all:

```rust
fn actionability_request(action: Action) -> ActionRequest {
    match action {
        Action::TypeText(_) => ActionRequest::focus_fallback(action),
        _ => ActionRequest::headless(action),
    }
}
```

Correct for the four actions that existed — and a trap for the fifth. Any future action would silently inherit headless policy, right or wrong, with no compiler complaint and no failing test. In a headless-first codebase where policy must flow explicitly from each command's declared base, a mirror that *infers* policy is exactly the drift the project's own learnings warn against. Code review flagged the catch-all as a silent-drift risk (auto memory [claude]: the repo treats policy inference as a standing hazard).

## Guidance

Three legs: explicit arms, a machine-derived universe guard, and per-case value pins.

**Leg 1 — one arm per name; the only "default" is an error.**

```rust
// crates/core/src/commands/wait_predicate.rs
/// Maps each `--action` name to the exact request its real command would
/// run with — policy included — so the preflight answers "would this action
/// succeed". Every name is an explicit arm: a catch-all here would let a
/// new action silently inherit the wrong policy.
fn parse_actionability_action(action: Option<&str>) -> Result<ActionRequest, AppError> {
    match action.unwrap_or("click") {
        "click" => Ok(ActionRequest::headless(Action::Click)),
        "type" => Ok(ActionRequest::focus_fallback(Action::TypeText(String::new()))),
        "set-value" => Ok(ActionRequest::headless(Action::SetValue(String::new()))),
        "clear" => Ok(ActionRequest::headless(Action::Clear)),
        other => Err(AppError::invalid_input_with_suggestion(
            format!("Unknown actionability action '{other}'"),
            "Use one of: click, type, set-value, clear.",
        )),
    }
}
```

The `other` arm rejects unrecognized input — it is input validation, not a semantic catch-all. Adding an action *requires* adding an arm; forgetting produces a user-visible error at this match, never a silent wrong-policy preflight.

**Leg 2 — a guard test that derives the case universe mechanically.**

```rust
// crates/core/src/commands/ref_policy_tests.rs
const POLICY_TESTED_COMMANDS: &[&str] = &[
    "check", "clear", "click", "collapse", "double_click", "expand",
    "focus", "right_click", "scroll", "scroll_to", "select", "set_value",
    "toggle", "triple_click", "type_text", "uncheck",
];

#[test]
fn all_context_request_callers_are_policy_tested() {
    // scans crates/core/src/commands/*.rs (excluding *_tests) for files
    // containing `context.request(` and fails, naming each stem, when one
    // is absent from POLICY_TESTED_COMMANDS
}
```

The universe is not hand-maintained: the test scans the filesystem for the call-site signature every ref-action command shares. A new command file that calls `context.request(` without a registered policy assertion fails CI with a message naming the stem and the required follow-up.

**Leg 3 — per-case value pins.**

Each listed case is pinned to its exact mirrored value, never covered by an "everything else" loop — `type` is asserted `focus_fallback` specifically, each remaining command asserted headless by name. The same shape guards CLI registration in `src/cli/contract_tests.rs`: `NON_COMMAND_MODULES` plus a filesystem scan asserts every command module is either a registered CLI subcommand or an explicitly declared helper.

## Why This Matters

Mirrors rot silently. When real dispatch gains a case but the mirror does not, nothing fails — the mirror's catch-all *answers confidently and wrongly*. Here that means a false "actionable: true" for an action whose real command would run a different policy: the agent's wait reports ready, the action then fails or behaves differently. The compiler cannot help across a string-keyed boundary, so the guard test substitutes for exhaustiveness checking by deriving the universe from the same source of truth the dispatcher uses (the files that exist, the call signature they share).

## When to Apply

- A function maps symbolic names to typed behavior where cases must genuinely differ (policy, routing, config)
- A test list or registry mirrors real per-case code and grows as the system grows
- The case universe is file-shaped or string-keyed, so `match` exhaustiveness cannot be compiler-enforced — derive it mechanically instead
- When the universe IS a closed enum, prefer matching on the enum directly and let the compiler enforce exhaustiveness; the guard-test pattern is for universes the compiler cannot see

## Examples

Before — the policy mirror with a catch-all (silently wrong for the next action):

```rust
match action {
    Action::TypeText(_) => ActionRequest::focus_fallback(action),
    _ => ActionRequest::headless(action),
}
```

After — the Guidance section above; the guard is `all_context_request_callers_are_policy_tested`, the value pin is `actionable_parse_mirrors_each_real_command_policy`.

## Related

- `best-practices/preserve-command-policy-semantics-during-refactor-2026-05-12.md` — the same risk class (per-case policy flattened by a structural abstraction) from the shared-helper angle; complementary defenses
- `best-practices/playwright-grade-desktop-reliability-2026-06-02.md` — the dispatch-correctness and test-ownership contract this pattern enforces at the match level
- `best-practices/macos-gesture-headless-capability-2026-06-10.md` — the per-gesture policy table whose explicitness named arms preserve
