---
title: Build desktop actions as an observe-resolve-preflight-dispatch contract
date: 2026-07-11
category: best-practices
module: core reliability contracts
problem_type: architecture_pattern
component: tooling
severity: high
applies_when:
  - "Adding or changing a ref-based command"
  - "Changing platform resolution, actionability, or physical fallback"
  - "Adding CLI or FFI automation surfaces"
tags: [reliability, refs, actionability, sessions, ffi, tracing]
---

# Build desktop actions as an observe-resolve-preflight-dispatch contract

## Context

Desktop accessibility trees are live, mutable, and platform-specific. A ref
from an earlier observation is evidence, not a native handle. Reliable
automation therefore has to make each boundary explicit and reject uncertainty
instead of guessing.

## Guidance

### 1. Scope observation before storing it

A snapshot is stored in either the global namespace or a selected session.
Explicit `--session` wins over `AGENT_DESKTOP_SESSION`; no process-global
“current session” exists. Qualified refs embed their producing snapshot. Bare
refs are accepted only with an explicit snapshot ID. Never search another
session when the selected namespace has no match.

### 2. Re-resolve strictly at action time

Load the saved `RefEntry`, then let the adapter re-identify the live element
from role, process generation, source, stable text identity, geometry, path,
and native evidence. Zero matches is `STALE_REF`; more than one plausible match
is `AMBIGUOUS_TARGET`. Mutable field values are not stable identity.

### 3. Separate actionability from dispatch

Semantic ref actions use the shared auto-wait and live actionability checks.
The command owns the base interaction policy: most actions are headless;
typing and ref-targeted key presses may use focus fallback; headed mode can
only elevate policy. A failed preflight must say why and preserve retry safety.

Pointer commands are a separate physical family. They resolve a live point,
verify visibility, geometry stability, and hit-test receipt, then require an
explicit cursor-moving policy before dispatch. Their terminal errors are not
silently converted into retries.

### 4. Track delivery honestly

Every adapter error carries structured delivery semantics. If a physical action
has started, an error must not claim a safe retry. Multi-step input owns its
cleanup guard and reports the best-known final state. Trace failures are
observability failures; they must not change whether an action was delivered.

### 5. Keep every transport on the same core contract

CLI, batch, and high-level FFI ref actions use the same strict resolution,
policy elevation, actionability, and envelope semantics. Low-level FFI native
handle calls are intentionally escape hatches and must document that they skip
those guarantees. ABI structs are versioned and layout-pinned alongside the
committed C header.

### 6. Verify at three layers

Use deterministic core tests for identity, policy, retry, and delivery rules;
C/C++ header compilation and layout tests for FFI; and a permissioned release
binary against the SwiftUI fixture for native observation and harmless
interaction. Real-app ignored tests protect platform seams such as window
identity and accessible-name agreement.

## Why This Matters

The strongest failure mode is a plausible success against the wrong live
element. This design makes uncertainty visible: callers receive a structured
stale, ambiguous, policy, timeout, or delivery-unknown result instead of a
best-effort click.

## Prevention

- Put platform-neutral rules in core; adapters translate native evidence only.
- Add a regression test at the boundary that failed, then run the fixture or a
  safe read-only native probe for adapter changes.
- Keep docs and help text in the same change as a command-contract change.
- Do not add a fallback that bypasses strict resolution, policy, or delivery
  accounting merely to improve apparent success rate.

## Related

- [Keep progressive snapshots namespace-scoped and ref-safe](../logic-errors/progressive-snapshot-review-contract-2026-04-16.md)
- [Document pointer actions from their own reliability pipeline](../documentation-gaps/hover-drag-skip-the-actionability-battery.md)
- [Real-app tests are the platform-adapter gate](real-app-tests-are-the-platform-adapter-gate.md)
- [Keep raw caller arguments out of trace-reachable error messages](../conventions/keep-raw-arguments-out-of-trace-reachable-error-messages.md)
