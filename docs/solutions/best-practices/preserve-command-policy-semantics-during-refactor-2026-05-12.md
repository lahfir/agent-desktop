---
title: Preserve command policy semantics during shared ref-action refactors
date: 2026-05-12
last_updated: 2026-07-12
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
boilerplate across commands. That cleanup was correct, but command-specific
policy and verification choices were accidentally flattened during earlier
review rounds:

- `clear` initially reported success after AXValue writes without verifying
  app-observable state, so web-backed controls could remain unchanged.
- `type` and `clear` lost the distinction between default headless delivery and
  explicitly headed natural input.

Both commands still compiled and returned structured responses. The regression
was semantic: the selected mechanism and its app-observable post-condition are
part of the command contract, not interchangeable helper details.

## Guidance

Shared ref-action dispatch should only remove repeated mechanics. It must not
choose the `InteractionPolicy` for a command, and it must not drop
post-condition verification that decides whether fallback steps should run.
Default CLI ref commands start from their least-permissive base policy. The
global `--headed` flag joins an explicit focus-and-cursor permission onto that
base; it changes delivery preference only for commands with a natural physical
equivalent.

Each command owns its policy:

- `Action::base_interaction_policy` is headless for ref actions, including
  `type` and `clear`; explicit `press` retains its focus-fallback base.
- Headless `type` writes `AXSelectedText`, while headed `type` uses PID-targeted
  physical text delivery. Headless `clear` uses verified `AXValue`; headed
  `clear` prefers the focus-and-keyboard path before the semantic step.
- Headed `click`, `right-click`, and `scroll` likewise prefer their physical
  mechanisms. Semantic-only commands stay semantic, and physical-only commands
  still fail closed without headed authorization.
- High-level FFI ref actions join the caller's explicit policy with the same
  action base. Low-level native-handle entrypoints remain escape hatches and do
  not imply CLI policy parity.

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

- `clear` must report semantic delivery headlessly and physical delivery when
  headed, while verifying the resulting empty value in both modes.
- `type` must report `AXSelectedText` delivery headlessly and PID-targeted
  physical delivery when headed.
- FFI policy-specific tests must prove the same headless-versus-headed
  mechanism split as the CLI high-level path.
- A generic ref-action helper should preserve both the `Action` variant and the
  caller's `InteractionPolicy`.

For AX value writes, treat "set returned success" as incomplete evidence on
web-backed controls. Read back the value when the field is not secure; a
mismatch must be a failed step so the command-specific chain can continue or
report an honest failure.

## Related

- [Exhaustiveness guards over catch-alls in policy mirrors](exhaustiveness-guards-over-catch-alls-in-policy-mirrors.md) — named arms and guard tests protect string-keyed mirrors where the compiler cannot prove coverage.
- [macOS gesture headless capability](macos-gesture-headless-capability-2026-06-10.md) — the per-gesture policy table whose explicitness this guidance preserves.
