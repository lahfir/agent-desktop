# Adapter Reliability Conformance

Every platform adapter must satisfy the same ref/action contract. The macOS
adapter is the first implementation, but the tests are written against
`PlatformAdapter` semantics so Windows UIA and Linux AT-SPI can reuse the same
expectations.

The executable smoke harness lives in `src/tests/conformance.rs`. It uses the
public `PlatformAdapter` contract to prove stale live identity blocks dispatch
and stable live identity permits dispatch.

## Required Gates

| Area | Required behavior |
|------|-------------------|
| Snapshot refs | Refs are depth-first, snapshot-scoped, and persisted in the caller session |
| Strict resolve | A ref resolves only when identity still matches; stale refs return `STALE_REF` |
| Ambiguity | Multiple plausible matches return `AMBIGUOUS_TARGET`, never an arbitrary click |
| Actionability | Ref actions check live visibility, enabled state, supported action, policy, and editability before dispatch |
| Wait recovery | `wait --element` can poll the latest session refmap when no snapshot is pinned, honors the caller timeout while resolving, and reports the last observed predicate state |
| Session scope | `--session <id>` and batch item `session` never read or write another session's refmap |
| Trace | `--trace <path>` writes JSONL diagnostics outside stdout and is best-effort unless strict |
| FFI parity | FFI ref actions use strict resolve and actionability checks before adapter dispatch |

## Platform Matrix

| Fixture | macOS AX | Windows UIA | Linux AT-SPI |
|---------|:--------:|:-----------:|:------------:|
| Two identical buttons produce ambiguity | Required | Required | Required |
| Ref disappears after snapshot | Required | Required | Required |
| Disabled button blocks click before dispatch | Required | Required | Required |
| Text field supports value/type actionability | Required | Required | Required |
| Session A latest refmap is invisible to Session B | Required | Required | Required |
| Batch item session overrides inherited session | Required | Required | Required |
| FFI `AdRefEntry` preserves full identity envelope | Required | Required | Required |

## Adding Windows or Linux

Add adapter-specific integration fixtures, but keep expected errors and JSON
shapes identical. Prefer semantic platform actions first (`AXPress`, UIA
Invoke/Value/Selection patterns, AT-SPI actions). Coordinate input is a lower
confidence fallback and must remain explicit in policy or command choice.
