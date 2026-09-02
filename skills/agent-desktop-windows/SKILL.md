---
name: agent-desktop-windows
version: 0.8.4
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
  mutations. Documents the cursor overlay, which renders on Windows for
  headless semantic actions. Names the honest gaps explicitly: quick-settings
  is absent on pre-Windows-11 builds (the refusal names action-center), and
  the cursor overlay's mixed-DPI monitor mapping is unit-tested but
  unverified on a live multi-monitor rig.
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
| Cursor overlay | `cursor-overlay` | Works; `enable` renders a click-through overlay and `data.rendered` is the renderer's own pipe acknowledgement, not merely a spawned process. `disable` carries no `rendered` field. See Cursor Overlay below |
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
- **`cursor-overlay enable` renders on Windows.** See Cursor Overlay below for
  what `data.rendered` means, why the overlay draws only for headless
  semantic actions, and where it deliberately differs from macOS.
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

## Cursor Overlay

`cursor-overlay enable` renders on Windows: a detached renderer process paints
a presentation-only cursor as a click-through, topmost, non-activating
layered window.

```powershell
$env:AGENT_DESKTOP_HOME = "$env:TEMP\ad-scratch"
$start = agent-desktop session start --cursor | ConvertFrom-Json
$env:AGENT_DESKTOP_SESSION = $start.data.session_id
agent-desktop cursor-overlay enable --label "Opening menu" --accent "#FF3B7B"
agent-desktop cursor-overlay disable
```

- **`data.rendered` is the renderer's own acknowledgement, not proof that a
  process merely started.** `enable` returns `true` only once a renderer has
  connected to the session's control pipe and acknowledged the control
  message; it returns `false` if a renderer could not be reached, refused to
  spawn, or never acknowledged within its budget. `disable` carries no
  `rendered` field, because a disable has nothing to render. `session start
  --cursor` is a second enable path through the same adapter call and emits
  no `rendered` field either — its own JSON only echoes `cursor_overlay`.
- **Semantic actions present the cursor only when headless.** A `--headed`
  action sends real pointer input, which moves the real cursor, so the
  per-action travel and click flourish are skipped for it — seeing no cursor
  animation around a `--headed` click is expected, not a bug. An explicit
  `cursor-overlay enable` still paints the resting cursor either way, so a
  headed session that enabled the overlay does show one: measured, `enable
  --headed` reports `rendered: true` and paints.
- **It draws above the shell's own topmost chrome, including the taskbar**
  (A29-3): a destination near the taskbar is not clipped by shell surfaces.
- **It does not collapse when the OS "Show animations in Windows"
  accessibility preference is off.** This is a deliberate difference from
  macOS, whose renderer collapses to a still pose under the OS's reduce-motion
  signal. On Windows the one API surface for this disagrees with itself, and
  reports animations disabled by default on a stock, unconfigured Windows
  Server host (A29-7, A29-8) — honouring it unconditionally would silently
  drop the travel animation on hosts nobody set that preference on for
  accessibility reasons. The overlay only ever draws because a caller
  explicitly enabled it on a session, so that opt-in is treated as the
  accessibility signal instead of the OS setting. The honest cost: a caller
  who reduced motion on Windows for genuine accessibility reasons still sees
  the full travel animation, where macOS would collapse it.
- **Mixed-DPI, multi-monitor coordinate mapping is unit-tested but not
  verified live.** The host this was measured on presented a single display,
  so the monitor-selection and coordinate-mapping logic has no live
  observation behind it on a scaled or multi-monitor desktop (A29-6).
- **The overlay costs about a third of a second per action, and that is the
  travel, not the plumbing.** The control roundtrip is 0.252 ms (A29-5). The
  figure that matters is end to end: a headless `click` cost 427 ms with no
  overlay and 782 ms with one, a delta of **+355 ms** per action, measured
  min-of-seven with the warm-up discarded (A30-5). Enabling costs a one-time
  49.9 ms for the renderer process and its window. Budget accordingly - a
  hundred overlaid clicks buys roughly 35 seconds of animation.
- **It rests at the centre of the primary monitor's work area** until an
  action moves it, so that is where the first frame appears - not over the
  application you are driving.
- **The first frame lands shortly after `enable` returns**, not before it. The
  command returns once the renderer acknowledges; poll for the pixels rather
  than screenshotting immediately.
- **`--fill` and `--rim` colour the label card too.** The card body takes
  `--fill` and its text takes `--rim`, so a fill matching the application
  behind it leaves the card nearly invisible. The default white fill against a
  white window is exactly that case.
- **Start the session before you take the snapshot.** Enabling the overlay
  requires a session, and a snapshot taken outside one lands in the global
  namespace where a session-scoped action cannot see it - the ref then fails
  `SNAPSHOT_NOT_FOUND` no matter how fresh it is. `session start`, then
  `cursor-overlay enable`, then `snapshot`, then act.
