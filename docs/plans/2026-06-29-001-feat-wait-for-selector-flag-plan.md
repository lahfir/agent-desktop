---
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
execution: code
type: feat
product_contract_source: ce-plan-bootstrap
origin: https://github.com/lahfir/agent-desktop/issues/84
created: 2026-06-29
---

# feat: add `--wait-for` selector polling flags

## Summary

Add global `--wait-for` / `--wait-for-gone` (with `-w` short for `--wait-for`) and `--wait-timeout` flags that poll the accessibility tree until an element matching a compact selector appears (or disappears), then return the resulting snapshot. This collapses the agent-side "snapshot → check → sleep → retry" loop into a single invocation, and lets action commands confirm a post-action UI change in one call (`click @e5 -w ":Saved!"`).

The feature is **pure CLI orchestration over existing engines** — it reuses `find`'s matcher and `wait`'s poll loop. No new platform/AX code, no CSS parser, no new adapter methods.

---

## Problem Frame

Today an agent that needs to wait for a UI state change (modal appears, spinner disappears, button becomes available) must hand-roll a polling loop: `snapshot`, inspect JSON, `sleep`, retry. That burns tokens, adds latency, and is re-implemented in every agent workflow.

The **engine** to avoid this already shipped in 0.4.4 as the `wait` command (`wait --text/--window/--element … --timeout`), which polls with 200ms→1s backoff and returns a `kind: "wait_timeout"` error envelope on expiry. What's missing is **ergonomics**: a single-string selector and a flag that fuses the wait onto the command that produces the snapshot, so `click; wait; snapshot` becomes `click … -w "…"`. Issue #84 asks for exactly this, and explicitly frames it as small CLI-only work.

The issue's `button:contains('Submit')` spelling is **illustrative, not a contract** — its own "Key design points" say "Reuses existing selector/filter infrastructure," which is `find`'s `--role/--name/--text/--value` + `node_matches`. We honor the latter.

---

## Requirements

- **R1.** A global `--wait-for <SELECTOR>` flag (short `-w`) blocks until an element matching `<SELECTOR>` is present in the polled tree, then returns the snapshot.
- **R2.** A global `--wait-for-gone <SELECTOR>` flag blocks until **no** element matches `<SELECTOR>`, then returns the snapshot (covers "spinner disappears").
- **R3.** A global `--wait-timeout <MS>` flag bounds the poll (default 30000ms, matching `wait`). On expiry the command exits 1 with a `kind: "wait_timeout"`, `predicate: "selector"` error envelope, consistent with `wait`.
- **R4.** `<SELECTOR>` is a compact `role:text` string parsed into a `FindQuery` and matched by the **same** `node_matches` used by `find`. Forms: `"button:Submit"` (role+text), `"button"` (role only), `":Saved!"` (text only).
- **R5.** `snapshot --wait-for …` polls the snapshot's own scope (`--app`/`--window-id`/`--surface`/depth) and returns the matched snapshot.
- **R6.** Ref-resolving action commands (`click`, `double-click`, `triple-click`, `right-click`, `clear`, `focus`, `toggle`, `check`, `uncheck`, `expand`, `collapse`, `scroll-to`, `type`, `set-value`, `select`, `scroll`) accept `--wait-for`/`--wait-for-gone`: perform the action, then poll **the app the acted-on ref belongs to** (`entry.source_app`), returning the post-action snapshot with the original action result preserved.
- **R7.** `--wait-for` and `--wait-for-gone` are mutually exclusive (clap `conflicts_with`, exit 2); `--root` and `--wait-for`/`--wait-for-gone` are mutually exclusive on `snapshot` (ref-rooted drill-down has no poll semantics).
- **R8.** Passing `--wait-for`/`--wait-for-gone` to a command that does **not** support it (anything other than `snapshot` and the R6 action commands) is an `INVALID_ARGS` error, not a silent no-op. The supporting set is a single `&[&str]` checked at dispatch time.
- **R9.** Batch mode does not honor an outer `--wait-for`; batch items run independently and never inherit the CLI-level selector.
- **R10.** No new platform-adapter methods; `agent-desktop-core` remains free of platform crates (CI `cargo tree` gate stays green).

