---
name: agent-desktop-windows
version: 0.8.4
tags: windows-automation, accessibility, uia, ai-agent, gui-automation, cli
requirements:
  - agent-desktop
description: >
  Windows platform guide for the agent-desktop CLI. Lists per command group
  what works on Windows and what refuses, the first-contact gotchas
  (PowerShell ref quoting, minimized windows, headed-only input), and where
  to find shell surfaces, notifications, the cursor overlay, UIPI elevation,
  Chromium/Electron settle behavior and troubleshooting.
  Triggers on: "windows desktop automation", "UIA tree", "automate Windows app",
  "agent-desktop on Windows", "elevated window input blocked", "E_ACCESSDENIED",
  or any Windows GUI interaction task.
---

# agent-desktop-windows

The observe-act loop, ref system, JSON envelope, sessions and tracing are the
same on every platform. This file covers only what differs on Windows.

Requires Windows 10 1809+ / Windows Server 2019+ (x64 or ARM64). Run it in the
interactive desktop session of the signed-in user. Session 0 (services and
SYSTEM tasks), Server Core, the secure desktop (UAC prompts, the sign-in
screen), a locked desktop and another user's session are not supported:
observation, actions and capture there fail or return nothing.

## Capability Table

| Group | Commands | Status on Windows |
|-------|----------|-------------------|
| Observation | `snapshot`, `find`, `get`, `is`, `screenshot` | Works; surfaces are `window`, `focused`, a Chromium modal reached as `sheet`, and an open application menu reached as `menu` |
| Surfaces | `list-surfaces` | Works; per-process inventory of `window`, `focused` and `sheet` surfaces plus a `menu` surface carrying `item_count` when a menu is open |
| Shell surfaces | `open-system-surface` | Works; raises `start-menu`, `taskbar`, `system-tray`, `system-tray-overflow` or `action-center` and returns the window identity `snapshot --surface <kind>` consumes; `quick-settings` returns `PLATFORM_NOT_SUPPORTED` naming `action-center` |
| Ref interaction | `click`, `right-click`, `type`, `set-value`, `clear`, `select`, `toggle`, `check`, `uncheck`, `expand`, `collapse`, `scroll`, `scroll-to` | Works; semantic delivery in headless and `--headed` modes alike, except `right-click`, which is a physical click and returns `POLICY_DENIED` unless `--headed` (`type` also requires focus permission and returns `POLICY_DENIED` under strict headless) |
| Multi-click and focus | `double-click`, `triple-click`, `focus` | Works with global `--headed`; focus is headed-required (A3-4, A19-5) |
| Keyboard and mouse | `press`, `hover`, `drag`, `mouse-click`, `mouse-move`, `mouse-wheel` | Works; cursor-moving input requires `--headed`; `press --app` synthesizes into the foreground queue after verification (fails closed headless if not already frontmost; non-interactive callers report `delivered_unverified`) |
| Held input | `key-down`, `key-up`, `mouse-down`, `mouse-up` | Fails closed with `ACTION_NOT_SUPPORTED` on every platform until a daemon owns held-input lifetime |
| App and window | `launch`, `close-app`, `list-windows`, `list-apps`, `focus-window`, `resize-window`, `move-window`, `minimize`, `maximize`, `restore` | Works; `launch` resolves an absolute path or a bare name under System32 or the Windows directory (A21-1), not display names |
| Displays | `list-displays` | Works |
| Clipboard | `clipboard-get`, `clipboard-set`, `clipboard-clear` | Works; typed text and image content |
| Wait | `wait` | Works for ms, `--element`, `--window`, `--text`, `--menu`, `--menu-closed`, `--event`, and `--notification` predicates |
| Notifications | `list-notifications`, `dismiss-notification`, `dismiss-all-notifications`, `notification-action` | Works over the Action Center; the commands that raise shell chrome take the foreground, so pass `--headed` when the center is closed |
| Cursor overlay | `cursor-overlay` | Works; `enable` renders a click-through overlay and `data.rendered` is the renderer's own pipe acknowledgement, not merely a spawned process. `disable` carries no `rendered` field. See Cursor Overlay below |
| System | `status`, `permissions`, `version`, `batch`, `skills`, `session`, `trace` | Works |

