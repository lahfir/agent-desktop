---
title: Native Agent Cursor Overlay - Persistent Presence Addendum
type: feat
date: 2026-08-21
topic: agent-cursor-overlay
supersedes: 2026-08-21-0003-feat-agent-cursor-overlay-plan.md
---

# Persistent Presence Addendum

This addendum records the user-directed lifecycle changes accepted during live macOS dogfood. It supersedes KTD3, AE10, the process-lifecycle and privacy paragraphs, and any short-lived-child assumption in the original plan. The remaining product and planning contract stays unchanged.

## Lifecycle Contract

- `cursor-overlay enable` stores one session-wide setting and starts one macOS renderer for that session. It shows `Hey, let's play with this computer!` beside the persistent cursor.
- The cursor window remains visible and alive between commands. An action updates its position, description, pointer state, and click cue; completing an action never removes it.
- Only `cursor-overlay disable` or ending the owning session stops the renderer and removes the cursor.
- Action commands and batch entries inherit the session setting. They never accept a cursor-enabled argument.
- A headed command synchronously hides an existing custom cursor before using the real pointer and restores the persistent custom cursor afterward. It does not start a missing renderer.

## macOS Transport

- The shipped executable hosts one detached renderer child per enabled session.
- The initial bounded control arrives through inherited stdin. Later bounded controls use a session-derived, owner-only Unix-domain socket; labels never enter process arguments.
- A process-scoped startup lock prevents parallel commands from spawning duplicate renderer children.
- Hide and disable are acknowledged after the renderer applies them. Presentation remains fail-soft and never changes action delivery, JSON, exit status, or retry guidance.
- Malformed or failed later presentation messages do not terminate the persistent renderer.

## Platform Boundary

Core owns only portable configuration, motion, placement, control messages, and the default no-op adapter method. The binary remains the target-selection wiring point. The macOS crate alone owns process hosting, AppKit windows, drawing, display cadence, and transport. Windows and Linux need only implement that adapter method with their native renderer; no command, batch, core, or response-contract changes are required.

## Updated Acceptance Examples

- After enable, the greeting card and cursor remain visible for the entire session.
- After any eligible click, the hand transition and ripple finish and the arrow remains at the destination.
- When the description changes, the old card eases out and the new text eases in without restarting the renderer. The current card remains until that change.
- A headed action shows only the real cursor during the action, then restores the custom cursor.
- Disable returns only after the renderer has removed its socket and visual surface, so an immediate enable starts a fresh renderer reliably.
