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

`hover` and `drag` resolve their point in two phases, mirroring the pre-lease
and leased split that ref actions use. `pointer_action::wait_for_point_with_deadline`
runs first, without exclusivity: for a ref target it retries strict resolution
within the command deadline, reads live bounds and state, scrolls a non-visible
target into view once, verifies valid bounds, and requires a stable bounds hash
across attempts. `pointer_action::resolve_point_under_lease` then re-resolves
once the interaction lease is held and performs the `receives_events` hit-test
before physical input.

Two different hit-tests exist and should not be conflated. The shared
actionability battery runs a multi-candidate-point check for actions whose
`Action::requires_hit_test()` is true — which now includes the click family,
not only hover and drag. The pointer pipeline runs its own single-point check
on the resolved coordinate. Hover and drag use the latter.

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