**Success criteria:** an agent replaces a multi-call poll loop with one `snapshot -w "<selector>"`; a `click @e5 -w ":Saved!"` returns only after "Saved!" is visible; a timeout returns exit 1 with a discriminable envelope.

---

## Key Technical Decisions

**KTD1 — Compact `role:text` selector, not a DSL.** Parse on the first `:` into role (left) and text (right); each side is **trimmed and an empty side becomes `None`** (unconstrained on that axis) — never `Some("")`. This matters: `search_text::contains` runs `haystack.as_bytes().windows(needle.len())`, which **panics on a zero-length needle**, so `"button:"`, `":"`, and `" : "` must collapse the empty side to `None` rather than threading `Some("")` into `node_matches`. The result is a `FindQuery` fed to `node_matches`. This keeps the single-string ergonomics the issue shows while adding ~5 lines of parsing instead of a CSS grammar that would become a permanent public contract. Role is normalized via `roles::normalize_role_query`; text via `search_text::normalize` (the exact path `find` uses), so selector semantics equal `find` semantics by construction. `node_matches`'s `text` axis already searches name/value/description (`search_text::node_contains`), so `window:Settings` matches a node whose role is `window` and whose text contains "Settings"; matching by *window title* specifically is out of scope (use `wait --window` for that).

**KTD2 — One matcher, promoted out of `find`.** `FindQuery` and `node_matches` are currently private in `commands/find.rs`. Promote them (plus the `role:text` parser) into a new `commands/query.rs`; `find` imports them. This is the anti-redundancy move: the wait path does not get a parallel matcher — both call sites share one. Behavior of `find` is unchanged (pure move + re-export).

**KTD3 — One poll helper, reusing the `wait` loop shape.** New `commands/wait_selector.rs` mirrors `wait_for_text`: `snapshot::build` (cheap, no persist) in a 200ms→1s backoff loop, run `node_matches` over the tree, persist the refmap and return the snapshot envelope only on match. Presence vs. absence is one boolean (`gone`). **Retryable poll set is wider than `wait_for_text`'s** — `Timeout`, `ElementNotFound`, **`AppNotFound`, and `WindowNotFound`** are all swallowed and surfaced as `last_error` on timeout. This is the correctness fix for the common post-action case where the action closes/navigates the target (`click @submit -w ":Saved!"` dismisses the dialog): without `AppNotFound`/`WindowNotFound` in the retryable set, the poll would propagate a raw `WINDOW_NOT_FOUND` instead of the promised `wait_timeout` envelope. The empty/match-everything selector guard lives **here** (before the loop) so both call sites (snapshot self-apply and post-action) reject it identically.

**KTD4 — Global flags carried in `CommandContext`, applied at two scoped call sites, both keyed to the *correct* app.** The flags are defined once as clap globals (like `--headed`) and threaded into `CommandContext` (new `WaitSelector` config, set via a `with_wait_selector(...)` builder mirroring `with_headed`). Two thin application points, both delegating to the single `wait_selector` helper:
  - `snapshot::execute` self-applies when a selector is set, scoped to its own `--app`/opts (correct ordering: wait *is* the snapshot; no wasted eager build).
  - The **ref-action path in `helpers.rs`** applies it after the action, scoped to **`entry.source_app`** — the app the resolved ref belongs to — embedding the action result under `data.after_action`.

  **Why not a binary post-dispatch hook (rejected).** A hook in `run_with_adapter` only sees the action's JSON result, not the ref's pid/app. In the default **headless** mode AXPress does not foreground the target app, so the frontmost app is usually the *terminal*, not the acted-on app — a focused-app poll would read the wrong tree and time out on the default code path. The ref-action path already holds the `RefEntry` (`source_app`), so scoping there is both correct and the natural place that covers the ref-resolving commands R6 enumerates. This is layering, not duplication — the loop, matcher, and persistence live solely in the helper.

  **Wiring reaches all 16 R6 commands via the two `helpers.rs` entrypoints** (verified against the codebase): 11 commands (`click`, `double-click`, `triple-click`, `clear`, `focus`, `toggle`, `check`, `uncheck`, `expand`, `collapse`, `scroll-to`) go through `execute_ref_action_with_context`; 5 (`type`, `set-value`, `select`, `scroll`, **`right-click`**) go through `execute_ref_action_result_with_context` and then serialize themselves. `right-click` is the only one that builds a custom JSON body (its menu probe), so its wait must run **after** the probe (action → menu probe → wait). `execute_by_ref` (the FFI bridge) also routes through `execute_ref_action_with_context` but is unaffected: the FFI `CommandContext` never carries a `WaitSelector`, so `apply_post_action_wait` early-returns.

