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
consequences of resolving the placeholder. Test:
`the_system_tray_chain_descends_through_the_pager_that_owns_the_icons`, which pins
the measured chain and fails when the hop is removed. **It was added after an audit of
this report found the fix unpinned**: the file this row originally named contains a
build-refusal test and an absent-class test, and neither would have noticed the hop
disappearing. No live assertion can stand in for it, because a machine whose tray is
legitimately empty reads zero either way.
`delivered_unverified` is the correct disposition for the click and not a defect — a
synthesized click cannot confirm what a vendor's tray icon did with it.

**D2 — a filtered `dismiss-all` targeted stale handles and swallowed its errors.**
*Fixed here.* The Action Center's list is virtualized, so removing one entry renumbers
and recycles the elements after it; a handle captured before the loop can name a
different notification by the time its turn comes. Each captured target is now re-read
from the live surface on its own turn and matched by app, title and body, and an invoke
error is recorded against the captured index instead of discarded. The settle read stays
the proof of what left. **Only one of the two halves is pinned, and the report says which.**
The failure-reporting half is covered by
`a_filtered_dismiss_all_that_removes_its_target_leaves_another_apps_entry_unreported`,
`a_recorded_invoke_error_names_its_own_reason_instead_of_the_generic_survivor_message` and
`an_invoke_error_recorded_for_an_entry_that_still_left_is_never_reported`, all three
invert-verified. The fresh-identity re-read itself has **no test**: it needs a live,
virtualized Action Center list holding two applications' entries, and this host's live
Action Center read is not reproducible — three consecutive runs of one untouched live
notification test failed three different ways. A test written against it would assert
nothing, so none was written.

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
surface the Win32 error. **Unpinned, and the reason is that the seam cannot reach it**:
the capture tests' failure injection fires *after* a successful selection, so it cannot
force `SelectObject` to fail. Building new injection scaffolding to reach one branch was
refused as over-engineering. The neighbouring `restore_selected_bitmap` failure *is*
pinned (R7), reached by calling it directly with an invalid prior selection.

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

#### Fixed here, each with a named invert-verified test

