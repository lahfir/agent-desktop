---
title: Select collection rows by live editable labels without changing ref identity
module: macos adapter
problem_type: runtime_error
tags: [xcode, selection, ambiguity, headless]
date: 2026-09-20
---

# Collection label selection

Xcode Project Navigator exposes file names as AXTextField values inside cells
and rows. Canonical names intentionally exclude mutable field values. Therefore
the old collection selection search failed to find a file visible in snapshot.
Broadening canonical identity would weaken stale-ref safety across all commands.

Collection selection now accepts a live non-secure AXTextField value as a query
match, then resolves its owning selectable member within the requested root.
The nearest row takes precedence over its wrapper cell. A field without a
selectable owner is never pressed. Existing canonically named button-only list
items remain supported. The ancestry walk must reach the requested collection;
missing parents and cycles fail before delivery.

The search checks the bounded collection before mutating, coalesces matches for
the same owner and refuses multiple owners with AMBIGUOUS_TARGET/not_delivered.
Depth truncation with remaining children cannot prove uniqueness. Menus retain
their separate name-based matching. Ref naming/identity rules are unchanged.
Collection delivery reuses the shared guarded container-selection path instead
of a duplicate AXSelected implementation.

## Candidates and observed outcomes

Initial candidate `/tmp/ad-native-eval/collection-label-candidate`, SHA-256
`44914c3b5d5f4c032b1dd37b08e947b7070f606fe616148c0e3cbd51bbe578a1`:

- Public find returned Project Navigator `@s21x71bw3p1ld3:e1` in Xcode
  w-19740, PID 24741, process instance macos-proc-v1:1789875340:341343.
- Selecting CursorCamAppDelegate.swift succeeded with semantic verified delivery.
- Selecting cursor-cam returned AMBIGUOUS_TARGET with safe non-delivery; both
  project and group have that label. Screenshot `/tmp/ad-native-eval/xcode-label-selection.png`
  was viewed and showed CursorCamAppDelegate selected afterward.
- The 45-second monitor had zero activations/input deltas and no pointer change;
  foreground PID 35000 remained unchanged.
- Reading old unnamed row ref `@s2j42fiokeiy6k:e38` failed bounds mismatch after
  the earlier window resize (14 identity candidates, none matching geometry).
  This remains a stale-recovery limitation, not a passing recovery trial.

Final candidate after extracting the same ancestor walk for deterministic tests:
`/tmp/ad-native-eval/collection-label-final-candidate`, SHA-256
`1a4972e3c06606bedcf4829d3eb89edff0ddca957924d8704746c1d113ce4f0c`:

- Selecting CursorCamApp.swift failed APP_UNRESPONSIVE with incomplete selection
  evidence, not_delivered/safe. Retain this failure; do not infer its cause from
  the generic error. It was not silently retried or counted as successful.
- Selecting CursorCamAppDelegate.swift succeeded, and cursor-cam remained safely
  ambiguous. The 30-second monitor recorded no activation/input/pointer change,
  with foreground PID 35000 unchanged. The duplicate check followed this monitor.
- SF Symbols outline `@s3jplttya4mw7g:e1` selected Math then All using the same
  ref. Both reported semantic verified delivery. Viewed screenshot
  `/tmp/ad-native-eval/collection-math.png` showed Math and 78 symbols.
  These SF Symbols actions were not covered by a headless monitor, so they prove
  functional regression behavior only, not complete headless qualification.

## Regression coverage and outstanding work

Deterministic tests exercise the production ancestor walk with scripted nodes:
field/cell/row ownership, table-cell ownership, refusing ownerless editable
fields even when named, preserving named button items, nearest nested row,
missing parent and an eight-read bound on cyclic ancestry. Existing tests retain
canonical label evidence and uncertain-delivery refusal coverage.

Full workspace tests, clippy, format and diff checks passed on the final source.
The safe-semantic fixture passed all seven assertions on the exact final hash;
log `/tmp/ad-native-eval/collection-label-final-semantic.log`.
Xcode's read-only performance comparison completed 12/12 candidate and baseline
calls with matching corresponding tree shapes. Candidate/base p50 ms:
interactive 291.4/283.1, skeleton 197.1/195.8, depth-30 288.8/296.2 and first-button
find 241.0/224.1. Find tail latency remained high in both (p95 2112.8/2960.3 ms);
this is not proof of uniformly low latency. Report:
`/tmp/ad-native-eval/collection-label-performance/report.html`.
The production changes remain local.

File opening after selection, the intermittent incomplete-evidence failure,
unnamed-row recovery after geometry changes and repeated qualification remain
unresolved. No source edits, build/run actions or physical input were used in
Xcode. The provisional rating remains 8/10 and the goal remains active.
