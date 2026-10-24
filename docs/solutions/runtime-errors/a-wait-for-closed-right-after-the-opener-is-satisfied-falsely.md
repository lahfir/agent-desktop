---
title: A wait for "closed" right after the opener is satisfied falsely
date: 2026-10-19
category: runtime-errors
module: crates/linux/src/system/interaction/action_clock.rs, crates/linux/src/system/interaction/action_clock_store.rs, crates/linux/src/system/menu/menu_wait.rs
problem_type: runtime_error
component: atspi-menu-wait
symptoms:
  - "wait --menu-closed returns at once after a click that opens a GTK4 popover, before the menu ever showed."
  - "The same sequence passes when click and wait run in one process and fails when they run as two CLI calls."
tags: [linux, at-spi, gtk4, menu, wait, race, persisted-state]
---

## Problem

A GTK4 popover menu appears in the accessibility tree 157 to 331 ms after its opener action returns (L18-2). The live probe saw it open at 540 ms on Calculator and 688 ms on Files (L33-51). A `wait --menu-closed` that starts right after the opener polls a menu that is not yet visible, reads "closed", and returns. The wait is satisfied by the state it was meant to outlast.

## Solution

A closed-direction wait holds until it has seen the menu open, or until 2 s pass since the last action, whichever comes first. `menu_wait.rs` asks `action_clock::recent_for(pid)`; the clock marks every action that started, that is, anything but one refused before it sent (`action_clock::mark_outcome`).

The memory has to outlive the process. An agent runs `click` and `wait` as two CLI calls, so an in-process `Instant` is empty in the second one. `action_clock_store.rs` writes a per-process record under `$XDG_RUNTIME_DIR/agent-desktop-actions`, stamped in milliseconds of boot time (suspend included) so two processes agree and a restart cannot make an old record look new, and keyed by a process-instance token so a reused pid does not inherit it. The first live legs used the in-process clock and passed 10 of 10 (L33-17); only the two-process case (L33-56) exercises the persisted store.

## Prevention

- Test a wait that depends on a prior action in two processes, not one. A unit test with an in-process clock cannot fail for this reason.
- Do not size the hold from one toolkit. The popover delay is measured per toolkit (L18-2, L33-51).
- Treat "not yet visible" and "closed" as different states; the wait must know which one it read.