| # | Subsystem | Finding | Test |
|---|-----------|---------|------|
| R1 | tree / observation | The resolver's stored-path fast tier accepted a landing on identity alone and returned before the broad search ever ran, so a list that reordered between snapshot and action put a duplicate-identity sibling at the stored index and an action landed on it silently. A landing whose live bounds hash is known and contradicts the stored one now falls through to the search that already tie-breaks on it. | `the_fast_path_refuses_a_contradicting_hash_and_still_accepts_an_agreeing_one`, plus `a_path_landing_with_an_unreadable_live_bounds_hash_is_still_accepted` — the second is the guard against turning an unread field into a refutation |
| R2 | tree / observation | A provider answering a documented-boolean property as an integer read as gate-closed everywhere except `IsPassword`, which had already been widened once for this exact divergence. A checkbox reporting `IsTogglePatternAvailable` as an integer lost its advertised action, its refined role and its checked state, in silence. | `a_nonzero_known_number_opens_the_gate_but_an_unread_gate_still_does_not`, `is_true_reads_a_nonzero_known_number_on_an_ungated_available_property` |
| R3 | tree / observation | A COM fault partway up the ancestor climb was folded into the same answer as a climb that reached the root and found nothing, so the occlusion gate was told a target was unclipped when the read that would have said otherwise never completed. Both callers now fail closed on the fault. | `a_completed_climb_with_no_scrollable_ancestor_reports_ok_none` guards the healthy path. **The fault arm ships unpinned and the report says so**: two independent attempts to force a parent-read fault on this host both returned the exhaustion answer instead, which is itself the measurement recorded as A28-8 |
| R4 | semantic actions | A rung that hard-errors discarded the steps recorded before it. The value-write chain deliberately continues past an unverified delivery, so a later rung's error reported `not_delivered` and `retry: safe` after the field had already been written. | `genuine_err_after_prior_delivery_upgrades_disposition_to_delivered_unverified`, plus `genuine_err_without_prior_delivery_keeps_classifier_disposition` — the second injects an `uncertain` disposition rather than `not_delivered`, so an unconditional-upgrade regression cannot pass it |
| R5 | semantic actions | A failed `IsReadOnly` read returned a clean not-delivered instead of the classification every other call in that file routes through, so a transport failure against a possibly-mid-mutation app was reported safe to retry. | **Unpinned, and the reason is stated**: the unit seam replaces the whole enclosing function, and no live element in this suite exposes the pattern at all. Building injection scaffolding to reach it was refused as over-engineering |
| R6 | capture | The window capture computed its BGRA buffer length in `i32`. A window large enough to overflow it allocated less than `GetDIBits` then wrote — an out-of-bounds heap write, not a wrong image. Both capture paths now refuse an oversized capture by name before any GDI resource exists and allocate through checked arithmetic. | `oversized_dimensions_are_rejected_while_ordinary_dimensions_are_accepted`, and the display counterpart |
| R7 | capture | A bitmap still selected into a DC cannot be deleted, so a failed restore leaks it permanently — and the balance counter was released anyway, erasing the only evidence. | `a_failed_restore_of_the_previous_selection_is_not_recorded_as_released` |
| R8 | notifications | A per-entry property read swallowed every failure into a default. A recycled element produced an empty title, the entry was dropped as unnamed, and a filtered `dismiss-all` built its captured set from that list — reporting a clean clear while a real notification stayed behind. | **Unpinned, and the reason is measured**: two tests were written against a killed fixture process and both failed, because a property read on a held reference returns cached values rather than erroring on this build. They were removed rather than shipped asserting something that is not true here |
| R9 | notifications | The walk enforced two bounds and was honest about only one: exceeding the entry count surfaced as an incomplete read, exceeding the depth silently stopped descending and returned success. | `a_walk_past_the_depth_cap_surfaces_as_an_error_not_a_silent_stop` |
| R10 | core contract | The window selector filtered on accessibility before visibility and focus, so a hidden background window whose read happened to succeed beat the visible focused window whose read momentarily failed. | `shared_window_selector_prefers_the_visible_focused_window_over_an_accessible_hidden_one` |
| R11 | core contract | `get --property bounds` fell back to the snapshot rectangle when the live read succeeded with no bounds and said nothing about it, so a caller piping it into a physical click clicked the old location. | `bounds_marks_snapshot_fallback_as_not_live_when_a_successful_live_read_finds_no_bounds`, and its live-read counterpart |
| R12 | probe corpus | The redaction gate's pid rule matched only a JSON key, so a capture writing a pid inside a formatted line — the shape four committed tree dumps already carry — read clean. | A must-catch fixture in the embedded shape; the gate refused it before the rule was widened |
| R13 | probe corpus | The ledger check's evidence-area list is hand-maintained, and an area missing from it got no deletion protection at all — an omission that already happened once, for twelve sub-phases. | The gate's own run: removing one id makes it fail naming the unaccounted area |
| R14 | probe corpus | Two ledger passages claimed the checker enforces hunk-index bijection and an eleven-area floor. It enforces neither. A reader deciding whether CI would catch their omission was being told yes. | Corrected in place; the ledger's own rule is that a row reads true |
| R15 | probe corpus | The capability-probe workflow filtered on a probe script that has never existed in this repository. | Removed |

#### Refuted — reported as a finding, disproved by measurement

One reviewer reported as a P0 that the workflow steps invoking a nested
PowerShell gate script do not propagate its exit code, so a failing redaction
gate would show green and the leaking artifact would upload anyway. Reproducing
the Actions `powershell` wrapper locally against the same nesting returned exit 1
to the outer process. The wrapper appends an exit on the last exit code, which
GitHub documents. **No change was made, and the finding is recorded as refuted
rather than quietly dropped** — a reviewer being wrong is worth the same amount
of writing as a reviewer being right, because the next reader will otherwise
rediscover it.

Two reviewers independently rediscovered defects this gate had already fixed on
its own branch — the `terminal_code` ordering and the `--app` exact-match gap —
because they read the integration branch rather than the tip. Both are
corroboration, not new work.

#### Owned elsewhere — written into §2.16's scope in `docs/phases.md` in this PR

