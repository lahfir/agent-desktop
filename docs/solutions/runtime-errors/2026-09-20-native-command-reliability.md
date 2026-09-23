---
module: agent-desktop
date: 2026-09-20
problem_type: runtime_error
tags: [native, scrolling, refs, identity, headless, reliability]
---

# Native command evaluation: scrolling and ref continuity

The evaluation now includes command-level scenarios as requested: scroll,
scroll-to, select, expand/collapse and drag policy. A failed Numbers text edit
remains an unmet task; it does not erase independently demonstrated command
improvements. These bounded trials do not establish universal reliability.

## Environment and ownership

SF Symbols 7.0 was not running before this probe. Agent Desktop launched it
without activation: PID 57617, process generation
`macos-proc-v1:1789865265:554603`, window w-17404, title All. Its native symbol
collection contains 6,984 items. Only this newly opened window was operated.
The namespace is `/tmp/ad-native-eval/symbols-state`.

Pages was not installed, so no Pages evaluation was run. Xcode and Numbers
retain their separate evidence ledgers. No app installation was attempted.

Baseline for this repair: `/tmp/ad-native-eval/activation-candidate`, SHA-256
`49c75115e6600306ee42947484bc0982126585c0a137909f3d9745740fe84ab7`.
Candidate: `/tmp/ad-native-eval/scroll-ref-candidate`, SHA-256
`f7ae0196e788acdbeca3f5dcc6c86c942ce347f2b7f81ec0dc4c1d8d49759276`.
Both include earlier local repairs; neither is the unmodified main release.

## Reproduction and minimal repair

1. An original skeleton exposed the main grid's scroll area as
   `@s3p24n9238yc7t:e44` and its scrollbar as `@s3p24n9238yc7t:e47`, value 0.
2. Headless `scroll ...:e44 --direction down --amount 1` returned
   delivered_verified. Before/after screenshots independently showed the
   viewport changing from the first symbols to sports symbols.
3. `get ...:e47 --property value` then failed after five seconds in broad
   resolution, with 9,969 attribute batches. A fresh snapshot and fresh ref
   read showed the same scrollbar geometry and value `0.10000000149011612`.
4. Core's shared mutable-value role list omitted `scrollbar` and `handle`.
   Consequently, the old value 0 was treated as stable identity and rejected
   the same live scrollbar after a legitimate scroll.
5. The production repair adds those two canonical roles to
   `is_mutable_value_role`. No timeout, fuzzy matching, path trust, geometry
   requirement, native identifier precedence or ambiguity rule was relaxed.
6. The original pre-scroll ref then returned the live value successfully on
   the candidate. The test for value-independent identity failed before the
   repair and passed afterward. Matching values alone remain insufficient
   identity; distinct named scroll controls remain distinct.

This is a core identity-classification defect, not an AX delivery defect.
All users of the shared ref-identity helpers receive the correction, including
macOS resolution and locator hydration. Changed geometry without stable identity
still requires safe refusal; the fix does not promise arbitrary thumb recovery.

## Observed command outcomes

| Scenario | Outcome | Evidence and limits |
|---|---|---|
| Native grid scroll down | Baseline 1/1 effect verified | Semantic AX path and before/after screenshots |
| Original scrollbar get after scroll | Baseline 0/1, candidate 1/1 | Same original qualified ref; changed live value |
| Five down/up cycles with original refs | Candidate 10/10 scrolls, 10/10 reads | Every read alternated between 0.10000000149011612 and 0; no response rewriting or retry after failure |
| Scroll-to original offscreen Math item | 1/1 verified | Math visible in screenshot; original ref get-states returned no offscreen state |
| Headless ref drag | 1/1 expected policy refusal | POLICY_DENIED, not_delivered, retry safe; no headed input invoked |
| Physical drag-and-drop effect | Not run | Requires headed policy, outside the strict headless trial |
| Select Math by the original outline ref | Baseline failed; repaired candidate 4/4 category transitions | Canonical child labels now reused; original selected-row refs verified each transition; Math screenshot confirms 78 symbols |
| Expand Weight combobox | Failed | Advertised Expand, but no readable expansion state; action chain refused without delivery |

