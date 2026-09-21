# Native reliability: real-app baseline and minimal repairs

Date: 2026-09-19
Status: active; bounded live trials completed, repeated acceptance suite incomplete

## Outcome and boundaries

Make snapshot → qualified ref → supported headless action dependable in real
macOS apps. Recover routine target changes internally when identity is sufficient;
never choose a lookalike or repeat a possibly delivered action to improve a score.
The calling agent still owns task intent and the observation/action loop.

Use Numbers and disposable documents as the active real-app evaluation. Do not
spend this phase on TextEdit or other easy-app demonstrations. Keep the existing
fixture to reproduce discovered failures, not as proof of real-app completion.
All mutations target owned test documents/windows only. No activation, physical
pointer movement, synthesized keyboard/mouse input, or clipboard-based typing.
A visual cursor overlay is allowed; its animation is not delivery evidence.

This phase adds no app-specific production automation, browser backend, generic
workflow engine, dependency, fuzzy identity matcher, or replacement test framework.
No publishing, commit, or push is part of this plan. Preserve existing Chromium
activation edits and unrelated assets/icon.png. Record the exact dirty diff and
binary hash; do not label an uncommitted build only by its HEAD commit.

## Current assessment

Initial provisional engineering confidence: **6/10**, not a measured completion score.
Strong foundations exist: strict resolution, shared actionability/auto-wait,
delivery dispositions, verification scopes, and native fixture coverage.
Real-app completion, persistent document mutation, and failure recovery across
UI changes are not yet demonstrated by a frozen repeatable suite.

The earlier **8/10 provisional assessment is withdrawn**: it was not earned by
the declared acceptance suite. Current measured overall score: **not established**.
The execution trajectory and frozen tasks are in
[acceptance v1](../../tests/real-apps/acceptance-v1.md).
Current evidence and failures are retained in the
Numbers, Xcode and native-command reports under `docs/solutions/runtime-errors/`.
Numbers editing, formulas and persistence remain required even as command coverage
expands to other complex apps.

## Additional command coverage authorized during execution

Use owned SF Symbols and Xcode windows alongside Numbers. Evaluate scrolling and
saved-ref continuity, scroll-to, named selection, menu discovery, expansion and
collapse. Apply the same headless and independent-outcome requirements. Record
drag's policy refusal separately from actual drag-and-drop completion; a correct
refusal cannot satisfy the requested drag task. Do not add physical input to make
an unsupported headless command pass.

Menu trials must observe app activation through the entire delivery and settling
period. Native AX success and menu presence do not by themselves prove headless
delivery. If unrelated foreground activity prevents attribution, mark that trial
unqualified and retain the events rather than infer a clean pass.

Prior evidence is a hypothesis source, not a current failure inventory:
- The September 16 Numbers probe reported chooser naming, selection, startup,
  mutation-timeout and editor-write problems.
- Current code already contains selection-aware presses, AXSelectedCells, and
  separate read/mutation timeout slices. Reproduce before proposing changes.
- Historical Numbers editor writes returned AX success without committing text;
  physical typing worked but violates this task's headless contract.
- The current Comet repair demonstrates that readable AX plus native success
  can still require renderer activation. It does not prove native app reliability.

## Caller-facing acceptance constraint

Use unmodified Agent Desktop responses for model-driven interaction. No custom
Python/JSON tree extraction, external ref selection, hidden target repair, or
application-specific wrapper in the calling workflow. Use documented snapshot,
skeleton, drill, find, action and verification commands directly. Record response
size, call count, missing identity/context and recovery instructions as usability
evidence. Raw AX and document-model probes are separate diagnostic/oracle paths;
they must not choose refs for or silently rescue the evaluated interaction.
Automated replay alone does not establish that a model can operate the CLI.

## Phase 1 — freeze the experiment before repairing behavior

1. Build/copy an immutable release binary and isolate AGENT_DESKTOP_HOME, session,
   trace and output paths. Record commit, diff hash, binary hash, macOS/app versions,
   permissions, process instance, window identity and initial accessibility modes.
2. Audit/reuse tests/e2e interaction locking, bounded subprocess execution, JSON
   parsing, safe-semantic guards and traces. Add only the small real-app scenario
   runner/result record that is missing. No dashboard or driver/plugin abstraction.
3. Identify/create disposable seed documents. Setup may use native document
   scripting strictly to prepare owned fixtures; report it separately and never
   count it as Agent Desktop task success. Setup must also preserve headless policy.
   If a safe seed cannot be prepared without focus changes, report that prerequisite
   blocked; do not silently foreground the application or touch existing documents.
4. Establish an independent oracle before scoring a case: raw AX selection/state
   reads and, for Numbers content/formulas/dimensions, read-only document-model
   queries. Capture the pre-state and verify the oracle detects a known different
   seed value. Do not assume AX text readback means document commit.
5. Run each frozen case once for discovery, then ten clean baseline repetitions.
   Keep every attempt. A failed run followed by a successful rerun stays a failure.
   Version case definitions when evidence requires a corrected test.