Nine findings, each with what was already measured about it, so §2.16's
implementer can act without reading this report: the clipboard worker that holds
the clipboard open past its caller's deadline; the menu surface and the menu
wait answering from different source sets while the code claims they cannot
disagree; two more fault-read-as-absence collapses of the shape R3 separated;
`set-value` advertised on a read-only range control; the budget arm of R4's fix,
left rather than fixed blind; the key synthesis that drops the layout's shift
requirement, which a US-layout rig cannot reproduce; the surface inventory that
discards what it already collected when one window hangs; and five e2e harness
legs that pass regardless of what they observe.

#### Found by CI, which is the one reviewer that could not be run locally

The macOS crate does not compile on the box this branch was built on, so no local
gate reaches it. Opening the PR was therefore the first execution of this branch's
macOS side, and it found two real failures on the first run — both now fixed, both
of a kind no amount of local checking could have caught:

- **`unused import: canonical_parts`** in the macOS blocked-combo tests, denied by
  `-D warnings`, which failed the whole macOS test build. The superset matcher this
  gate shipped stopped calling it from the test module and left it in the `use` line.
- **A compile-time `cfg(windows)` in `crates/core/src/commands/session.rs`**, which a
  CI rule forbids outside one allowlisted file. `activation_export` picked a
  PowerShell or POSIX export line that way, but the shell that matters is the one the
  caller pastes into. It is now a runtime branch, so both arms are compiled and
  type-checked on every lane rather than one being a hypothesis on the other platform
  — which is the reason the rule exists. **This one predates the branch**: the gate
  and the violation both exist at the merge-base, so that job has been red on
  `feat/windows-adapter`. Clearing it is what a hardening gate is for.

**CodeQL's two high-severity alerts are not this branch's, and are left standing.**
They point at assertion messages in `crates/core/src/commands/find_tests.rs`, a file
that is not in this diff, and the alerts were raised against `refs/heads/main` three
days before this PR opened. CodeQL says so itself — it warns that alerts it did not
introduce can surface when a diff is large enough. The rule is cleartext-logging
firing on an `assert!` message that interpolates a mock adapter's JSON. Silencing
`main`'s alerts from a Windows sub-phase branch would be the wrong fix in the wrong
place, so they are recorded here and left where they belong.

#### Accepted, with reasons

- **A duplicate-identity notification reports a successful dismiss as failed.**
  Identity is deliberately app-plus-title-plus-body so a mutable control value
  cannot break it, and this shell exposes no per-instance axis to add.
- **A session cleanup failure replaces an operation's success with the cleanup's
  error.** Intentional, and pinned by its own test.
- **The hang probe examines the first sixteen top-level windows of a process.**
  A bounded probe cost.
- **`gated_number`'s converse divergence is unfixed.** A boolean answer to a
  documented-integer property has not been measured, and this corpus measures
  before it builds.
- **A second write through `RangeValuePattern` can follow an unverified
  `ValuePattern` write.** Reported as having no envelope signal; that half is
  wrong — the step list carries both rungs and ships in the response. The
  clamping risk is real but the alternative leaves the caller with less.
- **The ownership guard on `AGENT_DESKTOP_HOME` is a no-op off unix.** It
  rejects a foreign-owned state root on macOS and Linux and accepts one on
  Windows. Writing a Windows ACL check into core would put platform code on a
  path this repository has already been burned by once.

### Runner registration and the live e2e lane

The plan's one hard external dependency was owner authorization to register a
self-hosted interactive Windows runner. It was declined, and the owner further
directed that the live e2e lane be removed from CI. **R20, R21 and R22 are
dispositioned as retired, not deferred** — there is no receiving sub-phase and
no infrastructure waiting to be provisioned.

The reason is not scheduling. A self-hosted runner's labels are reachable from
every `pull_request`-triggered workflow in the repository, not only from the one
file that names them, so a fork PR editing any of those workflows is code
execution on the owner's interactive desktop. `windows-e2e.yml` is deleted along
with the static assertion that policed its capture upload, and the one queued run
that had accumulated against it is cancelled.

**The live suite still runs, locally and on demand**, under the exclusive desktop
lease on a box holding the interactive session — which is where every scenario in
it has actually been verified since the harness landed. CI keeps everything that
needs no desktop and already covers the Windows surface: the core and Windows
library tests on both `windows-latest` and `windows-11-arm`, the x64/ARM64 parity
job, clippy over the Windows crates, the example tests, the e2e contract gate and
its self-test half, the seeded-failure run, the refusal-guard self-test, and the
capture-redaction and ledger-citation gates.

