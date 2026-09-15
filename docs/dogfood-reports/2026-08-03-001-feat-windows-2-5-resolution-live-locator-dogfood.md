# Dogfood report - Resolution and live locator (sub-phase 2.5)

**Date:** 2026-08-03 - **Branch:** `feat/windows-2.5-resolution` ** Plan:** `docs/plans/2026-08-02-001-feat-windows-resolution-live-locator-plan.md`

Resolution cannot be validated by a test that restates it. This is the run that
establishes whether the upgraded resolver - three-state matching, the graded
path-and-geometry fallback, the deadline retry loop and the five live readers -
actually works end to end on the release binary against software nobody in this
repository wrote, with the JSON read, never the suite's opinion of itself.

## Environment

| fact | value |
| --- | --- |
| OS | Windows Server 2019 Datacenter, build 17763 |
| UIA runtime | UIA3 COM (`CUIAutomation8`), `uiautomation` crate 0.25.0 |
| Client stack | `uia3-com` - the stack the adapter ships |
| Binary | `target/release/agent-desktop.exe` (1.95 MB release build) |
| Runner | `probes/windows/scratch/run-live-dogfood.ps1`, release binary driven directly, JSON read |
| Targets | classic Notepad, Explorer, WinForms fixture, WPF fixture, Obsidian (Chromium/Electron) |

## Targets

Every target shows **repo-controlled content**: Notepad and Explorer open a
scratch directory of synthetic file names; the fixtures are the repo's own;
Obsidian reports only counts, roles and re-resolution outcomes - never note
titles. Absent targets are skipped with a reason, never reported captured.

| target | UI stack | result | refs | complete | live reads | find | resolution judgement |
| --- | --- | --- | --- | --- | --- | --- | --- |
| classic Notepad on a scratch file | Win32 `EDIT` proxy | ran | 17 | true | get/is ok | button 1 | n/a |
| Explorer on a scratch dir | DirectUI shell | ran | 75 | true | get ok | n/a | n/a |
| WinForms scratch fixture | WinForms | ran | 27 | true | get/is ok | button 1 | **live value stable across content swap (non-list ref; see below)** |
| WPF scratch fixture | WPF | ran | 10 | true | snapshot only | n/a | n/a |
| Obsidian (Chromium/Electron) | Chromium + Electron | ran | 15 (shell) | true | n/a | n/a | **4/6 shell refs stale, 2/6 resolved** |

## The live loop works end to end on every reachable stack

