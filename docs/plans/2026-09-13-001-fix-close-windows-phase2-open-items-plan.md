---
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
execution: code
product_contract_source: ce-plan-bootstrap
type: fix
created: 2026-09-13
target_branch: feat/windows-adapter
---

# fix: Close Phase 2's remaining Windows open items

## Summary

Six items stand between `feat/windows-adapter` and a Phase 2 with no deferrals. Two are unimplemented adapter methods, two are behavioural defects in the Windows adapter, one is a build-target gap, and one is a source-of-truth document that over-reports the work remaining. Each closes with the smallest change that makes it genuinely true.

The binding constraint is blast radius. Two of these items have an obvious fix that reaches into shared core and changes macOS — a platform that cannot be compiled or tested on the Windows host this branch is developed on. Both are deliberately solved inside `crates/windows` instead, and the rejected core-level alternative is recorded with the specific macOS behaviour it would have changed.

---

## Problem Frame

`docs/phases.md` is the product's source of truth and the next sub-phase's planner reads it as fact. Today it is wrong in both directions: it lists nine defects as open that are fixed in code, and it is silent about a real capability gap. Meanwhile the adapter carries two methods that silently withhold a feature, one chain that can deliver a write twice, and one role whose expand verification can fail an action that worked.

Nothing here is a new feature. Every item is a gap between what the adapter claims and what it does.

---

## Requirements

- **R1** — A Windows `expand` on an element whose expanded state could not be read reports a successful `delivered_unverified`, never `ACTION_FAILED`. This must hold for `menuitem`, which is not in core's expandable-role list.
- **R2** — The Windows value-write chain never advances to a second mechanism after a rung has already delivered unverified.
- **R3** — `click --debug --screenshot <path>.html` produces an artifact on Windows whose image is the resolved target window, and fails closed rather than emitting a frame for a window that moved.
- **R4** — `type` on Windows reports `verification_scope: "element_value"` when the field's selection is readable, and the offsets it derives are correct for text containing a surrogate pair.
- **R5** — The Linux build-target status of `crates/windows` is settled and recorded, with the reason, rather than left as a silent green.
- **R6** — `docs/phases.md` §2.15 and §2.16 describe what is actually open, citing the code that closed each finding, corrected in place with no changelog or annotation language.

---

## Key Technical Decisions

### KTD1 — Close the menuitem expand gap inside `crates/windows`, not in core's role list

*(session-settled: user-directed — chosen over adding `menuitem` to `crates/core/src/roles.rs::is_expandable_role`: that predicate has six callers, and `crates/macos/src/tree/action_list.rs:103` reads `has("AXPress") && is_expandable_role(role)`, so widening it would make every macOS menu item with `AXPress` advertise `Expand` and `Collapse` actions it does not have — a capability-surface change on a platform this host cannot compile.)*

The defect is a mismatch between two predicates, and the Windows one is the wrong one. `push_expand_collapse_state` (`crates/windows/src/tree/states.rs`) emits the `expanded` token for **any** role, while `states_are_complete` (`crates/windows/src/tree/live_read.rs`) decides completeness from the role list. Any element that advertises ExpandCollapse but is not in the role list therefore claims complete evidence it may not have.

The fix makes the completeness predicate match the token-push condition: the expand half is satisfied only when the element neither has an expandable role **nor** advertises ExpandCollapse, or when `ExpandCollapseState` was actually read. `TreeProperty::ExpandCollapseAvailable` is already in the batched live read, so this costs no extra UIA round trip.

Keeping the role-list term in the disjunction is deliberate: `crates/windows/src/actions/disclosure.rs` ships an Invoke fallback rung, so an expand can be delivered to an element with no ExpandCollapse pattern at all. Dropping the role term would let a combobox expanded via Invoke report a false `ACTION_FAILED` — trading one instance of the bug for another.

### KTD2 — Flip the value-write chain's fallthrough; do not delete the flag

*(session-settled: user-directed — chosen over deleting `continue_after_unverified_delivery` entirely: deletion touches `chain.rs`, four chain definitions and `chain_budget_tests.rs`, whose subject is budget expiry rather than fallthrough, for no behavioural gain beyond the flip.)*

`continue_after_unverified_delivery: true` is set in exactly one production place — `crates/windows/src/actions/value_write.rs:19` (`VALUE_WRITE_CHAIN`). `CLEAR_CHAIN`, `DISCLOSURE_CHAIN` and `TOGGLE_CHAIN` already carry `false`. Flipping that one value is the entire behavioural change and it matches what 0.9.0 did on macOS.

The flag stays because `record_step_outcome` is a general mechanism and `chain_tests.rs` legitimately exercises it. What changes is that no shipped chain sets it.

