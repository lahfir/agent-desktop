# Dogfood and full-branch review: Hardening & integration gate (Sub-phase 2.15)

- **Date:** 2026-08-31
- **Branch:** `feat/windows-2.15-hardening-integration-review`, cut from and merging back
  into `feat/windows-adapter`.
- **Two exercises, one report.** The dogfood drove this gate's own surface against real
  software as an agent would. The full-branch review read the assembled platform branch
  — `main...feat/windows-adapter`, 1,161 files and ~193k insertions — fanned out one
  reviewer per coherent subsystem. Both produce findings, both use the same three
  dispositions, so both are recorded here rather than in two documents that would have
  to be read together anyway.
- **Channels exercised:** the release CLI binary against the machine's real shell, real
  notification infrastructure, real Chromium/Electron applications and real processes —
  never the fixture app.
- **Environment:** Windows Server 2019 Datacenter, build 17763, single display,
  interactive console session.
- **Capture safety:** every capture written during this run carries shapes and counts
  only — no titles, paths, pids, machine names, user names or message text.

## Disposition rule

Every finding below takes exactly one of three dispositions, and **"recorded" is not one
of them**:

- **Fixed here** — with a named test that is invert-verified: the fix is broken, the
  test is watched failing, the fix is restored.
- **Owned elsewhere** — written into the receiving sub-phase's scope in
  `docs/phases.md` in this same PR, in enough detail that its implementer can act on it
  without reading this report.
- **Accepted** — with the reason stated.

## Part 1 — Dogfood: driving this gate's surface

### Legs

| # | Leg | Target | Outcome |
|---|-----|--------|---------|
| 1 | `snapshot --surface system-tray` then `click @ref` | real notification area | the surface read **zero** items and a ref taken from the taskbar surface refused `WINDOW_NOT_FOUND` — finding D1 |
| 2 | `dismiss-all-notifications --app <name>` | real Action Center | the filtered branch invoked captured element handles in a virtualized list and discarded every per-entry error — finding D2 |
| 3 | ref action against a disabled, policy-refused target | real control | burned the whole wait budget and answered `TIMEOUT` for a refusal that could never change — finding D3 |
| 4 | `launch` / `close-app` against a name matching several running instances | real processes | `AMBIGUOUS_TARGET` whose suggestion named a flag the command does not have — finding D4 |
| 5 | `snapshot --app <not-installed>` | absent process | `WINDOW_NOT_FOUND` asserting the application is running — finding D5 |
| 6 | `screenshot` window and display paths | real windows | `SelectObject` unchecked in both capture paths — finding D6 |
| 7 | `--app` resolution by stem and by image name across every command | real processes | commands disagreed about which identifier forms they accept — finding D8 |
| 8 | Chromium content-tree exposure against a settled real editor | Chromium host | measured; recorded as A28-3 |
| 9 | repeated runs of the live Windows suite under load | this host | the suite cannot resolve a one-test A/B difference — finding D9 |

### Findings and dispositions

**D1 — the system-tray surface read nothing and its refs were not click-legal.**
*Fixed here.* Three `ToolbarWindow32` windows sit under `Shell_TrayWnd`; the shipped
class chain stopped one hop short and resolved a real, visible-flagged, zero-extent
placeholder whose automation element genuinely has no children. Adding the missing
`SysPager` hop resolves the toolbar that holds the icons. The surface now returns three
`button` refs carrying the stable GUID `AutomationId`s this corpus already recorded, and
a click on one is delivered. Recorded as A28-4, A28-5 and A28-7. An earlier three-
mechanism explanation was superseded: it was one cause, and the other two arms were
consequences of resolving the placeholder. Test: `shell_surface_kinds_tests`.
`delivered_unverified` is the correct disposition for the click and not a defect — a
synthesized click cannot confirm what a vendor's tray icon did with it.

**D2 — a filtered `dismiss-all` targeted stale handles and swallowed its errors.**
*Fixed here.* The Action Center's list is virtualized, so removing one entry renumbers
and recycles the elements after it; a handle captured before the loop can name a
different notification by the time its turn comes. Each captured target is now re-read
from the live surface on its own turn and matched by app, title and body, and an invoke
error is recorded against the captured index instead of discarded. The settle read stays
the proof of what left. Tests:
`a_filtered_dismiss_all_that_removes_its_target_leaves_another_apps_entry_unreported`,
`a_recorded_invoke_error_names_its_own_reason_instead_of_the_generic_survivor_message`,
`an_invoke_error_recorded_for_an_entry_that_still_left_is_never_reported`. **No live test
backs this**, and that is stated rather than papered over: three consecutive runs of one
untouched live notification test failed three different ways on this host, so a live
two-application test here would assert nothing.

