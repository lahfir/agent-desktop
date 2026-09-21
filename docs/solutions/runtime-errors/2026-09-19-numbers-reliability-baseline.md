# Numbers reliability baseline: direct model-facing CLI

Status: the follow-up timed sprint reached **7.5/10 provisional engineering
confidence**, from the user's 6/10 baseline. The full repeated acceptance gate
remains incomplete; this is not a measured overall success rate. Earlier results
below are retained chronologically, including failures.

The [native initialization follow-up](2026-09-20-native-accessibility-initialization.md)
repairs missing cells after a cold launch. Text replacement and commit remain unmet.

The subsequent [Xcode evaluation](2026-09-19-xcode-headless-evaluation.md) adds
verified persistent editor changes, an absent-live-bounds repair, and shared
deep-menu discovery/inventory repairs. It does not replace the Numbers gaps.

## Boundaries

The evaluation uses disposable `AD Reliability.numbers`, with duplicate
`Eval Grid` tables on two sheets and known guard values. Setup scripting and
independent document reads are not counted as Agent Desktop task completion.
After the user clarified the evaluation contract, all target selection used
unmodified public CLI responses. No external JSON extractor selected refs.
The earlier Python-filtered discovery output is not caller-facing acceptance
evidence. TextEdit exploration is excluded; the active evaluation is Numbers.

Baseline binary SHA-256:
`d8e6e24ed24d8a8f99233880bd9317abbd8220cf6c58cf63d3989335d45ce5b2`.
It includes the preceding Chromium activation repair. Run artifacts and the
baseline diff are local under `/tmp/ad-native-eval`.

## Observations

| Case | Evidence | Status |
|---|---|---|
| N1 discovery | Raw skeleton returned 79 refs; direct table drill exposed rows/cells | Usable, noisy; no compactness claim |
| N2 cell click | Initial click timed out at the CG/AX window bridge before dispatch; later same ref succeeded and entered edit mode | Mixed; failed attempt retained |
| N3 original cell ref after click | `seed-target` became `seed-target, Is Editing`; original ref-root drill returned STALE_REF | Unresolved identity gap |
| N4 editor replacement | `set-value` returned ACTION_FAILED after AXValue success with unchanged text | Task failed; false verified claim avoided |
| N4 editor insertion | Separate `type` trial returned ACTION_FAILED after AXSelectedText success with unchanged text | Task failed; no physical fallback |
| N5 formula | Headless cell editing not established | Not run; dependent task blocked |
| N6 sheet switch | AX click changed active sheet; independent model confirmed both guard cells intact | Switch worked |
| N6 command agreement | Same ref: `is checked=true`, `get states=[]` | Proven core defect; repaired |
| N6 read resolution | `get` repeatedly timed out at approximately 750 ms while `is` later resolved the same ref | Proven unused core deadline budget; repaired |
| N7 table dimensions | Original incrementor ref changed 8→9→8 rows; document model confirmed 9 and guard values after first change | Positive live evidence, no 30-run claim |
| N8 persistence | Background Save menu was reported disabled; close/reopen task not completed | Not run to completion |

Raw AX independently reopened the unchanged B2 editor and wrote AXValue. Native
return was success, but the before/after value remained `seed-target`. The editor
advertises writable AXValue and AXSelectedText. This establishes a native no-op
under the tested conditions, not an adapter-only defect or proof that no allowed
semantic path can ever work. Independent Numbers reads confirmed B2 and C2
unchanged. Raw AX is a diagnostic path, not a successful substitute action.

Window availability was intermittent: raw AX also failed to find the owned
window during one failure, while the document model still reported it visible.
Later both readers found it. Startup/lifecycle/native availability remains
unresolved; do not attribute all empty listings to Agent Desktop.

## Repairs and blast radius

1. `get` used saved ref states and bounds after resolving the live element.
   It now uses the same adapter live-state/bounds reads used by `is`. Native read
   failures propagate; the existing fallback for adapters without live support
   is preserved. No new state cache or command-specific platform reader exists.
