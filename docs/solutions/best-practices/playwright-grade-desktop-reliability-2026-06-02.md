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
"current session" exists. Qualified refs embed their producing snapshot. A bare
`@eN` ref is rejected with `INVALID_ARGS`. Never search another session when the
selected namespace has no match.

### 2. Re-resolve strictly at action time

Load the saved `RefEntry`, then let the adapter re-identify the live element
from role, process generation, source, stable text identity, geometry, path,
and native evidence. Zero matches is `STALE_REF`; more than one plausible match
is `AMBIGUOUS_TARGET`. Mutable field values are not stable identity.

### 3. Separate actionability from dispatch

Ref actions use the shared auto-wait and live actionability checks. Each command
owns its least-permissive base interaction policy: ref actions, including
typing, start headless, while explicit ref-targeted key presses keep their
focus fallback. Headed mode is an opt-in policy elevation, not a different
resolution path. A failed preflight must say why and preserve retry safety.

On macOS, headless natural-input commands use semantic accessibility delivery.
Headed `click`, `right-click`, `type`, `clear`, and `scroll` prefer physical
delivery; semantic-only commands remain semantic. Double- and triple-click,
hover, and drag are physical-only and fail closed without headed permission.
Results expose the delivered step mechanism, so tests and callers can verify
this split instead of inferring it from a successful response.

Pointer commands are a separate physical family. They resolve a live point,
verify visibility, geometry stability, and hit-test receipt, then require an
explicit cursor-moving policy before dispatch. Their terminal errors are not
silently converted into retries.

### 4. Track delivery honestly

Every adapter error carries structured delivery semantics. If a physical action
has started, an error must not claim a safe retry. Multi-step input owns its
cleanup guard and reports the best-known final state. Trace failures are
observability failures; they must not change whether an action was delivered.

Never interpolate a raw user- or app-supplied value into an error message that
can reach a trace sink. Report a shape instead: a character count, a role, or a
bounded enum. A raw value is safe inside `details` only if it sits under a key
matched by `SENSITIVE_KEYS` or has already been reduced to a shape. The redaction
rule lives in `sanitize_trace_value` in `crates/core/src/trace/trace_sanitize.rs`.

### 5. Keep every transport on the same core contract

CLI, batch, and high-level FFI ref actions share the same strict resolution,
policy elevation, actionability, and envelope semantics. `ad_execute_by_ref`
and its timeout variant run that core command path. Route a new language-binding
equivalent of a CLI command through the high-level path, and test both the
observed effect and the reported mechanism for headed and headless input.

`ad_execute_action` and the struct-based direct action entrypoints are
deliberately lower-level escape hatches. They act on a caller-held native handle
or entry, may apply the supplied policy verbatim, and do not claim RefStore-backed
CLI parity. Their safety docs must say which boundary they skip: strict
resolution, preflight, or base-policy elevation. ABI structs are versioned and
layout-pinned alongside the committed C header.

### 6. Verify at three layers

Use deterministic core tests for identity, policy, retry, and delivery rules.
Use C/C++ header compilation and layout tests for FFI. Use the platform's own
permissioned fixture or dogfood gate for native observation and harmless
interaction. On macOS that is a release binary against the SwiftUI fixture. On
Windows it is an in-repo fixture window plus a repo-controlled dogfood capture.
Real-app ignored tests protect platform seams such as window identity and
accessible-name agreement. Headed and headless fixture scenarios must assert
both the observed effect and the reported delivery mechanism.

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