The ten-scroll sequence is one continuous batch, not the planned 30 runs across
three independent resets. Native action acknowledgments plus AX values were
checked on every step. Screenshots independently verified the initial real
scroll and offscreen-target result, not every step of the repeated sequence.

Artifacts under `/tmp/ad-native-eval/`:

- `symbols-before-scroll.png`
- `symbols-after-scroll.png`
- `symbols-scroll-to-math.png`
- `scroll-ref-workspace-tests.log`
- `scroll-ref-safe-semantic.log`

Focused identity tests, full workspace tests, Clippy with warnings denied,
formatting and diff checks passed. The seven native semantic assertions passed
on the exact candidate, including single button delivery and rejection of a
silent text-insertion no-op. Performance comparison results are recorded below
when complete; do not infer a performance pass from unit tests.

The scroll candidate comparison completed at `scroll-ref-performance/report.html`.
Numbers completed 24/24 observations with matching shapes; interactive p50 was
523.5 ms baseline versus 611.1 ms candidate, while skeleton/depth-30/find were
405.5/524.2/352.3 ms versus baseline 418/535.1/353.3 ms. SF Symbols find-button
failed on both versions and depth-30 shapes differed, so that app did not pass
the complete comparison. Xcode was no longer running and was skipped.

## Canonical collection selection repair

The old `select` searched only AXTitle and AXDescription, whereas snapshots
derive the Math row's name from child content. On the original outline ref
`@s3p24n9238yc7t:e2`, selecting Math returned ELEMENT_NOT_FOUND before mutation.
The macOS selection search now uses the same bounded `read_node` name evidence
as snapshots. Candidates must advertise Click; inert text cannot become an
action target merely because its text matches. Unknown evidence fails closed.
Missing collection items now explicitly report not_delivered.

Candidate `/tmp/ad-native-eval/select-name-candidate`, SHA-256
`78bbc858c3ea81d595e38402f41e2290eda1bf6586b1dab1e42f0651757d33e1`,
completed Math, All, Math, All using the original outline ref. Every result
reported delivered_verified, and subsequent original-row reads confirmed
selected. `symbols-selected-math.png` independently shows Math and 78 symbols.
This remains four transitions in one app process, not a cross-app acceptance
score. Menu selection also uses this shared search; its live regression coverage
is still outstanding. Existing first-match behavior and the depth-eight search
limit remain review items, not claimed solved by canonical naming.

Eight focused selection tests, Clippy, formatting and diff checks passed.
The sandboxed workspace run failed two CLI event-wait tests; the complete
workspace suite passed when run with macOS observation access. Logs are
`select-name-workspace.log` and `select-name-workspace-native.log`.
The native safety suite stopped before testing because foreground identity was
missing or ambiguous (`select-name-safe-semantic.log`); it is not a pass.
The exact-candidate performance comparison is under `select-name-performance`.

That comparison completed and its generated HTML was reviewed. Numbers passed
all 24 observations with matching first-snapshot shapes. Candidate/baseline p50
in milliseconds: interactive 584.3/548.0, skeleton 429.5/407.2, depth-30
523.6/592.0, find 387.4/353.1. These three-round samples are not an isolated
measurement of the selection change. SF Symbols again failed find-button on
both versions and had differing depth-30 shapes; no complete performance pass
is claimed. Fixture comparison was skipped after the foreground guard failed.

## Raw AX disclosure diagnosis

A separate read-only Swift probe confirmed that both outer and inner Weight
controls are AXPopUpButton, publish AXShowMenu and AXPress, and return
kAXErrorAttributeUnsupported (-25205) for AXExpanded and AXDisclosing. The outer
control has an inner AXPopUpButton child; the inner control has no children
while closed. Family has the same structure. No raw mutation was performed.
This supports testing native menu-opening semantics rather than blindly toggling
AXPress when expansion state is unknown. No expansion repair is claimed yet.

