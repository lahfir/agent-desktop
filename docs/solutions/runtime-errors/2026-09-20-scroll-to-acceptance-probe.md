---
module: agent-desktop-macos
date: 2026-09-20
problem_type: runtime_error
tags: [scroll-to, sf-symbols, acceptance, headless]
---

# SF Symbols scroll-to acceptance probe

Candidate: `/tmp/ad-native-eval/window-context-candidate`, SHA-256
`2a4e3e248893e44432494c8b7d2a6d027ff8a76cc3058c52f029592eecaa6500`.
PID 57617, process instance macos-proc-v1:1789865265:554603, window w-17404,
namespace `/tmp/ad-native-eval/symbols-state`. Window observation reported
unfocused, visible, bounds x116/y107/1496x937.

The public skeleton returned All at `@s350voae14kj3g:e4`, selected and offscreen,
and sidebar scrollbar `@s350voae14kj3g:e38` at 0.8000000715255737.
The model selected the refs from the original JSON without extraction scripts.

One `scroll-to @s350voae14kj3g:e4` returned delivered_verified through the
semantic scroll_to_visible_verified step. `get` on the same ref returned only
selected; its current bounds were x124/y124.5/203x40. The original scrollbar
ref remained valid after the action and read 0.10000001639127731.
Screenshot `/tmp/ad-native-eval/acceptance-scrollto-all.png` was inspected:
All appeared at the top of the sidebar, partly underneath the translucent
titlebar. This establishes movement and surviving refs, not unobstructed
clickability or a full acceptance pass.

The forty-five-second headless observer reported no activation events and
foreground PID 83470 throughout. It also recorded pointer changes, keyboard,
mouse drag and wheel activity. Attribution is unknown, so the trial is
contaminated for strict headless acceptance. It does not count toward the
thirty valid runs or establish a new overall score. The user was asked for an
idle-desktop interval before repeating qualification, rather than relabeling
environmental activity as a clean run.

Source inspection found that scroll-to verifies intersection with the owning
window, whereas pointer hit testing additionally clips against ancestors. That
is a coverage concern for nested scroll areas, not a demonstrated defect in
this trial: partial visibility is currently accepted, and no code change was
made merely because the screenshot included a translucent titlebar.

Numbers N4/N5/N8 remain uncompleted. This independent S4 probe does not replace
those required outcomes.
