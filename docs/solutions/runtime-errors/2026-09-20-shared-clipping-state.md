---
module: agent-desktop-macos
date: 2026-09-20
problem_type: runtime_error
tags: [visibility, clipping, command-consistency]
---

# Shared clipping state across observation commands

Snapshot traversal previously reused window bounds for every descendant.
Live state and ref-root observation also used only the owning window. A native
button clipped by a smaller scroll area therefore lacked offscreen state even
after scroll-to had been repaired separately.

Before that, element-root observation seeded its build context with the root
element's own bounds as the viewport. A zero-size SF Symbols button was clipped
against itself, so find reported it offscreen while get-states, which read
AXWindow, returned an empty list. The mismatch was in observation context, not a
changed element or a stale ref. The owning-window lookup now lives in
`tree/element_bounds.rs`. It does not fall back to AXTopLevelUIElement: for a
window-less menu that resolves to the menu bar, a 29-point strip that would
report every open menu item offscreen. A failed AXWindow read still propagates.

Traversal now carries the effective clipping rectangle down each branch and
restores the prior rectangle before visiting siblings, including error paths.
Ref-root and live observations read clipping ancestors under the existing
deadline. Rectangle intersection and clipping-role definitions are shared with
the pointer hit-test implementation. No per-node ancestry IPC was added to a
full traversal. Empty intersections remain known empty. Unknown ancestor
geometry cannot become known again merely because a deeper container has bounds.

The owned fixture now includes a visible sibling outside the scroll area.
Candidate 9dedd81c6561a354a6550b328877a623f73f019306cbfdb33f62a1ea7d06c6c9
reported offscreen consistently through snapshot, get, find and root snapshot;
the sibling remained visible. On the same unchanged target/ref, the preceding
nested-final candidate returned an empty state list. After semantic scroll-to,
the same target ref became visible and the sibling stayed unchanged, confirmed
by the inspected `/tmp/ad-native-eval/clipping-after.png`. The observer recorded
no target activation but unrelated input, so that trial is not a clean headless
qualification pass.

Final candidate `/tmp/ad-native-eval/clipping-final-candidate`, hash
f0ed9f26f9bf74b4d75e4004d3d773df91772e12301cd44c2013ae06fafadd5d,
adds the unknown-ancestor preservation guard. Workspace tests, clippy and format
passed. One previously saved anonymous scrollarea ref timed out with safe
not-delivered STALE_REF evidence; it was not silently counted as recovery. A
fresh snapshot ref successfully reset the fixture viewport. The saved button
ref and a new exact find then both reported offscreen on the final candidate.
Final semantic trial 1 stopped after five passes at the ownership checkpoint:
fixture window identity changed. The cause was not established and the attempt
is retained as failed. A separate fresh-fixture trial passed all seven assertions
on the same final hash. Final performance completed: candidate SF Symbols
observations succeeded 12/12; baseline first-button find failed 3/3. Snapshot
interactive medians were 3063.9/3064.5 ms, skeleton 305.8/292.9 ms, depth-30
3062.1/3072.1 ms, and candidate find 4099.2 ms. Partial depth-30 shapes differ;
neither complete equivalence nor a clean aggregate baseline pass is claimed.

Tests cover non-clipping groups, disjoint nested viewports and unknown geometry
(`only_clipping_containers_narrow_the_viewport`,
`disjoint_viewports_remain_known_empty_through_descendants`,
`missing_clipping_geometry_is_unknown_not_empty` and
`a_window_less_element_reports_no_clipping_viewport` in `tree/element_bounds.rs`).
The earlier candidate passed seven semantic assertions and SF Symbols read-only
performance observations. Xcode was absent and skipped, not passed. Numbers is
excluded from active testing by the user's explicit instruction. These fixes do
not establish repeated real-app acceptance.

## Real-app S3 baseline on the final candidate

SF Symbols PID 57617, window w-17404. The model chose outline
@s1pw1hb4whof0b:e1 from a scoped find and Math @swkwt9hq8o9kg:e1 from an exact
find. Selecting Math then All on the same outline returned delivered_verified
for both transitions. Saved Math/All refs reported selected as appropriate;
Math became unselected after returning to All. Inspected screenshots
`/tmp/ad-native-eval/s3-final-math.png` and `s3-final-all.png` showed the expected
78 and 6,984 symbol collections. The sixty-second observer reported no app
activations, zero input-event deltas and no pointer movement, with the target
still running and foreground PID 860 unchanged. Result: one successful S3
baseline round trip, not the required thirty runs across three reset batches.

S4 attempt 1 on the final candidate used the saved offscreen All ref
@ssvl7e1srna75:e4. Scroll-to returned delivered_verified and get-states returned
selected without offscreen. The requested screenshot failed with TIMEOUT:
NSWorkspace app inventory timed out. The thirty-second monitor recorded input
activity and target activation near its end. Attribution is unknown. The attempt
is retained as unqualified/contaminated with incomplete visual verification, not
a pass; SF Symbols mutations stopped while it was foreground.

Xcode setup then launched PID 24741 without an activation flag and opened only
the owned AD Reliability.swift file using background setup. List-windows exposed
the owned file as w-19712 and a separate Welcome window. Exact find scoped to
w-19712 returned Source Editor @s1j4qlrp4icwd:e1 with the expected guarded source.
The setup observer recorded Xcode activation and input activity; its cause is
unattributed. Discovery is functionally observed, but no clean background setup
or X2 edit pass is claimed. Document edits were held while Xcode was foreground.