### KTD3 — Record the Linux build-target exclusion rather than writing stub modules

*(session-settled: user-directed — chosen over repairing 32 errors across ~20 files and adding a CI lane: that adds several hundred lines of `#[cfg(not(target_os = "windows"))]` stubs that no lane executes, which is the precise anti-pattern `docs/solutions/best-practices/never-ship-platform-code-that-ci-cannot-execute.md` records the repo paying for once.)*

`docs/phases.md` §2.15 already names this an allowed closure: "a `cargo check ... --target x86_64-unknown-linux-gnu` lane **(or a recorded exclusion with its reason)**". The exclusion is a closure, not a deferral.

It is also low-risk by construction. The binary's `Cargo.toml` declares the platform crates under `[target.'cfg(target_os = "...")'.dependencies]`, so a Linux build never pulls `agent-desktop-windows` in. The invariant that matters — `agent-desktop-core` staying platform-neutral — is separately gated and passes today.

The recorded reason must name what *is* gated, so a later reader does not mistake the exclusion for an oversight.

### KTD4 — Prove the UTF-16 conversion with a pure function, not a live control

*(Open area resolved: no surrogate-pair control is available on this rig, and one cannot be conjured reliably.)*

The UIA call and the offset arithmetic are separable. Extract the range-to-UTF-16 mapping into a pure function over the retrieved text and the range endpoints, and unit-test it directly with a surrogate pair (an emoji), a combining sequence, and a plain ASCII case. The UIA surface around it stays thin enough to read.

This is the only honest proof available here, and it is stronger than a live ASCII smoke — which cannot distinguish a correct conversion from a raw cast.

---

## Implementation Units

### U1. Make the Windows expand-completeness predicate match the token push

**Goal:** Close R1 — a delivered expand on a `menuitem` whose state was unreadable reports unverified, not failed.

**Requirements:** R1. Implements KTD1.

**Dependencies:** none.

**Files:**
- `crates/windows/src/tree/live_read.rs` — `states_are_complete`
- `crates/windows/src/tree/live_read_states_complete_tests.rs` — extend the existing truth table

**Approach:**
1. In `states_are_complete`, replace the expand half's role-only guard with a disjunction: the half applies when the role is expandable **or** `properties.is_true(TreeProperty::ExpandCollapseAvailable)`.
2. When it applies, it is satisfied only if `gated_number(TreeProperty::ExpandCollapseState).is_some()`.
3. Leave `push_expand_collapse_state` in `states.rs` untouched — the token push is already correct; it is the predicate that was narrow.
4. Do not touch `crates/core/src/roles.rs`.

**Patterns to follow:** the existing toggle half in the same function, and `expand_collapse_available` in `crates/windows/src/actions/disclosure.rs` for the `is_true` idiom.

**Test scenarios:**
- A `menuitem` advertising ExpandCollapse whose `ExpandCollapseState` read is `Known` → `states_complete` true.
- A `menuitem` advertising ExpandCollapse whose `ExpandCollapseState` read is `Unknown` → false.
- A `menuitem` advertising ExpandCollapse whose `ExpandCollapseState` read is `Absent` → false.
- A `menuitem` that does **not** advertise ExpandCollapse, with both reads absent → true (neither half applies).
- A `combobox` with no ExpandCollapse pattern (the Invoke-fallback shape) → still false, proving the role term was not dropped.
- Existing checkbox and button cases continue to pass unchanged.

**Verification:** the truth table fails if the expand half is reverted to a role-only guard, checked by breaking that line and watching the menuitem cases fail.

---

### U2. Stop the value-write chain advancing after unverified delivery

**Goal:** Close R2.

**Requirements:** R2. Implements KTD2.

**Dependencies:** none.

**Files:**
- `crates/windows/src/actions/value_write.rs` — `VALUE_WRITE_CHAIN`
- `crates/windows/src/actions/chain_tests.rs` — add the policy test

**Approach:**
1. Set `continue_after_unverified_delivery: false` on `VALUE_WRITE_CHAIN`.
2. Leave `record_step_outcome` and the `ChainDef` field as they are.
3. Update the doc comment on `chain.rs`'s module header only where it now describes behaviour no shipped chain has.

**Test scenarios:**
- `VALUE_WRITE_CHAIN` declares `continue_after_unverified_delivery: false`, pinned directly so a future flip is caught.
- A chain whose first rung returns `DeliveredUnverified` records that step and stops, with no second rung attempted.
- The generic mechanism test at `chain_tests.rs:212` still passes — it builds its own `ChainDef` and tests the mechanism, not the policy.

**Verification:** flipping the flag back to `true` fails the policy test.

---

### U3. Implement `screenshot_window_frame` on Windows

