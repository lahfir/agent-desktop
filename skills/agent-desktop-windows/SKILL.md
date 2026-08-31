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
  sessions and tracing. Documents the shell surface: open-system-surface,
  per-process list-surfaces, snapshot --surface kinds, and the Action Center
  notification commands with their foreground requirement and verified
  mutations. Names the honest gaps explicitly: quick-settings is absent on
  pre-Windows-11 builds (the refusal names action-center), and cursor-overlay
  enable returns rendered: false while nothing renders.
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
| Observation | `snapshot`, `find`, `get`, `is`, `screenshot` | Works; surfaces are `window`, `focused`, a Chromium modal reached as `sheet`, and an open application menu reached as `menu` |
| Surfaces | `list-surfaces` | Works; per-process inventory of `window`, `focused` and `sheet` surfaces plus a `menu` surface carrying `item_count` when a menu is open |
| Shell surfaces | `open-system-surface` | Works; raises `start-menu`, `taskbar`, `system-tray`, `system-tray-overflow` or `action-center` and returns the window identity `snapshot --surface <kind>` consumes; `quick-settings` refuses on pre-Windows-11 builds |
| Ref interaction | `click`, `right-click`, `type`, `set-value`, `clear`, `select`, `toggle`, `check`, `uncheck`, `expand`, `collapse`, `scroll`, `scroll-to` | Works; semantic delivery in headless and `--headed` modes alike (`type` requires focus permission and returns `POLICY_DENIED` under strict headless) |
| Multi-click and focus | `double-click`, `triple-click`, `focus` | Works with global `--headed`; focus is headed-required (A3-4, A19-5) |
| Keyboard and mouse | `press`, `hover`, `drag`, `mouse-click`, `mouse-move`, `mouse-wheel` | Works; cursor-moving input requires `--headed`; `press --app` synthesizes into the foreground queue after verification (fails closed headless if not already frontmost; non-interactive callers report `delivered_unverified`) |
| Held input | `key-down`, `key-up`, `mouse-down`, `mouse-up` | Fails closed with `ACTION_NOT_SUPPORTED` on every platform until a daemon owns held-input lifetime |
| App and window | `launch`, `close-app`, `list-windows`, `list-apps`, `focus-window`, `resize-window`, `move-window`, `minimize`, `maximize`, `restore` | Works; `launch` resolves an absolute path or a bare name under System32 or the Windows directory (A21-1), not display names |
| Displays | `list-displays` | Works |
| Clipboard | `clipboard-get`, `clipboard-set`, `clipboard-clear` | Works; typed text and image content |
| Wait | `wait` | Works for ms, `--element`, `--window`, `--text`, `--menu`, `--menu-closed`, `--event`, and `--notification` predicates |
| Notifications | `list-notifications`, `dismiss-notification`, `dismiss-all-notifications`, `notification-action` | Works over the Action Center; the commands that raise shell chrome take the foreground, so pass `--headed` when the center is closed |
| Cursor overlay | `cursor-overlay` | `cursor-overlay enable` returns `data.rendered` (`true` if drawn, `false` if unsupported; Windows reports `false` as no renderer ships yet); `cursor-overlay disable` carries no `rendered` field |
| System | `status`, `permissions`, `version`, `batch`, `skills`, `session`, `trace` | Works |

## First Contact

- **Quote every ref in PowerShell.** PowerShell reads a bare `@token` as its
  splatting operator and *deletes the argument* before the binary sees it, so
  `set-value @s8f3k2p9:e1 hi` arrives with no ref and fails `INVALID_ARGS`.
  Write `set-value '@s8f3k2p9:e1' hi`. This bites first because PowerShell is
  the default shell on Windows and the CLI's own examples are written
  unquoted for POSIX shells. cmd.exe and bash need no quoting.
- **A window merely behind another window is fully drivable - a minimized one
  is not.** Being backgrounded costs nothing: with the terminal frontmost and
  Notepad behind it, `set-value` and a File-menu `expand` both succeed
  `delivered_verified`. Minimizing is what changes the answer - every element
  then reports `offscreen`, which is UI Automation telling the truth, and ref
  actions fail. `restore --app <image>` and re-snapshot; refs taken while
  minimized carry the offscreen state and stay unactionable. Do not reach for
  `focus-window` reflexively - it steals the user's foreground for nothing.

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
- **`type` requires focus permission and fails closed headless.** `type` on
  Windows is physical keystroke synthesis and requires focus permission, so a
  strict-headless `type` returns `POLICY_DENIED`. macOS can insert text at the
  selection semantically and succeeds headless. UI Automation exposes no
  insert-at-selection equivalent, which is why the two differ.
