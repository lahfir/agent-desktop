---
title: Xcode project drilling and disclosure-state consistency
module: macos adapter
problem_type: runtime_error
tags: [xcode, accessibility, headless, disclosure, scoped-refs]
date: 2026-09-20
---

# Xcode project probe

The user opened cursor-cam in Xcode, PID 24741, window w-19740. This probe
used public Agent Desktop JSON and model-selected refs, without parsing wrappers.
Project source was not edited and no build/run was requested. Raw AX was used
only for separate read-only diagnosis. The provisional rating remains 8/10;
these trials do not establish the repeated acceptance denominator.

## Reproduced and fixed

Candidate `clipping-final-candidate` advertised Expand/Collapse on navigator
disclosures, but `expand @s2j42fiokeiy6k:e44` failed with ACTION_FAILED,
not_delivered: "All chain steps exhausted". Raw AX showed
AXDisclosureTriangle with AXValue 0 or 1 and absent AXExpanded/AXDisclosing.
The adapter's action verifier and snapshot state reader ignored this value.

The shared state reader now interprets AXValue only for AXDisclosureTriangle,
and only when explicit expanded/disclosing evidence is absent. Snapshot states,
live-state completeness and disclosure action verification reuse that rule.
Unknown/mixed values remain unknown; other roles never inherit it. The existing
single-delivery semantic chain is unchanged. No focus or input fallback was added.
Tests cover true/false, explicit precedence, missing/mixed values and other roles.

Repaired immutable candidate: `/tmp/ad-native-eval/disclosure-candidate`, SHA-256
`1d68e0d6d5f71e9bcc046f19a3b2c22ee31a3bf7dc49b2f6bad121513783b39c`.
The original disclosure ref expanded, get reported expanded, and the same ref
collapsed. Screenshot `/tmp/ad-native-eval/xcode-disclosure-expanded.png` was
viewed and showed MenuBarManager and SettingsPopoverManager under MenuBar.
The 45-second monitor reported no activations, zero input deltas and unchanged
pointer; foreground PID 35000 stayed unchanged and Xcode remained running.

## Column and command checks

- Skeleton `s1a9jwgx9ts97p` provided navigator e2, editor e5, debug e7 and
  inspector e9. Inspector, editor and debug drills succeeded on the pre-fix
  candidate; navigator drill had already succeeded under `s2j42fiokeiy6k:e2`.
- Inspector exact find of textfield Tab returned one match with value 4;
  the same query rooted at editor e5 returned zero matches. The inspector child
  e44 still returned 4 after drilling editor and debug siblings.
- On the repaired candidate, editor-scoped Source Editor find returned one
  match, and debug-scoped VariablesViewFilter find returned one match.
- Re-drilling inspector e9 replaced its child refs. Old e44 failed STALE_REF
  with safe not-delivered disposition; replacement e100 returned 4. Untouched
  editor child e53 still resolved. No mutation was attempted with the stale ref.
- Inspector Text Settings e96 collapsed and expanded with the same ref; get
  reported the intermediate empty state. Editor scrollarea e52 scrolled down
  and up with the same ref. Both commands reported semantic verified delivery.
  Viewed screenshots `xcode-editor-scrolled.png` and `xcode-editor-restored.png`
  under `/tmp/ad-native-eval/` showed the source viewport moving and returning
  to line 1, respectively. The second 45-second monitor also reported zero
  activations/input deltas and unchanged pointer/foreground PID 35000.

These are individual supporting trials, not thirty-run qualification. Older
refs were intentionally retained across the candidate change; fresh-candidate
full baseline/reset series is still required.

## Other findings retained

`select` on Project Navigator with CursorCamApp.swift returned ELEMENT_NOT_FOUND
despite the nested textfield exposing that value. Canonical child labels read
static text, while this navigator exposes editable textfield values. Do not
blindly broaden stable identity to all mutable values or click editable labels.
This inconsistency is unresolved.

Clicking the file row reported verified selection and visibly selected it, but
the editor remained on CursorCamAppDelegate.swift. Selection is not proof of
opening a file. Xcode displayed Preparing Editor Functionality throughout.
No duplicate activation was attempted to force the result.

Setting Project navigator filter to CursorCam visibly filtered the tree.
Clearing it reported delivered_unverified because the empty value was absent;
the final viewed screenshot showed the original unfiltered tree and empty
filter. Earlier monitors included external-session input and activation of
multiple apps; attribution was unavailable, so those trials are not clean
headless qualification. The file row selection remains CursorCamApp; the editor
remains CursorCamAppDelegate. No project contents were changed.

## Validation and limits

- Five focused disclosure tests passed. Full `cargo test --workspace` passed
  with macOS access (`disclosure-workspace-unsandboxed.log`). The sandboxed
  attempt failed two CLI event-wait batch tests before that rerun; retained at
  `disclosure-workspace.log` rather than erased.
- Clippy with warnings denied, format and diff checks passed.
- Owned safe-semantic fixture passed all seven checks on the exact candidate
  hash (`disclosure-semantic.log`).
- Read-only three-round performance comparison:
  `/tmp/ad-native-eval/disclosure-performance/report.html`.
  Xcode candidate/base p50 ms: interactive 261.7/265.7, skeleton 175.3/188.9,
  depth-30 278.8/258.5, first-button find 202.8/205.1. All 12 candidate and
  baseline calls succeeded and respective tree shapes matched. Depth-30 was
  20.3 ms slower; this small sample cannot isolate its cause within the cumulative
  branch diff. No extra per-node AX reads were added by this repair.
- SF Symbols candidate calls all succeeded; baseline first-button find failed
  all three rounds. Deep partial-tree shapes differed (937 vs 930), so this is
  not a clean aggregate performance pass or completeness-equivalence claim.

Headless InteractionPolicy already defaults both focus stealing and cursor
movement to false. This repair keeps that policy and adds no physical path.
This is not proof that arbitrary AX actions in arbitrary apps cannot activate
their app. Menus, file opening, semantic drag and repeated cross-app acceptance
remain outstanding; no 10/10 claim is supported. All changes remain local.
