---
module: agent-desktop
date: 2026-09-19
problem_type: runtime_error
tags: [macos, accessibility, headless, xcode, reliability]
---

# Xcode headless editor and menu evaluation

This follow-up adds a second complex native app without replacing the unresolved
Numbers cases. Overall engineering confidence remains provisional at 7.5/10;
neither the repeated acceptance gate nor universal reliability is established.

## Scope and build

Xcode 26.0, PID 17836, process identity `macos-proc-v1:1789862351:629080`.
Only `/tmp/ad-native-eval/AD Reliability.swift` was edited. Xcode was launched
through the CLI without activation. Opening the disposable file with `open -g`
was setup, not a completed Agent Desktop operation. All evaluated interactions
used refs selected directly from unmodified CLI JSON. No custom JSON extraction,
keyboard synthesis, pointer input or clipboard insertion was used.

Immutable binary: `/tmp/ad-native-eval/bounds-candidate`, SHA-256
`fb11a6615075439c5fb2562d85194015c54e2fe29ca722f4a0e338484559c033`.
State root: `/tmp/ad-native-eval/xcode-state`.

## Outcomes

| Case | Result and independent evidence |
|---|---|
| Discover editor | Window w-17124; skeleton then root drill returned Source Editor at `@s32ke2z9l0724g:e7` with SetValue and TypeText |
| Replace text | Original ref accepted AXValue replacement of seed-target with verified-headless; fresh get, screenshot and file read agreed; neighbor-guard retained |
| Insert text | Same ref accepted AXSelectedText insertion of a trailing comment; screenshot and file read contained exactly one insertion |
| Close document | Original Close ref e3 accepted AXPress, honestly reported delivered_unverified; subsequent list-windows showed only Welcome to Xcode |
| Persistence | Background setup reopened the file as w-17140; fresh find and screenshot retained both edits and the guard. This is independent persistence verification, not an all-CLI reopen workflow |
| Closed-window ref | Original editor ref refused before and after reopening; returned APP_UNRESPONSIVE after its deadline with resolution_window_bridge_miss. No substitution into the replacement window occurred |
| Editor options menu | CLI e14 timed out before dispatch because live hidden state was unavailable; no action was delivered |
| Deep menu discovery | Raw AX subsequently exposed a menu at depth nine; repaired snapshot returned 20 refs, inventory returned the same menu, and a menu-item click dismissed it |

Screenshots were captured and visually reviewed under `/tmp/ad-native-eval`:
`xcode-before.png`, `xcode-after-set.png`, `xcode-after-type.png`,
`xcode-after-raw-menu.png`, `xcode-reopened.png`, and
`xcode-after-raw-show-menu.png`.

## Menu diagnosis: native and adapter limitations

