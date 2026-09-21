---
module: agent-desktop-macos
date: 2026-09-20
problem_type: runtime_error
tags: [scroll-to, clipping, false-success, regression]
---

# Scroll-to must verify the scroll viewport

An owned AppKit fixture placed a button outside a 300x100 scroll viewport but
inside its 500x632 window. The previous immutable window-context candidate
returned a verified skipped step, not_delivered/safe, while the inspected
`/tmp/ad-native-eval/nested-before.png` showed no button. The target advertised
only AXPress: the experiment's initial custom action override was not exposed
by AppKit and was removed from the retained fixture. No acknowledged custom
scroll action is needed to reproduce the failure.

Root cause: ancestor-scroll fallback calculated direction against AXWindow.
When the target was inside that rectangle it returned SatisfiedNoDelivery,
without checking the actual clipping ancestors. AXScrollToVisible verification
also used window intersection alone.

The repair reuses scroll::find_scroll_area for the nearest viewport and the
existing hit-test ancestor-clipping reader for positive visibility verification.
Unknown/clipped geometry cannot become verified success. The pre-existing
missing-AXWindow refusal is preserved. No physical delivery, activation,
app-specific condition or new abstraction was introduced. Nested outer-viewport
cases that cannot be resolved still fail rather than falsely verifying.

The repaired candidate actually scrolled the button into view, confirmed in
`nested-after.png`. The final candidate was retested with the retained plain
NSButton fixture: original ref @s3rcpysuegza8h:e2 returned delivered_verified,
and inspected `nested-final-after.png` showed the button. The monitor recorded
unrelated input and app changes, but no fixture activation; that final trial
is contaminated for strict headless qualification.

Final candidate `/tmp/ad-native-eval/nested-final-candidate`, SHA-256
3714c3f3903d22ea430eb48a9f5645ca41bfc6b0f7d5f93f48ff7856fa882a1c.
Workspace, clippy and formatting checks passed. The focused geometry regression
covers a target inside its window but above its scroll viewport. Live fixture
before/after evidence covers the actual routing, not just rectangle arithmetic.

The preceding candidate d1b13c390e0a2e8eb78a1e92e562e88e13e082d891a963fcc9331a56501c14e3
also scrolled the saved Math ref in SF Symbols into view. Its forty-five-second
observer recorded no activation, input events or pointer movement; screenshot
`nested-sf-math.png` and same-ref get-states confirmed the result. Its seven
semantic fixture assertions passed. Do not count these as final-candidate
qualification: the missing-window guard was subsequently restored.

Remaining consistency gap: snapshot/get offscreen inference still uses window
bounds and does not uniformly represent nested clipping. This repair removes
the observed scroll-to false success; it does not close that separate shared
observation contract. Numbers editing and the full acceptance series remain
uncompleted. All work is local; no commit, push or release.

Final gates completed: seven semantic fixture assertions on the exact final
hash, full workspace tests, clippy, format and diff checks. Core dependency
inspection contained no platform crates. The release remains approximately
3.1 MB. Read-only performance comparison completed at
`/tmp/ad-native-eval/nested-final-performance/report.html`: candidate 24/24
observations succeeded across Numbers and SF Symbols. Numbers shapes matched;
candidate/base p50 milliseconds were 301.5/302.4 interactive, 236.6/233.4
skeleton, 297.1/280.1 depth-30, and 218.2/225.8 find. The 17 ms depth-30 delta
is recorded, not treated as statistically established equivalence from three
rounds. SF Symbols snapshot medians remained near their three-second budget;
partial depth-30 shapes differed (920 versus 954 nodes), so equal completeness
is not claimed. First-button find succeeded 3/3 on candidate versus 0/3 on base,
reflecting the earlier locator budget repair. No observation-path behavior was
changed by this scroll-only patch.