- **`press --app` synthesizes into the foreground queue.** `press --app` on
  Windows is synthesis into the foreground queue with no per-process targeting,
  so a headless press whose target is not already frontmost fails closed. macOS
  can deliver to a specific process and can also match a menu-bar accelerator
  semantically. A press issued from a non-interactive caller — a service, a
  scheduled task, a CI job — is far less reliable than the same command from an
  interactive session, and the envelope reports `delivered_unverified` because
  the synthesis API cannot confirm delivery.
- **`cursor-overlay enable` reports whether drawing occurred.**
  `cursor-overlay enable` returns `data.rendered`, a boolean saying whether the
  overlay was actually drawn. It is `true` on an adapter that implements the
  overlay and `false` on one that does not — Windows currently reports `false`,
  because no Windows renderer ships yet. `cursor-overlay disable` carries no
  `rendered` field, because a disable has nothing to render.
- **Commands that raise shell chrome take the foreground.** See Shell Surfaces
  and Notifications below; a strict-headless call is refused before anything
  is raised.
- **Install-time warnings are expected for an unsigned binary.** The npm path
  attaches no Mark-of-the-Web, so no SmartScreen prompt can fire there;
  browser downloads and Smart App Control are different stories — see
  `references/troubleshooting.md`.

## Shell Surfaces

`open-system-surface --surface <kind>` raises a shell surface and answers with
the identity of the window the surface actually presents: the same `w-<hwnd>`
identity the observation stack roots, so `snapshot --surface <kind>` consumes
it with no second lookup. Windows kinds: `start-menu`, `taskbar`,
`system-tray`, `system-tray-overflow`, `action-center`. `snapshot --window-id`
refuses that handle with `WINDOW_NOT_FOUND` by design — the window inventory
deliberately excludes shell windows — so the shell round trip routes through
`snapshot --surface <kind>`, not through `--window-id`.

- **The command takes the foreground.** Under strict headless it is refused
  with `POLICY_DENIED` before anything is raised; pass global `--headed`. An
  already-present surface is returned without being raised again.
- **`snapshot --surface <kind>` resolves a shell surface with no `--app`**, and
  because it only reads, it works headless: a closed surface returns
  `WINDOW_NOT_FOUND` with a suggestion naming `open-system-surface` as the way
  to raise it.
- **`quick-settings` refuses with `PLATFORM_NOT_SUPPORTED`**, and the
  `platform_detail` names the build and the surface that carries the
  capability instead: on pre-Windows-11 builds the quick actions are a pane
  inside the Action Center, so ask for `action-center`.
- **`start-menu` resolves to whatever the Win accelerator actually raises.**
  On pre-Windows-11 builds that is a search-hosted overlay whose root carries
  a `SearchTextBox`, not a tile grid (A26-9) — drive it as the search surface
  it is.
