---
module: agent-desktop-macos
date: 2026-09-20
problem_type: runtime_error
tags: [numbers, accessibility, initialization, headless]
---

# Native accessibility initialization hides Numbers cells

## Reproduction

The owned document is `/tmp/ad-native-eval/AD Reliability.numbers`. Reopening it
in the background produced Numbers PID 10520, window w-17920. The screenshot
showed the seeded grid, but both Agent Desktop find and an independent raw AX
walk exposed no cells or tables. The canvas AXScrollArea had only scrollbars.
Raw AX reported AXEnhancedUserInterface=false and settable=true; manual
accessibility was unsupported. A separate raw enhanced-mode write returned
-25208, but readback became true and the cell became discoverable immediately.
This repeats the earlier missing-grid/disabled-mode observation on a new process.

The exposed editor advertised AXShowMenu, TSAccessibilityAddCommentAction and
TSAccessibilityDeselectAllAction, with writable AXValue and AXSelectedText. It
exposed no confirm action. No new text write or speculative custom action was
attempted. The document oracle retained seed-target, neighbor-guard, eight rows
and five columns. The missing-grid diagnosis does not solve text commit.

## Shared repair and blast radius

The existing capability probe considered enhanced mode only when a web surface
was already observed. Native apps exposing the same supported attribute were
excluded. The probe now considers an explicitly advertised enhanced attribute
for native apps as well. Native activation requires a readable false value;
true and unknown values do not request activation. Legacy manual-activation
precedence and Chromium's existing readiness behavior remain unchanged.

The existing core activation lease, process-generation validation, setter
readback, deadline and two-second enhanced-mode settling delay are reused.
There is no app-name special case, new dependency, physical input or app
activation call. Unsupported apps receive no activation write. The broader
native capability probe is a deliberate blast-radius change: native apps with
advertised disabled enhanced mode can now receive an initialization write.

The first candidate's observation gate still required a sufficiently deep completed traversal
when no web surface is present. A shallow skeleton alone is not guaranteed to
initialize a native app. Full find/snapshot coverage is improved; do not claim
all observation forms had reached parity at that point.

## Controlled candidate check

Only the owned Numbers window was open; the unchanged document was closed
gracefully and reopened in the background as setup. New PID 25567, generation
`macos-proc-v1:1789867656:421528`, window w-18040.

Baseline `/tmp/ad-native-eval/menu-sibling-candidate` returned zero cell matches
for exact seed-target in this fresh process. Candidate
`/tmp/ad-native-eval/native-init-candidate`, SHA-256
`0dba97c4b8d3d1b0249b769e1d3beb8c853f1f20f3e862a5fcd9b4377668c87a`,
ran the same public find and returned the correct cell in approximately 2.88 s.
No raw AX setter was used in this candidate process. A second find completed in
0.45 s; the first qualified ref `@s1krllq8xd4krk:e1` still read selected state.

The 30-second monitor recorded foreground PID 2058 at both ends and no activation
events. Session-wide input activity occurred and cannot be attributed to the
agent by these counters. Source inspection confirms this path writes only the
accessibility attribute, not physical input. Screenshot
`numbers-native-init-candidate.png` and the independent document oracle confirm
unchanged seeded values and dimensions. This is one candidate cold-start trial,
not the repeated acceptance suite.

Artifacts under `/tmp/ad-native-eval/`: `numbers-native-init-monitor.jsonl`,
`numbers-reopened.png`, `numbers-native-init-candidate.png`,
`native-init-workspace.log`, `native-init-clippy.log`,
`native-init-safe-semantic.log`, and `native-init-performance/`.

Twenty-six focused activation-related tests and Clippy passed. The isolated
native semantic gate passed seven assertions on the exact candidate, including
one-shot button effects, text binding updates, silent-no-op rejection and no
fixture foreground activation. Full workspace and performance results must be
read from the corresponding completed logs; no Numbers edit success is implied.

## Shallow initialization follow-up