Independent `ax_probe.swift` inspection found an AXMenuButton advertising
AXShowMenu and AXPress with valid geometry. AXHidden and AXExpanded both returned
-25205 (unsupported). Apple documents AXHidden as an application-level attribute:
[kAXHiddenAttribute](https://developer.apple.com/documentation/applicationservices/kaxhiddenattribute).

The adapter's aggregate state-completeness flag becomes false when the expandable
role lacks expanded evidence. Core then treats absent typed hidden evidence as
unknown visibility. Thus expansion evidence affects click eligibility. This is
a real coupling to investigate; no visibility gate was relaxed in this pass.

A direct raw AXPress trial on the owned window returned -25204 after about
1505 ms. No menu surface appeared and the screenshot showed the editor. After
closing/reopening the document, a separate AXShowMenu trial also returned -25204
after about 1505 ms. CLI list-surfaces returned an empty array, but the screenshot
then showed the editor options menu open. This establishes an effect despite the
native timeout and a menu-discovery gap at that observation. Neither uncertain
action was retried. Foreground PID was 860 at both endpoints of both raw trials;
this is not continuous headless monitoring. The initial interpretation of the
second trial as no effect was corrected after visual inspection. Removing the
preflight block alone still does not solve delivery classification or discovery.

A later raw button inspection showed no children and a paired screenshot showed
the menu had disappeared. Therefore the empty CLI list and visible screenshot
are not proof of a simultaneous AX omission: asynchronous opening, dismissal or
client lifetime may explain the discrepancy. Keep this as a discovery/timing
investigation, not a validated traversal repair.

The closed-ref error also remains unresolved. Current resolution verifies an
exact CoreGraphics record before matching AXWindows. A retained native window
record may explain why the closed window is classified as temporarily inaccessible
instead of stale; that hypothesis requires independent CoreGraphics evidence.
No title-based retargeting or weakened window identity was introduced.

## Shared getter repair and validation

`get --property bounds` previously returned saved coordinates when the supported
live reader returned no rectangle. A deterministic regression reproduced that
stale success. The two-line fix preserves the distinction between absent live
geometry (JSON null) and an unsupported live reader (existing snapshot fallback).
Native read errors still propagate. A live get on the reopened Xcode editor
returned its current rectangle normally.

The focused observation suite passed 19 tests. The full workspace suite passed
with native process visibility; log `/tmp/ad-native-eval/bounds-workspace-tests.log`.
Clippy with warnings denied, formatting, release build and diff checks passed.
Release build emitted an existing SDK-discovery warning; it completed successfully.

Required passive Numbers performance comparison completed 24/24 observations
with matching recorded shapes. Base/candidate median ms: interactive 554.2/524.1,
skeleton 359.7/378.9, depth-30 544.3/527.7, find 346.2/330.2. Three rounds are
insufficient to claim statistical equivalence. Report:
`/tmp/ad-native-eval/bounds-performance/report.html`. The activating fixture was
skipped. All changes remain local; no release, commit or push occurred.

## Confirmed deep-menu repair

A subsequent read-only raw AX probe located the open menu at depth nine below
the owned window, with a submenu at depth eleven. The corresponding screenshot
`xcode-menu-held.png` showed it open. This resolved the earlier timing uncertainty:
the adapter's descendant search silently skipped nodes deeper than eight.

The shared menu resolver now traverses by its existing 2,048-node and absolute
deadline budgets, without the unrelated depth cutoff. Cycles terminate with an
explicit incomplete-search error. Deterministic tests at depths 9, 11 and 30
failed before the repair and passed afterward; an expired deadline performs no
read, and a cyclic tree cannot turn into a successful empty result.

The first repaired snapshot exposed 20 menu refs, but list-surfaces still
returned empty because inventory used separate shallow discovery. Inventory now
uses the shared resolver when its existing discovery found no menu. Existing
surface IDs and non-menu entries are preserved, and already discovered menus
are not duplicated. Tests cover preservation and error propagation.

Live final verification on raw-AX-prepared menu state:

- `list-surfaces`: context_menu, id resolved/menu, 16 immediate children.
- `snapshot --surface menu`: 20 refs including four submenu descendants.
- Click Show Editor Only by its menu ref: AXPress accepted; screenshot confirmed
  the view changed and menu closed. Its delivered_unverified result was checked
  independently rather than promoted based on native success.
- On a separately prepared menu, clicking Canvas requested restoration of the
  earlier view setting. A fresh list-surfaces returned empty after dismissal;
  the restored Canvas state was not independently read back.

Menu opening itself still uses raw AX in these trials and remains an unmet
CLI capability under the observed preflight conditions. No uncertain action
was replayed, and no visibility safety gate was weakened. The helper now records
raw menu depth and supports a bounded 15-second client hold for diagnostics.

Final candidate `/tmp/ad-native-eval/menu-inventory-candidate`, SHA-256:
`26e20ce385f91f1c6bc67e651930005d7acfe3091868fb28aea45f9f25c1a783`.
The final workspace suite passed; log
`/tmp/ad-native-eval/menu-inventory-workspace-tests.log`. Clippy with warnings
denied, formatting and diff checks passed. The surface test selection passed
13 tests, including native-read recovery cases inherited from the preceding work.

One empty-menu inventory probe on Numbers completed in 0.23 seconds wall time.
Comet was not running, so its dense-browser latency remains untested. An earlier
Xcode performance attempt had zero successful observations on both revisions:
the app had two background windows and unscoped commands correctly returned
AMBIGUOUS_TARGET. The evaluation-owned welcome window was closed before the final
comparison; no user document was closed to make that benchmark pass.

Final semantic gate passed seven assertions on the final binary, including
exactly-once effects, silent no-op rejection and background fixture operation;
log `/tmp/ad-native-eval/menu-safe-semantic.log`. Final screenshot
`xcode-final.png` was visually reviewed; the file retains the guard and both edits.

Final three-round baseline comparison completed 48/48 observations across Numbers
and Xcode with matching recorded tree shapes. Xcode base/candidate median ms:
interactive 262.0/259.1, skeleton 225.4/223.4, depth-30 258.5/259.7, find
229.9/243.8. Numbers: interactive 556.0/735.7, skeleton 418.5/393.1, depth-30
533.5/600.8, find 366.7/367.6. Report:
`/tmp/ad-native-eval/menu-inventory-performance/report.html`. The Numbers delta
requires follow-up; successful commands and matching shapes do not establish
latency equivalence.

A seven-round isolation check compared the final binary with bounds-candidate
(the build immediately before the menu repairs): 56/56 observations passed with
matching shapes. Earlier/final Numbers medians were 524.0/505.8 ms interactive,
377.8/382.9 skeleton, 520.4/526.6 depth-30, and 344.6/346.7 find. This did not
reproduce the large median regression attributable to the menu-only delta;
skeleton p95 remained higher (403.6/466.2 ms), so no blanket performance claim
is made. Raw comparison: `/tmp/ad-native-eval/menu-only-numbers-performance.json`.
## Follow-up: visibility and menu delivery, September 20 UTC

The earlier editor-options failure is now reproduced and repaired at two
separate macOS adapter boundaries. These are additional local changes, not a
claim that Numbers editing or the repeated acceptance gate is complete.

1. With the earlier binary, `wait --predicate actionable` on
   `@s2anr09ovh7k3f:e1` timed out because missing expansion state also made
   visibility unknown. The native state read itself was complete. The repaired
   reader retains known hidden-state evidence independently of unknown
   expansion state. With `/tmp/ad-native-eval/visibility-candidate`, the same
   wait passed in 104 ms. Its subsequent single click opened the menu but
   returned APP_UNRESPONSIVE / delivery_uncertain after AXPress timed out.
2. A screenshot, raw AX, list-surfaces and a fresh menu snapshot all confirmed
   that menu was open. Raw AX showed it as a direct AXMenu child of the clicked
   AXMenuButton, with AXHidden=false and positive geometry. The existing
   activation verifier observed only focus and selection transitions, so it
   missed this effect.
3. The shared activation observation now also reads direct owned menus on
   native menu controls. Only a known closed-to-visible transition verifies
   delivery. Unknown reads, an already-open menu, a hidden or zero-size menu,
   and menus elsewhere in the app cannot establish this condition. The existing
   deadline, single mutation and uncertain-outcome rules are retained.

The immutable `/tmp/ad-native-eval/activation-candidate` has SHA-256
`49c75115e6600306ee42947484bc0982126585c0a137909f3d9745740fe84ab7`.
One live click using the original qualified ref returned delivered_verified,
one semantic AXPress step, and the resolved/menu surface with 16 immediate
items. `/tmp/ad-native-eval/activation-xcode-menu.png` independently shows the
menu open. This is **1/1 candidate menu-opening trials**, not the required
30 repetitions across three reset batches. A later attempted menu-item click
on the earlier snapshot safely timed out because that menu had disappeared;
retain this failed attempt rather than treating it as a completed menu action.

The focused activation tests cover every known/unknown before/after menu-state
combination and visibility/geometry negatives. The visibility tests preserve
unknown native evidence and unknown expansion state. Full workspace tests,
Clippy with warnings denied, formatting and diff checks passed. The seven
permissioned safe-semantic assertions passed on this exact candidate, including
single button effect and rejection of silent text-insertion no-ops. These gates
do not prove Numbers document mutation.

The required three-round comparison completed all 48 observation commands with
matching recorded snapshot shapes in Numbers and Xcode. Artifacts:
`/tmp/ad-native-eval/activation-performance/report.html`. Base/candidate medians
in milliseconds were Numbers interactive 525.6/524.5, skeleton 379.2/368.2,
depth-30 531.4/535.2 and find 343.9/385.0; Xcode interactive 262.5/273.6,
skeleton 219.6/218.5, depth-30 265.2/265.4 and find 241.3/244.4. The Numbers
find sample was about 12% slower; neither changed activation function runs in
find. Three samples do not establish equivalence or identify its cause. The
new activation reads need action-specific latency coverage beyond this passive
comparison. Live menu opening took about 2.93 seconds, including native timeout;
this repair improves outcome recognition, not that native waiting time.

A seven-round isolated comparison against the prior menu-inventory candidate
completed 56/56 observations with matching recorded shapes. The Numbers find
regression did not reproduce: prior/new medians 399.9/395.5 ms. Other prior/new
medians were interactive 623.0/638.1, skeleton 425.6/448.7 and depth-30
619.4/622.1. Artifact:
`/tmp/ad-native-eval/activation-only-numbers-performance.json`. This bounds the
observed variation but does not establish statistical equivalence. The final
binary is 3,088,656 bytes and core's dependency tree contains no platform crates.
