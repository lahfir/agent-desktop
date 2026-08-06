---
title: Keep gesture capability in the platform adapter and policy in core
date: 2026-07-11
category: best-practices
module: core actions and macos adapter
problem_type: architecture_pattern
component: tooling
severity: high
applies_when:
  - "Adding a new action or physical fallback"
  - "Porting an action to Windows or Linux"
  - "Explaining a headless policy failure"
tags: [interaction-policy, actions, macos, adapters, headless]
---

# Keep gesture capability in the platform adapter and policy in core

## Context

Whether a platform can perform an intent semantically is platform-specific;
whether the caller permitted focus stealing or cursor movement is a portable
side-effect contract. Mixing those decisions into a CLI command would make
future adapters copy macOS assumptions.

## Guidance

Core creates an `ActionRequest` with the action's least-permissive base policy.
Semantic actions, including `type`, start strictly headless. Explicit `press`
may use focus fallback. Headed mode can elevate policy but may not weaken it.

The adapter chooses the requested legal implementation: strict headless uses
semantic accessibility APIs, while headed natural-input commands prefer
physical delivery. `hover` and
`drag` are physical pointer commands by definition and require a cursor-moving
policy before resolving or moving the pointer. A semantic reorder capability,
if a future platform offers one, is a distinct action contract rather than a
hidden change to physical drag.

Two different `receives_events` hit-tests exist and must not be conflated,
because they answer different questions. The shared actionability battery
(`crates/core/src/actionability/receives_events.rs`) tries **five** candidate
points from the element's bounds — the center plus four quadrant points — and
passes if *any* of them reaches the target; it is asking whether the element is
reachable at all, so a partially occluded control still passes. The pointer
pipeline calls `require_receives_events` with the **single** coordinate it has
already resolved and will actually move the cursor to; it is asking whether
*that exact point* is deliverable, so there is no second candidate to fall back
on. A target that satisfies the battery can therefore still fail the pointer
check. Keep the distinction when changing either path: widening the pointer
check to multiple points would silently move the cursor somewhere the caller
did not resolve.

Tests must verify the observed effect, not merely native API success. Native
controls and accessibility implementations vary; a successful AX call is not
proof that an application executed its handler.

## Prevention

- Put shared policy and action vocabulary in core; do not import platform
  capability details into core.
- Keep adapter dispatch explicit about semantic versus physical mechanisms.
- Add a real-fixture assertion for every new physical fallback.
- Return `POLICY_DENIED` when the requested fallback is not authorized.

## Related

- [Preserve command policy semantics during refactor](preserve-command-policy-semantics-during-refactor-2026-05-12.md)