2. One-shot ref resolution used a 750 ms polling slice, but never polled.
   It now passes its existing caller deadline to strict resolution. The shared
   helper also serves pointer preparation, so that path is included in workspace
   regression coverage. No deadline is enlarged beyond the caller's budget;
   action polling retains its original slices, identity rules and retry safety.
3. Surface array reads bypassed the existing bounded read-recovery helper used
   by tree traversal. They now reuse it for AXWindows, menus and sheets. Recovery
   is limited to three read attempts inside the original absolute deadline;
   permission denial and invalid native handles remain terminal. No mutation
   retry behavior changes. A surface-boundary regression fails with the previous
   one-shot read and passes with recovery; additional tests cover terminal errors,
   empty windows, expired budget and retained native failure telemetry.

Six deterministic regressions cover checked-state transitions in both directions,
live geometry, unsupported-read compatibility, failed-read propagation, caller
budget preservation, and no native resolution after expiry. State/error tests
failed against the old getter. The budget test explicitly checks effective
capping rather than the nominal timeout label and failed against the old helper.
No test sleeps or browser/app process is required by these regressions.

The candidate's public batch response on the original Numbers sheet ref now
returns `get states=[checked]`, `is checked=true`, and `get value=true` together.
This proves the observed contradiction is fixed; it does not establish all-command
consistency or complete Numbers automation.

The reverse live transition also passed: the original Sheet 1 ref returned empty
states and `checked=false`, while Eval Other returned `[checked]` and
`checked=true`. The independent document model reported Eval Other active and
both sheet guard values unchanged.

Validation: the full workspace test run passed (2,237 reported passes, four
ignored, including subprocess test invocations); Clippy with warnings denied,
formatting and diff checks passed. The candidate executable is 3,072,144 bytes,
SHA-256 `70154de6eb4e089885736c2266c2895f4f7eac52dd79d46700616a93cc4c9af2`.

The required performance comparison ran against main with passive Numbers
observations and the activating fixture skipped. All 24 observation commands
succeeded with matching snapshot shapes. Three-round baseline/candidate median
milliseconds: interactive snapshot 1322/699, skeleton 469/500, depth-30 734/810,
first-button find 463/492. These small samples and native timing variability do
not prove universal performance equivalence or a speedup. The changed getter
intentionally performs live reads where it previously returned cached fields.
Report: `/tmp/ad-native-eval/performance/report.html`.

## Remaining evidence required

Visual follow-up: earlier editor probes lacked screenshot verification. A new
controlled attempt captured the owned window before and after requesting
`visual-check-0919` through `set-value @srksb992r2n4y:e1`. Both screenshots show
`other-sheet-guard` in Eval Other / Eval Grid / B2; the document model and AX
readback agree. That visible string was inserted by fixture setup, not this
replacement. The CLI returned ACTION_FAILED with delivered_unverified and unsafe
retry after AXValue reported success. This attempt establishes no visible change
at the observation times; it does not prove every Numbers editing path fails.
Artifacts: `/tmp/ad-native-eval/numbers-visible-check.png` and
`/tmp/ad-native-eval/numbers-after-visual-check.png`.

Further native diagnosis on Sheet 1 / Eval Grid / B2: AXSelectedTextRange changed
from `{11,0}` to `{0,11}` and AXSelectedText became `seed-target`. A subsequent
raw AXSelectedText write requested `selection-probe-0919`, returned native success,
but after two seconds still exposed `seed-target`. The screenshot and document
model also retained the seed and unchanged neighbor. This rules out an empty
selection as the sole cause for this trial. Screenshot:
`/tmp/ad-native-eval/numbers-after-selection-replace.png`.

The preceding CLI AXPress returned `APP_UNRESPONSIVE`, delivery_uncertain and
unsafe retry, but fresh raw AX found the editor open. It was not retried. This
is direct evidence that native timeout does not mean non-delivery. Two earlier
find calls failed while reading AXWindows; subsequent raw AX and CLI reads
recovered. Screenshots confirmed the owned window remained present. The surface
read repair addresses the missing bounded recovery, but does not establish that
every observed native stall fits within the caller deadline.

