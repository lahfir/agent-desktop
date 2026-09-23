---
module: agent-desktop-macos
date: 2026-09-20
problem_type: runtime_error
tags: [states, viewport, find, snapshot, consistency]
---

# Ref-rooted observations used the element as its own viewport

SF Symbols first-button find reported offscreen while get-states on the same ref
returned an empty list. Its bounds were x=194, y=82, width=0, height=0. The owning
window was x=0, y=30, width=1496, height=937.

Element-root observation initialized TreeBuildContext with entry.geometry.bounds
as window bounds. Selected-match hydration therefore clipped the zero-size button
against itself. Live get instead read AXWindow. The mismatch was in observation
context, not a changed element or a stale ref.

The owning-window lookup and its existing tests move from action post-state code
to tree/element_bounds.rs. Live state and element-root observation both use it.
No fallback to AXTopLevelUIElement is introduced: window-less menus must not use
the menu bar as their viewport. Failed AXWindow reads still propagate errors.
Window-root observation retains the resolved window geometry.

Candidate `/tmp/ad-native-eval/window-context-candidate`, SHA-256
`2a4e3e248893e44432494c8b7d2a6d027ff8a76cc3058c52f029592eecaa6500`,
returned no offscreen state for first-button ref `@sk3p38f86rkv8:e1`.
Public get-states and a snapshot rooted at that same ref agreed. The Numbers
table-rooted exact-value editor search still returned seed-target, focused and
selected. No UI mutation was needed to verify this repair.

Full workspace, Clippy, formatting and diff checks pass. The initial sandboxed
macOS run had six socket-permission failures; the escalated workspace run passed.
The existing owning-window tests cover absent windows and failed reads without
weaker fallback. The native semantic suite passed all seven assertions on the
exact candidate, including no fixture foreground activation. The completed
`/tmp/ad-native-eval/window-context-performance/` comparison passed all candidate
observations. Numbers snapshot shapes matched baseline; candidate/base p50 ms
were 509.9/523.8 interactive, 376.2/373.8 skeleton, 523.5/529.6 depth-30 and
346.7/359.8 first-button find. SF Symbols find passed 3/3 versus 0/3 baseline;
its interactive and skeleton shapes matched, while depth-30 node counts differed.
This establishes bounded regression evidence, not full-content equivalence or
the thirty-run acceptance requirement.

Blast radius: ref-rooted observation now performs an owning-window read and uses
its current geometry. This changes inferred offscreen state for elements whose
root geometry differed from their window. It does not establish clipping by
every intermediate scroll container, or prove a zero-size control is clickable.
