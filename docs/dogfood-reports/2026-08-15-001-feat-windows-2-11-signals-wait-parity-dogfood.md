# Dogfood report - Signals & wait parity (sub-phase 2.11 U11)

**Date:** 2026-08-15 | **Branch:** `feat/windows-2.11-signals-wait-parity` | **Plan:** `docs/plans/2026-08-12-001-feat-windows-signals-wait-parity-plan.md`

`wait --event` is a poll-and-diff loop over an adapter-supplied baseline; `wait
--menu`/`--menu-closed` is an adapter-owned polling loop over a platform with no
single "is a menu open" query. Both surfaces were driven against real software
(Notepad, Explorer, mspaint, Obsidian) rather than the crate's own fixtures,
including a busy desktop with unrelated real windows already present
(wezterm-gui, Cursor, Server Manager). Judgements use the exact JSON envelope,
elapsed time, and independent confirmation of true platform state (a follow-up
`list-windows` / `wait --menu` probe) - never `ok:true` alone. The corpus
safety envelope applies: titles, pids, window ids, and process-instance
tokens are redacted as `<title>` / `<pid>` / `<window-id>` /
`<process_instance>` in this report and in the capture; shapes and counts are
exact.

## Environment

| fact | value |
| --- | --- |
| OS | Windows Server 2019 Datacenter, build 17763 |
| Binary | `target/release/agent-desktop.exe` (2,363,392 B release build), version `0.8.1`, envelope version `2.3` |
| Runner | manual PowerShell driving the release binary directly against Start-Job-orchestrated transitions |
| Capture | `docs/dogfood-reports/2026-08-15-001-captures/signals-wait-parity-dogfood-run.json` (redacted) |
| Targets | Notepad (Win32/GDI), Explorer (shell), mspaint/win32calc (Win32), Obsidian (Electron/Chromium) |
| Desktop state | not quiesced - wezterm-gui, Cursor, and Server Manager windows were present and counted in every baseline throughout the run |

No WPF or WinUI target was driven in this run; the task scope for this leg
named Notepad, Explorer, mspaint, cmd, PowerShell, and Obsidian as the real
software available and explicitly excluded the crate's own ScratchWpf/ScratchForms
fixtures. WPF/WinUI coverage against real (non-fixture) software is not
available on this host and is not claimed here.

## Per-target matrix

| target | UI stack | result | legs |
| --- | --- | --- | --- |
| Notepad | Win32/GDI | ran | L1-L2, L7-L11 (menu, surfaces, --app contract, two-instance) |
| Explorer | Shell (Win32) | ran | L15 |
| mspaint / win32calc | Win32/GDI | ran | L2-miss/hit set, L3-L4, L16 |
| Obsidian | Electron/Chromium | ran | L13-L14 |
| Busy desktop (background wezterm/Cursor/Server Manager present throughout) | mixed | ran | all legs; L2, L16 specifically exercise concurrent transitions |

## Leg 1-6 - All seven `--event` tokens, positive and negative

Each token was driven positive (a real transition caused by a background job
mid-wait) and negative (`--timeout` expires honestly with no transition
caused). Full envelopes in the capture; summary:

| token | positive | negative |
| --- | --- | --- |
| `window-opened` | HIT, 2314ms wall, `elapsed_ms:2256` | TIMEOUT, 4107ms wall for 4000ms budget |
| `window-closed` | HIT (pre-opened target), 2318ms wall | TIMEOUT, 4127ms wall; **see Finding F1 for the open-then-close-mid-wait case** |
| `app-launched` | HIT, 2552ms wall | TIMEOUT, 4129ms wall |
| `app-terminated` | HIT, 2324ms wall | TIMEOUT, 4100ms wall |
| `focus-changed` | HIT, 2329ms wall, `kind: focus_changed_window` | TIMEOUT, 4110ms wall |
| `surface-appeared` | HIT (5/6 attempts), `surface: menu` or `surface: sheet` depending on which transition is first | TIMEOUT, 4051ms wall |
| `surface-dismissed` | HIT, `surface: sheet` | TIMEOUT, 4095ms wall |

**J1. Positive/negative coverage: pass for six of seven tokens outright;
`surface-appeared` passed 5/6 with one unexplained miss (see Findings F2/F3).**
Every negative case returned an honest `TIMEOUT` with `baseline_counts`
populated and no false positive was ever observed. `window-closed`'s positive
row above is the "target already existed before the wait" case; the "target
opened and closed entirely inside the wait" case is a separate, deterministic
failure documented as **F1**.

