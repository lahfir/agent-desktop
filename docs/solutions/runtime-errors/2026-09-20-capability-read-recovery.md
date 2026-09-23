---
title: Recover transient capability reads before selection delivery
module: macos adapter
problem_type: runtime_error
tags: [xcode, capability, recovery, headless]
date: 2026-09-20
---

# Capability read recovery

The previous candidate intermittently failed Xcode collection selection with
APP_UNRESPONSIVE and a generic incomplete-evidence message. No action was
delivered. Adding phase and query-stat diagnostics to selection errors identified
`actions_unknown`, four cannot-complete reads, one native-read failure and no
deadline exhaustion. This was reproduced by scanning for an absent diagnostic
label, so the diagnostic itself made no selection. Two subsequent scans on the
same diagnostic candidate completed with expected ELEMENT_NOT_FOUND.

The capability boundary had no retry for AXUIElementCopyActionNames or
AXUIElementIsAttributeSettable. These readers are shared by action-list
observation, readonly checks, renderer probes and advertised-action checks.
They now retry only kAXErrorCannotComplete, at most three calls, respecting the
same absolute deadline. Backoff is capped at 1 ms and remaining time. Mutation
APIs are unchanged and never enter this helper. All other native errors remain
terminal. Returned CF arrays are released by the existing per-attempt reader.

Deterministic tests cover transient-transient-success including a legitimate
false result, persistent transient failures capped at three calls, terminal
permission/invalid-element/generic failures with one call, and no native call
after an already expired deadline. Existing selection diagnostics now distinguish
unknown actions/name/description from missing ancestry or its depth limit.

## Live evidence

Candidate `/tmp/ad-native-eval/capability-recovery-candidate`, SHA-256
`4ea254eb2ddaa095df58d84c9874875c3fcf2e1caf3de51765dea33b510e95af`.
Public CLI only, raw responses, namespace `/tmp/ad-native-eval/xcode-state`.
Xcode PID 24741, w-19740, instance macos-proc-v1:1789875340:341343.

- Three independent absent-label scans of collection `@s21x71bw3p1ld3:e1`
  returned expected ELEMENT_NOT_FOUND / not_delivered. No incomplete-evidence
  failure recurred in these three scans. This is not statistical qualification.
- Select CursorCamApp.swift succeeded with semantic verified delivery; select
  CursorCamAppDelegate.swift restored the original row using the same ref.
- Select cursor-cam retained AMBIGUOUS_TARGET / not_delivered for duplicate names.
- The 45-second monitor recorded zero activations, all input deltas zero and
  unchanged pointer; foreground PID 35000 remained unchanged and Xcode survived.
- Viewed `/tmp/ad-native-eval/capability-xcode-alternate.png` showed the alternate
  row selected while the editor still displayed CursorCamAppDelegate. File
  opening is still unresolved; row-selection verification does not prove opening.

Full workspace, lint and focused capability tests passed. All code stays local.
Exact-candidate native fixture passed all seven assertions, including background
ownership, exactly-once effects and rejection of silent text no-ops. Performance
comparison completed three rounds against the merge base: interactive snapshot
p50 811.8 vs 772.2 ms; skeleton 282.1 vs 313.0 ms; depth-30 1094.5 vs 680.2 ms;
find 394.2 vs 415.8 ms. All commands succeeded with matching reported shapes.
The deep-snapshot slowdown remains unexplained and is an open regression concern;
this small sample does not establish causality or approve the latency increase.
Artifacts: `/tmp/ad-native-eval/capability-recovery-semantic.log` and
`/tmp/ad-native-eval/capability-recovery-performance/app-xcode.json`.

Provisional rating remains 8/10; repeated qualification,
unnamed-row recovery after geometry changes and remaining application tasks
remain open.

## Additional column-drilling trial

Same immutable candidate, public raw CLI responses, snapshot `s287cvyqznmcqt`:

- Skeleton returned 22 refs; inspector e9, editor e5, debug e7 and navigator e2
  each drilled successfully with complete=true and the same window context.
- Inspector Tab e44 remained readable after all other columns were drilled.
- Exact textfield name Tab scoped to inspector returned one match; the same
  query scoped to editor returned zero without truncation.
- Inspector Text Settings e40 collapsed and expanded through the same ref,
  both delivered_verified. A live get between actions showed no expanded state.
- Re-drilling inspector invalidated e44 with safe STALE_REF. Replacement e152
  returned value 4; editor e53 still returned states; the independent scoped-find
  ref `@s2rmk0kyenv31d:e1` also still returned value 4.
- The 60-second monitor covering initial drills and disclosure interactions
  recorded no activation, no pointer change and zero input-event deltas.
  Foreground PID 35000 remained unchanged. Later read-only checks and screenshot
  were outside that monitor interval.
- Viewed `/tmp/ad-native-eval/xcode-drill-final.png`: original file and selected
  row retained, inspector Text Settings expanded. No source editing was attempted.

This is one additional successful trial, not the repeated acceptance campaign.