The diagnostic initially refused its title guard: raw AX exposes the title
`All – 6,984 Symbols`, while list-windows reports `All`. AXIsProcessTrusted was
true and AXWindows succeeded. Retargeting the exact observed raw title allowed
inspection. The probe source is `/tmp/ad-native-eval/symbols-disclosure-probe.swift`.

### Monitored native menu trial and discovery defect

A separate raw AXShowMenu trial kept its client alive for ten seconds and
registered NSWorkspace activation notifications. It checked that the inner
Weight popup had no children and SF Symbols was not frontmost. AX returned
success. A targeted Agent Desktop drill then exposed nine menu items under both
outer and inner popups. The new outer ref was `@s3p24n9238yc7t:e75`, inner e76.

Foreground PID started at 2058; activation events were 860, 2058, then 57617;
the ending foreground PID was 57617 (SF Symbols). This trial did not preserve
the background state and cannot qualify as a headless success. Other foreground
activity occurred, so these observations alone cannot attribute every activation
to AX. No menu-opening fallback was added. Later raw observation showed popup
children empty again; no further menu mutation was performed.

Despite the targeted drill exposing the menu, app-wide `snapshot --surface menu`
failed at the unrelated 6,984-item grid: AXChildren loaded_count 4096, incomplete.
The shared search aborted immediately on that branch. It now searches
breadth-first and defers AppUnresponsive branch errors while inspecting siblings.
A positively observed menu can be returned; otherwise the incomplete error is
preserved. Permission errors and deadlines remain terminal, and the 2,048-node
limit remains. This changes observation only, not native action delivery or policy.

The sibling-menu regression failed before the repair and passed afterward.
Seven surface-search tests passed, including unknown absence, cycles, deep menus,
deadlines and terminal permission errors. Candidate
`/tmp/ad-native-eval/menu-sibling-candidate` has SHA-256
`105540fcf083bd93ac835cbc9ddf8d1a7af472557bf8270b1675a0d641bb8f67`.
With the real menu subsequently gone, the candidate retained the incomplete
search error. Positive live discovery on this candidate is unverified; the menu
was not reopened to manufacture a passing trial.

Clippy, formatting and diff checks passed. The sandboxed library run hit thirteen
loopback-binding tests denied by the sandbox. The full workspace run with required
access and the read-only performance comparison are recorded in
`menu-sibling-workspace-native.log` and `menu-sibling-performance/`.
The complete workspace run passed. The performance report completed and was
reviewed: Numbers was no longer running and was skipped. SF Symbols interactive
and skeleton shapes matched, with candidate/baseline p50 3067.0/3081.8 ms and
295.0/339.8 ms. Depth-30 shapes differed and find-button failed on both versions.
This is not a complete performance or live-menu acceptance pass.

## Stronger headless observation

`tests/real-apps/headless_probe.swift` observes activation notifications,
session-wide input counters and sampled pointer position. It is a bounded,
read-only diagnostic, not a driver or scoring framework. Its counter arithmetic
self-test and Swift compilation passed. It records no key contents.

A 60-second monitored Numbers trial completed two sheet transitions using
original refs. Public waits and screenshots confirmed both states; the final
document oracle returned Sheet 1, seed-target, neighbor-guard and other-sheet-guard.
The monitor recorded 2,999 pointer samples and activation events for three other
PIDs, none for Numbers. Input counts increased during unrelated foreground
activity. Therefore the evidence supports no observed Numbers activation, but
cannot establish clean input isolation or attribute those events to an agent.
Preserve `/tmp/ad-native-eval/numbers-headless-monitor.jsonl` and the two
`monitored-*.png` screenshots. No rating credit is inferred from unknown attribution.
