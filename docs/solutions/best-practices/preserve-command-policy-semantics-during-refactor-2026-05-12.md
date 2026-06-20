---
title: Preserve command policy semantics during shared ref-action refactors
date: 2026-05-12
last_updated: 2026-06-10
category: best-practices
module: crates/core, crates/macos
problem_type: best_practice
component: command-policy
severity: high
applies_when:
  - Ref-consuming commands are moved onto a shared execution helper
  - A command has semantic AX steps plus explicit keyboard, clipboard, focus, or cursor paths
  - A command reports success after AXValue writes without verifying app-observable state
  - A DRY cleanup changes ActionRequest or InteractionPolicy construction
tags:
  - command-policy
  - ref-actions
  - interaction-policy
  - macos
  - regression-prevention
---

# Preserve command policy semantics during shared ref-action refactors

## Context

The unified ref-action helper removed repeated `resolve_ref + execute_action + to_value`
boilerplate across commands. That cleanup was correct, but two command-specific
policy choices were accidentally flattened during earlier review rounds:

- `clear` initially reported success after AXValue writes without verifying
  app-observable state, so web-backed controls could remain unchanged.
- `type` initially rejected non-ASCII text after an AXValue failure instead of
  using the explicit focus-permitted paste path.

Both commands still compiled and returned structured responses. The regression
was semantic: web-backed fields can report AXValue success while leaving the app's
JS model unchanged, so post-condition verification is part of the command
contract.

## Guidance

Shared ref-action dispatch should only remove repeated mechanics. It must not
choose the `InteractionPolicy` for a command, and it must not drop
post-condition verification that decides whether fallback steps should run.
Default CLI ref commands must stay headless: no focus stealing, no cursor
movement, and no synthetic keyboard or pasteboard use unless an explicit policy
path opted into it.

Each command owns its policy:

- Use `ActionRequest::headless` when the command is purely semantic AX work and
  must not focus, move the cursor, or synthesize input.
- Use `ActionRequest::focus_fallback` only for APIs that have explicitly opted
  into focus-changing behavior, such as CLI `type` after AXValue failure or FFI
  callers selecting `AD_POLICY_KIND_FOCUS_FALLBACK`.
- Use `ActionRequest::headed` (formerly `physical`) only for explicit physical
  interaction commands or FFI callers selecting `AD_POLICY_KIND_HEADED`. Ref
  commands no longer select it directly — the global `--headed` flag upgrades
  any command's base policy to headed via `CommandContext::request`. Note the
  headed physical path's side effects go beyond app-level focus stealing: the
  physical click fallback also raises the target element's own window (AXRaise,
  AXMain fallback) before posting events, gated on the same
  `allow_cursor_move && allow_focus_steal` policy as the rest of that path.

Do not infer policy from the fact that a command consumes a ref. `click`,
`check`, `expand`, `collapse`, `scroll-to`, `clear`, and `type` all consume refs,
but they still need command-specific success verification and error guidance.

## Review Rule

When a patch consolidates ref-consuming commands, review each call site for the
specific `ActionRequest` constructor. A helper like `execute_ref_action` is safe
only when the caller passes the already-chosen request. If a helper internally
constructs a default policy for many commands, treat that as a regression risk.

## Regression Tests

Backfill tests at the command or adapter boundary for commands whose policy is
part of correctness:

- `clear` must dispatch headlessly from the CLI and must verify AXValue writes
  before reporting success.
- `type` must attempt AXValue first from the CLI, then use only its explicit
  focus-fallback tier when AXValue cannot update the target.
- FFI policy-specific tests must prove focus and physical paths are available
  only when the caller explicitly selects that policy.
- A generic ref-action helper should preserve both the `Action` variant and the
  caller's `InteractionPolicy`.

For AX value writes, treat "set returned success" as incomplete evidence on
web-backed controls. Read back the value when the field is not secure; a
mismatch must be a failed step so the next command-specific fallback can run.

## Related

- `best-practices/exhaustiveness-guards-over-catch-alls-in-policy-mirrors.md` — the same risk class (per-case policy flattened by a structural abstraction) from the string-keyed dispatch-mirror angle: named arms plus machine-derived guard tests where the compiler cannot enforce exhaustiveness.
- `best-practices/macos-gesture-headless-capability-2026-06-10.md` — the per-gesture policy table whose explicitness this guidance preserves.