## Initial cases and outcome oracles

| ID | Scenario | Required effect |
|---|---|---|
| N1 | Discover Numbers document, skeleton, drill to table | Correct owned document/table and actionable identity evidence |
| N2 | Snapshot cell, select via ref with Numbers background | Raw table selection identifies that exact sheet/table/cell |
| N3 | Change selection or scroll, then act on saved ref | Original target recovered, or explicit ambiguity/removal result |
| N4 | Enter unique text and numeric values, commit editor | Document model has exact values in exact cells; neighbors unchanged |
| N5 | Enter formula over known inputs | Stored formula and calculated result both match |
| N6 | Switch sheets with repeated table/control names | Correct sheet changes; no writes to the lookalike table |
| N7 | Change table dimensions through exposed semantic control | Document row/column counts change, not only displayed AX text |
| N8 | Save, close owned document, reopen | Values/formula persist; no other document altered |

Freeze exact step recipes, timeouts, seed checksums and oracle expectations after
capability discovery and before the repeated baseline. Unsupported behavior remains
visible as unmet task coverage; it is not silently dropped from the denominator.
Numbers template selection is a separate capability probe if the grid is absent
from AX; do not make unrelated cases depend on an unobservable chooser selection.

## Cross-command consistency gate

Test public command relationships directly using their unmodified responses:
- list-windows → snapshot --window-id → find in that window → ref action must
  preserve window ID and process generation. Different window capabilities
  (listed, readable, actionable) must be explicit rather than inferred identical.
- snapshot/find refs → get/is/root drill/action must identify the same element
  within its owning session. Mutable control state must not masquerade as stable
  identity; genuine replacement or ambiguity must fail safely and consistently.
- Advertised available_actions → matching command preflight and dispatch must
  agree for unchanged state and the same headless policy. Record native advertised
  support without observable effect as an unmet capability contract, not a pass.
- click/toggle/check/uncheck and type/set-value/clear must share applicable
  capability, resolution, delivery and verification rules; preserve intentional
  semantic differences such as replacement versus insertion.
- CLI/batch/high-level FFI must use the same core contract. Reuse existing
  window inventory/resolution, native capability readers, ref resolver and action
  execution rather than creating an all-purpose new registry or parallel helpers.

Classify disagreement under unchanged conditions as a defect. Record explicit
intervening UI changes separately; do not call every dynamic-state change a
contradiction or conceal a transient read failure as an empty successful result.
For each actual divergence, fix the lowest existing shared boundary and add a
paired-command regression proving agreement and a changed-state negative case.

## Phase 2 — assign every failure to the correct boundary

Use matched clean-state trials, never an AX retry after a possibly delivered AD
mutation. Compare the same logical element, native role/attributes/actions,
selection/editor state, foreground state, timeout and accessibility mode.

| Evidence | Diagnosis and permitted next step |
|---|---|
| Raw tree exposes target/capability; AD omits or mislabels it | Inspect macOS traversal/mapping; if raw adapter output is correct, inspect core filtering/ref allocation |
| AD resolves a different target or rejects identity | Compare saved evidence to live identity; separate adapter resolution from core ref storage/scope rules |
| AD never dispatches although target is supported and ready | Trace core preflight/policy/wait decisions and adapter live-read answers |
| Same target and preconditions: raw semantic mutation works, AD native dispatch fails | Adapter mechanism, preparation, timeout or verification candidate; reproduce before editing |
| Native dispatch works but AD reports the wrong disposition/effect | Trace adapter classification and core postcondition handling separately |
| Raw and AD both return success with no effect | Investigate AX prerequisites/commit semantics; not enough evidence to blame either implementation |
| Raw and AD both cannot perform operation | Record current platform limitation or unresolved prerequisite; no physical fallback |
| Saved result differs from AX field readback | Commit/oracle gap; element verification must not claim document persistence |

Raw AX probe is a small independent Swift helper using ApplicationServices, not
Agent Desktop internals or its normalization. Capture attribute/action names,
settable flags, native errors, selected target identity, elapsed time and pre/post
state. Allow mutations only in the owned document. Read-only scripting is an
oracle, never the evaluated action path. Change one precondition at a time and
repeat successful/failed conditions before attributing causality.

If core vs adapter remains unclear, use a minimal adapter-level diagnostic test
on the same owned target plus a scripted PlatformAdapter mock reproducing the
returned reads/errors. Do not add a production bypass API to facilitate diagnosis.

## Phase 3 — smallest shared repair, one failure class at a time

Prioritize demonstrated blockers by number of frozen cases helped and blast radius:
1. Incorrect target/capability evidence preventing a supported action.
2. Incorrect readiness/selection/commit handling where raw AX proves a working path.
3. Safe stale-target recovery with sufficient stable identity.
4. Misclassified delivery, false verification or duplicate retries.
Safety defects take precedence over this ordering whenever observed.

