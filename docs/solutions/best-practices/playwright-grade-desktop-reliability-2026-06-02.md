---
title: Playwright-grade desktop reliability contract
date: 2026-06-02
last_updated: 2026-06-10
category: best-practices
module: crates/core, crates/macos, crates/ffi, src
problem_type: best_practice
component: tooling
severity: high
applies_when:
  - Ref resolution, action dispatch, wait semantics, session scope, trace output, or FFI ref-action paths change
  - Platform adapters add live state, bounds, action, or resolution behavior
  - CLI or FFI command paths are refactored around shared helpers
  - Adapter conformance tests are added or changed
tags:
  - reliability
  - ref-actions
  - strict-resolution
  - actionability
  - capabilities
  - wait
  - sessions
  - tracing
  - ffi-parity
---

# Playwright-grade desktop reliability contract

## Context

Playwright is reliable because actions flow through a consistent ladder:
resolve a locator, prove it is actionable, wait when the state is changing, and
fail with structured recovery when the target is stale or ambiguous. Desktop
automation cannot copy browser semantics directly, but it can use the same
engineering shape.

The reliability enhancement moved agent-desktop toward that model without
turning the CLI into a browser tool. Core owns the contract. Platform adapters
provide native evidence. CLI and FFI callers share the same ref-action semantics
unless a lower-level API explicitly documents that it bypasses the strict ref
path.

This learning updates the original contract doc with the implementation details
that matter for future changes.

## Guidance

### Keep Hidden Identity Evidence

Snapshot output may hide pixel bounds for compact, agent-friendly responses, but
the persisted ref identity still needs bounds evidence. The safe pattern is:

1. Ask the adapter for bounds when building a ref-bearing snapshot.
2. Allocate refs and store `bounds_hash` from that full internal tree.
3. Strip visible `bounds` only from the returned output when the caller did not
   request them.
4. Keep `bounds_hash` in the refmap so later actionability can detect stale refs.

Do not let presentation options erase identity evidence. Compact output is a
serialization concern, not a weaker ref contract.

### Treat Mutable Values As Volatile Identity

Strict ref resolution should separate stable labels from mutable control values.
Text field content, selected combobox values, slider values, and incrementor
values can change between snapshot and action without changing the target
element. Core should own that role-conditional identity policy, and platform
adapters should only supply native candidates, live attributes, and primitive
actions.

### Centralize Strict Ref Actions

Ref actions must pass through the same ladder:

1. Load the refmap from the caller's session.
2. Resolve the ref with strict platform identity checks.
3. Return `STALE_REF` when the old element no longer matches.
4. Return `AMBIGUOUS_TARGET` when multiple candidates match.
5. Run live actionability checks before adapter dispatch.
6. Dispatch the adapter action chosen by the command's policy.
7. Release native handles through the adapter boundary.

The CLI path and FFI strict ref-action path should share this model. A lower
level native-handle FFI call may remain available, but it must be documented as
lower-level: it acts on a handle the caller already resolved and therefore does
not have ref identity evidence to re-check.

Command-specific policy still belongs at the command edge. Centralization should
remove duplicated mechanics; it should not flatten `ActionRequest` choices or
post-condition verification.

### Treat Actionability as a Core Contract

Actionability is not an adapter-specific convenience. Core decides whether a
resolved ref is safe to act on using native evidence supplied by the adapter:

- visibility from live bounds
- stability from live bounds hash versus snapshot bounds hash
- enabled state from live state
- supported action from live or snapshot actions
- interaction policy from the command request
- editability from role and supported actions

Unavailable native evidence should be `unknown`, not a false failure, when the
platform cannot provide it. A non-empty live action list can narrow capabilities;
an empty transient live action list should not erase snapshot capabilities.

### Own Capability Vocabulary in Core

Supported action names are part of the cross-platform contract, not incidental
strings. Put the canonical vocabulary, action-to-capability mapping, role
defaults, and membership helpers in one core module. Actionability, ref
allocation, `is` predicates, FFI tests, and platform adapters should refer to
that vocabulary instead of re-declaring string literals.

Platform adapters may discover capabilities differently. macOS maps AX actions
and settable attributes; Windows should map UIA patterns; Linux should map
AT-SPI actions and states. Those native differences must converge into the same
core capability names before core evaluates actionability.

Do not keep pass-through wrappers such as `Action::semantic_capabilities()` when
call sites can use the canonical capability helper directly. Thin wrappers make
future command additions touch multiple files without clarifying ownership.

### Keep Waits Bounded and Honest

Wait commands must not hide permanent adapter failures behind timeout polling.
The reliable split is:

- Retry transient resolution states such as stale, not found, ambiguous, or
  timeout while the caller's timeout budget remains.
- Propagate permanent adapter errors immediately.
- Preserve the last observed retryable state in timeout details. TIMEOUT
  details carry a `kind` discriminant: `"wait_timeout"` for wait-loop expiry
  (predicate, timeout_ms, last observed state) and `"chain_deadline"` for a
  chain step expiring mid-increment or mid-disclosure (observed value or
  expanded state, plus a `mutated` flag) — agents key on `kind` before
  inspecting other fields.
- For `wait --element` without `--snapshot`, refresh the latest-ref cache on a
  bounded cadence; for a fixed `--snapshot`, treat missing refs as invalid input
  instead of silently switching snapshots.
- `--predicate actionable` checks readiness for a specific action via
  `--action` (`click` default, `type`, `set-value`, `clear`); each name maps to
  the exact request its real command runs — policy included — through explicit
  per-name arms, so an unknown name errors instead of silently inheriting a
  default policy.

