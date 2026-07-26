---
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
execution: code
type: refactor
product_contract_source: ce-plan-bootstrap
created: 2026-06-30
---

# refactor: session-first trace architecture

## Summary

Make the **session** the first-class container that owns its trace, instead of threading a `--trace <path>` flag through every command. A session created by `session start` records tracing automatically — each process appends to its own segment file under the session directory — and flag/env/pointer activation lets an agent set the session once and never repeat a flag. This closes the footgun where an agent that forgets `--trace` on one command silently loses that command's trace, lights up structured tracing for FFI consumers that opt in, and makes concurrent multi-agent sessions safe by convention enforced with precedence order and a start-time guardrail. The trace **viewer** is a deliberate follow-up plan; this plan builds only the architecture it will sit on.

Trace-on is gated by the session **manifest** (`session.json`, `trace: on`), not by the mere presence of a `--session` id — so existing `--session` callers and FFI embedders who never run `session start` see today's behavior unchanged and never get surprise files on disk.

---

## Problem Frame

Sessions and snapshots are already a coherent, lock-protected, hierarchical model on disk (`~/.agent-desktop/sessions/{id}/` with `refstore.lock`, `latest_snapshot_id`, `snapshots/{snapshot_id}/refmap.json`). Tracing is the one piece that does not belong to the session: it is selected per-invocation by `--trace <path>` (`src/cli/mod.rs`, built once in `crates/core/src/context.rs::CommandContext::new`). Three consequences follow, all evidence-backed:

1. **Silent trace holes.** A command run without `--trace` emits nothing and returns `Ok(())` — no warning. A multi-command run only produces a continuous trace because the agent re-passes the identical path every time. One dropped flag = an invisible gap.
2. **FFI has no structured trace.** `crates/ffi/src/adapter.rs` builds its `CommandContext` with `trace_path: None`, so the entire structured event catalog is CLI-only; Python/Swift/Go/Node consumers get only unstructured log-callback lines.
3. **Shared traces corrupt.** `crates/core/src/trace.rs::write_event` does two separate syscalls (JSON body, then newline) with no buffering and no cross-process lock. Two agents appending to one trace file interleave into invalid JSONL. This bug exists today and becomes the common case the moment a session owns a shared trace.

Two more gaps block a session-first model: there is **no session lifecycle** (sessions are created implicitly and never cleaned — directories leak, and `discover_snapshot_base` slows as they accumulate), and the **512-snapshot prune** would silently delete snapshots a long session's trace *references* (the trace log itself is unaffected — see KTD5 for the precise boundary).

Non-goals for this plan: the trace viewer, per-step screenshot/tree artifacts, swapping the lock implementation, and deleting the legacy migration shim (see Scope Boundaries).

---

## Requirements