- **The tray path is the generic command surface** — no Windows-specific tray
  commands exist. The tray surfaces list: `snapshot --surface system-tray`
  returns the promoted notification-area items (it reads whatever the shell
  currently promotes, which may be zero items on a machine with no tray icons —
  that is a correct empty answer, not a failure), `snapshot --surface taskbar`
  refs the notification area's tray `button`s, and `snapshot --surface
  system-tray-overflow` (raised first with `open-system-surface`) refs its
  items. Clicking a tray item by ref is delivered, and the envelope reports
  `delivery: delivered_unverified` with `retry: unsafe` — a synthesized click
  cannot confirm what the owning application chose to do with it, so plan for
  an unverified delivery rather than a confirmed effect.

## Notifications

The notification commands read and drive the Action Center through UI
Automation; the WinRT `UserNotificationListener` is never consulted, so its
per-machine consent state cannot fail a call. The reader keys on measured
landmark `AutomationId`s: `MainListView` when notifications are present, an
empty-state shape when none are, and a top-level `ClearAllButton`.

- **The center must come up for a read.** `list-notifications` adopts an
  already-present center headless; when it is closed, a strict-headless call
  is refused with `POLICY_DENIED` before the center is raised, and a
  `--headed` call raises it and restores the desktop afterwards. The mutations
  (`dismiss-notification`, `dismiss-all-notifications`,
  `notification-action`) require `--headed` everywhere, and `wait
  --notification` refuses on its first poll under the same floor.
- **An empty center honestly lists zero entries.** A center that carries
  neither the notification list nor the empty-state landmarks returns
  `PLATFORM_NOT_SUPPORTED` with a `platform_detail` naming the missing
  landmark — a tree this adapter does not recognize is an error, never an
  empty answer.
- **The index is not the identity.** Every mutation re-reads the center and
  compares the entry at the requested index against the caller's
  `--expected-app` / `--expected-title` fingerprints; the center reorders as
  notifications arrive and expire, so a mismatch is `NOTIFICATION_NOT_FOUND`.
  An action name the entry does not offer is `ACTION_NOT_SUPPORTED`.
- **A dismiss that does nothing fails loudly.** The shell can accept an
  entry's `DismissButton` invoke without acting on it; when the target is the
  center's sole entry the verified clear-all control is invoked instead, and
  otherwise the call reports `ACTION_FAILED` (`delivered_unverified`) rather
  than a false success. `dismiss-all-notifications` is judged against the
  identity set captured before the clear, so entries arriving while it runs
  are new arrivals, not failures.
- **`wait --notification` opens and closes the center per poll**, exactly as
  on macOS: each poll runs in its own one-call session that adopts an
  already-present center and restores the entry state afterwards — no
  long-lived session is held. Each poll of `wait --notification` opens and closes
  the Action Center, measured at 1243.5 ms per poll at the minimum and 1254.2 ms
  at the median on a reference machine. Size timeouts accordingly — a five second wait buys roughly four
  polls, and a notification that appears and is dismissed inside one interval can
  be missed. On this shell a toast joins the center only while it is open
  (A26-3), so toasts posted while the center sits closed between polls never
  land; if you are staging arrivals, hold the center open yourself and the
  wait's polls will adopt it without closing it.
- **This output is sensitive.** The notification-area surface publishes the
  shell's names of installed background agents — security and remote-access
  products among them — and `list-notifications` returns notification titles
  and bodies verbatim. Nothing is redacted at the command layer; this is
  ordinary output for the driving agent, so a caller routing it onward should
  treat it as sensitive.

## Hosted (UWP) Window Identity

An `ApplicationFrameHost`-hosted application is reported through its frame:
`focused_window` and `list-windows` give the frame's handle as the window
`id` — the handle every window operation targets — while `app` and `pid` name
the hosted application, read from its `Windows.UI.Core.CoreWindow` one level
down (A26-8). A suspended hosted application drops its `CoreWindow` while the
frame survives, so until it resumes the entry reads as its frame host, and
identities verified against the hosted pid fail closed for exactly that long.

## Menu Detection Coverage

The `menu` surface detector is measured per host family, and each family it
covers has its own detection source: classic Win32 menus (A23-1), WPF context
menus and menu-bar dropdowns (A23-11), and Chromium/Electron DOM context
menus — a DOM menu inside the application's own window that neither other
source can see (A26-12). WinUI3/MSIX hosts are unevaluated. Read "no menu is
open" from an app in an uncovered family as "not detected there", not as
proof the menu is closed.

## Saving a document, headless, without keyboard input

No `save` command exists — saving is a menu path plus a dialog, and the whole
chain is semantic, so it works with no `--headed` and no keystrokes. The shape
generalises to any app whose save flow is File → Save As.

```bash
agent-desktop restore --app notepad.exe               # only if minimized; behind is fine
agent-desktop snapshot --app notepad.exe              # keep the snapshot_id
agent-desktop set-value '@<snap>:e1' "the document body"
agent-desktop expand '@<snap>:e13'                    # the File menu item
agent-desktop snapshot --app notepad.exe --surface menu   # the open menu is its own surface
agent-desktop click '@<menu>:e4'                      # Save As...
agent-desktop list-windows --app notepad.exe          # the dialog is a new window
agent-desktop find --name "File name" --window-id <dialog-id>
agent-desktop set-value '@<found>:e2' 'C:\\out.txt'    # full path goes in the name field
agent-desktop find --role button --name "Save" --window-id <dialog-id>
agent-desktop click '@<found>:e1'
```

Two things that surprise a first-time caller:

- **`find --name "File name"` returns three matches** — a `statictext` label, a
  `combobox`, and the `textfield` inside the combobox. The textfield is the one
  that accepts `set-value`; match on `role` as well as name.
- **Setting the full path into the name field is what selects the directory.**
  There is no separate folder-navigation step, and navigating the file list by
  ref is far more fragile than writing the path.

The dialog's Save button returns `delivered_unverified` — a synthesized invoke
cannot confirm what the shell dialog did with it. Verify by reading the file
back from disk, not from the envelope.
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