Surface events without `--app` were rejected in under 100ms with
`INVALID_ARGS` and a `suggestion` naming exactly what to add - this is the one
error message in the whole run that told the caller precisely what to do next.

## Leg 7 - `window-closed` and the seed-baseline boundary (Finding F1)

**J2. `window-closed` never fires for a window whose entire open-close
lifetime happens inside one wait call - reproduced deterministically.**

Discovery context: on the busy desktop, launching mspaint mid-wait (amid
calc/notepad noise) and then killing it produced a full-timeout miss, 3 times
running (20000ms, 10000ms, and an isolated 10000ms attempt with no other
concurrent opens). The process's death was independently confirmed via
`list-windows` and the OS process list each time - the window really closed.

A discriminating pair isolated the variable:

- **(a) mspaint opens 800ms into the wait (absent from the seed baseline),
  closes 1.5s later, no other noise.** `wait --event window-closed --timeout
  8000` → **MISS**, full 8055ms wall, `TIMEOUT`.
- **(b) mspaint opens, the wait starts 2s later (present in the seed
  baseline), closes 2s into the wait.** Same command → **HIT**, 2321ms wall,
  `elapsed_ms:2257`.

Six of six runs across this run are consistent with one rule: **a window not
present in the wait's seed baseline is invisible to `window-closed`, even if
it opens and closes entirely within the wait's own lifetime.** This is
distinct from "busy desktop flakiness" - the busy-desktop framing was how it
was found, not the actual variable; both the isolated and the busy-desktop
version missed identically, and both the notepad positive test and the
Explorer leg (L15, target pre-existing before the wait started) hit reliably.

By the same diffing logic (core seeds a baseline once and holds it for the
wait's lifetime, per the plan's read of `wait_event.rs:29-37, 64-67`), an app
that is **launched and terminated** entirely inside one `wait --event
app-terminated` call is likely subject to the identical gap. This is **stated
as an untested prediction**, not verified in this run - it was not directly
reproduced against `app-terminated`.

**Verdict:** disappointing - a real, common shape (launch a short-lived
window, wait for it to close) silently never fires, timing out
indistinguishable from "it never opened." Whether this is Windows-adapter-
specific or an inherited property of core's seed-once baseline design (and
therefore present on macOS too) was not determined here - out of scope for
this dogfood to diagnose, in scope for the owner to characterize before
disposing it.

## Leg 8 - `wait --menu` / `wait --menu-closed` against Notepad's real menu

**J3. `wait --menu` (positive and negative) is reliable.** Opening Notepad's
Help menu via `press alt+h --app notepad.exe` from a background job was
detected in 2205ms wall (`elapsed_ms:2145`); with `--app NOTEPAD.EXE` and no
menu opened, an honest `TIMEOUT` fired at exactly the requested budget with
`platform_detail: "No menu opened before the deadline"`.

**J4. `wait --menu-closed` timed out 4/4 times when the closing keystroke was
delivered by a background PowerShell job - Finding F3 (see below); it
succeeds when the identical command is run interactively.** Root cause was
isolated, not assumed: after each `wait --menu-closed` timeout, a direct
`wait --menu --app notepad.exe` probe (run separately, outside the timing
wait) confirmed the menu was genuinely **still open** - the background job's
`press escape --app notepad.exe` had not dismissed it, even though `press`
itself returned `ok:true` with `disposition.delivery:
"delivered_unverified"`. The identical `press escape --app notepad.exe`
invoked directly from the interactive foreground console dismissed the menu
every time it was tried. `wait --menu-closed`'s `TIMEOUT` was therefore an
honest report of true platform state; the defect is upstream, in key
delivery from a non-interactive/background process context reaching a
native `GUI_INMENUMODE` modal loop.

**Verdict:** `wait --menu`/`--menu-closed` themselves are correct against
what they observe. The finding is about `press`'s keystroke delivery when
invoked from a process without the caller's own foreground/interactive
context - directly relevant to any orchestrating agent that spawns `press`
as a detached or background subprocess, which is a normal way to drive a
concurrent transition.

## Leg 9 - The `--app` contract

**J5. Confirmed exactly as decided (KTD5/R10): `--app notepad.exe` and
`--app NOTEPAD.EXE` both resolve (case-insensitive image name); `--app
Notepad` does not.**

