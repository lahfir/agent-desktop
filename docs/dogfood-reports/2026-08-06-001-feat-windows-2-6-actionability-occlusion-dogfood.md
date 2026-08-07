# Dogfood report - Actionability & occlusion (sub-phase 2.6 U6)

**Date:** 2026-08-06 · **Branch:** `feat/windows-2.6-actionability` · **Plan:** `docs/plans/2026-08-06-001-feat-windows-actionability-occlusion-plan.md`

The occlusion gate, corroboration honesty, and scroll path cannot be validated by
a test that restates them. This run drives the release binary against real
software — including a foreign-process occluder the repository does not own —
and judges by reading JSON envelopes, never the suite's opinion of itself.

## Environment

| fact | value |
| --- | --- |
| OS | Windows Server 2019 Datacenter, build 17763 |
| UIA runtime | UIA3 COM (`CUIAutomation8`), `uiautomation` crate 0.25.0 |
| Binary | `target/release/agent-desktop.exe` (2,060,800 B release build) |
| Runner | `probes/windows/scratch/run-actionability-dogfood.ps1`, release binary driven directly, JSON read |
| Capture | `docs/dogfood-reports/2026-08-06-001-captures/actionability-dogfood-run.json` (redaction gate passed) |
| Targets | WinForms scratch (+ Notepad occluder), Explorer, Obsidian, WPF scratch corroboration |

## Per-target matrix

| target | UI stack | result | judgements |
| --- | --- | --- | --- |
| WinForms scratch + Notepad occluder | WinForms / Win32 Notepad | ran | J1 pass, J2 pass, J3 pass, J6 pass |
| Explorer on synthetic scratch dir | shell DirectUI | ran | J4 pass (`delivered_unverified`) |
| Obsidian (Chromium/Electron) | Chromium + Electron | ran | J5 shell-bound (A18-3 measurement of record) |
| WPF scratch overlay | WPF | ran | J3b recorded — five-point gate passed (HWND-less overlay cannot be force-covered) |

Every target shows **repo-controlled content**: synthetic scratch files, repo
fixtures, Obsidian counts/shapes only. Absent targets would be skipped with a
reason; none were absent.

## J1. Foreign-process occluder names the occluder

Scratch `btnAction` covered by owned Notepad (foreign pid). `--headed click
--timeout-ms 0` fails with occlusion, not a blind gate pass.

**Envelope shape (quoted keys only):**

- `ok: false`, `command: "click"`, `error.code: "ACTION_FAILED"`
- `disposition.delivery: "not_delivered"`, `disposition.retry: "safe"`
- `suggestion` present (honest recovery guidance)
- `details.checks[]` carries `receives_events` / `fail` / reason shape
  `"occluded by <role>"` with structured `occluder.role: "document"` and
  `occluder.name` present (redacted in durable capture beyond presence)

**Verdict:** pass — foreign occluder named; recovery suggestion honest.

## J2. Unoccluded headed click reaches honest pre-2.7 dispatch

Same target after Notepad dismissed. Gate passes; dispatch fails closed.

**Envelope shape:**

- `ok: false`, `error.code: "PLATFORM_NOT_SUPPORTED"`
- `message` names `execute_action` (pre-2.7 honest arm)
- `disposition.delivery: "not_delivered"`, `disposition.retry: "safe"`

**Verdict:** pass — gate ordering holds.

## J3. Same-root in-window overlay names the occluder

Scratch `btnCovered` fully covered by resized `btnOverlay` (parent-client
`SetWindowPos` on the overlay HWND so all five candidate points intercept).

**Envelope shape:**

- `ok: false`, `error.code: "ACTION_FAILED"`
- `receives_events` / `fail` / `"occluded by <role>"`
- `occluder.role: "button"`, occluder name present
- `disposition.delivery: "not_delivered"`, `disposition.retry: "safe"`

**Verdict:** pass — same-root arm names the in-window occluder against real
rendering.

## J4. Below-fold Explorer list item — scroll seam judged

Nineteen `listitem` candidates; a below-fold (or last) item clicked headless so
the auto-scroll seam fires without requiring hit-test focus.

**Envelope shape:**

- `ok: false`, `error.code: "ACTION_FAILED"`
- `disposition.delivery: "delivered_unverified"`, `disposition.retry: "unsafe"`