**KTD5 — Timeout returns the `wait` error envelope **plus** the last snapshot's id.** The issue says "return the last snapshot on timeout," but the established `wait` convention is a `TIMEOUT` error (`kind: "wait_timeout"`) carrying a compact `last_error`, and `finish()` maps `Err` → exit 1. We keep the error-envelope convention (consistency, no bespoke success-on-timeout path) **but honor the issue's debuggability intent**: on timeout, persist the last successfully-built tree and include its `snapshot_id` in `details` alongside `last_error`, so the agent can inspect the final tree state without racing a fresh `snapshot`. For post-action timeouts the original action result is also embedded under `details.after_action` so the caller learns the action ran.

**KTD6 — CLI-only; FFI parity deferred.** The flags live in the binary/CLI + core helper. The FFI cdylib does not gain a `--wait-for` surface in this PR (the issue scopes this as "purely a CLI orchestration feature"). The `wait_selector` core helper is reusable by a future `ad_wait_for` entrypoint; noted under Deferred.

---

## High-Level Technical Design

```
                    --wait-for / --wait-for-gone / --wait-timeout  (clap globals)
                                         |
                                CommandContext (WaitSelector)
                                         |
                 +-----------------------+------------------------+
                 |                                                |
        snapshot::execute                          helpers.rs ref-action path
        (selector set?)                            (after action, selector set?)
                 |                                                |
                 | scope = --app/opts                            | action runs first -> ActionResult
                 |                                                | scope = entry.source_app  (correct in headless)
                 +----------------------+   +---------------------+
                                        |   |
                                  commands/wait_selector::execute
                                        |
                  loop: snapshot::build -> node_matches (present|absent)
                        match -> persist refmap, return snapshot envelope
                        timeout -> wait_timeout::selector(...) (exit 1)
                                        |
                                 commands/query.rs
                          FindQuery + node_matches + parse_selector
                                   (shared with find.rs)
```

---

## Implementation Units

### U1. Extract shared selector matcher into `query.rs`