- **R1.** When a session **whose manifest has `trace: on`** is active and no explicit `--trace` is given, tracing records automatically to the session directory. No per-command flag is required. A bare `--session <id>` with no manifest selects the snapshot namespace only (today's behavior) and does **not** trace.
- **R2.** Each OS process writes its own trace **segment** (`<session>/trace/<pid>-<proc_start_ts>.jsonl`), computed once per process; concurrent processes write to distinct files and never interleave. Each event line carries a per-process monotonic `seq` so a future reader can tie-break equal `ts_ms`. (Merging segments into a timeline is a reader/viewer concern, deferred.)
- **R3.** Explicit `--trace <path>` still works and overrides the session sink, writing a single file (back-compatible for CI/one-off use). Every event is written with one `write_all` of a fully-buffered line (atomic append).
- **R4.** The active session is resolved once, in precedence order: explicit `--session` > `AGENT_DESKTOP_SESSION` env var > `current_session` pointer file > none. The pointer is written **only** by `session start`.
- **R5.** FFI consumers get the same structured, session-bound trace as the CLI when they use a `trace: on` session — with no per-call flag and no ABI change (the session setter already exists). Setting a session id purely for snapshot scoping does **not** turn on disk writes.
- **R6.** A session lifecycle exists: `session start [--name] [--no-trace]`, `session end [id]`, `session list`, `session gc`. `start` creates the session dir + `trace/`, writes a `session.json` manifest (`trace: on` unless `--no-trace`) and the `current_session` pointer, and prints the id; `end` seals the manifest and clears the pointer; `list` reports the manifest fields (id/name/created/ended); `gc` removes ended and provably-stale sessions.
- **R7.** The trace **log** is never removed by snapshot pruning (segments live outside `snapshots/`). A referenced `snapshot_id` may still be pruned in a long session — full tree replay for pruned state is a viewer concern (KTD5).
- **R8.** Shared-session contract: a session is a shared container; every agent acts on the `snapshot_id` returned by its own snapshot call. Implicit "latest" is a single-agent convenience, not a multi-agent guarantee.
- **R8a.** Independent-session contract: concurrent independent sessions isolate per-process via `AGENT_DESKTOP_SESSION`, never via the global `current_session` pointer (KTD3a). `session start` guards the pointer against silently clobbering a still-live session (KTD3b).
- **R9.** Trace redaction, file-permission hardening (`0600`/`O_NOFOLLOW`, `0700` dirs), and the per-file oversize guard are preserved for the session sink; the manifest `--name` is validated/scrubbed before persistence.
- **R10.** `session gc` will not reap a session with a **live writer** — liveness is checked (active `refstore.lock` pid or recent `trace/` mtime), not creation-age alone. Directory removal uses the same symlink-safe pattern as snapshot prune.
- **R11.** Activating a session relocates the **snapshot/ref namespace** as well as the trace (a session owns both) — this is intentional and documented; explicit `--snapshot <id>` still resolves cross-session, so only implicit "latest" is affected across a `session start` boundary.
- **R12.** No new platform-adapter methods; `agent-desktop-core` stays free of platform crates (CI `cargo tree` gate stays green). Standing constraint, verified by the DoD gate.

**Success criteria:** an agent runs `session start` (or sets `AGENT_DESKTOP_SESSION`), issues a sequence of commands with no `--trace`, and finds complete, uncorrupted, per-line `ts_ms`+`seq`-ordered segments under the session — even with two agents sharing it — then `session gc` reclaims it without disturbing a live session.

---

## Key Technical Decisions

**KTD1 — The session owns the trace; `--trace` becomes an override.** Tracing is bracketed by the session lifecycle, mirroring Playwright (`context.tracing.start/stop`), CDP (`Tracing.start/end`), and Appium (session-scoped artifacts). Activation follows the kubectl/ssh-agent convention (set once via env/pointer, per-command override still available) — the correct fit for a one-process-per-command CLI that keeps state on the filesystem rather than in a daemon.

**KTD1a — Trace-on is gated by the manifest, not by the session id.** Auto-trace requires `session.json` with `trace: on` (written by `session start`; `--no-trace` sets it off). This is the keystone decision: it keeps a bare `--session <id>` a pure namespace selector (today's behavior, no surprise files), wires `--no-trace` through one flag read, and prevents FFI embedders who set a session id for snapshot scoping from getting unexpected data-at-rest. `--trace <path>` still forces tracing regardless of manifest.

**KTD1b — A session owns snapshots *and* trace (activation relocates both).** `CommandContext.session_id()` already routes `RefStore::for_session()` — where refmaps/`latest_snapshot_id` live — so widening activation (env/pointer) moves the snapshot namespace too, not just the trace. This is intentional and consistent with "a session is the container." Consequence, documented and tested: across a `session start` boundary, implicit "latest" points at the new session; a snapshot taken before is reached only by explicit `--snapshot <id>` (which still resolves cross-session via `discover_snapshot_base`). Without this decision named, the coupling would surface as a mysterious `STALE_REF`.

**KTD2 — Per-process trace segments, not one shared file.** Each OS process writes `<session>/trace/<pid>-<proc_start_ts>.jsonl`. Lock-free and multi-agent-safe: the two-syscall interleaving bug cannot occur because no two processes share a file. The `(pid, proc_start_ts)` pair is memoized **once per OS process** in a `OnceLock` — so a long-lived FFI host that constructs many `CommandContext`s still writes to one segment (not one per call), and pid reuse across time yields distinct filenames. Each line also carries a per-process monotonic `seq`. The single-file `--trace` override gets the build-line-then-one-`write_all` atomicity fix (R3). Readers must tolerate a truncated final line (crash/OOM/NFS mid-write) by skip-and-warn — a foundation guarantee the viewer relies on.

**KTD3 — Opt-in per run (a run = a session), stated honestly.** The chosen answer to "the agent forgets `--trace`" is to make the session the trace boundary and start it once per run (`session start`), matching the user's "make each run a session" framing and the Playwright/CI norm. This **reduces** the footgun from once-per-command to once-per-run; it does **not eliminate** it — forgetting `session start` still yields no trace. The rejected alternative, ambient default-on (trace always, zero setup), would fully eliminate it but change default behavior for every existing/CI caller and always write files; it is offered as a deferred, electable option. To make the residual *observable* rather than silent, `status` reports whether a session is active and tracing (no per-command stderr noise).

**KTD3a — The `current_session` pointer is a single-active-session convenience; concurrent independent sessions use the env var.** The pointer (`~/.agent-desktop/current_session`) is one global file written only by `session start`. It is **not** per-process. Concurrent independent sessions each set `AGENT_DESKTOP_SESSION` (per-process isolation, precedence above the pointer); the pointer is only for a single active session.

**KTD3b — `session start` guards the pointer against silent clobbering.** Because two agents each calling `session start` would clobber the pointer and cross-contaminate, `start` refuses (with `--force`) when the existing pointer references a **still-live** session (reusing `RefStoreLock`'s pid-liveness). This turns the highest-risk silent-contamination case into a loud failure, so "safe by construction" is honestly "safe by precedence + a start-time guardrail," not a bare convention.

**KTD4 — Shared container, explicit `snapshot_id` for multi-agent; no per-agent latest.** The `latest_snapshot_id` pointer is a single-agent convenience. The multi-agent contract is: read the shared pool, act on the id your own snapshot returned. No per-agent latest machinery.

**KTD5 — Trace *log* is decoupled from the working-snapshot cache; referenced-state fidelity is a viewer concern.** Segments live at `<session>/trace/`, never under `snapshots/`, so the 512-cap prune cannot delete the trace. The trace's own per-event data (identity, actionability report, activation-chain steps, post-state) is self-contained. It does **not** guarantee that a `snapshot_id` referenced by an old event still resolves to its full tree — that state can be pruned. Copying per-step refmaps into the trace for full time-travel is deferred to the viewer with a recorded decision (**copy**, Playwright-style).

**KTD6 — A small `session` core module owns the on-disk contract.** Manifest (`session.json`), the `current_session` pointer, activation resolution, liveness/gc, and list live in one core module; the CLI `session` subcommands are thin wrappers. Reuses `write_private_file` hardening. `gc` uses the snapshot-prune symlink-safe removal pattern.

---

## High-Level Technical Design

Trace sink selection in `CommandContext::new`:

```
explicit --trace <path>?  ──yes─▶ single file at <path>   (buffered, one write_all)
     │ no
     ▼
active session has manifest trace:on?  ──yes─▶ segment  <session>/trace/<pid>-<procTs>.jsonl
     │ no                                             (dir pre-created by `session start`;
     ▼                                                 sink opens lazily on first event)
no trace (writer: None)   (bare --session, or no session → today's behavior)
```

Active-session resolution (once, at process start; drives BOTH snapshot namespace and trace):

```
--session <id>  ─▶ AGENT_DESKTOP_SESSION ─▶ current_session pointer ─▶ none
```

Session directory:

```
~/.agent-desktop/
├── current_session                 # pointer, written ONLY by `session start` (clobber-guarded)
└── sessions/run-42/
    ├── session.json                # id, name(validated), created_at, ended_at?, trace: on|off
    ├── refstore.lock               # (unchanged) — also the gc liveness signal
    ├── snapshots/<id>/refmap.json   # 512-cap prune applies HERE only
    └── trace/                       # pre-created by `session start`; NEVER pruned
        ├── 1837-2291.jsonl          # agent A (pid+procTs, memoized once/process)
        └── 1904-2295.jsonl          # agent B
```

---

## Implementation Units

### U1. Expose the session directory from `RefStore`

**Goal:** Give trace wiring and the session module a way to locate a session's directory.
**Requirements:** R1, R2.
**Dependencies:** none.
**Files:**
- `crates/core/src/refs_store.rs` — add `pub(crate) fn base_dir(&self) -> &Path` and `pub(crate) fn trace_dir(&self) -> PathBuf` (`base_dir/trace`). `base_dir` is currently a private field with no accessor.
- `crates/core/src/refs_store_tests.rs` — accessor tests.
**Approach:** Pure additive path accessors, no filesystem side effects (no dir creation). `trace_dir()` on the default-root store is defined for symmetry but is unreachable under the sink-selection logic (no session ⇒ no trace).
**Patterns to follow:** existing private-path helpers (`snapshots_dir`, `lock_path`).
**Test scenarios:**
- `for_session(Some("run-42")).trace_dir()` returns `.../sessions/run-42/trace`.
- Accessors create no directories (assert no side effects).
**Verification:** `cargo tree -p agent-desktop-core` clean; store tests pass.

### U2. Segment trace writer: per-process filename, lazy open, atomic line, `seq`

**Goal:** Let `TraceConfig` write a per-process segment in a directory, opened lazily, with atomic lines and a tie-break counter.
**Requirements:** R2, R3, R9.
**Dependencies:** none.
**Files:**
- `crates/core/src/trace.rs` — add a `TraceSink` enum (`File(path)` | `SegmentDir(dir)`). Memoize `(pid, proc_start_ts)` in a process-wide `OnceLock` so all `TraceConfig`s in one process resolve to the same segment filename. **Defer the file open until first `write_event`** (store the path/dir at construction; open on first emit) so no empty segment is created for `version`/`skills`/help invocations. In the segment branch, **create the parent `trace/` dir** (recursive, `0700`) at first open if absent — the current `open_trace_file` does not `mkdir` the parent. Fix `write_event` to serialize the full line (event + `ts_ms` + per-process monotonic `seq` + redacted fields + `session_id`) into a buffer and issue one `write_all`. Preserve `0600`/`O_NOFOLLOW`/oversize hardening and `sanitize_trace_value` for both sink kinds.
- `crates/core/src/trace_tests.rs` — segment/atomicity/seq/lazy-open tests.
**Approach:** Redaction and `session_id` injection are unchanged (run in `write_event`). `--trace-strict` semantics preserved. The `seq` is a per-`OnceLock` `AtomicU64`.
**Patterns to follow:** current `open_trace_file`/`write_event`; the `Arc<Mutex<File>>` in-process guard stays for batch sharing.
**Test scenarios:**
- Two `TraceConfig`s constructed in the same process → same segment filename (OnceLock memoization).
- A fresh session dir with no `trace/` → first `write_event` creates `trace/` and a non-empty segment (the silent-hole regression).
- A `version`/no-op invocation writes **no** segment file (lazy open).
- One `write_event` → exactly one well-formed JSONL line; body and newline never split.
- `seq` increments per event within a process; present on every line.
- Redaction, `session_id`, permission + oversize guards identical to single-file path.
- Reader tolerance: a manually-truncated final line does not make the segment unparseable line-by-line.
**Verification:** trace unit tests pass; each segment is valid JSONL line-by-line.

### U3. Manifest-gated auto-trace in `CommandContext::new` + snapshot coupling + batch

**Goal:** Wire the session sink, gated by the manifest; handle batch session overrides; document/test the snapshot coupling.
**Requirements:** R1, R3, R9, R11.
**Dependencies:** U1, U2, U5.
**Files:**
- `crates/core/src/context.rs` — in `CommandContext::new`, select the sink: explicit `trace_path` → single file; else if the active session's manifest has `trace: on` → `RefStore::for_session(session_id)?.trace_dir()` segment sink; else → no trace. In `for_batch_item`, if the item overrides to a **different** session while tracing, re-derive the sink for that session (not just swap `session_id`), or reject the override — never write an item's events into the parent session's segment.
- `crates/core/src/context_tests.rs` — sink-selection + batch + coupling tests. If `context.rs` approaches the 400-LOC limit, extract inline tests to `context_tests.rs` first.
**Approach:** The manifest read goes through the U5 module. `RefStore::for_session` here is a path computation (no writes). Segment opens lazily (U2).
**Patterns to follow:** existing `CommandContext::new`; `with_headed`/`with_wait_selector`.
**Test scenarios:**
- `trace: on` session, no `--trace` → events in `<session>/trace/<pid>-*.jsonl`.
- Bare `--session foo` with **no** manifest → namespace selected, **no** trace (existing-caller regression).
- `session start --no-trace` session → no trace; snapshots still namespaced to it.
- `--trace <path>` overrides regardless of manifest.
- **Coupling:** a snapshot taken before `session start`, then `click @ref` after with implicit latest → resolves against the new session (documents R11); the same ref with explicit `--snapshot <old-id>` still resolves (cross-session).
- Batch item overriding to a different session does not write into the parent's segment.
**Verification:** `cargo test -p agent-desktop` + core context tests pass.

### U4. FFI: session-bound structured trace (verification + opt-out)

**Goal:** FFI consumers get the trace via a `trace: on` session; setting a session id alone does not write files.
**Requirements:** R5.
**Dependencies:** U3.
**Files:**
- `crates/ffi/src/adapter.rs` — `ad_adapter_create_with_session` already validates + stores `session_id`, and `command_context()` already passes it to `CommandContext::new`; with U3's manifest gate this yields trace only for a `trace: on` session. **No ABI change** — verification + a smoke test. Confirm an FFI consumer can create a `trace: on` session (via the CLI or a documented call) and that a plain session-id set stays trace-off.
- `crates/ffi/` C-ABI tests — smoke: FFI call under a `trace: on` session writes a segment; under a plain session writes none.
**Approach:** Behavior falls out of U3 + KTD1a; scope is verification + tests. Because the OnceLock memoizes per process (U2), a long-lived FFI host writes one segment, not one per call.
**Patterns to follow:** existing FFI context construction; header/drift tests.
**Test scenarios:**
- FFI call under a `trace: on` session → one segment for the process, structured events.
- FFI call with a plain (no-manifest / `--no-trace`) session → no trace files.
- Header/codegen drift gates green (no ABI change).
**Verification:** `cargo test -p agent-desktop-ffi --tests`; drift gates green.

### U5. `session` core module: manifest, pointer, resolution, liveness, gc

**Goal:** One core module owning the session on-disk contract, activation resolution, and safe gc.
**Requirements:** R4, R6, R8, R8a, R9, R10.
**Dependencies:** U1.
**Files:**
- `crates/core/src/session/mod.rs` (new) — `session.json` manifest (id, validated `name`, created_at, ended_at?, `trace: on|off`); `current_session` pointer read/write; `resolve_active_session(explicit, env) -> Option<String>` (flag > env > pointer > none, env above pointer); `is_live(session)` (active `refstore.lock` pid via `RefStoreLock`'s liveness, or recent `trace/` mtime); `list()` (manifest fields only); `gc()` (remove ended + provably-stale-and-not-live; symlink-safe removal mirroring `refs_store_prune`). Validate/scrub `name` before persistence (it bypasses trace redaction).
- `crates/core/src/session/session_tests.rs` (new).
- `crates/core/src/lib.rs` — register `session` (pub boundary).
**Approach:** Resolution is a pure function. `gc` never removes a live or pointer-referenced session. `list` is manifest-only (no subtree walk) to stay within R6's scope.
**Patterns to follow:** `refs_store.rs` path/private-file conventions; `RefStoreLock` pid-liveness; `refs_store_prune` symlink-safe removal; `validate_session_id`.
**Test scenarios:**
- Precedence: explicit > env > pointer > none; env beats a *different* pointer.
- Pointer absent → none (bare command → default behavior).
- Manifest round-trips with/without `ended_at`; `--name` with a control char is scrubbed/rejected.
- `gc` removes ended + stale; **leaves a session with a live `refstore.lock` pid**; leaves a session with recent `trace/` mtime; never removes the pointer-referenced session; refuses to follow a symlinked session dir.
- `list` reports manifest fields without walking `snapshots/`.
**Verification:** core session tests pass; `cargo tree` clean.

### U6. `session` CLI commands (`start` / `end` / `list` / `gc`)

**Goal:** The user-facing lifecycle, with the clobber guard and trace-dir pre-create.
**Requirements:** R6, R8a, R9.
**Dependencies:** U5.
**Files:**
- `crates/core/src/commands/session.rs` (new) — `execute()` over the four subactions. `start` creates the session dir + `trace/`, writes the manifest (`trace: on` unless `--no-trace`) and the pointer, prints the id; **refuses to clobber a live pointer without `--force`** (KTD3b). `end` seals + clears the pointer. `list`/`gc` render U5 results.
- `crates/core/src/commands/mod.rs` — register.
- `src/cli/mod.rs` + `src/cli_args/` — `Session` subcommand: `start [--name] [--no-trace] [--force]`, `end [id]`, `list`, `gc [--older-than] [--ended]`.
- `src/dispatch/mod.rs` — dispatch arm.
- `src/cli/contract_tests.rs` — CLI contract.
**Approach:** Follows the Extensibility Pattern (new command file + cli variant + dispatch arm). `start` pre-creating `trace/` removes any first-write dir race.
**Patterns to follow:** a multi-action command (`skills`) + dispatch registration.
**Test scenarios:**
- `session start` creates dir + `trace/` + manifest + pointer, prints a valid id; a subsequent bare command traces.
- `session start` over a **live** pointer without `--force` → refused (loud); with `--force` → overrides.
- `session start --no-trace` → session exists, bare commands do not trace.
- `session end` clears the pointer; subsequent bare command no longer traces/attaches.
- `--session X` explicit overrides an active pointer.
- Envelope/exit-code contract per subaction.
**Verification:** `cargo test -p agent-desktop` pass; `--help` shows `session`.

### U7. Activation wiring, retention guard, and docs

**Goal:** Resolve the active session once in the binary, guarantee trace survives pruning, and document the model.
**Requirements:** R4, R7, R8, R8a, R11.
**Dependencies:** U3, U5, U6.
**Files:**
- `src/main.rs` — resolve the active session once via `session::resolve_active_session(cli.session, env)` and thread the resolved id into `CommandContext::new` (batch inherits; not re-resolved per item).
- `crates/core/src/refs_store_prune.rs` — a test asserting prune scans only `snapshots/` and never `trace/` (prune already scopes to `snapshots/`; this pins it).
- `skills/agent-desktop/references/*.md`, `src/cli/help_after.txt`, `CLAUDE.md` — document: session owns trace **and** snapshots (R11 coupling + the explicit-`--snapshot` escape); manifest-gated trace; activation precedence + `AGENT_DESKTOP_SESSION`; per-process segments + `seq`; the shared-vs-independent contracts (R8/R8a); the clobber guard; that trace is opt-in per run and `status` shows tracing state.
**Approach:** Activation resolution lives at the binary edge so batch inherits one resolved id. The prune guard is a test, not new logic.
**Patterns to follow:** `--headed` global-flag threading; existing prune tests.
**Test scenarios:**
- A session past the 512-snapshot cap keeps every `trace/*.jsonl`.
- `AGENT_DESKTOP_SESSION` set + no `--session` → attaches to that session (resolved in `main`).
- `--session` explicit overrides env and pointer.
- Docs updated: `Test expectation: none`.
**Verification:** prune retention test passes; full gate set green.

---

## Scope Boundaries

**In scope:** manifest-gated session-owned trace, per-process segments (memoized filename, `seq`, lazy open, atomic line, reader-tolerance), `--trace` override, activation (flag/env/pointer), snapshot-coupling documentation, FFI opt-in trace, `session start/end/list/gc` with clobber guard + gc liveness, retention guard, docs.

### Deferred to Follow-Up Work
- **Trace viewer** — the timeline/tree/activation-chain UI that reads a session's segments and **merges them by `ts_ms`+`seq`** (same-ms ties, cross-process clock skew, truncated-final-line tolerance are the viewer's problems). Separate plan; this plan is its foundation.
- **Per-step artifacts for replay** — `screenshot_id`/`tree_snapshot_id` per action + copying per-step refmaps into the trace (recorded decision: **copy**, Playwright-style). With the viewer.
- **Ambient default-on tracing** — a bounded, auto-gc'd default-session trace that eliminates the forget-`session start` case entirely; electable later if opt-in proves insufficient (KTD3).
- **Session-level trace budget** — a total-bytes cap across a session's segments (per-file 64MB still applies); add if unbounded growth between `gc` runs proves a problem.
- **Session zip/bundle export**, and **legacy `last_refmap.json` shim removal** (low-risk, separable).

### Out of scope (not this product)
- **Swapping `RefStoreLock` for `flock`** — `flock` is unreliable over NFS and `~/.agent-desktop` can be a network home; the PID+token lock may exist for that portability. Its own PR if ever revisited.
- **Cross-OS-user session isolation / ownership tokens** — sessions are per-OS-user (`0700` home); management commands are trusted within that user. Multi-tenant shared-`$HOME` hardening is a separate concern.
- **A session daemon/server** — the filesystem-session model is the stateless-CLI equivalent.

---

## System-Wide Impact

- **CLI contract:** new `session` subcommand; new sticky behavior *after* `session start`. Bare-command and bare-`--session` behavior for non-adopters is unchanged (KTD1a). `--trace`/`--trace-strict` preserved.
- **Snapshot resolution:** activation relocates the snapshot namespace, not just trace (R11/KTD1b) — documented + tested; explicit `--snapshot` resolves cross-session.
- **FFI:** structured tracing activates only for `trace: on` sessions (no surprise data-at-rest); no ABI change.
- **Batch:** the resolved session id flows through `for_batch_item`; a per-item session override re-derives its own sink.
- **Two-PR seam:** U1–U4 (manifest-gated session trace + FFI) is the shippable core that kills the footgun; U5–U7 (lifecycle + gc + docs) can land as a second PR. `ce-work` may split there; the plan stays one document.

---

## Risks & Dependencies

- **R: activation silently relocates snapshot state** across a `session start` boundary. Mitigated: named as intentional (KTD1b/R11), documented, and covered by the U3 coupling test; explicit `--snapshot` still resolves.
- **R: existing `--session` / FFI callers get surprise trace files.** Mitigated by KTD1a — trace-on requires a manifest, which only `session start` writes.
- **R: FFI segment fragmentation** (one segment per call in a long process). Mitigated by the per-process `OnceLock` filename memoization (U2).
- **R: silent trace hole from a missing `trace/` dir.** Mitigated: `session start` pre-creates it and U2's segment branch recursively creates it on first open; regression test.
- **R: `current_session` pointer clobbering** cross-contaminates two agents. Mitigated by KTD3b (`session start` refuses a live pointer without `--force`) + env precedence (KTD3a) + the resolver test.
- **R: `gc` reaps a live env-bound session.** Mitigated by R10 liveness (active lock pid / recent `trace/` mtime), not creation-age alone.
- **R: opt-in-per-run does not eliminate the forget case.** Acknowledged (KTD3); `status` surfaces tracing state; ambient-default deferred as electable.
- **R: crash/NFS truncates the final segment line.** Mitigated: one `write_all` per event + a stated reader skip-and-warn tolerance (KTD2).
- **R: files near the 400-LOC limit** (`context.rs` ~368, `trace.rs` ~322, `refs_store_tests.rs` ~369). Mitigated: extract inline tests to sibling files before adding production code (U2/U3).
- **Sequencing note:** this is core/CLI/FFI plumbing that does not block or compete with the Phase 2 Windows/Linux adapter work (different surface, parallelizable).
- **Dependency:** none external; reuses `RefStore`, `RefStoreLock` (liveness), `TraceConfig`, `write_private_file`, `validate_session_id`, `refs_store_prune`.

---

## Definition of Done

- A `trace: on` session ⇒ trace records automatically to per-process segments (memoized filename, `seq`, lazy open, one `write_all`); `--trace` overrides to a single atomic file; a bare `--session`, `--no-trace` session, or no session ⇒ no trace.
- Activation resolves flag > env > pointer > none; pointer written only by `session start`; `session start` refuses a live-pointer clobber without `--force`.
- Activating a session relocates snapshot namespace too (documented + tested); explicit `--snapshot` resolves cross-session.
- FFI under a `trace: on` session produces one segment per process; a plain session writes nothing; drift gates green.
- `session start/end/list/gc` work; `gc` never reaps a live or pointer-referenced session and is symlink-safe; `list` is manifest-only.
- A session past the 512-snapshot cap retains all trace segments.
- Contracts documented (shared vs independent sessions; snapshot coupling; opt-in tracing).
- Gates: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --lib --workspace`, `cargo test -p agent-desktop`, `cargo test -p agent-desktop-ffi --tests`, `cargo tree -p agent-desktop-core` (no platform crates), and E2E green.

---

## Sources & Research

- Investigation (this session): session/snapshot store map, trace subsystem map, over-engineering/debt audit, external-pattern research, and a six-persona doc review — evidence for every problem and decision.
- Current code: `crates/core/src/{refs_store.rs, refs_store_prune.rs, refs_lock.rs, trace.rs, context.rs, snapshot.rs, commands/helpers.rs, commands/batch.rs}`, `crates/ffi/src/adapter.rs`, `src/cli/mod.rs`, `src/main.rs`.
- External patterns (load-bearing): Playwright `context.tracing.start/stop`; Appium/WebDriver session lifecycle; CDP `Tracing.start/end` + Target/session; kubectl/`docker context`/ssh-agent set-once active-context. These shaped KTD1/KTD1a (session-bounded, manifest-gated trace), KTD2 (segments), and the activation model (R4).
- Debt note: a `ponytail:`/TODO/FIXME harvest found **zero** in-source markers — deferrals are undocumented, not absent.