One sheet transition passed same-ref get/is consistency. Six malformed batch
requests were rejected before execution because the caller used `ref` outside
`args` instead of `args.ref_id`; these are driver mistakes, not completed trials.
No repeated acceptance series has been completed.

The surface-recovery candidate passed the full workspace suite, Clippy with
warnings denied, formatting and diff checks. Release size remains 3,072,144
bytes; SHA-256 is
`862804a33c4aeb4864a5fc182e8b9f1e0473fad1d6f844746c5aefba2d5eec9f`.
A live candidate find still failed after repeated AXWindows read errors, while
a subsequent window-scoped snapshot succeeded with 168 refs and complete=true.
Do not describe bounded retries as a complete fix for native availability.

The follow-up required performance run completed all 24 observation commands,
but interactive snapshot shapes differed (candidate 4 nodes/1 ref versus base
84 nodes/79 refs in the recorded sample). Therefore its latency comparison is
inconclusive. Skeleton and depth-30 recorded shapes matched. Preserve the report
at `/tmp/ad-native-eval/surface-performance/report.html`; investigate scope and
completeness before making a performance-equivalence claim.

A separate raw AXWindows timing probe alternated 250 ms and one-second timeouts
across count, paged-value and single-value reads. All 18 calls succeeded; the
slowest was approximately 56 ms. This did not reproduce the slow-read hypothesis
and does not justify changing the shared IPC slice. Numbers already reported
AXEnhancedUserInterface=true and AXFrontmost=false; AXManualAccessibility was
unsupported. No accessibility-mode or foreground-state write was made.

## Repeatability runner and fresh-process trial

`tests/real-apps/numbers-sheets.sh` consumes manually selected qualified refs,
prints/saves original CLI JSON, screenshots every transition, checks both sheet
values with public waits, and stops on failure. It never parses responses or
replays a failed mutation. Its failure-stop regression and shell syntax check pass.
This is a command runner requiring evidence review, not an automatic success judge.

The first ten-transition attempt stopped on its first click with native
`kAXErrorCannotComplete`, delivery_uncertain and unsafe retry. Its immediate
screenshot still showed Eval Other; a later independent document read showed
Sheet 1 active with both guards intact. Keep this as a failed timely command
with a subsequently observed effect, not a clean transition or safe retry.
Evidence: `/tmp/ad-numbers-sheets.st8KXa`.

A one-second process sample found the main thread waiting in the normal event
loop and a HIServices support thread suspended beneath
`SOME_OTHER_THREAD_SWALLOWED_AT_LEAST_ONE_EXCEPTION`. This is diagnostic evidence,
not a demonstrated cause. Only the owned document was open. It was closed for
fixture reset, the empty app quit, and the seed reopened with activates=false.
The foreground PID remained 860 at both endpoints. New Numbers PID 72620 and
window w-16255 replaced PID 19934/window w-15245; old refs must not be reused.

In the fresh process, screenshot-visible cells were absent from both CLI find
and raw AX; AXEnhancedUserInterface was false. Enabling the advertised flag
returned -25208 but read back true, and the cell became discoverable. The next
headless CLI cell click completed in 143 ms with delivered_verified. A new editor
set-value attempt requesting `fresh-process-0919` still failed verification:
AX, screenshot and document model retained `seed-target`, with neighbor unchanged.
Images: `/tmp/ad-native-eval/fresh-before.png` and
`/tmp/ad-native-eval/fresh-after-set.png`.

The follow-up flag off/on comparison was not a reversible grid-hiding A/B:
after initialization, cells stayed exposed with the flag false. Their names
changed to include table/row/column context when false, and shortened again when
true. Mode was restored to true; Numbers remained non-frontmost. This supports
an initialization/identity-mode hypothesis, but is insufficient to enable enhanced
mode for every native app or claim it fixes text insertion. Repeat cold starts
before changing the shared activation policy.

