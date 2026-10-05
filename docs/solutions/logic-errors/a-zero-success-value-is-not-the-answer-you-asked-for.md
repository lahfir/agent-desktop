---
title: A zero success value is not the answer you asked for
date: 2026-08-08
category: logic-errors
module: crates/windows/src/actions/physical_target.rs, crates/windows/src/tree/hit_test_corroborate.rs
problem_type: logic_error
component: platform-adapter
severity: high
applies_when:
  - "Reading a native handle, id, or pointer that the API returns as 0 for absence"
  - "A helper elsewhere in the crate already resolves the same thing"
  - "Writing a precondition that decides whether an action may proceed"
tags: [uia, native-handle, sentinel, reuse, preconditions, windows]
---

# A zero success value is not the answer you asked for

## Context

Windows physical clicks must confirm the target's window owns the desktop
foreground before injecting, because `SendInput` goes to the foreground
queue and has no per-process targeting. The check read the element's window
handle and asked whether it was foreground:

```rust
fn target_window_is_foreground(element: &UIAElement) -> bool {
    element.0.get_native_window_handle().ok()
        .map(|handle| is_root_foreground_window(HWND::from(handle).0))
        .unwrap_or(false)
}
```

That reads correctly. It is also wrong for most of the desktop.

## Problem

UIA returns `NativeWindowHandle` **0** — as a success, not an error — for
every element that is not itself a window. That is the normal shape for WPF,
WinUI, UWP and Chromium/Electron content, where one HWND hosts a whole tree
of elements. So `.ok()` succeeded, the handle was null,
`is_root_foreground_window` returned false for a null handle, and the gate
concluded "this element has no window, therefore it is not foreground."

`double-click`, `triple-click` and `right-click` were refused before any
injection, with a plausible-looking `ACTION_FAILED` / `not_delivered`, on
the majority of modern desktop UI — while working on plain Win32 controls,
which do carry their own HWND. The failure was invisible to the test suite
because every test passed the resolved `foreground_ready` boolean in
directly, so the function that computed it never ran.

The same crate already had the correct resolution, written for the occlusion
gate a sub-phase earlier:

```rust
if let Ok(handle) = current.0.get_native_window_handle() {
    if value != 0 { return Some(value); }   // else: keep climbing
}
```

`first_native_hwnd` treats `Ok(0)` as "not the answer, look at the parent"
and walks up until an ancestor owns a handle. The new code re-derived a
naive version of a question the crate had already answered correctly.

## Root cause

Two failures compounding.

The first is treating a sentinel as data. `Result::ok()` answers *did the
call succeed*, not *did it give me a usable value*. When an API encodes
absence as a valid-looking zero, the `Ok` arm still needs a domain check,
and the absence usually means "ask somewhere else" rather than "the answer
is no."

The second is that the correct resolution existed and was not reused. It sat
in a module named for the occlusion gate (`hit_test_corroborate`), not for
the concept it implements — resolving an element's host window — so a
developer working on physical input had no reason to look there. Naming a
general capability after its first consumer is how it gets re-derived, and
the re-derivation is rarely as careful as the original.

## Solution

Resolve the handle the way the rest of the crate resolves it, and say why
the climb exists so the next reader does not flatten it back:

```rust
/// UIA reports `NativeWindowHandle` 0 - success, not failure - for every
/// element that is not itself a window, which is the normal shape for WPF,
/// WinUI, UWP and Chromium content. Reading the leaf's handle alone would
/// therefore answer "no window, not foreground" for most modern UI.
fn target_window_is_foreground(element: &UIAElement, deadline: Deadline) -> bool {
    host_window_handle(element, deadline).is_some_and(is_root_foreground_window)
}
```

Pin it where the shape actually exists. A unit test with a fabricated
element cannot express "a real control whose own handle is 0", so the pin is
a live one that asserts both halves: the leaf handle *is* zero, and the host
still resolves. If the first assertion ever fails, the test says so — it has
stopped covering the climb rather than silently passing.

## Prevention

- For any native read that can return 0 / null / empty as success, decide
  explicitly what that value means before using it. If it means "not here",
  the code must continue looking, not conclude.
- Before writing a resolution over a platform handle, grep for one. If a
  helper exists in a module named after another feature, that is a naming
  problem to fix, not a reason to write a second one.
- A precondition that decides whether an action runs at all deserves a test
  that executes it. Injecting its result as a literal tests the branch, not
  the decision.

## Related

- [Tri-state evidence collapses under negation](tri-state-evidence-collapses-under-negation.md)
- [A test that cannot fail is not coverage](../best-practices/a-test-that-cannot-fail-is-not-coverage.md)
- [Deduplicate the ref allocator via a config struct](../best-practices/deduplicate-ref-allocator-via-config-struct-2026-04-14.md)
