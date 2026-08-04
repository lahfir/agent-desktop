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
| WinForms scratch fixture | WinForms | ran | 27 | true | get/is ok | button 1 | **resolved-correct after content swap** |
| WPF scratch fixture | WPF | ran | 10 | true | snapshot only | n/a | n/a |
| Obsidian (Chromium/Electron) | Chromium + Electron | ran | 15 (shell) | true | n/a | n/a | **4/6 shell refs stale, 2/6 resolved** |

## The live loop works end to end on every reachable stack

The post-2.5 binary drives `snapshot`, `find`, `get` and `is` against real
programs. The five live readers answer (Notepad, WinForms: `get --property
bounds` and `is --property enabled/visible` return ok), `find` hydrates refs
(WinForms: a button locate round-trips to one match and its `ref_id` is a
usable ref). The WinForms fixture's ref re-resolves after a `WM_APP` content
swap lands `resolved-correct` - the tri-state matcher neither went
`AMBIGUOUS_TARGET` nor silently resolved a neighbour, matching the A17-3
live 0/1/N fixture evidence and the A7-3 wrong-target pin carried through U2.

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
by the fixture-driven live tests (U3/U5) and by the WinForms ref that
re-resolves after a real mutation. The web-content rate is owned by the 2.12
self-hosted-runner environment, `closure: 2.12` per A17-8.

## Ambiguity observations against U1 item 7's census

U1's census found the native fixtures ambiguity-clean under the composed
matcher: zero zero-extent ref-able elements, zero duplicate positive-area
bounds, and only the deliberately-constructed duplicate pairs resolving 2 (the
A17-3 live N case). This dogfood run corroborates it on the real binary: across
all targets **no resolution returned `AMBIGUOUS_TARGET`** - every ref either
re-resolved to the same element or went `STALE_REF` (the Obsidian shell's
4/6). There were no offscreen/virtualized degeneracy cases observed on the
native stacks, consistent with A17-7's "zero-extent is offscreen/virtualized-
only" finding. The WPF DataGrid's `cell` refinement question (A16-10, carried
from 2.4) still stands and is not re-litigated here.

## Residuals

| residual | owner | status |
| --- | --- | --- |
| Obsidian web-content STALE_REF rate inside the file tree is unmeasurable on this host (first-contact shell) | 2.12 self-hosted-runner environment (A17-8) | recorded, `closure: 2.12` |
| WPF live reads (`get`/`is`) not sampled in this run - the WPF leg records snapshot only | U7-next / dogfood extension | recorded |
| `find --count` vs materialized agreement not asserted on a real app (no CI assertion may name a real-app count) | per VC "Evidence honesty" | **closed on fixtures** - agreement is now pinned against the hosted fixture, where a count is repo-controlled rather than an `app/provider` fact |
| Traced correction to this sub-phase's own framing: `wait --selector` polls through `resolve_query` with materialization `None`, which reaches `observe_tree` only - it never calls `resolve_locator_anchor`. The anchor carries default `find`'s selected-match hydration, not the selector wait. The wait path is pinned against the mechanism it actually uses | read from `crates/core/src/commands/wait_selector.rs` while closing the coverage gap | recorded, and the pin added |
| Fake-driven end-to-end drives for mid-descent vanish and dead-token descent would need the resolver search generalized over `TreeSource`; the classification itself is pinned arm-by-arm at unit level instead | a later sub-phase touching the resolver's test seams | recorded |
| Pure dedup left unapplied to keep this diff's blast radius honest: the role gate repeated at three resolver sites, the secure-element DFS helper repeated across three test modules, and the two enumeration shells whose failure dispositions deliberately invert | maintainability follow-up | recorded |

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