**Goal:** Close R3.

**Requirements:** R3.

**Dependencies:** none.

**Files:**
- `crates/windows/src/system/adapter.rs` — trait wiring only

**Approach:**

Research collapsed this unit. `crates/windows/src/system/screenshot.rs::capture_window` **already implements the entire contract** `screenshot_window_frame` requires: it refuses a window with no process-instance token, resolves through `resolve_window_strict`, captures, and then re-proves identity after the capture, discarding the bytes and returning `STALE_REF` with `not_delivered` if the window can no longer be proved. `screenshot_tests.rs::post_capture_window_identity_failure_discards_bytes` already pins the fail-closed half.

So the gap is not the capture — it is that nothing wires the trait method to it. `ScreenshotTarget::ExactWindow` already routes there for the ordinary `screenshot` command; `SystemOps::screenshot_window_frame` simply has no Windows override and falls through to core's `not_supported()` default.

1. Add the `screenshot_window_frame` override to the `SystemOps` impl in `crates/windows/src/system/adapter.rs`, delegating to `screenshot::capture_window(window, deadline)`, mirroring `crates/macos/src/system/adapter.rs:152`.
2. Add nothing else. No new capture path, no second bounds check — duplicating the identity re-proof would be the over-engineering this plan forbids.

**Patterns to follow:** the adjacent `screenshot` override in the same impl block.

**Test scenarios:**
- The adapter's `screenshot_window_frame` is wired through `SystemOps` and reaches the exact-window capture rather than core's `not_supported` default (assert on the error/behaviour distinguishing the two, in the style of `adapter_press_key_for_app_is_wired_through_system_ops`).
- A window with no process-instance token is refused by the existing guard rather than capturing the foreground.

**Verification:** deleting the override makes the wiring test fail with `ACTION_NOT_SUPPORTED`.

---

### U4. Implement `get_text_selection` on Windows

**Goal:** Close R4.

**Requirements:** R4. Implements KTD4.

**Dependencies:** none.

**Files:**
- `crates/windows/src/tree/text_selection.rs` — new module: the UIA Text pattern read plus the pure offset mapping
- `crates/windows/src/tree/mod.rs` — module registration
- `crates/windows/src/adapter.rs` — `ObservationOps::get_text_selection` wiring
- `crates/windows/src/tree/text_selection_tests.rs` — offset-mapping truth table

**Approach:**
1. This is new UIA surface — the crate uses no Text pattern today. Acquire the pattern, read the selection, and retrieve the document text.
2. Keep the pure part separate: a function that maps the selection's character offsets to **UTF-16 code-unit** offsets over the retrieved text. Core expects UTF-16 code units; UIA text ranges are not indexed that way, so this is a conversion, never a cast.
3. Return `Ok(None)` when the element exposes no Text pattern or no selection — core treats absent evidence as `verification_scope: "unavailable"`, which is the correct degraded answer, not a failure.
4. Keep the file under the 400-line cap; split the pattern read from the mapping if it approaches it.

**Execution note:** write the offset-mapping tests first — the mapping is the part that can be silently wrong, and a live ASCII check cannot tell a correct conversion from a raw cast.

**Test scenarios:**
- ASCII text, selection in the middle → offsets match the character indices.
- Text containing a surrogate pair (an emoji) **before** the selection → UTF-16 offsets exceed the character offsets by one per pair.
- Text containing a combining sequence before the selection → offsets follow UTF-16 units, not grapheme clusters.
- Selection at index 0 and at end-of-text → boundary offsets are correct, not off by one.
- An empty selection (a caret) → an empty range at the caret's offset, not `None`.
- An element with no Text pattern → `Ok(None)`.

**Verification:** replacing the conversion with a direct cast fails the surrogate-pair and combining-sequence cases.

---

### U5. Record the Linux build-target exclusion

**Goal:** Close R5.

**Requirements:** R5. Implements KTD3.

**Dependencies:** none.

**Files:**
- `docs/phases.md` — §2.15's Linux cross-compile bullet

**Approach:**
1. Rewrite the bullet to state the settled outcome: `crates/windows` is not built for non-Windows targets, and why that is safe — the binary declares platform crates under `[target.'cfg(target_os = "...")'.dependencies]`, so a Linux build never links it.
2. Name what *is* gated in its place: `agent-desktop-core` cross-compiles to `x86_64-unknown-linux-gnu` and `x86_64-pc-windows-msvc`, and `cargo tree -p agent-desktop-core` proves core carries no platform crate.
3. Cite the learning that argues against the alternative.
4. Correct in place — no "previously said", no note, no changelog line.

**Test expectation:** none — documentation only. The claims it makes are already gated by existing CI jobs.

