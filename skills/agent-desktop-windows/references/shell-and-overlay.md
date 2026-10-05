# Shell surfaces, notifications and cursor overlay

Open this file to raise shell surfaces, read or drive notifications, use the cursor overlay, or handle hosted (UWP) window identity and menu detection.

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
- **`quick-settings` returns `PLATFORM_NOT_SUPPORTED`** and names
  `action-center`, where the quick actions live. Ask for `action-center`.
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
  items. Clicking a `system-tray` item by ref is delivered, and the envelope
  reports `delivery: delivered_unverified` with `retry: unsafe` — a synthesized
  click cannot confirm what the owning application chose to do with it, so plan
  for an unverified delivery rather than a confirmed effect. Overflow items read
  as refs, but a click on one has not been measured: if the flyout is not
  visible the click is refused by the actionability occlusion check, so raise
  the flyout with `open-system-surface --surface system-tray-overflow` first.

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
  at the median on a reference machine. Size timeouts accordingly: `wait --notification` also sleeps up to
  500 ms between polls, and a notification that appears and is dismissed inside one interval can
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