## Part 3 — Performance baseline

Taken by the Windows vehicle — the probe corpus cost methodology: seven runs, the
warm-up discarded, reported as min with median and max beside it — through the release
binary, against the merge-base and this branch's tip in turn, with the same script
pointed at each binary by path. Recorded as A28-10 and committed as
`probes/windows/captures/27-cost-baseline/cost-baseline-{devbox,mergebase}.json`.

| Command | Merge-base (min / median) | Tip (min / median) | Delta |
|---------|---------------------------|--------------------|-------|
| `snapshot` | 119.3 / 181.5 ms | 117.4 / 184.6 ms | flat |
| `list-apps` | 37.4 / 39.6 ms | 37.5 / 42.1 ms | flat |
| `list-windows` | 59.5 / 80.7 ms | 54.1 / 66.1 ms | flat |
| `status` | 58.6 / 114.2 ms | 66.2 / 112.6 ms | flat |
| `list-displays` | 33.1 / 34.2 ms | 36.1 / 68.3 ms | flat |
| ref action, live target | 227.3 / 309.6 ms | 196.7 / 243.7 ms | flat |
| **ref action, dead target** | **5060.5 / 5114.9 ms** | **59.0 / 82.1 ms** | **86x at the min, 62x at the median** |

The dead-target result is the one this baseline exists to check, and it holds. The old
path polled out the whole default wait budget before failing; the new one answers
terminally on the first resolution attempt. The live-target leg is deliberately
unchanged — a live owner keeps the retryable path, and a drop there would have meant the
fix was too broad.

**Two gaps in the measuring harness were closed to take this.** The snapshot leg wrote a
silent `null` because it looked for a window handle this host's probe process never has,
and no ref-action leg existed at all — so the command this gate changed most was the one
command the baseline could not see. A baseline that cannot measure the thing under test
is the same defect class as a test that cannot fail.

**The macOS baseline is not taken, and that is recorded rather than skipped.**
`scripts/perf-baseline-compare.sh` is structurally macOS-bound — it opens the `.app`
fixture bundle — so this box cannot run it. Three changes in this gate land in shared
core and are therefore visible on macOS: the actionability verdict's severity ordering,
the two error-envelope corrections, and the window selector's tier order. Each is a
branch-selection change on a path that was already being walked, not new work per call,
so no macOS latency delta is expected; that is an argument, not a measurement, and it is
labelled as one.

## Part 4 — A second stranger run, and what it found

