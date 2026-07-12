---
title: Document pointer actions from their own reliability pipeline
date: 2026-07-11
category: documentation-gaps
module: crates/core pointer actions
problem_type: documentation_gap
component: documentation
severity: high
applies_when:
  - "Documenting hover or drag behavior"
  - "Adding a ref action that resolves a screen point"
  - "Explaining timeout or occlusion failures to an agent caller"
tags: [pointer-actions, hover, drag, actionability, documentation]
---

# Document pointer actions from their own reliability pipeline

## Context

`hover` and `drag` are physical, cursor-moving commands. They do not use the
same dispatch ladder as semantic ref actions, so prose that says all ref
actions have identical actionability behavior becomes wrong as the pointer
path evolves.

## Guidance

Follow the code path, not a command-family generalization:

`hover` and `drag` enter `pointer_action::resolve_point_with_deadline`. For a
ref target it retries strict resolution within the command deadline and reads
live bounds and state. It scrolls a non-visible target into view once, verifies
valid bounds, requires a stable bounds hash across attempts, and then performs
the `receives_events` hit-test before physical input.

An occluded, invalid, or permanently non-visible target returns a structured
terminal error. Only transient resolution and stability conditions consume the
retry budget. `hover` and `drag` also require headed policy because they move
the cursor; headless mode must reject them before any physical input.

Do not describe these commands as either “full semantic actionability” or
“occlusion only.” Their contract is a dedicated point-resolution pipeline.

## Prevention

- Document pointer commands as a separate family and link to
  `crates/core/src/commands/pointer_action.rs`.
- When changing its checks, update the agent-facing interaction reference in
  the same change and add a regression test for the terminal/retry boundary.
- Keep the final physical dispatch separate from resolution so an error can
  state whether delivery may have started.

## Related

- [Playwright-grade desktop reliability contract](../best-practices/playwright-grade-desktop-reliability-2026-06-02.md)
- [Abort-state guidance on multi-step physical input errors](../best-practices/abort-state-guidance-multi-step-physical-input.md)
