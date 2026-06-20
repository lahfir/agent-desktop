---
title: Abort-state guidance on multi-step physical input errors
date: 2026-06-10
category: best-practices
module: crates/macos
problem_type: best_practice
component: macos-adapter
severity: high
applies_when:
  - Implementing a multi-step physical input sequence (drag, gesture, press-hold) using CGEvent synthesis
  - An early return or error exit can leave cursor or button state at an intermediate position
  - A Drop guard is introduced to cancel a partially committed OS operation
  - An error from a multi-phase input helper needs recovery guidance attached
  - The caller must distinguish "no drop committed" from "drop committed at the wrong target"
tags:
  - drag
  - cgevent
  - abort-state
  - error-recovery
  - mouse-input
  - headless-headed
  - macos
  - physical-fallback
---

# Abort-state guidance on multi-step physical input errors

## Context

A synthetic drag is a multi-step OS mutation: `LeftMouseDown` at the origin, interpolated `LeftMouseDragged` events, a dwell over the destination, then a final `LeftMouseUp`. Any step after the mouse-down can fail, and the original `MouseUpGuard` in `crates/macos/src/input/mouse.rs` had two bugs at once. The happy path disarmed the guard *before* posting the final `LeftMouseUp`, so a failure on that last post left the button logically held down system-wide — the exact defect the guard existed to prevent. And the guard's corrective release on `Drop` fired at the *destination*: CGEvents resolve at the coordinates embedded in the event, so a "corrective" up at the destination is indistinguishable from a successful drop. The file landed in the target folder while the command reported failure — the agent's perception (error) and the system's state (dropped) irreconcilably diverged.

## Guidance

The pattern has three coordinated legs.

**Leg 1 — the guard owns the release, and disarms only after the post succeeds.**

```rust
// crates/macos/src/input/mouse.rs
impl MouseUpGuard {
    fn release_at(&mut self, point: CGPoint) -> Result<(), AdapterError> {
        post_event(CGEventType::LeftMouseUp, point, CGMouseButton::Left)?;
        self.armed = false;
        Ok(())
    }
}
```

The happy path calls `release.release_at(to)` as its final statement. If that post fails, `armed` stays `true` and `Drop` still runs the abort path.

**Leg 2 — abort at the origin, never the unreached destination.**

```rust
impl Drop for MouseUpGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = post_event(CGEventType::LeftMouseDragged, self.origin, CGMouseButton::Left);
            let _ = post_event(CGEventType::LeftMouseUp, self.origin, CGMouseButton::Left);
        }
    }
}
```

`origin` is captured at `LeftMouseDown`. Releasing where the gesture picked up is a self-drop — a no-op to most drop targets — which gives the abort genuine cancel semantics. There is no "current cursor position" escape hatch: the coordinate in the event is where the OS resolves the release.

**Leg 3 — every error from the sequence carries the end state.**

```rust
pub fn synthesize_drag(params: DragParams) -> Result<(), AdapterError> {
    drag_sequence(params).map_err(|err| {
        if err.suggestion.is_some() {
            return err;
        }
        err.with_suggestion(
            "The drag was aborted: the button was released back at the origin (best-effort) and no drop was committed at the destination. The cursor ends at the origin. Re-check the source state before retrying.",
        )
    })
}
```

One `map_err` at the public boundary; inner errors that already carry a tailored suggestion pass through unchanged. The guard's doc comment also states the best-effort limits honestly: the corrective posts can themselves fail (typically the same systemic CGEventSource failure that aborted the drag), and a drop target sitting under the origin still sees a self-drop.

## Why This Matters

Without leg 1, a failure on the final post leaves the button logically held: the OS treats every subsequent cursor movement as a drag and every click target as a drop target, corrupting unrelated interactions until something releases the button.

Without leg 2, an aborted drag silently commits as a real drop. This is the worst failure class for an agent caller: the error says "retry", but the filesystem or UI already changed — retrying duplicates the operation.

Without leg 3, an agent holding `ACTION_FAILED`/`INTERNAL` cannot tell whether the world changed. With the suggestion, it knows: button released at origin, no drop committed, cursor at origin — and whether a re-snapshot is needed before retry.

## When to Apply

- A command posts a sequence of OS-level events where a later step can fail after an earlier irreversible step succeeded
- The abort path could itself mutate state (a corrective event at the wrong coordinates commits, not cancels)
- Errors must communicate post-failure system state, not just the failure cause, for callers to retry safely

Specifically necessary for anything backed by `CGEventPost`-style APIs that resolve coordinates at event-creation time.

## Examples

Before (both bugs, as originally shipped):

```rust
// happy path disarmed BEFORE the final fallible post
release.armed = false;
post_event(CGEventType::LeftMouseUp, to, CGMouseButton::Left)

// and Drop released at the DESTINATION, committing the aborted drop
impl Drop for MouseUpGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = post_event(CGEventType::LeftMouseUp, self.to, CGMouseButton::Left);
        }
    }
}
```

The fixed version is the Guidance section above. Verified by the E2E drag scenario observing the canvas effect, not the command's `ok:true`.

## Related

- `best-practices/macos-gesture-headless-capability-2026-06-10.md` — drag/hover/mouse-* are always physical and policy-gated; establishes when this multi-step path runs at all
- `best-practices/playwright-grade-desktop-reliability-2026-06-02.md` — the upstream principle that fallbacks must be explicit and failures honest; abort-state cleanup is that principle applied to mid-sequence failure
- `best-practices/preserve-command-policy-semantics-during-refactor-2026-05-12.md` — when the physical path is even entered (policy ownership) — context for why aborts are headed-mode territory