An independent agent was handed the skill and the binary and asked to write a
poem in Notepad and save it to `C:\`, headless and without keyboard input. It
succeeded. Triaging its transcript produced four findings and two non-findings,
and the non-findings are recorded because two of them were mine.

**S1 — the agent was reading a stale skill.** *Fixed here.* `.claude/skills/`
held real directory copies of the Windows and FFI skills alongside a symlink for
the shared one. The copies are gitignored, so they rot while the sources move:
the Windows copy still advertised `list-surfaces` and the notification commands
as `PLATFORM_NOT_SUPPORTED` — corrected two sub-phases earlier — and carried
neither the PowerShell quoting warning nor the save recipe. That accounts for
most of the run's stumbles. All three are symlinks now.

**S2 — `--skeleton` offered drill targets that could not be drilled.** *Fixed
here.* An anonymous boundary earns its ref from geometry, and allocation then
dropped the rect from the persisted entry unless the caller had asked to *see*
bounds — a presentation flag deciding whether a ref resolves. The entry kept its
bounds hash, which looks sufficient and is not: geometry promotion needs a
positive-area rect as well, because zero-extent elements share hashes (A17-7).
Measured: three of four anonymous anchors answered `STALE_REF`, and all four
resolved when the same snapshot was taken with `--include-bounds`. After the fix,
none fail. Tests: `unlabeled_bounded_boundary_gets_drill_ref` (assertion
corrected — it pinned the stripped rect) and
`a_named_bounded_boundary_still_has_its_rect_stripped`, which stops the fix
reading as *always keep bounds*.

**S3 — `find` had no traversal budget.** *Fixed here.* Fixed at five seconds
with no flag, while `snapshot` beside it has always taken `--timeout-ms`. On a
shell file dialog that simply had no answer but to fail. Measured after: 800 ms
fails at 1093 ms, 5000 ms answers in 4096 ms. Tests:
`an_explicit_budget_reaches_the_traversal_and_absence_keeps_the_default` drives
the same function `execute` calls — a first draft restated its arms beside it
and passed while the flag was still ignored.

**S4 — that timeout blamed the application.** *Fixed here.* It suggested the
target may be busy or unresponsive; the app was healthy and the tree was large.
`find` now supplies its own suggestion naming the two levers a caller has. The
shared constructor is deliberately untouched — every other command reaches it,
and a test elsewhere asserts on its wording, so fixing one caller's message
there would have rewritten every command's envelope. Wiring is pinned by a
zero-budget run through `execute`, because the direct test of the mapping
function does not notice the mapping being removed.

### Recorded as not-findings

- **`click` on a menu-bar item opening the wrong menu.** Observed once in the
  transcript. Re-ran the identical sequence **five times and got the right menu
  every time**. One observation is not a defect, and a claim measured in one
  direction names whichever cause happened to be present.
- **My own first two explanations of S2.** I called it a structural contract
  violation on anonymous boundaries, then a settle-time race. Both were wrong;
  the same nodes passed and failed under conditions neither explanation
  predicted. Only the third measurement — toggling `--include-bounds` — held up.
  Recorded because the gate now requires measuring a claim in both directions
  before writing it down, and this is what that rule is for.

## Part 5 — A second full-branch review, and its dispositions

A further multi-persona review ran over the assembled diff. Its findings and
what became of each:

**R16 — `get --property text` does not return what the reference promised.**
*Fixed as documentation here; the contract decision is owned by §2.16.* `text`
and `value` are byte-identical reads, and the accessible name is reachable only
through `title`. Measured on a real control: a button named `Close` answers
empty for `text`, empty for `value`, and `Close` for `title` — and `text` is the
**default**, so it is the first thing a caller reaches for. The reference now
describes what ships, says plainly that a labelled control's `text` comes back
empty and `title` is the one that answers, and
`text_reads_the_value_and_title_reads_the_name_as_the_reference_states` pins it:
making `text` name-preferring fails that test, which forces the wording to move
with the code. **Changing the read was declined here on purpose** — it flips
every named textfield's `text` from content to label on both adapters, a larger
behavioural change than any normalization this gate shipped, for a defect no
dogfood and no stranger run ever hit. A reviewer found it by reading.

**R17 — find's flags were undocumented.** *Fixed here.* `--timeout-ms`, added
earlier in this gate, and `--window-id`, which predates it, were both missing
from the observation reference's flag table. A flag that ships undocumented is a
flag nobody uses.

**R18 — the stale-ref gate had no self-test.** *Fixed here.* Its two sibling
gates each carry one; this one policed the tree while nothing policed it. Its
counting is not trivial — it skips `*_tests.rs` files and brace-tracks inline
`#[cfg(test)]` items, and its own comment admits the tracking could desync.
Three committed fixtures now pin the three behaviours, including the one the
brace tracking exists for: a gated caller skipped without losing the production
caller that follows it. The self-test is itself invert-verified — removing that
trailing caller makes the gate refuse to scan the real tree at all.

### Declined, with reasons

- **`FindArgs` exceeds the seven-field limit.** *Accepted.* It was already at
  eight before this gate; the traversal budget took it to nine. Splitting it
  touches the CLI argument mirror, dispatch, and nine construction sites, across
  three layers, to satisfy a ceiling the struct already broke. The split belongs
  to whoever next adds a field.
- **Test helpers described as dead code in `find.rs`.** *Not a defect.* They are
  `#[cfg(test)]`-gated, so they do not ship, and the compiler reports no dead
  code. Verified rather than assumed.
- **The `wait_event` baseline clone and `app_name_matches`' redundant branch.**
  *Raised again, declined again, same grounds* — one clone per poll on a loop
  that sleeps far longer than the allocation costs, against restructuring a
  borrow inside correctness-sensitive wait logic; and a case-sensitivity claim
  that is unreachable because the comparison is already case-insensitive. Noted
  as repeats so a third review need not re-litigate them.
- **A trivial wrapper, and files sitting near the 400-line cap.** Observations,
  not work. The cap is a limit, not a target to stay far from.

