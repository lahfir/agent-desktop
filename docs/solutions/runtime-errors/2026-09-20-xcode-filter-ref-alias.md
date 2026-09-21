---
title: Filtered Xcode row aliases an earlier editable-label ref
module: core and macos resolution
problem_type: runtime_error
tags: [xcode, identity, stale-ref, wrong-target]
date: 2026-09-20
---

# Confirmed wrong-target read after filtering

Qualification failure, not a passing stale-recovery case. No wrong-target write
was attempted. Candidate `/tmp/ad-native-eval/empty-text-final-candidate`, SHA-256
`6e9626edf059424869caba2519b8bbb2d749c7443512e5c8045ca9c8809789c1`.
Namespace `/tmp/ad-native-eval/xcode-state`, Xcode PID 24741, w-19740.

## Reproduction through public CLI

Drill navigator root `@s287cvyqznmcqt:e2`. Its result contains:

- e265: unnamed treeitem for CursorCamApp.swift
- e267: AXTextField, native ID title, value CursorCamApp.swift
- e268: unnamed treeitem for CursorCamAppDelegate.swift
- e270: AXTextField, native ID title, value CursorCamAppDelegate.swift

Using the same immutable binary and namespace:

```
set-value @s287cvyqznmcqt:e246 CursorCamAppDelegate
get @s287cvyqznmcqt:e267 --property value
get @s287cvyqznmcqt:e268 --property states
clear @s287cvyqznmcqt:e246
get @s287cvyqznmcqt:e267 --property value
```

The first get incorrectly returned CursorCamAppDelegate.swift. The saved Delegate
row e268 instead returned safe STALE_REF with five identity candidates and no
matching bounds. Clear verified; the final get returned CursorCamApp.swift again.
Thus the original App field ref aliases another logical row while filtering.
The 45-second monitor recorded no activation, physical input or pointer change.

## Independent native comparison

`/tmp/ad-native-eval/xcode-retained-row.swift` uniquely found and retained the
original App AXTextField before filtering. The helper performs no mutation.
After a separate public CLI filter action, it reread the retained object:

```
retained AXValue: error=0, value=Optional(CursorCamApp.swift)
```

The CLI get on e267 during that same filtered state again returned
CursorCamAppDelegate.swift. This distinguishes native object preservation from
Agent Desktop's re-identification error. The filter was then cleared once and get
confirmed its original empty value. This second diagnostic trial was not covered
by the first trial's monitor and must not inherit its headless evidence.

## Root cause and repair constraints

Core ref_identity::identity_match returns Match immediately when the saved native
identifier occurs in the candidate's identifiers. Both fields advertise title.
The macOS resolver permits path-based resolution with identifier/bounds evidence;
the replacement field occupies the original path and geometry. The stored
RefEntryIdentity has no row-context identity or native-identifier uniqueness
evidence. Mutable text values intentionally do not identify text fields, so
comparing all field values would break legitimate same-ref edits.

The correction must preserve a distinction between an editable control's value
and the identity of the collection item containing it. Simply disabling the
path optimization is insufficient: a broad search can still match the repeated
title identifier and the same bounds. Likewise, a filename/Xcode-specific check
would leave the same bug in other collections. Do not globally freeze text-field
values, silently rename the canonical role, or count a wrong-target refusal as
full task completion.

Required regression outcomes: unchanged row resolves; filtering/removing it
cannot resolve to a replacement with the same role/native ID/path/bounds;
restoration recovers the original logical row; legitimate edits to an ordinary
text field remain usable through its original ref. Resolution must agree across
get, action, scoped snapshot and find roots. Snapshot capture and resolution both
need the same additional identity evidence; the platform/core boundary must be
reviewed before adding fields. Product repair remains pending.

## Native identity inventory and implementation constraints

A fresh raw attribute-name/value probe confirmed the field exposes AXIdentifier
title, but no AXURL, AXDocument or AXDOMIdentifier. Its AXCell parent has no
identifier/name/value. The AXOutlineRow parent likewise has no identifier/name/
value; AXIndex is positional. The enclosing outline has identifier project
navigator. This rules out a simple switch to an already-exposed document URL.
Raw helper: `/tmp/ad-native-eval/xcode-row-read.swift`.

Current references serialize process, element identity, geometry, source and
path, but no containing-item identity. Native objects themselves are not retained
across CLI processes. The raw retained-object experiment proves a long-lived
handle can preserve the original object for this scenario, but adding a daemon
is a separate architectural change and not a minimal adapter patch.

Do not promote CFHash to a unique runtime identifier: Core Foundation hashes may
collide and the current identifier shortcut would treat equality as identity.
Do not globally require unchanged values: ordinary text inputs must remain
editable through the same ref. Do not rely on broad-search uniqueness: after
filtering, only the wrong editable label can remain in the original position.

The next implementation must capture containing-item evidence at observation
time and validate it before resolving a descendant control. The decisive tests
must include repeated native IDs and identical replacement geometry, plus normal
text editing on an unchanged item. A row label may itself be editable; any label
derived from the target's value must explicitly account for that rather than
silently freezing a mutable value. Until that contract is implemented and tested,
these navigator label refs are not safe mutation targets after collection changes.

## Isolated reproduction surface

Added opt-in `AGENT_DESKTOP_FIXTURE_REF_CHURN=1` mode to the existing fixture.
RefChurnView has Alpha and Beta fields with the same shared-title native ID, a
Filter rows/Restore rows button, a separately named stable editor and visible
backing-value labels. Filtering removes Alpha and moves Beta into its position.
The default fixture mode remains selected unless this environment variable is 1.

Built with `AGENT_DESKTOP_FIXTURE_BACKGROUND_BUNDLE=1` using the existing build
script, and launched with `AGENT_DESKTOP_FIXTURE_NO_ACTIVATE=1`. The inspected
fixture PID was 75044, instance macos-proc-v1:1789879561:749580, window w-19980.
Snapshot s1wegvust2ghvt exposed e1 filter button, e2 Alpha, e3 Beta, e4 stable
editor. On the same candidate used above:

1. Click e1 reported delivered_unverified; read e2 returned Beta, establishing
   the filtering effect without repeating the mutation.
2. Click e1 to restore; read e2 returned Alpha.
3. Set e4 to updated-stable, get e4 returned updated-stable, then set e4 back to
   Stable value. Both writes verified through the same ref.
4. Viewed `/tmp/ad-native-eval/ref-churn-restored.png`; all three backing-value
   labels retained their original values.

The 45-second monitor recorded no activation, no pointer movement and zero input
events, with foreground PID 860 unchanged. This covers interaction, not launch.
The fixture was closed by TERM only after verifying its exact executable path;
the owning process handle reported terminal exit 143.

This fixture demonstrates the defect outside Xcode without a collection-row
native role, so a patch restricted to AXRow ancestry is insufficient. Its normal
same-ref editor is the control case that an eventual correction must preserve.
It is a reproducible failing product scenario, not a passing acceptance result.

After adding this mode, the default background fixture passed its existing seven
semantic assertions using the same production binary. Log:
`/tmp/ad-native-eval/ref-churn-default-regression.log`. No production code changed
in this fixture-only step.