**Goal:** Single matcher shared by `find` and the wait path; add the `role:text` parser.
**Requirements:** R4, KTD1, KTD2.
**Dependencies:** none.
**Files:**
- `crates/core/src/commands/query.rs` (new) — move `FindQuery`, `node_matches`, `count_matches`'s predicate use; add `pub fn parse_selector(&str) -> FindQuery`.
- `crates/core/src/commands/find.rs` — delete the moved items, import from `query`.
- `crates/core/src/commands/mod.rs` — register `query`.
- `crates/core/src/commands/query_tests.rs` (new).
**Approach:** Pure refactor + addition. `FindQuery::from_args` stays in `find` (it depends on `FindArgs`); the reusable pieces are `FindQuery` the struct, `node_matches`, and the new `parse_selector`. `parse_selector` splits once on `:`; **trims each side; an empty-after-trim side becomes `None`, never `Some("")`** (a `Some("")` text would reach `search_text::contains` and panic on `windows(0)` — see KTD1); role through `roles::normalize_role_query`, text through `search_text::normalize`. Also add `pub fn tree_has_match(&AccessibilityNode, &FindQuery) -> bool` here (short-circuiting presence check) so `wait_selector` shares the exact predicate. `find`'s public behavior must not change.
**Patterns to follow:** existing `FindQuery`/`node_matches` in `find.rs`; `roles::normalize_role_query` and `search_text::normalize/contains/node_contains`.
**Test scenarios:**
- `parse_selector("button:Submit")` → role normalized to `button`, text normalized to `submit`.
- `parse_selector("button")` → role set, text `None`.
- `parse_selector(":Saved!")` → role `None`, text set.
- `parse_selector("")`, `parse_selector(":")`, `parse_selector(" : ")` → **both `None`** (match-everything; rejected by `wait_selector`, see U2).
- `parse_selector("button:")` → role `button`, text **`None`** (not `Some("")`).
- `parse_selector(" button : Submit ")` → trimmed to role `button`, text `submit`.
- `parse_selector("textfield:a:b")` → splits on first colon only; text is `a:b`.
- A `FindQuery { text: None }` never causes `search_text::contains` to be called with an empty needle (guards the `windows(0)` panic).
- `node_matches`/`tree_has_match` over a fixture tree returns identical results before/after the move (regression).
**Verification:** `find` tests pass unchanged; new parser tests pass; `cargo tree -p agent-desktop-core` shows no platform crates.

### U2. `wait_selector` poll helper

**Goal:** The reusable poll-until-present/absent engine returning a snapshot envelope.
**Requirements:** R1, R2, R3, R5, KTD3, KTD5.
**Dependencies:** U1.
**Files:**
- `crates/core/src/commands/wait_selector.rs` (new).
- `crates/core/src/commands/wait_timeout.rs` — add `pub(crate) fn selector(selector: &str, gone: bool, timeout_ms, last_error, last_snapshot_id: Option<String>) -> Result<Value, AppError>` (`predicate: "selector"`, `kind: "wait_timeout"`, includes `snapshot_id` in details when a tree was built).
- `crates/core/src/commands/mod.rs` — register `wait_selector`.
- `crates/core/src/commands/wait_selector_tests.rs` (new).
**Approach:** `execute(WaitSelectorInput { query, gone, app, opts, timeout_ms }, adapter, context)`. **First, reject a match-everything query** (both axes `None`) with `INVALID_ARGS` — this single guard covers both call sites (snapshot self-apply and post-action). Loop mirrors `wait_for_text`: `snapshot::build(adapter, &opts, app, None)`; `present = query::tree_has_match(&tree, &query)`; `matched = if gone { !present } else { present }`. On match: `RefStore::for_session(...).save_new_snapshot(&result.refmap)`, return a JSON object with **the same field shape `snapshot::execute` emits** (`app`, `window`, `ref_count`, `snapshot_id`, `tree`) plus `elapsed_ms` and `matched_selector`. `snapshot::format_result` is private — either promote it to `pub(crate)` and reuse it, or replicate the shape (the field set is the contract). Swallow the retryable set (`Timeout`, `ElementNotFound`, `AppNotFound`, `WindowNotFound`) into `last_error`, **and on each successful build remember the built tree's persisted id as `last_snapshot_id`**. On deadline call `wait_timeout::selector(... , last_error, last_snapshot_id)`. Reuse the 200ms→1s doubling backoff verbatim.
**Patterns to follow:** `wait::wait_for_text` (loop, backoff, retryable-error handling, persist-on-match, `last_error`); `snapshot::format_result` (envelope shape — promote to `pub(crate)` if reused).
**Test scenarios** (MockAdapter; deterministic trees):
- Match-everything query (`FindQuery` both `None`) → `Err` `INVALID_ARGS` before any poll.
- Element present on first poll → returns snapshot envelope with `matched_selector`, `elapsed_ms`, a persisted `snapshot_id`.
- Element absent then present on Nth poll → returns once present (mock whose tree changes across calls).
- `gone=true`, element present then absent → returns when absent.
- `gone=true`, element never present → returns immediately (already gone).
- Never matches within timeout → `Err` `code == TIMEOUT`, details `kind=="wait_timeout"`, `predicate=="selector"`, `last_error` populated, **`snapshot_id` of the last built tree present in details**.
- Each retryable error (`ElementNotFound`, `AppNotFound`, `WindowNotFound`) mid-poll is swallowed, not surfaced, until timeout — covers the "action closed the window" case.
- Persisted snapshot is loadable by the returned `snapshot_id` (refs usable by a follow-up action).
**Verification:** all unit tests pass; timeout envelope is JSON-discriminable from a `chain_deadline` timeout.