```
wait --menu --app notepad.exe   --timeout 500  -> resolves, honest TIMEOUT
wait --menu --app NOTEPAD.EXE   --timeout 500  -> resolves, honest TIMEOUT
wait --menu --app Notepad       --timeout 500  -> APP_NOT_FOUND
wait --event surface-appeared --app Notepad --timeout 500 -> APP_NOT_FOUND
```

**Finding F4:** `APP_NOT_FOUND`'s message ("Application 'Notepad' was not
found with exact process identity") carries **no `suggestion` field at all**.
Contrast the surface-events-without-`--app` case (`INVALID_ARGS`), which does
carry a concrete `suggestion`. An agent hitting `APP_NOT_FOUND` after passing
a display name gets no hint that an image name (`notepad.exe`) is what's
expected.

## Leg 10 - Two `notepad.exe` instances under `--app`

**J6. `AMBIGUOUS_TARGET` fires correctly for `wait --menu` and `wait --event
surface-appeared` with two live `notepad.exe` instances; `wait --event
app-launched` correctly does not ambiguate (R4's deliberately-unresolved
case), returning a plain `TIMEOUT` with `baseline_counts.apps:2`.**

**Finding F5:** the `AMBIGUOUS_TARGET` error's `suggestion` is a generic
ref-resolution template - *"Re-run a snapshot to refresh refs, then retry
with a more specific ref"* - copied onto a command family that has no
concept of a ref or a snapshot. `details.candidates` usefully lists each
candidate's `pid` and `process_instance`, but no `wait` flag accepts either
as a selector (`--help` shows only `--app`, `--window`, and `--window-id`,
and `--window-id` narrows only the `window-opened`/`window-closed`/
`focus-changed` events per its own help text, not `--menu` or the
`app`-scoped surface events). The error names the ambiguity precisely and
then gives the caller no path to resolve it short of closing one instance.

## Leg 11 - Obsidian (Electron/Chromium)

**J7. `window-opened` sees Obsidian's window: HIT, 4822ms wall
(`elapsed_ms:4675`) - slower than the Win32 targets' ~2.2-2.6s but still well
inside a 20s budget.** `app-terminated --app Obsidian.exe` also HIT (2249ms
wall) when all 4 processes sharing the `Obsidian.exe` image name were killed
together.

