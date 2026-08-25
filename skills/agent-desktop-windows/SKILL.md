---
name: agent-desktop-windows
version: 0.8.3
tags: windows-automation, accessibility, uia, ai-agent, gui-automation, cli
requirements:
  - agent-desktop
description: >
  Windows platform guide for the agent-desktop CLI. States per command group
  what works on Windows and what refuses: UI Automation observation (snapshot,
  find, get, is, screenshot), semantic ref interaction, headed physical input
  via SendInput, app and window lifecycle, typed clipboard, wait events,
  sessions and tracing. Names the honest gaps explicitly: list-surfaces and
  the four notification commands return PLATFORM_NOT_SUPPORTED, and
  cursor-overlay records its session setting while nothing renders.
  Covers UIPI elevation boundaries, Chromium/Electron first-contact settle
  behavior, and Windows-specific troubleshooting.
  Triggers on: "windows desktop automation", "UIA tree", "automate Windows app",
  "agent-desktop on Windows", "elevated window input blocked", "E_ACCESSDENIED",
  or any Windows GUI interaction task.
---

# agent-desktop-windows

Windows facts for agents driving desktop applications through agent-desktop.
The observe-act loop, ref system, JSON envelope, sessions, and tracing are
identical on every platform; this package documents only what differs on
Windows.

Requires Windows 10 1809+ / Windows Server 2019+ (x64 or ARM64).

## Capability Table

| Group | Commands | Status on Windows |
|-------|----------|-------------------|
| Observation | `snapshot`, `find`, `get`, `is`, `screenshot` | Works; surfaces are `window`, `focused`, and a Chromium modal reached as `sheet` |
| Surfaces | `list-surfaces` | Unavailable — returns `PLATFORM_NOT_SUPPORTED` |
| Ref interaction | `click`, `right-click`, `type`, `set-value`, `clear`, `select`, `toggle`, `check`, `uncheck`, `expand`, `collapse`, `scroll`, `scroll-to` | Works; semantic delivery in headless and `--headed` modes alike |
| Multi-click and focus | `double-click`, `triple-click`, `focus` | Works with global `--headed`; focus is headed-required (A3-4, A19-5) |
| Keyboard and mouse | `press`, `hover`, `drag`, `mouse-click`, `mouse-move`, `mouse-wheel` | Works; cursor-moving input requires `--headed`; `press --app` focuses then synthesizes after verification |
| Held input | `key-down`, `key-up`, `mouse-down`, `mouse-up` | Fails closed with `ACTION_NOT_SUPPORTED` on every platform until a daemon owns held-input lifetime |
| App and window | `launch`, `close-app`, `list-windows`, `list-apps`, `focus-window`, `resize-window`, `move-window`, `minimize`, `maximize`, `restore` | Works; `launch` resolves an absolute path or a bare name under System32 or the Windows directory (A21-1), not display names |
| Displays | `list-displays` | Works |
| Clipboard | `clipboard-get`, `clipboard-set`, `clipboard-clear` | Works; typed text and image content |
| Wait | `wait` | Works for ms, `--element`, `--window`, `--text`, `--menu`, `--menu-closed`, and `--event` predicates |
| Notifications | `list-notifications`, `dismiss-notification`, `dismiss-all-notifications`, `notification-action` | Unavailable — returns `PLATFORM_NOT_SUPPORTED`; `wait --notification` depends on listing and is unavailable too |
| Cursor overlay | `cursor-overlay` | Enable records the session setting (`ok: true`) but nothing renders; disable removes it |
| System | `status`, `permissions`, `version`, `batch`, `skills`, `session`, `trace` | Works |

## First Contact

- **No permission dialog exists on Windows.** UI Automation reads of
  same-integrity targets need no grant; `permissions` probes UIA live and
  reports `automation` as `not_required`. Elevation boundaries are the one
  access control — see `references/permissions-and-elevation.md`.
- **Chromium and Electron apps read thin before they settle.** A first-contact
  read can understate the settled tree by an order of magnitude and the gap can
  take tens of seconds on a cold launch — pass `--timeout-ms` to `snapshot`
  instead of concluding the tree is empty. Read
  `references/chromium-and-electron.md` before automating Electron apps.
- **Headless by default.** Ref actions stay semantic; only explicit `--headed`
  commands verify exact-window focus and synthesize physical delivery.
  Dangerous shortcuts (`alt+f4`, `win+l`, `win+d`, `alt+tab` and modifier
  supersets) are refused without `--force`.
- **Install-time warnings are expected for an unsigned binary.** The npm path
  attaches no Mark-of-the-Web, so no SmartScreen prompt can fire there;
  browser downloads and Smart App Control are different stories — see
  `references/troubleshooting.md`.

Three focused references, loaded as needed:

- [permissions-and-elevation.md](references/permissions-and-elevation.md) —
  integrity levels, UIPI, blocked combos, protected processes, the
  cross-process interaction lease.
- [chromium-and-electron.md](references/chromium-and-electron.md) — settle
  timing, covered-window hazard, the WPF wrong-provider trap, unnamed web
  content.
- [troubleshooting.md](references/troubleshooting.md) — symptom-to-cause map
  for empty trees, denied access, COM apartment errors, DPI coordinates, and
  launch-time execution controls.

Extract any of these from the binary itself:
`agent-desktop skills get agent-desktop-windows references/troubleshooting.md`.
