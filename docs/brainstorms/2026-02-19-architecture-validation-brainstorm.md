---
date: 2026-02-19
topic: architecture-validation
category: design
tags: [rust, architecture, platform-adapter, refmap, testing]
---

# Architecture Validation: agent-desktop PRD v2

## What We're Validating

The PRD v2 is a complete engineering blueprint. This session pressure-tested three
gaps the PRD leaves underspecified that need decisions before code is written.

## Decisions Made

### 1. PlatformAdapter Trait — Stay Unified

**Question:** Should the trait split into `PlatformObserver` (reads) + `PlatformActor` (writes)?

**Decision:** Keep the unified 13-method trait as the PRD specifies.

**Rationale:** Splitting does not help command extensibility. The §4.5 extensibility
pattern (one new file + 2 registration lines) never touches the trait — commands call
existing adapter methods. Splitting adds two trait objects, more indirection, and no
benefit for the primary extensibility goal. The 13 methods are manageable.

---

### 2. NativeHandle Persistence — Optimistic with STALE_REF

**Question:** `AXUIElement` on macOS is a live CFTypeRef. It cannot be serialized
across process invocations. How should action commands resolve `@e1` on a fresh CLI
invocation?

**Decision:** Optimistic re-identification. The RefMap stores metadata per ref:
`(pid, role, name, bounds_hash)`. Action commands use this to locate the element in
the current AX tree. If the element is not found or metadata mismatches, return the
`STALE_REF` error (already in the PRD error taxonomy) with suggestion "Run 'snapshot'
to refresh, then retry with updated ref."

**Rationale:** Fast path — most of the time the UI hasn't changed, so no re-traversal
needed. Clean failure path — `STALE_REF` is already documented and agents know how to
handle it. In Phase 4, the daemon holds the RefMap in memory, making this moot.

**RefMap JSON entry shape:**
```json
{
  "@e3": {
    "pid": 1234,
    "role": "textfield",
    "name": "Search",
    "bounds_hash": "a3f9c2",
    "available_actions": ["SetValue", "SetFocus"]
  }
}
```

**Refmap write behavior:** Each snapshot REPLACES the refmap file entirely (not merges).
Agents should snapshot before acting.

---

### 3. Testing Strategy — MockAdapter + Golden Fixtures

**Question:** macOS GitHub Actions runners have no active display session. How do you
test `SnapshotEngine`, `RefAllocator`, and serialization without live AX access?

**Decision:** Both approaches in parallel:

- **MockAdapter** (`agent-desktop-core` test module): An in-memory `PlatformAdapter`
  implementation returning a hardcoded `AccessibilityNode` tree. Exercises the full
  pipeline (adapter → filter → ref-allocate → serialize) with no OS dependency.
  Used for unit tests.

- **Golden JSON fixtures** (`tests/fixtures/`): Real snapshots captured once from
  Finder, TextEdit, etc. Checked into the repo. Used to regression-test that
  serialization format changes don't silently alter the JSON contract.

macOS CI integration tests (GitHub Actions macOS runner) test the real AX adapter
against live apps.

---

### 4. Workspace Bootstrap — All Platform Crates from Day One

**Decision:** Phase 1 creates all crates upfront:

```
crates/
  core/          # agent-desktop-core — fully implemented in P1
  macos/         # agent-desktop-macos — fully implemented in P1
  windows/       # agent-desktop-windows — stub: all methods return Err(not_supported)
  linux/         # agent-desktop-linux — stub: all methods return Err(not_supported)
  mcp/           # agent-desktop-mcp — stub (implemented in P3)
src/             # agent-desktop binary
```

**Rationale:** Enforces the platform isolation boundary (`core` never imports platform
crates) from the first commit. Prevents accidental coupling. The `PLATFORM_UNSUPPORTED`
error already exists in the PRD error taxonomy for stub responses.

---

## Architectural Validations (PRD Is Correct)

The following PRD decisions were validated as sound:

- **Additive phase model** — Phase 1 builds the complete vertical. Phases 2-4 add
  adapters/transports without modifying core. This is the right design.
- **Command extensibility** — One file + 2 registration lines per command. Elegant.
  Strongly enforce this in code review.
- **Ref system** — Depth-first `@e1, @e2` on interactive-only roles. Combined with
  the <500 token budget via compact serialization, this is the right approach.
- **Dual-mode entry** — `--mcp` flag triggers MCP server mode, otherwise CLI.
  Invariant: every MCP tool maps 1:1 to a CLI command.
- **Sync trait for Phase 1** — All macOS AX APIs are synchronous. The async runtime
  (tokio) is only needed for Linux AT-SPI (Phase 2) and MCP server (Phase 3).
  The trait stays sync; adapters handle async internally in Phase 2 via `block_on`.

## Open Questions for Planning

- Token estimation: the <500 token budget (G4) requires a measurement strategy during
  development. Should `tiktoken-rs` or a simple char-count heuristic be used for the
  optional warning?
- Ref stability for multi-window snapshots: confirm that a `snapshot --window w-123`
  only writes that window's refs to the refmap (not a global merge across windows).

## Next Steps

→ `/workflows:plan` to create the Phase 1 implementation plan (10-week milestone breakdown)
