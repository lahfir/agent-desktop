---
title: Make FFI ref actions share CLI policy semantics
date: 2026-07-11
category: best-practices
module: crates/ffi action execution
problem_type: best_practice
component: tooling
severity: high
applies_when:
  - "Adding an FFI action entrypoint"
  - "Changing Action base policy or headed behavior"
  - "Documenting a low-level native-handle escape hatch"
tags: [ffi, interaction-policy, cli-parity, actions]
---

# Make FFI ref actions share CLI policy semantics

## Context

FFI exposes both high-level ref commands and low-level native-handle calls.
They must not be described as having the same safety contract: only the
high-level ref path can reproduce CLI observation-to-action behavior.

## Guidance

`ad_execute_by_ref` and its timeout variant use the core command path. They
load the ref map, resolve strictly, apply actionability, and compute policy
through `Action::base_interaction_policy` joined with the caller's explicit
policy. Ref actions, including `type`, have a headless base; explicit `press`
retains its focus fallback. With the headless policy, natural-input commands
use semantic accessibility delivery. A caller-selected headed policy makes
`click`, `right-click`, `type`, `clear`, and `scroll` physical-first, matching
the CLI `--headed` contract, while semantic-only commands remain semantic.

`ad_execute_action` and struct-based direct action entrypoints are deliberately
lower-level escape hatches. They operate on a caller-held native handle or
entry, may apply supplied policy verbatim, and do not claim RefStore-backed CLI
parity. Their safety docs must say so.

## Prevention

- Route new language-binding equivalents of CLI commands through the high-level
  core command path.
- Test both the observed effect and reported mechanism for headed and headless
  natural-input actions, plus every action with a non-headless base.
- Make any skipped boundary—strict resolution, preflight, or base-policy
  elevation—explicit in the public FFI documentation.

## Related

- [FFI repr(C) struct size pinning](ffi-repr-c-struct-size-pinning.md)
- [Build desktop actions as an observe-resolve-preflight-dispatch contract](playwright-grade-desktop-reliability-2026-06-02.md)