### U3. Global flags + `CommandContext` wiring + snapshot self-apply

**Goal:** Define the flags once, carry them, reject them on unsupported commands, and make `snapshot` honor them in its own scope.
**Requirements:** R1, R2, R3, R5, R7, R8, R9, KTD4.
**Dependencies:** U2.
**Files:**
- `src/cli/mod.rs` — add three `global = true` args: `--wait-for`/`-w` (`Option<String>`, `conflicts_with = "wait_for_gone"`), `--wait-for-gone` (`Option<String>`), `--wait-timeout` (`u64`, default 30000). The `--wait-for`/`--wait-for-gone` pair is mutually exclusive via clap `conflicts_with` (exit 2).
- `crates/core/src/context.rs` — add `WaitSelector { query_raw: String, gone: bool, timeout_ms: u64 }` carried in `CommandContext`; `with_wait_selector(Option<WaitSelector>)` builder mirroring `with_headed`; accessor. **Update `for_batch_item` to set `wait_selector: None`** — the struct literal is exhaustive, so this is a required compile fix and the correct semantics (R9: batch items never inherit the outer selector).
- `src/main.rs` — build `WaitSelector` from the parsed globals and attach via `with_wait_selector`. **Before dispatch, when a selector is set, reject it with `INVALID_ARGS` unless the command is `snapshot` or one of the R6 action commands** (R8) — a single `const WAIT_SUPPORTED: &[&str]` checked against `cmd.name()`. This closes the `find -w …` silent-no-op footgun.
- `src/cli_args/mod.rs` (`SnapshotArgs`) / `src/cli/mod.rs` — make `--root` and `--wait-for`/`--wait-for-gone` mutually exclusive (R7); since the wait flags are global, enforce this in `snapshot::execute` (or `main`) as an `INVALID_ARGS` check rather than clap (a global cannot `conflicts_with` a local arg cleanly).
- `crates/core/src/commands/snapshot.rs` — when `context` carries a wait selector **and `root_ref` is `None`**, delegate to `wait_selector::execute` with snapshot's `opts` + `app`; `parse_selector` the raw string. If `root_ref` **and** a selector are both set → `INVALID_ARGS`.
- `crates/core/src/commands/snapshot_tests.rs` — selector-path tests.
**Approach:** Flags are global so they attach to every subcommand with one definition (no per-command duplication; avoids a clap conflict between a global `-w` and a local one). `snapshot::execute` checks the selector branch **after** the existing `root_ref` early-return guard so the two never silently co-apply; the explicit `INVALID_ARGS` makes the combination loud. `--wait-timeout` default 30000 matches `wait`.
**Patterns to follow:** existing globals (`--headed`, `--session`) in `src/cli/mod.rs`; `CommandContext::with_headed` and `for_batch_item` in `crates/core/src/context.rs`; `cmd.name()` dispatch in `src/main.rs`.
**Test scenarios:**
- `snapshot -w "button:Submit"` on a tree containing it → returns matched snapshot (MockAdapter).
- `snapshot --wait-for-gone "progressindicator"` when absent → returns immediately.
- `--wait-for` + `--wait-for-gone` together → clap parse error, exit 2 (`conflicts_with`).
- `find -w "button:Submit"` → `INVALID_ARGS` (unsupported command, R8), not a silent find result.
- `snapshot --root @e5 -w ":button"` → `INVALID_ARGS` (R7), exercised in `snapshot_tests`.
- `snapshot --app Foo -w "…"` polls scope `Foo` (assert app passed through).
- Batch item does not inherit an outer `-w` (assert `for_batch_item` context has `wait_selector: None`).
- snapshot with no wait flags → identical output to today (regression).
- clap: `-w` resolves to `--wait-for`; `--wait-timeout 5000` parses.
**Verification:** `cargo test -p agent-desktop` (binary contract tests) + core snapshot tests pass; `--help` shows the three flags as global.

