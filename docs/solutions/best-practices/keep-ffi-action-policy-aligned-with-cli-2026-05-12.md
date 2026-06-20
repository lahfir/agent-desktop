---
title: Keep FFI action policy aligned with CLI action policy
date: 2026-05-12
last_updated: 2026-06-10
category: best-practices
module: crates/ffi
problem_type: best_practice
component: ffi
severity: high
applies_when:
  - FFI exposes an action path that mirrors CLI ref commands
  - A core action gains an InteractionPolicy or other side-effect gate
  - A new FFI wrapper calls PlatformAdapter directly instead of command dispatch
tags:
  - ffi
  - interaction-policy
  - cli-parity
  - abi
---

# Keep FFI action policy aligned with CLI action policy

## Context

The CLI action path moved to `ActionRequest { action, policy }`, but the FFI
`ad_execute_action` wrapper initially constructed the cursor/focus policy (then
named `physical`, since renamed `ActionRequest::headed`) for every action. That
meant C, Swift, Python, Go, and Node consumers received focus-stealing and
cursor-moving behavior for actions that the CLI treats as headless by default.

## Guidance

FFI wrappers that mirror CLI actions must use the same default side-effect
contract as the CLI. For direct action execution, the default is headless:
no focus stealing and no cursor movement.

When a lower-level FFI consumer intentionally wants broader behavior, expose the
policy as an explicit ABI value. Do not hide it inside a wrapper default.

## Review Rule

Any change to `ActionRequest`, `InteractionPolicy`, or command preflight must
include a pass over `crates/ffi/src/actions/`. If the CLI and FFI can perform the
same action, they must document the same default and expose any divergence as an
explicit parameter.

Behavioral parity is only half the FFI review: any structural change to a public
`repr(C)` type — adding, removing, or reordering fields, or growing a struct that
is embedded by value inside another — must also update the three-layer size pin
(Rust const assert, header `_Static_assert`, layout integration test). Size drift
in an embedded struct silently propagates to every outer struct that embeds it.

## Related

- `best-practices/ffi-repr-c-struct-size-pinning.md` — the structural-parity companion: the full three-layer pinning protocol and the AdAction silent-growth incident that motivated it.