The window-root probe now checks advertised mode even after shallow traversal.
Missing renderer evidence still requires a complete traversal ending before the
depth cap for the manual protocol. Native enhanced mode instead requires explicit
disabled readback. Element-root traversal without web content remains ineligible.
The regression `shallow_observation_requires_explicit_mode_evidence_not_renderer_absence`
covers disabled, enabled and unknown enhanced mode and the shallow manual case.

Candidate `/tmp/ad-native-eval/shallow-init-candidate` has SHA-256
`1463071575bfa2e162686e328ce750f4538da45d93fe335ce67b053c9bb03d98`.
Fresh Numbers PID 46296, window w-18244, was opened in the background after
graceful closure of the sole owned document. The old native-init candidate's
first skeleton exposed no table. The new candidate's skeleton exposed
`@s1god9wm90dl5i:e2`, Eval Grid, eight rows and five columns, in 2.77 seconds.
Public find rooted at that exact ref returned the selected seed-target cell in
54 ms. No raw setter initialized this process. The 30-second monitor recorded
other app activations but none for Numbers; session input activity is unattributed.
This is one matched old/new cold-start trial, not the full repeated acceptance suite.

Twenty-seven focused activation tests, full workspace tests, Clippy and formatting
passed. Logs use the `shallow-init-` prefix under `/tmp/ad-native-eval/`.
The isolated native semantic gate passed all seven assertions on this exact
binary, including text binding changes and rejection of a silent no-op without
retry. Core dependency isolation passes and release size is 3,088,656 bytes.
The completed performance report is `shallow-init-performance/report.html`.
Numbers completed all 24 observations with matching snapshot shapes; depth-30
p50 was 22.9% slower, while the other p50 samples were slightly faster. SF Symbols
revealed a regression: both full snapshot modes failed in all candidate rounds
while baseline snapshots succeeded; skeletons succeeded on both. The public
candidate command reproduced TIMEOUT, accessibility deadline exhausted before IPC.
The new probe had run after incomplete traversal consumed the deadline. Its gate
now preserves the existing incomplete-tree return path; only complete native
observations probe activation. This follow-up requires new live validation.
The SF Symbols button find failed on both versions and remains unresolved.

## Direct cell and editor write outcome on the same process

The public `set-value @s2afdbd2ja0t3j:e1 direct-cell-0920` was rejected before
delivery: the cell exposes no SetValue capability. Raw AX confirms AXValue is
absent and not writable. Public click on this ref opened the editor successfully.
Find under the table returned editor `@s4xc5dnexktei:e1` with seed-target value.
Setting this editor to direct-editor-0920 returned ACTION_FAILED after native
AXValue success, because readback remained seed-target. A matched raw AXValue
write to raw-editor-0920 also returned native success in 0.077 ms and retained
seed-target. This reproduces the failed write outside Agent Desktop.

The viewed `shallow-editor-set.png` shows seed-target in B2. Independent document
readback confirms seed-target, neighbor-guard, eight rows and five columns.
Neither a committed edit nor persistence is proven. Do not add automatic retry
or reinterpret native success as task completion. The process remains in cell
editing state; future probes must observe that state before acting.

## Candidate after partial-tree guard

`canonical-find-candidate`, SHA-256
`f04ade8e00bac484be1a150b5c01fcc2110b7d6d58f396e3ae71e1cc42fd5a0e`,
restored the same SF Symbols public snapshot to success in 2.95 seconds with
`complete:false`, `truncated:true`, 979 observed nodes and 81 refs. The incomplete
grid remains explicitly marked; this does not claim complete grid discovery.
Full workspace tests, focused find tests, formatting, diff checks and Clippy
passed. The completed `canonical-find-performance/` comparison restored SF Symbols
snapshot success in all rounds. Interactive and skeleton shapes match baseline;
depth-30 shapes differ (985 versus 949 nodes), so that latency pair is not a
content-equivalence result. Numbers completed all 24 observations with matching
snapshot shapes; its earlier depth-30 slowdown did not repeat (547 versus 561 ms
p50). SF Symbols first-button find still failed on both versions; its separate
hydration-budget diagnosis and candidate are recorded in the adjacent report.