### U4. Post-action wait in the ref-action path (`helpers.rs`)

**Goal:** Ref-resolving action commands perform their action, then wait — scoped to the acted-on ref's app — returning the post-action snapshot with the action result preserved.
**Requirements:** R6, KTD4, KTD5.
**Dependencies:** U2, U3.
**Files:**
- `crates/core/src/commands/helpers.rs` — add `pub(crate) fn apply_post_action_wait(result: Value, app: Option<&str>, context: &CommandContext) -> Result<Value, AppError>`: returns `result` unchanged when context carries no selector; otherwise runs `wait_selector::execute` scoped to `app` (which owns the match-everything guard, U2) and returns the snapshot envelope with `result` embedded under `after_action`. On timeout it surfaces the `wait_timeout` error with the action result in `details.after_action`.
- `crates/core/src/commands/helpers.rs` — in `execute_ref_action_with_context`, stop discarding `_entry`: keep it and pass `entry.source_app.as_deref()` to `apply_post_action_wait` before returning. **This covers the 11 simple ref actions** (`click`, `double-click`, `triple-click`, `clear`, `focus`, `toggle`, `check`, `uncheck`, `expand`, `collapse`, `scroll-to`) in one edit.
- `crates/core/src/commands/{type_text,set_value,select,scroll}.rs` — each receives `(entry, result)` from `execute_ref_action_result_with_context` and ends with `Ok(serde_json::to_value(result)?)` (they do **not** build custom JSON). Wrap that return: `helpers::apply_post_action_wait(serde_json::to_value(result)?, entry.source_app.as_deref(), context)` (4 sites).
- `crates/core/src/commands/right_click.rs` — the **one** command that *does* build a custom JSON body (its menu probe). Call `apply_post_action_wait` **after** the menu-probe JSON is assembled, so ordering is action → menu probe → wait (5th bespoke site).
- `crates/core/src/commands/helpers_ref_action_tests.rs` (**modify — file already exists**) — add post-action merge, scope, timeout, and right-click-ordering tests.
**Approach:** One shared wrapper, **six call sites** (1 helper edit covering 11 commands + 5 bespoke: type/set-value/select/scroll/right-click) — every site already holds the `RefEntry`, so scoping by `entry.source_app` is a field read, not a pid→name lookup. Total coverage = the 16 R6 commands. `execute_by_ref` (FFI bridge) also routes through `execute_ref_action_with_context` but is behaviorally unaffected: the FFI `CommandContext` never carries a `WaitSelector`, so `apply_post_action_wait` early-returns. Snapshot is untouched here (it self-applied in U3), so there is no double-wait. Scoping by `entry.source_app` is the correctness fix over a focused-app hook: in headless mode the acted-on app is generally not frontmost.
**Patterns to follow:** `execute_ref_action_with_context`/`execute_ref_action_result_with_context` in `helpers.rs`; the custom-response assembly in `right_click.rs`; envelope shape in `commands/snapshot::format_result`.
**Test scenarios** (MockAdapter):
- `click @e5 -w ":Saved!"` where the mock tree gains "Saved!" after the action → envelope contains the snapshot **and** `after_action` with the click result.
- Post-action wait is scoped to `entry.source_app`, not the focused app (assert the app handed to `wait_selector` equals the ref's source app even when a different app is "focused" in the mock).
- Post-action selector never matches → `Err` `code==TIMEOUT`, `details.kind=="wait_timeout"`, `details.after_action` carries the action result, `details.snapshot_id` present.
- Action that closes the source app/window mid-wait → times out with the `wait_timeout` envelope (AppNotFound/WindowNotFound swallowed, U2), not a raw `WINDOW_NOT_FOUND`.
- An action command with no wait flag → result returned byte-for-byte unchanged (regression).
- `right-click @e -w ":menuitem"` → wait runs after the menu probe; `after_action` carries the right-click body including its `menu` field.
- A simple ref action (`toggle`), a bespoke serialize-only action (`type`), and `right-click` all merge correctly (covers all three wiring paths).
**Verification:** `cargo test --lib -p agent-desktop-core` ref-action tests pass; `cargo tree -p agent-desktop-core` clean; real smoke deferred to E2E (U5).

### U5. Docs, help text, and E2E coverage

**Goal:** Document the flag and prove it against a real app.
**Requirements:** R1–R6 (user-facing surface).
**Dependencies:** U3, U4.
**Files:**
- `skills/agent-desktop/references/commands-interaction.md` and the observation reference — document `--wait-for`/`--wait-for-gone`/`--wait-timeout`, the `role:text` selector, that post-action waits poll the **acted-on ref's app** (`entry.source_app`), the timeout envelope (with `snapshot_id` of the last tree), the unsupported-command `INVALID_ARGS` (R8), the `--root` exclusion (R7), and that batch does not honor an outer `-w` (R9). Note the canonical pattern for unsupported commands like `find`: `snapshot -w "<sel>"` then `find`.
- `src/cli/help_after.txt` — add a `--wait-for` usage example if examples live there.
- `tests/e2e/run.sh` + fixture interactions — one appearance wait and one disappearance wait against the SwiftUI fixture, verified by observation (a control that appears/disappears on action).
**Approach:** Documentation-only + E2E. Keep help text terse; the skills reference carries the detail. E2E asserts by independent observation per the harness contract, not the command's own `ok`.
**Patterns to follow:** existing `--force` documentation added in 0.4.4; `tests/e2e/README.md` conventions.
**Test scenarios:** `Test expectation: none` for docs. E2E: (a) trigger a control that reveals a "Saved!" label, assert `-w ":Saved!"` returns only after it appears; (b) trigger a spinner that clears, assert `--wait-for-gone` returns after it's gone; (c) timeout case returns exit 1 with the `wait_timeout` envelope.
**Verification:** `bash tests/e2e/run.sh` green (headless + `--headed`); skills reference renders.

---

## Scope Boundaries

**In scope:** the three global flags, the `role:text` selector, the shared matcher extraction, the poll helper (incl. window-close-tolerant retryable set and timeout `snapshot_id`), snapshot self-apply, post-action wait for all 16 R6 commands, the unsupported-command `INVALID_ARGS` guard (R8), the `--root` exclusion (R7), batch isolation (R9), docs + E2E.

### Deferred to Follow-Up Work
- **FFI `ad_wait_for` entrypoint** — the core `wait_selector` helper is reusable; a cdylib surface is a separate, additive PR (KTD6).
- **Richer selector axes** (`name:`, `value:`, `state:` predicates, multiple constraints in one string) — only `role:text` ships now; the matcher already supports name/value, so a future grammar can expand without re-architecting.
- **`--wait-for` on non-ref commands** (e.g. `launch Foo -w "button:Login"`, `find`) — rejected with `INVALID_ARGS` this PR (R8); the workaround is two calls (`launch Foo; snapshot --app Foo -w "button:Login"`). Extending support to `find`/`launch` is additive follow-up.
- **`--wait-for` in batch mode** — batch items run independently and ignore an outer `-w` (R9). Per-item wait selectors in the batch JSON schema are deferred.
- **Polling-interval flag** — backoff is fixed (200ms→1s) to match `wait`; expose only if measured need arises.
- **`--wait-for-gone` stabilization guard** — see Risks; an optional "require one present observation before accepting absent" guard is deferred unless flakiness is observed.

### Out of scope (not this product)
- CSS / `:contains()` selector grammar (KTD1).
- A success-on-timeout return path that diverges from the `wait` envelope (KTD5).

---

## Risks & Dependencies

- **R: post-action wait reads the wrong app's tree (headless default).** Resolved by design: scope is `entry.source_app` (the resolved ref's app), not the focused app — a binary post-dispatch hook was rejected for exactly this reason (KTD4/U4). Test asserts the ref's app is used even when a different app is focused.
- **R: post-action wait errors instead of timing out when the action closes the window.** Resolved: `AppNotFound`/`WindowNotFound` are in the retryable poll set (KTD3/U2), so a window-closing `click @submit -w ":Saved!"` times out with the `wait_timeout` envelope (action result preserved) rather than a raw `WINDOW_NOT_FOUND`.
- **R: empty-needle panic in the matcher.** Resolved: `parse_selector` collapses an empty axis to `None` (KTD1/U1) so `search_text::contains` is never called with a zero-length needle (`windows(0)` panic); U1 tests cover `"button:"`, `":"`, `" : "`.
- **R: `--wait-for-gone` false-positive on a transient empty/partial tree** (first poll fires before the action's AX notification propagates, reads no match, returns "gone"). Confidence moderate. Mitigation this PR: documented limitation + the standard 200ms first-interval gives the tree time to populate; a "require one present observation before accepting absent" guard is deferred (Scope Boundaries) pending observed flakiness.
- **R: `find` regression from the matcher move.** Mitigation: U1 is a pure move with `node_matches`/`tree_has_match` parity tests; `find`'s existing suite must pass unchanged before U2 starts.
- **R: 30s default `--wait-timeout` is long for a blocking post-action call** in an agent pipeline. Mitigation: matches `wait` for consistency; agents set `--wait-timeout` explicitly for tight loops. Documented in U5.
- **R: global `-w` short-flag collision.** Verified clear — only `-v` (global) and `-i` (snapshot) exist today.
- **Dependency:** none external; reuses `snapshot::build`, `RefStore`, `node_matches`/`tree_has_match`, `wait_timeout`, and `RefEntry.source_app`.

---

## Definition of Done

- All five units landed; `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --lib --workspace`, `cargo test -p agent-desktop`, and `cargo tree -p agent-desktop-core` (no platform crates) green.
- `snapshot -w "<role:text>"`, `--wait-for-gone`, and post-action `click @e -w "…"` work against the E2E fixture in headless and `--headed` mode.
- Timeout exits 1 with a `kind:"wait_timeout"`, `predicate:"selector"` envelope carrying the last tree's `snapshot_id` and `last_error`; post-action timeout preserves the action result in `details.after_action`.
- All 16 R6 commands honor `--wait-for` (verified: 11 via `execute_ref_action_with_context`, 5 bespoke incl. `right-click`); `find`/`launch`/other unsupported commands return `INVALID_ARGS` (R8); batch ignores an outer `-w` (R9).
- `--wait-for` + `--wait-for-gone` together → clap parse error (exit 2); `--root` + `--wait-for` → `INVALID_ARGS` (R7); a match-everything selector → `INVALID_ARGS`.
- Skills reference and `--help` document the flags; issue #84 referenced in the commit/PR.

---

## Sources & Research

- Issue #84 (origin) — proposed shape and "why small" framing.
- `crates/core/src/commands/wait.rs` (`wait_for_text`) — poll loop, backoff, persist-on-match, `last_error` pattern reused by `wait_selector`.
- `crates/core/src/commands/find.rs` (`FindQuery`, `node_matches`) — the matcher promoted to `query.rs`.
- `crates/core/src/commands/snapshot.rs` (`build`, `format_result`) — cheap build + envelope shape.
- `crates/core/src/commands/wait_timeout.rs` — `kind:"wait_timeout"` envelope convention (KTD5).
- `src/cli/mod.rs`, `src/main.rs` — global-flag mechanism (`--headed`) and the single post-dispatch seam (`run_with_adapter`/`finish`).