**Verification:** the stated core cross-compile and isolation claims both pass when run.

---

### U6. Correct `docs/phases.md` §2.15 and §2.16 to what is actually open

**Goal:** Close R6.

**Requirements:** R6.

**Dependencies:** U1, U2, U3, U4, U5 — this unit describes their outcome, so it lands after them.

**Files:**
- `docs/phases.md` — §2.15, §2.16, and the §2.18 items this change closes

**Approach:**
1. Remove the nine inherited findings from §2.16 that are fixed, citing for each the code that closed it (named in the Problem Frame's source material — `clipboard_worker_state.rs`'s outstanding-worker guard, `locate_menu`'s Chromium arm, `first_native_hwnd`'s `Result<Option<isize>, BudgetExpired>`, the captured `EnumDisplayMonitors` return, the `RangeValueIsReadOnly` gate, `exhaustion_disposition(&steps)` on budget expiry, `resolved_from_scan`'s high byte, `list_surfaces_for_process`'s partial result, and `role_text::value_is_the_readable_text`).
2. Remove the tray bullet from §2.15 — `snapshot --surface system-tray` reads its toolbar's children.
3. Remove the e2e "legs that cannot fail" bullet — the named legs carry `Add-Fail` gating and a pixels-produced check.
4. Fix the stale sequencing claim: §2.15 and §2.16 both call §2.16 "the last sub-phase" although §2.17 and §2.18 follow it.
5. Close §2.18's two adapter-method items, which U3 and U4 implement, and the chain and menuitem items, which U2 and U1 implement.
6. Correct in place throughout. No annotation, no history.

**Test expectation:** none — documentation only.

**Verification:** no statement in §2.15, §2.16 or §2.18 describes as open anything this branch has closed; `scripts/check-no-phase-references.sh` still passes.

---

## Scope Boundaries

**In scope:** the six items above, and only the files each names.

**Not in scope:**
- The `latest_snapshot_id` multi-agent hazard (§2.17). It is a concurrency contract decision — per-agent pointer, refusal of bare refs once a session has two writers, or mandatory qualified refs with an error — and each option changes a shipped cross-platform contract. It is not a Windows adapter gap and does not belong in a Windows close-out.
- Any change to `crates/core/src/roles.rs`, `crates/macos/**`, or the `ChainDef` mechanism, per KTD1 and KTD2.
- The Phase 2 → `main` promotion itself, which `docs/phases.md` §2.16 defines as a separate release-noted `feat!` merge.

---

## Verification Contract

Run package-scoped — bare workspace cargo fails on this host.

- `cargo test -p agent-desktop-core` — no regression
- `cargo test -p agent-desktop-windows --lib` — no regression beyond the known live-desktop contention set (`notifications::`, `system::shell_surface::`, `input::clipboard`, `tree::fixture_clipboard`), which fails under parallel load and passes serially
- `cargo test -p agent-desktop` and `cargo test -p agent-desktop-ffi`
- `cargo clippy -p agent-desktop-core -p agent-desktop-windows -p agent-desktop -p agent-desktop-ffi --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `cargo check -p agent-desktop-core --all-targets --target x86_64-unknown-linux-gnu` and `--target x86_64-pc-windows-msvc`
- `cargo tree -p agent-desktop-core` — no platform crate
- `cbindgen crates/ffi --config crates/ffi/cbindgen.toml --verify`
- All five guards CI runs: `check-bash3-compat`, `check-no-phase-references`, `check-release-consistency`, `check-rust-file-size`, `check-stale-ref-constructor-misuse`

## Definition of Done

- R1–R6 each hold, and each has a test that fails if its requirement is violated, except R5 and R6 which are documentation whose claims are gated by existing jobs.
- Every new test is invert-verified: break the guarded line, watch that test fail, restore, re-run.
- No file exceeds 400 lines; no inline comments; no `unwrap()` outside tests.
- `crates/core` and `crates/macos` are unmodified.
- `docs/phases.md` describes only work that is genuinely open.

---

## Assumptions

- The UIA Text pattern is reachable from the crate's existing element wrapper without a new COM initialization path; if it is not, that surfaces at implementation and the pattern acquisition follows the crate's existing pattern-availability idiom.
- The known live-desktop test contention set is environmental, established by repeated runs where the failing set shifts and each member passes in isolation.

## Risks

- **U4 is the only unit on genuinely new UIA surface.** Its failure mode is a silently wrong offset, which is why the mapping is a pure function tested directly rather than proven by a live ASCII smoke.
- **U1 changes a predicate that gates verification verdicts.** The combobox-via-Invoke scenario is the case that would regress if the role term were dropped, and it has its own test for exactly that reason.