- **The card appears only when there is something to say.** `enable` with no
  `--label` greets itself once, and from then on the card is drawn only for
  an action that carries a description - an unlabelled click draws the
  cursor, the ripple and the element outline, and no card. The card is
  replaced wholesale by each `enable` and each action, so it never narrates
  one step with the caption from the last.
- **What animates, and what does not.** The card eases in over 180 ms once
  the cursor has landed, rather than during the travel, so it is read after
  the eye has followed the cursor. After 6 s with no instruction the whole
  overlay fades out over about 150 ms and leaves the screen; the next
  command brings it straight back at full strength, with no fade in. That
  asymmetry is deliberate and matches macOS: a disappearance should not draw
  attention, and a reappearance should not delay the action behind it.
- **`session end` closes the overlay and stops the trace.** It tears the
  renderer down without needing `cursor-overlay disable` first, and an ended
  session stops accumulating trace - a command run against it afterwards
  writes no further segment. The one segment written as the session closes
  is the `session end` command recording itself, which is part of the
  record rather than a leak.
- **A harness that contains its children takes the overlay with them.** The
  renderer is a detached process, and it outlives the CLI invocation that
  started it - but not a job object carrying `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`,
  which CI runners and some agent harnesses wrap every child in. Measured
  (A30-6): inside such a job, `enable` answers `rendered: true` and the
  overlay is drawn, and both go away the moment the harness closes the job.
  Nothing functional is lost - the overlay is presentation-only and never
  fails an action - but do not read a vanished overlay in that setting as a
  renderer defect. The renderer deliberately does not break out of such a
  job: escaping a containment the operator chose would be worse than not
  persisting.
- **Its control pipe trusts any process running as you.** The pipe rejects
  remote clients and refuses another user, and the client checks that the
  renderer is this tool's own image — but it carries no security descriptor
  and its name is derivable, so a same-user process can hold the name (leaving
  `rendered: false`) or drive a live renderer's cursor and label text. See
  "The Cursor Overlay's Control Pipe" in `references/permissions-and-elevation.md`.
- **Teardown.** `cursor-overlay disable` ends the renderer process for its
  session. A session that ends out of band — a crash, `session gc`, an
  operator who simply stops — is reclaimed by the renderer itself on its next
  idle-tick reads, bounded at two ticks of 1500 ms each, so reclaim completes
  within 3000 ms even with no `disable` ever sent.

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

Runs as written against an open Notepad, with no ref typed by hand — every
ref is captured out of the JSON of the step before it, which is what keeps the
sequence correct when the tree shifts under it.

```powershell
$out = "$env:TEMP\notepad-save-demo.txt"

agent-desktop restore --app notepad.exe                 # only if minimized; behind is fine
$doc = (agent-desktop find --app notepad.exe --role textfield | ConvertFrom-Json).data.matches[0]
agent-desktop set-value $doc.ref_id "the document body"

$file = (agent-desktop find --app notepad.exe --role menuitem --name "File" | ConvertFrom-Json).data.matches[0]
agent-desktop expand $file.ref_id
agent-desktop wait --menu --app notepad.exe             # the open menu is its own surface
$menu = (agent-desktop snapshot --app notepad.exe --surface menu | ConvertFrom-Json).data.tree
agent-desktop click ($menu.children | Where-Object name -eq 'Save As...').ref_id

agent-desktop wait --window "Save As" --app notepad.exe # the dialog is a new window
$dialog = (agent-desktop list-windows --app notepad.exe | ConvertFrom-Json).data |
    Where-Object title -eq 'Save As'
$name = (agent-desktop find --name "File name" --window-id $dialog.id | ConvertFrom-Json).data.matches |
    Where-Object role -eq 'textfield'
agent-desktop set-value $name.ref_id $out               # full path goes in the name field

$save = (agent-desktop find --role button --name "Save" --window-id $dialog.id | ConvertFrom-Json).data.matches[0]
agent-desktop click $save.ref_id
agent-desktop wait --window "notepad-save-demo - Notepad" --app notepad.exe
Get-Content $out
```

Three things that surprise a first-time caller:

- **`find --name "File name"` returns three matches** — a `statictext` label, a
  `combobox`, and the `textfield` inside the combobox. The textfield is the one
  that accepts `set-value`; match on `role` as well as name.
- **Setting the full path into the name field is what selects the directory.**
  There is no separate folder-navigation step, and navigating the file list by
  ref is far more fragile than writing the path.
- **The file does not exist the moment the Save click returns.** Measured on
  this flow, the shell took 10.9 s between the click's envelope and the file
  appearing on disk. Wait for the title change with `wait --window` rather
  than sleeping: a fixed sleep of a second or two reads as a missing file and
  looks like a defect that is not one.

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