The post-2.5 binary drives `snapshot`, `find`, `get` and `is` against real
programs. The five live readers answer (Notepad, WinForms: `get --property
bounds` and `is --property enabled/visible` return ok), `find` hydrates refs
(WinForms: a button locate round-trips to one match and its `ref_id` is a
usable ref). The WinForms fixture's ref re-resolves after a `WM_APP` content
swap with its live `value` (read off the resolved live element, not the
stored `RefEntry`) unchanged before and after - the stronger identity check
this run's judgement uses in place of a bare `get --property role` success,
since `role` is served from the stored entry and cannot by itself distinguish
a correct resolve from a silently-resolved neighbour. This result does **not**
exercise the A7-3 index-keyed wrong-target shape, and does not corroborate it:
A17-2 measured that this fixture's single plain `ListBox` (`lstItems`) - the
default provider's index-keyed list, extended for this arm precisely because
index keying is the shape A7-3 found - exposes zero `ListItem`s to a COM
client through any resolver search (raw-walker count 0,
`find_all` count 0 both by children and by descendants), so the rows the
`WM_APP` message swaps are never in the walked/reffed tree at all. The ref
sampled here is the snapshot's first ref - on this fixture that is a control
outside the list - so what this run shows is narrower: a non-list ref's live
identity survives an unrelated content-changing message to the same window.
The A7-3 wrong-target pin itself stays proven only by the synthetic unit
evidence U2 carries forward (A17-2's own recorded branch), not by this
fixture.

## Electron resolution judgement (the reason this sub-phase exists)

Obsidian on this box presents the **first-contact Chromium shell**: 15 refs,
all `group`/`document`/`region` wrappers carrying only `ScrollTo`, `complete:
true`. This is the A1-5/A16-11/A17-8 shape reproduced. Six shell refs were
sampled for re-resolution on a **fresh binary invocation** (a new client, so the
shell is served again rather than the settled tree): **4 returned `STALE_REF`
and 2 resolved**. That is the honest, judged rate for shell-held refs on this
box - and it is exactly why the plan's pre-committed branch (A17-8, U1 item 8)
records that the identifier-free fallback's aggregate real-world `STALE_REF`
rate **inside web content** stays unmeasurable on this host: the file tree
never reaches a fresh client within the settle window, so there are no
web-content refs to resolve. The path-and-geometry tier's correctness is proven
by the fixture-driven live tests (U3/U5); the WinForms ref's identity-stability
result above corroborates that a ref's live content survives an unrelated
window message, which is a narrower claim than a proof of wrong-target
resistance (see "The live loop works end to end" above and A17-2). The
web-content rate is owned by the 2.12 self-hosted-runner environment,
`closure: 2.12` per A17-8.

## Ambiguity observations against U1 item 7's census

U1's census found the native fixtures ambiguity-clean under the composed
matcher: zero zero-extent ref-able elements, zero duplicate positive-area
bounds, and only the deliberately-constructed duplicate pairs resolving 2 (the
A17-3 live N case). This dogfood run corroborates it on the real binary: across
all targets **no resolution returned `AMBIGUOUS_TARGET`** - every re-resolution
attempt either succeeded (WinForms, with a stable live-value identity check;
Notepad, Explorer, WPF sampled by `get`/`is` alone, no swap) or went
`STALE_REF` (the Obsidian shell's 4/6). There were no offscreen/virtualized degeneracy cases observed on the
native stacks, consistent with A17-7's "zero-extent is offscreen/virtualized-
only" finding. The WPF DataGrid's `cell` refinement question (A16-10, carried
from 2.4) still stands and is not re-litigated here.

## Residuals

| residual | owner | status |
| --- | --- | --- |
| Obsidian web-content STALE_REF rate inside the file tree is unmeasurable on this host (first-contact shell) | 2.12 self-hosted-runner environment (A17-8) | recorded, `closure: 2.12` |
| The WinForms fixture's single plain `ListBox` (`lstItems`, the default provider's index-keyed list) exposes zero `ListItem`s to a COM client (raw-walker and `find_all` both 0, by children and by descendants), so no owned fixture reproduces A7-3's index-keyed wrong-target shape and this run's swap judgement cannot exercise it - it checks a non-list ref's identity stability instead | A17-2, pin stays the synthetic unit evidence U2 carries (`a_matching_native_id_with_a_mismatched_role_does_not_resolve`) | recorded, not reproducible under any owned fixture |
| WPF live reads (`get`/`is`) not sampled in this run - the WPF leg records snapshot only | U7-next / dogfood extension | recorded |
| `find --count` vs materialized agreement not asserted on a real app (no CI assertion may name a real-app count) | per VC "Evidence honesty" | **closed on fixtures** - agreement is now pinned against the hosted fixture, where a count is repo-controlled rather than an `app/provider` fact |
| Traced correction to this sub-phase's own framing: `wait --selector` polls through `resolve_query` with materialization `None`, which reaches `observe_tree` only - it never calls `resolve_locator_anchor`. The anchor carries default `find`'s selected-match hydration, not the selector wait. The wait path is pinned against the mechanism it actually uses | read from `crates/core/src/commands/wait_selector.rs` while closing the coverage gap | recorded, and the pin added |
| Fake-driven end-to-end drives for mid-descent vanish and dead-token descent would need the resolver search generalized over `TreeSource`; the classification itself is pinned arm-by-arm at unit level instead | a later sub-phase touching the resolver's test seams | recorded |
| Pure dedup left unapplied to keep this diff's blast radius honest: the role gate repeated at three resolver sites, the secure-element DFS helper repeated across three test modules, and the two enumeration shells whose failure dispositions deliberately invert | maintainability follow-up | recorded |
| A cross-process `SetWindowTextW` edit to a legacy Win32 `EDIT` control (`HostedFixture`, a separate owning process) was not observed by `GetCurrentPropertyValue` for `Value` or `LegacyValue` under any bound tried - a multi-second poll, a fresh `ElementFromHandle` taken after the mutation, and an explicit `NotifyWinEvent` - so the legacy-control value bridge did not converge in this environment; the same edit against an in-process `LocalFixture` control is seen on the next read. This is a statement about production `get --property value` against a legacy out-of-process control, not only about the test harness (`crates/windows/src/tree/live_read_edit_tests.rs`) | closure: 2.12 self-hosted-runner environment / a later sub-phase with time to characterize the legacy bridge's actual convergence window | recorded, unresolved |
| `descent_failure`'s incomplete-wins reordering (`crates/windows/src/tree/resolve_search.rs`) marks a search attempt incomplete for **any** retryable/unavailable enumeration failure anywhere in the searched subtree, not only in the region containing a candidate. A single permanently-unreadable node can therefore turn a unique, confident match into retry-until-deadline (`AppUnresponsive` + `deadline_elapsed`) where the pre-change ordering would have resolved it. This mirrors macOS's own ordering and is unmeasured on a first-contact Chromium/Electron tree, where an unreadable node is more plausible than on the native fixtures this sub-phase exercised | a later sub-phase with first-contact Electron access (see the Obsidian web-content residual above) | recorded, unmeasured |
| Stored-evidence window resolution now corroborates handle ownership as well as process generation (`WindowIdentityEvidence::verify_stored`'s `GetWindowThreadProcessId` check, routed through by `resolve_window_root`), closing the cross-process HWND-recycle case. It does not close the same-process case: an HWND destroyed and reused by another window of the **same still-running process** resolves against the recycled window, and element-level exact-evidence resolution does not catch it - two instances of one dialog present identical `AutomationId`/`ControlType`/`Name`, so `candidate_outcome` matches and `classify_search`'s sole-candidate arm resolves with no geometric corroboration. Bounds corroboration cannot substitute (`bounds_hash` is exact over absolute screen coordinates and would fail any ref whose window moved). Closing it needs a per-window immutable identity `RefEntry` does not carry (UIA `RuntimeId` or a creation ordinal); `RefEntry` cannot gain fields in this sub-phase. No probe has measured the HWND uniqueness-counter wrap rate under real churn, and staging churn needs an interactive desktop with a fixture that can create and destroy windows in bulk | 2.12 Fixture App & Live E2E Harness (the wrap-rate measurement, on the first rig that can stage churn) → 2.12.1 Window Identity in Stored Refs (the `RefEntry` schema addition, shaped by 2.12's measured rate) | recorded, unmeasured, corrected in `docs/phases.md` §2.5, §2.12 and §2.12.1 |
| `identity_unknown_error` (`crates/windows/src/tree/resolve_search.rs`) and `mark_deadline_elapsed` (`crates/windows/src/tree/resolve.rs`) are verbatim copies of macOS's `identity_unknown` (`crates/macos/src/tree/resolve_errors.rs`) and `mark_deadline_elapsed` (`crates/macos/src/tree/resolve.rs`). They construct the error `details` payload core reads back - `AdapterError::with_details` derives `Retryability` from the `retryable` key (`crates/core/src/retryability.rs`), which is what `is_explicitly_retryable`/`permits_retry_by_default` gate core's retry consumers on (`live_locator/hydrate.rs`, `commands/wait_element.rs`, `commands/wait_selector.rs`). Duplicated per platform the payload can drift silently: a renamed key or a dropped `retryable` turns a retryable incomplete into an unretried failure on one OS only, and both crates' tests still pass because each asserts its own copy. Not fixed here - promotion is a second core touch beyond this sub-phase's single sanctioned visibility promotion, and it changes the macOS crate, the GA line for the whole platform phase | 2.15 Hardening & Integration Review (cross-platform promotion needs both adapters reviewed, e2e'd and perf-baselined together) | recorded, 2.5 review finding #11, corrected in `docs/phases.md` §2.5 and §2.15 |

## Verification Contract result (this unit's part)

| gate | result |
| --- | --- |
| run with repo-controlled content | yes - synthetic notepad/explorer content, repo fixtures, Obsidian counts-only |
| skips reasoned | n/a - all targets present and ran |
| findings closed-with-failing-test or escalated | the 2.4 `cell` question was not re-litigated; the Obsidian rate is owned/escalated; no new defect was found and fixed in this run |
| durable redaction-compliant report | this report + `docs/dogfood-reports/2026-08-03-001-captures/live-dogfood-run.json` (redaction gate passed) |
| environment header + per-target matrix | above |
| 0/1/N and Electron judgement with committed probe evidence | the 0/1/N live fixture cases are U1's committed evidence; the Electron rate is recorded here as the measurement of record |

The release binary is 1.95 MB (under the 15 MiB cap); every changed `.rs` file
in this sub-phase is under the 400-line cap; full repo gates run in CI.