A second cold start (PID 77926, window w-16288) exposed the grid immediately:
the first skeleton returned complete=true, 80 refs and the Eval Grid table.
Enhanced mode read true without a diagnostic write in that process. Consequently
the first cold-start observation is not a reproducible activation defect. Do not
broaden native activation based on it. Initialization timing and other AX clients
remain uncontrolled factors.

## Goal completion audit

The requested 10/10 native-app reliability is not achieved. The three shared
repairs have deterministic regressions and passing workspace checks, but this
does not establish complete task coverage. The sheet repeatability run failed;
cell replacement failed in both direct AX and CLI trials across process resets;
formula entry and saved/reopened edited content remain uncompleted. Stable cell
identity across editing and continuous headless monitoring also remain unproven.
No release, commit or push was performed.

The essential external blocker is a working semantic text-write/commit path for
Numbers under the required background conditions. AXValue and AXSelectedText
advertise support and report native success without the requested value becoming
visible or appearing in the document model. Verified full-range selection did
not repair replacement. A working independent raw AX reproduction or a change
in Numbers' accessibility behavior is needed before an adapter fix can honestly
claim to solve this operation. No physical input, focus stealing, application
scripting writes or speculative private-API fallback was added to the product.

The first follow-up document query used `Sheet1` and failed. The actual name is
`Sheet 1`; a fresh inventory and corrected read returned `seed-target`,
`neighbor-guard`, and `other-sheet-guard`. Do not treat that failed oracle query
as evidence about document contents.

- Repeat supported cases from clean seeds and test both sheet transitions.
- Resolve stable grid identity before relaxing any name-based matching rule.
- Diagnose Numbers headless editor/commit prerequisites through matched raw AX
  trials; do not replace them with keyboard synthesis or scripting writes.
- Establish save/reopen and formula completion, or retain them as unmet coverage.
- Complete continuous foreground/input monitoring. Before/after samples and
  semantic-only command traces do not prove absence of transient side effects;
  concurrent user pointer movement prevents a global pointer-stability claim.
- Report full test/performance results separately from task completion. The
  planned 30-run, three-reset-batch acceptance gate has not been satisfied.

## Twenty-minute sprint, 2026-09-19

Timebox: 23:37:59–23:57:59 UTC. Score progression: 6.0 initially, 6.5 after
deterministic core repairs, 7.0 after repeated sheet transitions and workspace
validation, 7.5 after independently verified row transitions. The requested
8/10 acceptance level has not been established.

Six additional failure classes were repaired with failing-before/passing-after
regressions:

1. `get text/value` no longer substitutes stale snapshot content when a supported
   live reader reports an absent value. Unsupported-reader fallback remains.
2. `is --enabled` respects explicit live enabled evidence; unknown is reported
   as inapplicable rather than optimistically enabled, consistent with waits.
3. `wait --element` uses the shared ref loader, so a missing pinned ref produces
   the same STALE_REF recovery contract as other ref commands.
4. Ref resolution and wait predicates reject successful observations arriving
   after the caller's deadline. Tests expire the supplied deadline rather than
   asserting fragile elapsed-time thresholds.
5. Actionability polling honors an explicit non-retryable disposition.
6. macOS live observation recovers transient CannotComplete reads up to three
   attempts inside the original deadline. Permission, stale-handle, unrelated
   incomplete evidence and explicit non-retryable errors remain terminal.
   Recovery only repeats reads; it never repeats a delivered mutation.

The sixth case was reproduced live: a row-count change took effect in Numbers,
but the command failed on its post-action read. Screenshot and document-model
inspection confirmed nine rows. No blind mutation retry was attempted. The
candidate recovered observation in deterministic injected tests; the subsequent
live series passed, without establishing that all native stalls are recoverable.

Final immutable binary: `/tmp/ad-native-eval/sprint-final`, SHA-256
`7ede8cb02945a561033f01f9688e7b74b1a36b3b6b3d24f552e20c3afb74d0d4`.
Size: 3,072,144 bytes. No production changes followed this build.