This keeps `wait` useful for changing desktop state without making it a blanket
error suppressor.

Resolver deadline checks should also have one owner. If both root selection and
tree traversal need the same timeout error and remaining-budget logic, share a
small resolver-deadline helper instead of copying the deadline branch into each
module. That keeps `wait --element` bounded through every native AX read without
growing resolver files toward the size limit.

### Make Tracing Diagnostic, Not Behavioral

Trace output belongs in the requested JSONL file and never in stdout. It must be
safe for agents to use with machine-readable command output:

- create private trace files
- reject symlink trace paths on Unix
- redact sensitive text/value/name/message fields
- let `--trace-strict` fail on setup and pre-action trace writes
- keep post-action success trace writes best-effort after the desktop mutation
  has already happened
- with a trace-enabled session manifest from `session start`, write per-process
  JSONL segments under `sessions/<id>/trace/<pid>-*.jsonl` automatically;
  `--trace <path>` still overrides to one file for CI or one-offs

Reporting a successful desktop mutation as failed because the final trace write
failed is worse than losing that final diagnostic event.

Do not describe the current trace as equivalent to Playwright Trace Viewer. It
is a JSONL diagnostic stream, not a bundled timeline artifact with before/after
snapshots, screenshots, source metadata, and an inspection UI. A Playwright-like
desktop trace should be introduced as a separate artifact format if needed.

### Keep the Foundation Cross-Platform

Core owns the contract; adapters own native evidence. Windows and Linux should
not fork CLI semantics. UIA and AT-SPI implementations must map their native
identity fields into the same `RefEntry` concepts: role, name, value,
description, state, bounds, supported actions, source surface, root ref, and
tree path.

Actionability should prefer one native live-state read that returns state,
bounds, and supported actions together. Platform adapters may fall back to
separate reads, but the CLI behavior must remain identical: empty transient
action reads do not erase snapshot capabilities, while a non-empty live action
set that lacks the required action can block dispatch.

Do not add macOS-only assumptions to core to solve a macOS bug. If a behavior is
part of the CLI or FFI contract, core should express the contract and each
adapter should provide native evidence or return a structured unsupported error.

### Triage Review Findings Against Project Constraints

Not every simplification suggested by a reviewer is a good simplification. In
this repo:

- One command per file is intentional; collapsing small command wrappers fights
  the project structure.
- Keeping helpers extracted is correct when inlining would push a file toward
  the 400 LOC limit or duplicate a shared contract.
- Centralizing strict ref-action behavior in core is better than moving it only
  into FFI because CLI and FFI parity is the invariant.
- Splitting tests by owner is better than letting one broad test file become the
  dumping ground for ref maps, ref stores, sessions, and legacy migration.
- Windows/Linux portability means platform-neutral contracts in core, not
  pretending current macOS AX evidence already exists on other platforms.

False positives should be marked as such. Do not add code only to satisfy a
review comment that contradicts the architecture.

## Why This Matters

The user-visible failure modes are severe:

- acting on a stale ref can mutate the wrong UI element
- hidden bounds in compact snapshots can accidentally weaken future stale-ref
  detection
- wait loops that swallow permanent errors waste time and obscure permissions or
  adapter failures
- FFI and CLI divergence makes language bindings less reliable than the command
  line
- trace failures after mutation can make successful desktop actions look failed

Playwright's reliability comes from a predictable action pipeline. agent-desktop
needs the same predictability across desktop platforms, even though native
accessibility APIs expose weaker and less uniform evidence than a browser DOM.

## When to Apply

Any change to ref resolution or action dispatch must include tests for:

- stale ref rejection
- ambiguous target rejection
- actionability failure before dispatch
- retrying waits that honor timeout and report last observed state
- session isolation
- FFI parity when the behavior is exposed through C ABI
- compact snapshot output preserving hidden identity evidence in the refmap
- capability mappings using the central vocabulary instead of copied strings
- resolver deadlines applied before native reads and shared by resolver modules
- trace strictness without post-mutation false failures

If a platform needs a coordinate fallback, the fallback must be explicit and
lower confidence. Do not silently replace a failed semantic action with a pixel
click.

## Examples

Snapshot construction should request identity bounds internally, then hide only
presentation bounds when needed:

```rust
let identity_opts = opts.with_ref_identity_bounds();
let tree = adapter.get_tree(&window, &identity_opts)?;
let (tree, refmap) = allocate_refs(tree, opts)?;
let tree = strip_ref_bounds_when_hidden(tree, opts);
```

Ref action execution should keep the command-selected policy while centralizing
the strict ladder:

```rust
let (entry, handle) = resolve_ref_with_context(ref_id, snapshot_id, adapter, context)?;
check_actionability_with_trace(ref_id, &entry, handle.handle(), adapter, &request, context)?;
let result = adapter.execute_action(handle.handle(), request)?;
```

The platform adapter should expose a single live element read when possible:

```rust
LiveElement {
    state: live_state,
    bounds: live_bounds,
    available_actions: live_actions,
}
```

Windows UIA and Linux AT-SPI adapters can fill those fields differently, but the
core actionability decision must stay the same.

## Related

- [Keep FFI action policy aligned with CLI action policy](keep-ffi-action-policy-aligned-with-cli-2026-05-12.md)
- [Preserve command policy semantics during shared ref-action refactors](preserve-command-policy-semantics-during-refactor-2026-05-12.md)
- [Guard OS-reordered resources with an identity fingerprint, not a raw index](identity-fingerprint-against-os-reorder-2026-04-16.md)
- [Progressive snapshot contract fixes after review](../logic-errors/progressive-snapshot-review-contract-2026-04-16.md)