**J8. Electron's multi-process identity does not ambiguate `--app
Obsidian.exe` for `wait --menu` / `wait --event surface-appeared`.** Four
live processes shared the `Obsidian.exe` image name; `baseline_counts.apps`
was **1**, not 4, and both waits returned an honest `TIMEOUT` (no menu/surface
open) rather than `AMBIGUOUS_TARGET` or `APP_NOT_FOUND`. This indicates the
signal path's app resolution keys on the single window-owning process rather
than every process sharing the image name - a positive result worth
recording since Leg 10 shows genuinely multiple *user-facing* instances of
one image name **do** ambiguate. This is a confirmation, not a finding.

## Leg 12 - Explorer and a busy desktop

**J9. `window-opened`/`window-closed` both HIT cleanly for Explorer** (2557ms
and 2329ms wall respectively; the closed case used a pre-existing window per
the F1 boundary above).

**J10. Busy-desktop sanity: `window-opened` correctly matched the first
genuine transition (a Calculator window) in 1082ms wall while further
opens/closes were still queued behind it in the background job** - no
confusion, no double-fire, no stale match.

## Findings and dispositions

| id | finding | disposition | owner / proof |
| --- | --- | --- | --- |
| F1 | `window-closed` never fires for a window that both opens and closes entirely within one wait call (seed-baseline-absence), reproduced 4/4 on a discriminating pair. **The `app-terminated` prediction is no longer a prediction: measured at owner review, an app launched 2s into a 14s wait and terminated 3s later produced `TIMEOUT`, not `app_terminated`.** The gap therefore generalises to every disappearance event - `window-closed`, `app-terminated`, `surface-dismissed` - for any entity absent from the seed baseline | **owned elsewhere - §2.15** | capture legs L2-miss-1/2/3, L2-discriminator-a/b |
| F2 | `surface-appeared` missed a genuine menu-to-dialog transition in 1 of 6 identical attempts (dialog confirmed open via `list-windows` immediately after the timeout); not reproducible on 5 immediate retries | **accepted** - a single unreproducible miss whose most likely mechanism is F3's background-process keystroke delivery (same shape: a keystroke posted from a background job driving a native menu transition). Accepting it as a 2.11 defect would mean guessing; it is recorded with its capture leg so the next observation is the second data point rather than the first, and it is re-examined when F3 is settled | capture leg L7-pos-menu-to-dialog-x5, attempt 1 |
| F3 | `press escape --app notepad.exe` invoked from a background/non-interactive process does not dismiss an open native `GUI_INMENUMODE` menu, 4/4 reproductions, despite returning `ok:true` / `delivered_unverified`; the identical command from the interactive foreground console dismisses it reliably. `wait --menu-closed`'s resulting `TIMEOUT` is honest given true platform state - the defect is upstream in keystroke delivery | **owned elsewhere - §2.15**, whose existing `press --app` divergence entry already records that Windows `SendInput` injects into the foreground queue with no per-pid targeting. This run adds the *background-process caller* as a second, distinct arm of that same entry: not a non-foreground **target**, but a non-interactive **caller**. `delivered_unverified` is the honest envelope for an unverifiable synthesis, so this is a documented reach limit rather than a false success | capture leg L10-menu-closed-miss-x3 (4 attempts) plus the isolated verification probe |
| F4 | `APP_NOT_FOUND` carries no `suggestion` field, unlike `INVALID_ARGS` in the same run; a caller who passes a display name (`Notepad`) instead of an image name gets no hint toward the accepted form | **owned elsewhere - §2.15**, in the same cluster as the `--app` stem-matching question. The error is raised in `crates/core/src/app_lookup.rs`, shared with macOS, so improving it changes both adapters' output; 2.11 ships zero `crates/core` changes and that claim is load-bearing for its review | capture leg L11-app-contract |
| F5 | `AMBIGUOUS_TARGET`'s `suggestion` text references refs and snapshots, concepts `wait --menu`/`wait --event` do not have; `details.candidates` carries a disambiguating `pid`/`process_instance` per candidate but no `wait` flag accepts either, so the error is precise and the recovery path is absent | **owned elsewhere - §2.15**, same core-owned `app_lookup.rs` cluster as F4. The missing half is a product decision - whether `wait` gains a pid selector - not a message fix | capture leg L12-two-instances |

Every finding carries a disposition and none is left at "recorded".

**No finding was fixed in product code, and that is worth stating plainly
rather than glossing.** Four of the five are not 2.11 defects at all: F1 is
the fixed-baseline semantics of `diff_signals`, which lives in
`crates/core` and behaves identically on macOS; F3 is the shipped reach
limit of Windows `SendInput`, already recorded as a cross-platform
divergence; and F4/F5 are error envelopes raised in `crates/core`'s shared
`app_lookup.rs`. Fixing any of them from here would mean changing core
behaviour on both adapters from inside a Windows sub-phase, which is the
thing the platform delivery model exists to prevent. They land at the one
gate that reviews both adapters together.

The honest cost of that is concentration: four findings landing on §2.15
makes that gate heavier, and a reviewer should read this as evidence about
where the remaining cross-platform contract debt actually sits, not as
this sub-phase deflecting work. F2 is the only accepted finding, and it is
accepted because acting on one unreproducible observation would be
guessing, not because it is unimportant.

## Second pass - adversarial

The first pass fixed nothing in product code and routed four of five
findings onward. That is a weak result for a gate whose whole purpose is to
find what this sub-phase got wrong, so a second, deliberately hostile pass
ran against the same release binary along three axes: deadline and
degenerate-input boundaries, load/churn/concurrency, and the calling-agent
view. The concurrency axis alone ran ~120 churn trials, 100 concurrent
invocations, ~550 combined polls under handle and memory sampling, and 15
menu open/close rounds against live waits.

| id | finding | disposition |
|---|---|---|
| F6 | `wait --menu`/`--menu-closed` reported a target that died mid-wait with `disposition: {delivery: unknown, retry: unknown}` and no `suggestion` - while every other stale-reference error in the same binary sets both. `DeliverySemantics` defaults to unknown, and `stale_process_error` never opted out of that default, so an agent whose target exited during an ordinary churn scenario got no machine-readable signal about whether to retry or re-resolve. This project's envelope contract permits reading `recovery` only when the retry disposition is `safe`, which a defaulted `unknown` never is | **fixed here** - `crates/windows/src/system/wait.rs` now sets `not_delivered` and a process-oriented recovery, deliberately not core's ref-and-snapshot wording, which names a refresh this surface has no refs for. Pinned by `a_target_that_died_mid_wait_reports_a_disposition_and_a_recovery`, invert-verified: removing the disposition fails it |
| F7 | `wait_for_menu`'s doc comment promised the predicate is evaluated at least once "even for a near-zero timeout", so `--menu-closed` against an already-closed menu succeeds immediately. True of the function, false of the command: the caller resolves the application first, and on a budget smaller than that resolution costs the command returns the generic deadline error without ever reaching the loop. Measured at `--timeout 0` and `--timeout 1` | **fixed here** - the doc now states the guarantee as the function's rather than the command's, and says plainly that a loop handed an already-spent budget cannot rescue it. A comment that promises behaviour the shipped path cannot deliver is a defect in the same way a wrong assertion is |
| F8 | A scoped wait for a **disappearance** event fails `APP_NOT_FOUND` instead of observing the disappearance, when the target dies before application resolution completes. Deterministic when the target is already gone; racy and non-monotonic when it dies during resolution (5ms miss, 20ms hit, 60ms miss). The discriminating pair isolates it: the identical transition **unscoped** hits at the same timing where `--app`-scoped fails. `APP_NOT_FOUND` tells an agent "no such application", implying a bad `--app` value, when the truth is the opposite - the app existed and the disappearance the agent asked to observe is what broke the lookup | **owned elsewhere - §2.15** - `crates/core/src/commands/wait_event.rs`'s `signal_filter` gates every non-`app-launched` wait on `resolve_app` before the poll loop begins and treats its failure as fatal. Its `AppTerminated` escape hatch requires a seeded baseline, which a plain CLI invocation never has, so on this path it is dead code. Platform-neutral and shared with macOS |
| F9 | Concurrent waits launched through a PowerShell job broker returned `APP_NOT_FOUND` in 40 of 48 trials against a stable, untouched target; the identical scenario driven by direct process spawning was clean across 100+ invocations | **accepted** - not reproducible against the binary under genuine OS-level concurrency, so the effect tracks the job broker rather than `agent-desktop.exe`. Recorded because it is the same shape as F3 (a background caller observing what a foreground caller never does) and an orchestrating agent may well drive waits this way |

**One code-level asymmetry was attacked and not confirmed**, and is recorded
rather than dropped: `list_apps_live` has none of the retry and
self-consistency hardening `signal_inventory.rs` was purpose-built with for
conceptually the same mid-walk identity race, and it can return `INTERNAL` -
a code outside the set core's poll loop retries. Forty trials sweeping the
kill offset from 0 to 800ms produced a clean threshold and zero `INTERNAL`
codes, so the risk is latent rather than demonstrated.

**What held up under hostile load**, stated because a dogfood that only
lists failures misrepresents what was tested: rapid open/close churn across
25 cycles, concurrent waits against multiple targets with no
cross-contamination, capture cost inside the 200ms budget with 27 live
windows, the tool-window exclusion holding end-to-end so 15 menu open/close
rounds never fabricated a `window-opened`, focus churn across 20
alternations, three-instance ambiguity degrading correctly as instances
died, and flat handle, thread and working-set counts across ~550 polls -
including the snapshot open/close cycle that runs every 50ms on the menu
path.

**The consumer axis was re-run after losing its session mid-run**, and it was
the most productive of the three - six further findings, including the two
shipped-documentation defects below. Its results are recorded here rather
than left as the coverage gap the first attempt produced.

| id | finding | disposition |
|---|---|---|
| F10 | In batch mode, `launch` followed by a disappearance-event wait for the same app can never fire. Batch pre-seeds the baseline for an event wait *before the preceding entry runs*, so for `[launch, wait window-closed]` the seed is captured before the app exists: baseline has zero windows, the post-close state has zero windows, and the diff sees no net change. A full-timeout `TIMEOUT` reporting `baseline_counts.apps: 0` immediately after a `launch` succeeded in the same batch reads as though the app was never observed at all | **owned elsewhere - §2.15**. The pre-seed lives in `src/batch/execution.rs`, the platform-agnostic binary crate, and the same defect is reachable on macOS through the identical path. Note the pre-seed is a genuine *improvement* for `[launch, wait window-opened]`, which is why it exists - it is one entry too early, not wrong in principle |
| F11 | Every event except `app-launched` resolves `--app` before the poll loop, so scoping to an application that is not running yet returns `APP_NOT_FOUND` in under 100ms while ignoring `--timeout` entirely. An agent racing a concurrent launch against a scoped wait - a normal orchestration pattern, and this dogfood's own methodology - gets an immediate hard failure instead of the budget it asked for | **owned elsewhere - §2.15** for the core gate (same `signal_filter` mechanism as F8), **fixed here** for the documentation half: the shipped `--app` flag description and a new note now state that the application must already be running and point at `app-launched` or an unscoped wait for the racing case |
| F12 | A scoped wait pins to the one process instance resolved at wait start, so a second process of the same image name starting mid-wait is invisible - a silent miss indistinguishable from nothing happening. The unscoped wait catches the same transition. The flag read "Scope the wait to a specific application", which a reasonable agent takes to mean processes named X | **owned elsewhere - §2.15** for the core behaviour, **fixed here** for the documentation: the flag now says it scopes to one already-running process instance resolved once at wait start, and the note tells a caller who needs any instance of a name to run unscoped and filter the event |
| F13 | `wait --event surface-appeared` reports `"surface": "menu"` on Windows, and the obvious follow-up `snapshot --surface menu` returns `PLATFORM_NOT_SUPPORTED`. The shipped docs tell agents to use `--surface menu` with no platform caveat, and `agent-desktop skills` serves that text from the Windows binary | **fixed here**. This is the asymmetry §2.14 owns closing, but an undocumented asymmetry is a defect now: the observation doc marks `--surface menu` macOS-only, tells callers to read `supported_surfaces` from `status` first, and names this exact case - the event says a menu opened; it is not an invitation to snapshot it as a surface on Windows |
| F14 | The global snapshot namespace can be left unusable by a file-ownership mismatch, breaking `status` and `snapshot` with an `INTERNAL` error carrying no suggestion. Verified not to affect this surface: neither `wait --event` nor `wait --menu` touches `RefStore` | **accepted** for this sub-phase - outside the 2.11 surface, pre-existing, and reachable identically on either adapter. Recorded so it is not rediscovered as new |
| F15 | `SKILL.md`'s platform-universal sections are not Windows-aware, and that text is served verbatim by `agent-desktop skills` from the Windows binary | **owned elsewhere - §2.15**, whose scope already names docs and skills sync. The 2.11-specific half - the surface asymmetry and the `--app` semantics - is fixed here rather than deferred, because those are the lines this sub-phase's own behaviour made wrong |

**Two documentation defects were fixed here rather than routed.** Shipped
skill markdown is `include_str!`d into the binary and served to agents by
`agent-desktop skills`, so a wrong line in it is a shipped defect in the same
sense as a wrong error code - and both of these lines were made wrong by what
this sub-phase shipped.

## Notes (do not implement here)

1. Across roughly a dozen `TIMEOUT` envelopes recorded in this run, none
   carried a `last_error` breadcrumb - no retryable capture failure
   (`APP_UNRESPONSIVE` / `ELEMENT_NOT_FOUND`) was observed on this desktop
   during the run, so R5's retry-and-report path was not exercised here.
2. `focus-changed`'s event `kind` is `focus_changed_window`, not
   `focus_changed` - noted for shape-parity awareness, not flagged as a
   finding on its own.
3. `TIMEOUT` wall time consistently ran ~50-130ms past the requested
   `--timeout` across every negative/timeout leg in this run (e.g. a 4000ms
   budget landed at 4095-4129ms wall; 20000ms landed at 20059ms) - small,
   consistent overhead, not flagged as a finding.
4. No WPF/WinUI target was available as *real, non-fixture* software on this
   host within this task's named target list; that gap is pre-existing and
   already tracked by the plan's own Deferred section (measurement-sourced
   menu coverage owned by section 2.12), not introduced by this dogfood.

## Verification Contract result (U11 dogfood gate set)

| gate | result |
| --- | --- |
| driven against real software, not crate fixtures | yes - Notepad, Explorer, mspaint, win32calc, Obsidian |
| all seven `--event` tokens, positive and negative | yes - see Leg 1-6 table |
| `wait --menu` / `--menu-closed` against a real menu | yes - Leg 8 |
| `--app` contract validated as user-visible fact | yes - Leg 9 |
| two-instance `--app` behaviour observed | yes - Leg 10 |
| Chromium/Electron target (Obsidian) | yes - Leg 11 |
| busy-desktop sanity | yes - Leg 12, and the discovery context for F1 |
| safety envelope enforced | yes - titles/pids/window-ids/process-instance tokens redacted in report and capture |
| findings with dispositions | 5 findings, each assigned: F1/F3/F4/F5 owned elsewhere (§2.15), F2 accepted with reason. None left at "recorded". Zero-findings rule satisfied |
| durable redaction-compliant report | this report + capture JSON |

Release binary 2,363,392 B (~2.25 MiB, under the 15 MiB cap). Product code
changes during dogfood: **none**. No commit made.