Patch the macOS adapter when the defect is platform-specific. Patch core only
when the cross-platform contract is wrong and reproduce it with a mock. Reuse
existing resolution, deadline, lease and verification helpers; do not add another
retry loop or increase all timeouts. No app-name production special cases.

Before every repair, record: failing case, cause, affected callers, behavior kept
unchanged, deterministic regression, real-app acceptance check, and rollback diff.
An unchanged post-state after a short wait is not universal proof of non-delivery.
In particular audit selection fallback for delayed side effects: do not broadly
continue after unverified AXPress or turn unknown delivery into a safe retry.

Tests exercise stable identity, replacement with the same identity, removed target,
lookalike ambiguity, delayed readiness, delayed effect after native timeout,
unsupported attributes, read failures and deadline exhaustion only as implicated
by the repair. Use scripted responses/clock control at existing seams rather than
wall-clock sleeps. Assert target identity, native dispatch count and disposition,
not just ok=true. Failed core preflight must cause zero native mutation calls.

## Phase 4 — verification and repeatability

- Serialize complete document-edit workflows; per-command locking is insufficient
  for an app with one active editor. Never automate concurrently with another run.
- Poll explicit readiness/effect predicates under a bounded deadline. No fixed
  sleeps as the success oracle and no automatic rerun-until-green in the evaluator.
- Capture and inspect same-window screenshots before and after text mutations.
  Compare the exact requested token against visible editor text, AX readback and
  the document model. Distinguish seeded content from action results, and visible
  editor changes from committed cell values. Conflicting evidence stays unresolved.
- Capture foreground changes throughout the run, physical pointer samples and
  available delivery/input instrumentation. Endpoint equality alone cannot prove
  no temporary side effects. Report monitoring limitations and user interference.
  Interference invalidates a run separately; retain its evidence and count.
- Keep raw logs local/redacted; owned content only. Record pass, task failure,
  unsupported, prerequisite blocked, contaminated and not-run distinctly.
- Compare immutable before/after binaries against equivalent fresh seeds in
  alternating order. Report per-case counts, workflow completion and p50/p95
  CLI latency separately from model/tool/overlay overhead.
- Run focused regression, workspace tests, fmt, Clippy, release size and core
  dependency-isolation checks. Run applicable permissioned semantic/native gates;
  first inspect their setup for app activation. A gate that violates headless
  constraints is blocked, not silently run or counted green.
- Run the existing performance comparison with compatible scenarios and a
  same-window supplement where name-based probes are ambiguous. Review artifacts;
  explain intentional cold costs and investigate warm regressions.
- After fixes, require 30 consecutive valid runs of each claimed supported case
  across at least three independently reset batches. Failures remain in totals;
  a subsequent new revision starts a separately identified acceptance series.
  This is a local gate, not a statistically established universal success rate.

## Scorecard and exit criteria

Keep the provisional 6/10 distinct from the first measured score. After the baseline,
score each category from frozen cases, showing numerator/denominator and evidence:

| Category | Points | Measurement |
|---|---:|---|
| Real-app task completion | 4 | Mean per-case verified completion fraction; unsupported/incomplete tasks do not pass |
| Safe recovery | 2 | Predeclared recoverable scenarios complete; ambiguity/removal cases correctly refuse |
| Delivery and verification correctness | 2 | Reported scope/disposition agrees with independent observed outcome |
| Headless compliance | 1 | No prohibited effects with sufficient monitoring; unknown evidence earns no credit |
| Repeatability | 1 | Fraction of eligible cases completing all required reset batches |

Do not inflate the total with fixture counts or hide a weak app behind another app's
success rate. Publish per-app/category results, coverage and unsupported task gaps.
An incomplete denominator is an incomplete score, not a rounded-up rating.
Any wrong-target mutation, duplicate non-idempotent effect, false verified claim,
or prohibited input/focus effect is a hard release failure regardless of score.

Target 8/10: measured coverage and all claimed supported cases meet the repeated
acceptance gate with no safety failure. Target 9/10 additionally needs a second
native app, recovery perturbations and persistent outcomes across separate runs.
Passing the entire declared suite may be labeled local acceptance complete.
It does not establish universal reliability or Playwright parity. Any 10/10 label
must explicitly state this limited scope and retain unmet required tasks.

## Deliverables and stop rules

Deliver a small runnable real-app evaluator, frozen case definitions, baseline and
candidate JSON results, core/adapter/platform failure ledger, minimal tested fixes,
and a concise report with measured score, headless evidence and unresolved limits.

First execution milestone is the Numbers baseline and differential diagnosis.
If cell editing fails in raw AX under all tested allowed preconditions, preserve
it as an unmet Numbers task and improve proven AD defects; do not spend the whole
project searching for physical or private-API workarounds. If independent outcome
verification is unavailable, report the case unverified rather than upgrading it.
Finish each repair with its live reproduction and blast-radius checks before
starting another. Stop adding infrastructure once the evidence loop is runnable.
