---
title: Ref actions are not uniform — hover/drag skip the actionability battery
date: 2026-07-05
category: documentation-gaps
module: core/actionability, ref-action dispatch
problem_type: documentation_gap
component: documentation
severity: medium
applies_when:
  - "Writing or reviewing agent-facing capability docs for ref actions"
  - "Reasoning about why a hover or drag failed with ACTION_FAILED instead of TIMEOUT"
  - "Adding a new ref action or a new actionability check"
tags: [actionability, hover, drag, receives-events, ref-actions, doc-accuracy]
---

# Ref actions are not uniform — hover/drag skip the actionability battery

## Context

A code-read review of a skill-doc change caught that `skills/agent-desktop/references/commands-interaction.md` and `SKILL.md` claimed **every** ref action runs the full actionability battery (`visible`/`stable`/`enabled`/`supported_action`/`policy`/`editable` + occlusion) and polls every ~100ms until actionable or `TIMEOUT`. That is the model for the *dispatch* actions (click, type, set-value, etc.) — but it is **false for `hover` and `drag`**, which take a structurally different path. The docs were written by generalizing the click-path model to all ref actions. The inaccuracy compiled fine, passed every test, and was invisible until a reviewer cross-checked the prose against the actual per-command code path.

## Guidance

There are **two distinct actionability paths** for ref actions. Do not assume uniformity.

**1. Dispatch actions** (`click`, `double-click`, `right-click`, `triple-click`, `type`, `set-value`, `select`, `toggle`, `check`, `uncheck`, `expand`, `collapse`, `clear`, `focus`, `scroll`, `scroll-to`):

`crates/core/src/ref_action.rs` → `ref_action_wait::execute_with_auto_wait` → `actionability::check_live`, which runs the **full battery** (`visible`, `stable`, `enabled`, `supported_action`, `policy`, `editable`) plus `receives_events` occlusion for the four click variants. It **polls** every ~100ms until the target is actionable or `--timeout-ms` expires, then fails with `TIMEOUT` (trace `kind: "actionability_timeout"`).

**2. `hover` and `drag`:**

`crates/core/src/commands/hover.rs` | `drag.rs` → `helpers::resolve_point_with_wait` → `point_resolve::resolve_point_from_ref_or_xy_with_context` → `actionability::require_receives_events`, which runs **only** the `receives_events` occlusion check (a single-entry `checks` array — no `visible`/`stable`/`enabled`/etc.). Here `--timeout-ms` budgets only the ref-**resolution** retry: `STALE_REF`/`AMBIGUOUS_TARGET`/`TIMEOUT` are retried within the budget, but an occlusion failure returns **`ACTION_FAILED` immediately**, never polled to `TIMEOUT`.

**Prevention:** validate agent-facing capability docs against the actual per-command code path, not the mental model. When a doc makes a "does **every** X do Y?" claim and X spans commands with structurally different execution paths (here: `execute_with_auto_wait`/`check_live` vs `resolve_point_with_wait`/`require_receives_events`), treat the claim as suspect and grep the real dispatch path for each command family before asserting uniformity.

## Why This Matters

An agent following the inaccurate docs mis-diagnoses failures:

- It waits for a `TIMEOUT` on an occluded `hover`/`drag` that will never come — the command already returned `ACTION_FAILED` on the first attempt.
- It expects a `visible`/`enabled` check to gate `hover`/`drag` and reasons about recovery accordingly, when only occlusion is ever evaluated.

This class of defect is uniquely dangerous because it is **invisible to the type system, the compiler, and the test suite** — the docs are prose. Only an adversarial reviewer who cross-checks each claim against the code catches it. (In this case the review that caught it was itself a third pass whose *finding line numbers were bogus diff-offset artifacts* — e.g. `execute.rs:16197` in a repo with a hard 400-LOC/file cap — a reminder to locate the real code by symbol, not by cited line.)

## When to Apply

- Writing or reviewing any agent-facing capability doc that enumerates behavior "for all ref actions."
- Debugging a `hover`/`drag` that failed with `ACTION_FAILED` (occlusion) rather than the `TIMEOUT` a dispatch action would give.
- Adding a new ref action: decide explicitly which of the two paths it takes, and document it on the correct side of the divergence.

## Examples

Doc correction that fixed the defect (agent-facing docs):

```
- ref actions check live visibility, stability, enabled state, ... and (for
-   click/.../hover/drag) hit-test occlusion before dispatch.
+ dispatch ref actions (click, type, set-value, ...) check the full battery and
+   poll until actionable or TIMEOUT; hover/drag resolve a target point and run
+   ONLY the receives_events occlusion check, failing fast with ACTION_FAILED.
```

The one-line grep that distinguishes the paths: a command whose `execute` reaches `execute_with_auto_wait`/`check_live` is a dispatch action (full battery, polls to `TIMEOUT`); one that reaches `resolve_point_with_wait`/`require_receives_events` is `hover`/`drag` (occlusion-only, immediate `ACTION_FAILED`).

## Related

- `docs/solutions/best-practices/playwright-grade-desktop-reliability-2026-06-02.md` — the actionability reliability contract (the `check_live` battery + `actionability_timeout` this learning contrasts against).
- `docs/solutions/best-practices/real-app-tests-are-the-platform-adapter-gate.md` — why mock/unit tests can't catch platform- or prose-level divergence; adversarial review + real-app checks are the gate.
- Deferred, disproportionate-blast-radius findings from the same review: GitHub issues #94 (process_state PID-reuse) and #95 (test-adapter boilerplate macro).