## First Contact

- **Quote every ref in PowerShell.** PowerShell reads a bare `@token` as its
  splatting operator and deletes the argument before the binary sees it, so
  `set-value @s8f3k2p9:e1 hi` fails `INVALID_ARGS`. Write
  `set-value '@s8f3k2p9:e1' hi`. cmd.exe and bash need no quoting.
- **A window behind another window is drivable. A minimized one is not.**
  Every element of a minimized window reports `offscreen` and ref actions
  fail. Run `restore --app <image>` and snapshot again. Do not use
  `focus-window` by reflex: it steals the user's foreground.
- **No permission dialog exists.** `permissions` reports `automation` as
  `not_required`. Elevation boundaries are the only access control.
- **Chromium and Electron apps read thin before they settle.** Pass
  `--timeout-ms` to `snapshot` instead of concluding the tree is empty.
- **Headless by default.** Ref actions stay semantic. Only `--headed` commands
  verify exact-window focus and synthesize physical input. `alt+f4`, `win+l`,
  `win+d`, `alt+tab` and modifier supersets are refused without `--force`.
- **`type` needs focus permission.** It is physical keystroke synthesis, so a
  strict-headless `type` returns `POLICY_DENIED`. It always ends
  `delivered_unverified` (`verification_scope: "unavailable"`): confirm with a
  fresh `get`.
- **`press --app` writes to the foreground queue** with no per-process
  targeting. A headless press to a non-frontmost app fails closed. A
  non-interactive caller (service, scheduled task, CI job) gets
  `delivered_unverified`.
- **`launch` needs an absolute path** or a bare name under System32 or the
  Windows directory, not a display name. Use `list-apps` to find a running app.
- **Notifications:** `list-notifications` with the Action Center closed needs
  `--headed`. Single-item `dismiss-notification` and `notification-action` need
  `--headed` and `--expected-app` or `--expected-title` for the entry at INDEX.
- **Surfaces:** an open or context menu is the `menu` surface. Windows does
  not advertise a `menubar` surface.
- **Commands that raise shell chrome take the foreground.** Strict headless
  refuses them with `POLICY_DENIED` before anything is raised.
- **Stateful actions run once, then verify by re-reading.** `delivered_verified`
  means the readback matched. `ACTION_FAILED` with
  `details.kind: "post_action_verification"` means the action landed but the
  readback differs: read `details.post_state` and do not retry. A `set-value`
  the app reformats reads as failed. See `references/troubleshooting.md`.
- **Unsigned binary warnings are expected.** See `references/troubleshooting.md`.

## What differs on Windows

- Ref actions are semantic UI Automation calls, in headless and `--headed` mode.
- Notifications work over the Action Center and need `--headed`.
- `launch` takes an absolute path or a bare name under System32 or the Windows
  directory, not a display name.
- Held-input commands fail closed on every platform.
- The cursor overlay renders only for headless semantic actions.
- In key combos, `cmd` and `meta` are the Windows key, so `meta+c` is a shell
  shortcut. Write `ctrl+c` to copy.
- Headless `type` is refused with `POLICY_DENIED`: use `set-value`, or pass
  `--headed` to type through `SendInput`. `focus` also needs `--headed`.

## References

- `references/shell-and-overlay.md`: open it for `open-system-surface`, tray
  and taskbar surfaces, notifications, the cursor overlay, hosted (UWP) window
  identity and menu detection coverage.
- `references/permissions-and-elevation.md`: integrity levels, UIPI, blocked
  combos, protected processes, the cross-process interaction lease.
- `references/chromium-and-electron.md`: settle timing, covered-window hazard,
  WPF wrong-provider trap, unnamed web content.
- `references/troubleshooting.md`: symptom-to-cause map, action verification
  outcomes, saving a document headless, install warnings.

Read any file from the binary:
`agent-desktop skills get agent-desktop-windows references/troubleshooting.md`.