**Verdict:** pass — honest observation-judged arm (KTD5), not
`PLATFORM_NOT_SUPPORTED` from the pre-2.6 trait default. Scroll was invoked;
post-invoke visibility was not proven within the verification window on this
Explorer surface. Not assumed to be verified-visible scroll success.

## J5. Chromium hit-test (A18-3) — U6 measurement of record

Obsidian after cold launch: snapshot completed with **15 refs**, `complete:
true`, **0** positive-area leaves among twelve sampled refs —
`target_absent_or_shell_bound`. Matches A18-3 / A16-11 / A17-8 first-contact
shell. Five-point web-content classification remains unobtainable on this host.

**Verdict:** ran — shell-bound branch taken; this run is the measurement of
record for U6 (as U1 deferred).

## J6. Minimized-window guard does not invent InterceptedBy

Owned ScratchForms minimized (`IsIconic` true before click). Headed focus
restores the window before `hit_test` runs; the product envelope is
`PLATFORM_NOT_SUPPORTED` with **no** `receives_events` / `"occluded by <role>"`
check — no phantom desktop occluder.

**Verdict:** pass — guard holds on the binary path (no invented occlusion).
Direct `hit_test` IsIconic→`Unknown` remains pinned by
`hit_test_live_tests::minimized_on_screen_fixture_yields_unknown`.

## Finding fixed during this run

`--headed click` previously failed at the trait default
`resolve_window_strict is not supported` before the occlusion battery could
run. Thin Windows implementations of `resolve_window_strict` and `focus_window`
were added over existing 2.5 window identity (`crates/windows/src/system/window_resolve.rs`),
with regression tests, so the headed product path reaches `hit_test`.

## Residuals (owners for U7)

| residual | owner | status |
| --- | --- | --- |
| Obsidian web-content hit-test unmeasurable on this host (first-contact shell, 15 refs, zero positive-area leaves) — A18-3 branch confirmed by U6 | 2.12 self-hosted-runner / settled Chromium environment (A17-8 / A18-3) | recorded; U7 restates §2.6 Chromium wording to the shell-bound measurement |
| WPF same-root overlay (J3b) cannot be force-covered: HWND-less peers; five-point sweep reaches → `PLATFORM_NOT_SUPPORTED`. WinForms same-root (J3) is the live proof | fixture/docs honesty; U7 notes WPF HWND-less limit if §2.6 exit text implies both stacks | recorded |
| Explorer below-fold scroll delivered `delivered_unverified` rather than verified-visible `Ok` then dispatch — honest KTD5 arm on this surface; census `ScrollItem` thinness stands | §2.7 ancestor-scroll ladder (already planned deferral); U7 ensures §2.6 does not claim verified Explorer auto-scroll | recorded |
| Headed `focus_window` restores iconic windows before `hit_test`, so the IsIconic guard is not reached on a successful headed focus path; binary proof is "no phantom occluder", lib proof is the live unit pin | U7 CONCEPTS / skill wording: fails-open `Unknown` + headed focus restore ordering | recorded |
| `resolve_window_strict` / `focus_window` landed as U6 unblockers over 2.5 identity; fuller activation/focus policy remains with later lifecycle/input work | 2.8 / 2.9 review of focus policy depth | recorded, not deferred as unowned — shipped thin; depth owned later |
| Ancestor-scroll fallback for non-`ScrollItem` elements | §2.7 (plan Scope Boundaries; U7 writes into `docs/phases.md`) | already planned |
| Mixed-DPI live verification | A16-4 deferral chain | unchanged |
| Scratch fixture partial overlay is insufficient for five-point occlusion without harness resize (WinForms); document for 2.12 fixture occlusion targets | 2.12 fixture app | recorded |

## Verification Contract result (U6 dogfood gate set)

| gate | result |
| --- | --- |
| run with repo-controlled content | yes — synthetic notepad/explorer content, repo fixtures, Obsidian shapes-only |
| skips reasoned | yes — none absent; J5 shell-bound recorded rather than faked |
| findings closed-with-failing-test or escalated | headed path unblocked with `window_resolve` tests; residuals escalated with owners above |
| durable redaction-compliant report | this report + capture JSON (redaction gate passed) |
| environment header + per-target matrix | above |
| every judgement backed by a quoted envelope | J1–J6 above; capture JSON retains shapes/counts only |

Release binary 2.06 MB (under 15 MiB). U7 docs are **not** in this unit — residuals
above are the handoff.