Live evidence on the owned Numbers document:

- Ten sheet transitions passed using the original qualified refs. Public wait,
  get and is responses agreed. Independent final document reads confirmed the
  active sheet and unchanged guards. Raw JSON and ten screenshots are retained
  under `/tmp/ad-numbers-sheets.BCbdHd`; first/last pairs were visually reviewed.
  Runner labels are positional: `sheet-1` files show Eval Other and `other` files
  show Sheet 1 because those refs were supplied in that order.
- Ten row transitions (8→9→8, five cycles) passed using the original incrementor
  ref. Every change was followed by public wait/get and an independent read-only
  document-model check of row count and both guard values. Runner:
  `/tmp/ad-native-eval/sprint-rows.sh`. Final screenshot was visually reviewed:
  `/tmp/ad-native-eval/sprint-rows-final.png`; eight rows and guards are intact.

Full workspace tests passed with native process visibility, Clippy with warnings
denied passed, formatting/diff checks passed, and core dependency isolation held.
The initial sandbox-only full run failed two process-level CLI tests during
native preflight; the native-visible rerun passed. The separate headless semantic
fixture gate passed seven assertions, including a silent no-op and exactly-once
effects. These fixture results do not replace real-app coverage.

Final required performance comparison: five rounds, 40/40 successful observations
with matching recorded shapes. Base/candidate median milliseconds: interactive
snapshot 613.5/562.1, skeleton 371.0/372.5, depth-30 529.2/551.7, first-button find
347.8/346.4. Report: `/tmp/ad-native-eval/sprint-final-performance/report.html`.
The activating benchmark fixture was skipped; semantic fixture tests ran
separately. Small samples do not establish performance equivalence.

Remaining gaps: cell replacement/commit, formula and save/reopen completion,
stable grid identity across editing, repeated reset batches, a second complex
native app, and continuous foreground/pointer monitoring. A transient snapshot
offscreen/geometry discrepancy was observed but not reproduced under controlled
conditions; no speculative geometry change was made. Twenty successful warm
transitions across two operations are useful evidence, not universal reliability.
All changes remain local on `codex/fix-chromium-ax-activation`; no release or push.
## Follow-up: semantic commit probe, September 20 UTC

The owned Numbers process/window remained PID 77926 / w-16288. A fresh public
find returned B2 as `@s2gs3slvqloag0:e1`. One click using the visibility candidate
returned uncertain AXPress delivery; no retry was made. Fresh find and the
screenshot `activation-numbers-editor.png` confirmed the B2 editor was open.

Using immutable `activation-candidate` (SHA-256
`49c75115e6600306ee42947484bc0982126585c0a137909f3d9745740fe84ab7`),
`set-value @s2nelupcc724hz:e1 commit-probe-0920` returned ACTION_FAILED:
AXValue reported success but post-state still contained `seed-target`.
A separately selected neighbor ref, `@s3qfcpezf9cbvy:e1`, was clicked once
to leave B2 semantically and test whether a commit would expose the replacement.
This click returned delivered_verified. The independent document read returned
eight rows, B2=`seed-target`, C2=`neighbor-guard`. Screenshot
`/tmp/ad-native-eval/numbers-after-semantic-commit.png` agrees and shows C2 selected.

Result: **0/1 replacement-and-leave-editor trials passed on this candidate**.
The setter failure cannot be explained solely by omitting a subsequent cell
selection in this trial. Formula entry and persistence remain unmet. The new
Xcode menu verification repair must not be counted as a Numbers text-edit pass.
No synthesized input, focus activation or document-scripting write was used.

The independent probe now records app-level focused-element ancestry. After
the neighbor click, AXFocusedUIElement was an AXTextArea whose immediate parent
was `neighbor-guard, Is Editing`, then row 2 and Eval Grid. The app exposed
this hierarchy with Numbers in the background (foreground PID 860 at both
endpoints). Thus this observation does not justify increasing the adapter's
three-level focus ancestry walk or replacing semantic input with key events.