**D3 — a permanent refusal was masked by a transient gap.**
*Fixed here.* `terminal_code()` returned the first blocking check's code, so a check that
blocks without carrying a code masked every terminal code behind it. Gate order puts
`enabled` before `supported_action` and `policy`, so a target that is both disabled and
policy-refused reported no terminal code at all. It now returns the first blocking check
that *carries* a code. Test:
`policy_denied_disabled_target_fails_fast_without_exhausting_wait_budget` — before the
fix it ran the full budget and returned `Timeout` where `PolicyDenied` was expected.
Shared core, so macOS answers the same way.

**D4 — `AMBIGUOUS_TARGET` suggested a flag the command does not have.**
*Fixed here.* The envelope carried the shared ref suggestion — refresh the snapshot and
retry with an updated ref — but `launch`, `close-app`, `wait`, `screenshot`, `press`,
`list-surfaces` and `window-target` select a process by name and take no pid, instance or
ref flag. It now names the candidate pids and says plainly that this command has no flag
to disambiguate. That is the honest answer even though it is not a recovery the caller
can apply. Tests: `ambiguous_instance_suggestion_names_pids_instead_of_a_ref_retry` in
core and in the Windows launch path.

**D5 — `WINDOW_NOT_FOUND` asserted an application was running when it was absent.**
*Fixed here.* A `--app` snapshot whose window filter came back empty always answered that
the application is running but has no matching window, and suggested waiting for a window
— advice that can never come true for a process that does not exist. The empty-filter
path now asks the same app resolution `close-app` uses and returns `APP_NOT_FOUND` only
when that resolution specifically says the process is absent. Test:
`app_name_resolution_discriminates_absent_from_windowless`.

**D6 — `SelectObject` unchecked in both capture paths.**
*Fixed here.* `SelectObject` returns NULL on failure and neither capture path checked it.
An unselected bitmap means `PrintWindow` and `BitBlt` paint into the DC's default
monochrome bitmap, so a blank or corrupt capture is returned as a success. Both sites now
surface the Win32 error.

**D7 — a value-write gate test returned early instead of failing.**
*Fixed here.* The test returned when its fixture lookup failed, reporting success without
exercising anything. It now fails with a message naming what was missing.

**D8 — commands disagreed about which `--app` identifier forms they accept.**
*Fixed here.* One predicate, `app_name_matches`, now backs every command, with `.exe`
suffix tolerance so a bare stem resolves a process whose image name carries the
extension.

**D9 — this host's live suite is load-sensitive.**
*Accepted.* Recorded as A28-6. The live Windows test suite cannot resolve a one-test A/B
difference on this box: the same untouched test produces different failure shapes across
consecutive runs. The consequence is stated wherever it matters — a live failure is
attributed to a change only after the unmodified tree is shown to fail the same way — and
it is why several fixes in this report carry deterministic unit coverage instead of a
live test, each saying so explicitly.

**D10 — Chromium content exposure.** *Measured*, recorded as A28-3.

## Part 2 — Full-branch review

Reviewed `main...feat/windows-adapter` — the whole platform phase, not this sub-phase's
diff. At 1,161 files and ~193k insertions this is not one review pass and was not
attempted as one: it fanned out one reviewer per coherent subsystem — tree and
observation, the semantic action tier, input synthesis, system and process lifecycle,
capture and clipboard, notifications and shell surfaces, core contract changes, the e2e
harness and gate scripts, and the probe corpus — each reading only its own paths and
reporting findings with evidence.

Two reviewers independently rediscovered defects this gate had already fixed on its own
branch (the resolver's `terminal_code` ordering and the `--app` exact-match gap), which
is the expected result of reviewing the integration branch rather than the tip, and is
reported as corroboration rather than as new work.

### Findings and dispositions

*(This section is completed as each finding's disposition is settled; see the table
below.)*
