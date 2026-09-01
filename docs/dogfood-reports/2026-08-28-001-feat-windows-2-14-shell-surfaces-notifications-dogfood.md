# Dogfood: Shell Surfaces & Notifications (Sub-phase 2.14)

- **Date:** 2026-08-28
- **Branch:** `feat/windows-2.14-shell-surfaces-notifications` (HEAD `b9399a1`)
- **Channels exercised:** the release CLI binary (as an agent), against the machine's
  real shell, real notifications infrastructure, and two real Chromium/Electron
  applications — never the fixture app.
- **Binary identity:** `agent-desktop 0.8.3`, release profile,
  sha256 `642B4BB6C142AE004D4B64794E3ECF458B613A9E4D7382CB656CA96FD024E621`.
- **Environment:** Windows Server 2019 Datacenter, build 17763.7434, single display,
  interactive console session, installed UI culture es-ES (session culture en-US).
  State root was a scratch directory for the whole run; no repo state was written.

## Legs

| # | Leg | Target | Outcome | Delivery disposition |
|---|-----|--------|---------|----------------------|
| 1a | `list-notifications` on the real center | real Action Center | center currently carries **zero** real entries; the empty listing is a successful empty read, not an error | read-only |
| 1b | staged synthetic toast via the plan's AUMID route (fixed synthetic literals only) | real toast infrastructure | listing shows exactly 1 entry with the full field set; the staged entry is the only one | read-only |
| 1c | `dismiss-notification 1` with expected identity, promptly after staging | staged entry | `ok:true`; re-read proves the entry gone (count 1 → 0) | delivered, verified by re-read |
| 1d | `dismiss-notification 1` after the staged entry's Action Center retention elapsed | expired entry | `NOTIFICATION_NOT_FOUND`, nothing delivered, entry absent on re-read | not_delivered, retry safe |
| 1e | `notification-action 1 <name>` on a toast that advertises no actions | staged entry | `ACTION_NOT_SUPPORTED` naming the index; the entry is unchanged afterwards | not_delivered, retry safe |
| 1f | `dismiss-all-notifications` promptly after staging two toasts | staged entries | `ok:true` with the captured-set outcome: `dismissed_count` equals the captured members verified gone (2 of 2); re-read 0 | delivered, verified by re-read |
| 1g | `dismiss-all-notifications` on an empty center | real center | `ok:true`, `dismissed_count:0`, no `failures` key — the unambiguous no-op shape (captured set empty), not an ignored clear | delivered (no-op), verified |
| 2a | `snapshot --surface system-tray` | real notification area | resolves the promoted toolbar's window; the promoted read carries **zero** items while `snapshot --surface taskbar` in the same session refs the same toolbar's three `Button` tray items | read-only (finding F1) |
| 2b | `click @ref` on a tray-area item taken from the taskbar surface | real tray area | refused `WINDOW_NOT_FOUND` ("no longer an agent-visible top-level window") — the tray window is deliberately outside the agent window inventory | not_delivered (finding F3) |
| 2c | `snapshot --surface system-tray-overflow` | real overflow flyout | resolves the (hidden) overflow window and refs **five** `Button` items | read-only |
| 2d | `open-system-surface --surface system-tray-overflow` + `click` on an overflow ref | real overflow flyout | open returns `ok:true` but the flyout never becomes visible (independent Win32 visibility poll, 4 s); the click passes visible/stable/enabled/supported-action/policy and fails only the occlusion check — 5/5 hit-tests occluded | not_delivered, retry safe (finding F2) |
| 2e | overflow close via Esc | — | nothing to close (flyout never rose); Esc issued as restore regardless | n/a |
| 3 | `open-system-surface --surface start-menu` → `snapshot --surface start-menu` → `set-value` into the surfaced search field → `press escape` | real Start overlay | open returns the overlay identity; snapshot refs the surface; the search interaction delivers `delivered_verified` with the value read back in post-state and the surfaced tree changes shape (76 → 32 refs); Esc closes; post-close snapshot returns `WINDOW_NOT_FOUND` with the `open-system-surface` suggestion | set-value: delivered_verified; Esc: delivered_unverified (honest SendInput) |
| 4 | `list-surfaces --app <image>` against two real applications | Chromium app; terminal app | first app: 1 window surface; second app: window + focused surfaces with ids | read-only |
| 5a | Settings via the `ms-settings:` route; `focus-window --app <hosted>` and `list-windows --app <hosted-image>` | real UWP-hosted target | both return the **same frame handle** with the **hosted application's** image name and pid — the frame/hosted split reports one identity per command | read-only |
| 5b | `snapshot --surface focused` with Settings frontmost | same | frame handle identical to 5a; `data.app` names the hosted application | read-only |
| 5c | `close-app <hosted-image>` on the UWP-hosted app | same | `APP_NOT_FOUND` ("not found with exact process identity") while the process runs and `list-windows` sees its window — the hosted app is windowless in the inventory; cleaned up externally | not_delivered (finding F8) |
| 6a | `snapshot --app <Chromium app>` with bounds | real Electron window (19 refs) | the tree contains the A24-11 shape: nameless positive-area elements (one with `Click`), and nameless zero-extent elements | read-only |
| 6b | `get` on the nameless positive-area ref | same | **resolves** (`ok:true`) — before this branch's fix this ref failed closed `STALE_REF` before any candidate search (A24-11). The click half was skipped with a stated reason: the only nameless positive-area `Click` element reachable is the application's content root, whose click semantics are not verifiable as harmless, and the editor surface was not reachable this session | resolution verified |
| 6c | A26-13 re-attempt: classify nameless leaves on a settled Chromium tree | Obsidian (self-updated to 1.13.7 since the ledger) | **failed again, honestly**: fresh-client snapshots pinned at a 15-ref skeleton through 60 s of settling (12 polls), window front and focused, vault picker's DOM controls never exposed — no qualifying leaf population; the FINDINGS row is left untouched per its own protocol | measurement not taken (finding F10) |
| 7 | `wait --notification --headed` with a toast staged mid-wait | real wait loop | returned at 5.8 s with the staged entry's identity, `matched:true` — only an arrival during the wait returns | delivered |
| 8 | `open-system-surface --surface quick-settings` | build-conditional refusal | `PLATFORM_NOT_SUPPORTED` whose `platform_detail` names the build and `action-center` | not_delivered, retry unsafe (by design) |

## Product cost baseline

Taken through the release binary — wall clock per CLI invocation including process
spawn, since spawning the binary is the shipped path. Corpus methodology: one
discarded warm-up, seven timed runs, min reported with median and max beside it
(A15-13, applied in A18-7). **This table is the product cost baseline for the
sub-phase; A26-10 is the pre-implementation platform reference and is not the
shipped path's cost.** Notification center state during leg 2: **empty** (zero
entries) — the read scales with content (A26-10).

| Operation | min (ms) | median (ms) | max (ms) |
|---|---|---|---|
| `snapshot --surface system-tray` | 156.6 | 180.3 | 230.1 |
| `list-notifications --headed` (center empty; raise + read + close per invocation) | 1243.5 | 1254.2 | 1308.4 |
| `open-system-surface --surface action-center --headed` + close (one full cycle) | 1233.9 | 1331.6 | 1354.3 |
| `snapshot --surface action-center` (center open) | 368.8 | 412.7 | 466.3 |
| `dismiss-notification` verified round trip (center held open; one staged synthetic toast via the plan's AUMID route, fixed synthetic literals; the timed invocation is the prompt dismiss + its verification re-read, spawn included) | 4368.2 | 4417.2 | 4631.4 |

The verified-mutation round trip is the number most worth having: a prompt
`dismiss-notification` that re-reads to verify measured 4368.2 / 4417.2 /
4631.4 ms (min / median / max, last row of the table above, taken as its own
corpus through the release binary) — the two-cross-process-round-trip floor
KTD6 predicted, dominated by the settle-poll, not by spawn or UIA reads.

## Findings and dispositions

### F1 — The dedicated tray surface's promoted read returns zero items; the same toolbar's items ref through the taskbar surface

`snapshot --surface system-tray` roots at the promoted toolbar's HWND and read
zero children on four attempts across the session (including with the overflow
"open"); `snapshot --surface taskbar` in the same session allocated refs to that
toolbar's three `Button` items with stable machine-local `AutomationId`s, `Invoke`
and positive-area bounds. An HWND-rooted walk versus tree-descent divergence of
the same family A26-5 recorded across client stacks; the promoted tray population
is session-dependent, so the exit criteria's "three promoted" claim did not hold
at dogfood time. **Disposition: owned elsewhere** — written into §2.15's scope in
this PR (the bullet beginning *"Repair the tray click path"*), and §2.14's exit
criteria and P2-O18 row corrected in this PR to read true.

### F2 — The overflow raise is accepted and does not raise; every overflow ref then fails actionability on occlusion

`open-system-surface --surface system-tray-overflow` returns `ok:true`, but an
independent Win32 visibility poll (the binary did not perform it) never saw the
overflow flyout visible in 4 s after the raise — the `ChevronInvoke` raise is an
invoke, and the shell can accept an invoke without acting, the exact shape the
notification adapter's clear-all substitute exists for (A26-3). The five overflow
refs then pass visible/stable/enabled/supported-action/policy and fail only the
`receives_events` occlusion check, 5/5 hit-tests occluded, retryable to no effect.
The tray click capability does not deliver live through this path.
**Disposition: owned elsewhere** — same §2.15 bullet as F1.

### F3 — A tray click from a non-tray surface ref refuses on inventory visibility

`click` on a tray-area item ref taken from the taskbar snapshot refused
`WINDOW_NOT_FOUND` ("no longer an agent-visible top-level window"): `Shell_TrayWnd`
is deliberately outside the agent window inventory (KTD1), and the click
preflight gates on that same inventory. Correct per KTD1, but it means no route
delivered a tray click this session. **Disposition: owned elsewhere** — same
§2.15 bullet; the bullet names the contract question (which surface's refs are
click-legal).

### F4 — Staged toasts have short Action Center retention; the failure envelopes are fail-closed correct

Toasts posted through the plan's AUMID route vanish from the center within roughly
a minute on this build. A `list-notifications` → mutation round trip that outlives
the retention returns `NOTIFICATION_NOT_FOUND` (nothing delivered; the entry is
genuinely absent) or the empty-captured-set no-op (`ok:true`, `dismissed_count:0`,
no `failures` key — unambiguous by construction since `dismissed_count` + failures
partition the captured set). Prompt mutations inside the retention verified
correctly every time (F-1c, F-1f). **Disposition: accepted** — the behavior is the
platform's toast retention for this notifier identity, not the product's; every
envelope observed is the designed fail-closed shape, and the report's leg table
records the retention so the next agent stages and mutates promptly.

### F5 — `trace_sanitize`'s field list is narrower than the notification envelope it may someday be handed

`SENSITIVE_KEYS` (`crates/core/src/trace_sanitize.rs`) covers `title` but not
`body`, `app_name`, `attribution` or `actions` — the remaining serialized
`NotificationInfo` field names. Measured live: no adapter's notification path
emits payload-bearing trace events on this branch (traced `list-notifications`
and `dismiss-notification` segments carry meta/start/end only; staged content's
literals are absent from the trace file), so the plan risk's premise
("notification bodies reach on-disk trace segments and the FFI log callback") is
currently unexercised rather than a live leak. **Disposition: owned elsewhere** —
§2.15 bullet (*"Extend `trace_sanitize`'s field list to the full notification
envelope"*), naming the four fields and the per-field invert-verified tests that
close it. The gap is cheapest to close while no trace site emits payloads.

### F6 — Two live E2E legs fail identically at this branch's merge-base

`headed-double-click` (actionability preflight `hit_test` occluded 5/5 on an
unattended desktop) and `interaction-scroll-to-visibility` (scroll-to
`ACTION_FAILED`) fail the same way with this sub-phase's diff stashed — measured
at the merge-base, cited as given. The dogfood independently observed the
occlusion half live: the tray-click preflight's occluder was the driving console's
own window. **Disposition: owned elsewhere** — §2.15 bullet (*"Re-baseline the two
live E2E legs…"*), naming both legs, the observed mechanism, and the harness fix
that closes it.

### F7 — A shell surface's returned handle cannot root through `snapshot --window-id`

`snapshot --window-id` on the promoted tray toolbar's handle returned
`WINDOW_NOT_FOUND` — the window inventory deliberately excludes shell windows
(KTD1's correct behaviour), so the shell round trip routes through
`snapshot --surface <kind>` while the identity wording reads as if the handle
were rootable. **Disposition: owned elsewhere** — §2.15 bullet (*"Decide whether a
shell surface's returned handle is rootable…"*).

### F8 — `--app` identifier form diverges between Windows commands, and a UWP-hosted app is invisible to `close-app`

Measured live: `list-windows --app <stem>` succeeds (zero rows) while
`list-surfaces --app <stem>` refuses `APP_NOT_FOUND` and the image name resolves
both — the accepted identifier form differs per command family, not only per
platform. And `close-app <hosted-image>` on the running, focused UWP-hosted
Settings app returned `APP_NOT_FOUND` while `list-windows --app <hosted-image>`
returned its window with the hosted pid: the hosted app owns no top-level window,
so the window-owning-process join skips it — a second instance of the
windowless-`close-app` class. **Disposition: owned elsewhere** — this PR extends
the two existing §2.15 entries that own these exact questions (*"Settle whether
Windows `--app` accepts an application's stem…"* and *"Settle steady-state
windowless `close-app`"*) with the newly measured instances.

### F9 — `list-surfaces` and `list-windows` disagree about an application's window population

For the running Chromium app, `list-surfaces --app <image>` returned its window
while `list-windows --app <image>` returned zero rows (the window is excluded from
the top-level inventory — cloaked/virtual-desktop state — while the per-process
UIA descent still finds it). Both answers are within each command's documented
inventory. **Disposition: accepted** — the two commands intentionally read
different inventories (KTD1's filtered top-level walk versus per-process UIA
descent); recorded here so a caller comparing them knows the disagreement is
inventory, not defect. If the identity-split decision in §2.15 wants one
population, it already owns the question.

### F10 — A26-13 re-attempt failed again: no settled Chromium tree this session

Obsidian self-updated 1.12.7 → 1.13.7 since the ledger was written. With the
scratch-vault protocol staged (obsidian registry backed up and restored, scratch
vault removed afterwards), fresh-client snapshots pinned at a 15-ref skeleton
through 60 s of settling (12 polls), the window front and focused, and the vault
picker's DOM controls never exposed — the exposure floor moved but did not lift,
and no qualifying nameless-leaf population existed to classify. The
zero-identity **resolution** half of A24-11 was verified live instead (leg 6b).
**Disposition: owned elsewhere** — §2.15 bullet (*"Take the A26-13 nameless-leaf
population classification…"*); the FINDINGS row itself is left untouched per its
protocol, and the failed attempt is recorded here.

### F11 — `wait --notification` has no caller-settable deadline flag

`--wait-timeout` is refused for the notification mode (`INVALID_ARGS`, requiring
`--wait-for`), and a positional duration counts as a second mode; the wait runs on
its default deadline. The refusal is immediate, fail-closed and names the
constraint. **Disposition: accepted** — the envelope is precise and the default
deadline is sane; if a caller-visible bound is wanted, it is one flag in §2.15's
wait-contract review rather than a defect.

## Safety envelope

Every leg ran against the interactive console session of the machine the binary
runs on. Restores performed: Start overlay closed via Esc (verified closed);
Action Center closed via its toggle (verified closed); staged toasts dismissed or
expired with a final center count of zero; the UWP Settings target closed and its
process gone; the Obsidian attempt fully unwound (process stopped, registry
backup restored, scratch vault deleted); Cursor only ever read (snapshot/get; no
click was delivered into it); the console terminal untouched. No notification
text, tray item name, window title, user name, machine name, filesystem path or
pid appears in this report; notification-area content is described by counts,
shapes and outcomes only. The committed captures are authored shape files, not
raw envelopes.

## Verification Contract result

| Requirement | Borne by |
|---|---|
| R1 | U3/U4/U15 tests; live: action-center, start-menu round trips root through `--surface <kind>`; the `--window-id` rooting of shell handles is F7's §2.15 bullet |
| R2 | U3/U4/U14 tests; live leg 8 (refusal detail names build + `action-center`) |
| R3 | U5 tests; live: open-center read works app-less; closed surfaces return `WINDOW_NOT_FOUND` naming `open-system-surface` (legs 3, baseline verify) |
| R4 | U5's three set assertions; live corroboration: `status` advertises menu + the five shell kinds, `quick-settings` absent |
| R5 | U6 tests; live leg 4 (real-app surfaces with ids) |
| R6 | U1/U3 (A26-1, A26-2); live corroboration: tray family resolvable while excluded from the window inventory; immersive surfaces resolvable app-less |
| R7 | U3/U5/U15 harness assertion (independent COM count); live: overflow 5, promoted population session-dependent (F1) |
| R8 | U1/U15 (A26-7 stability); live: overflow items carry `Click`; promoted GUID `AutomationId`s via the taskbar path |
| R9 | U5/U15; probe A26-7 — not re-staged live this session (menu staging skipped; no harmless tray context menu) |
| R10 | U9/U15 field-by-field tests; live: listing entries carry index/app/title/body/actions |
| R11 | U9 tests (invert-verified); live: dismissal proven by re-read (1 → 0) |
| R12 | U9 tests; live: `dismissed_count` = captured members verified gone (2 of 2), distinct from the empty-capture no-op |
| R13 | U9 tests; live: unknown action refused `ACTION_NOT_SUPPORTED`, entry unchanged |
| R14 | U9 tests; live: expired entry refused `NOTIFICATION_NOT_FOUND`, nothing delivered |
| R15 | U1 (A26-4) + U14 skill text; no shipped code reads the listener |
| R16 | U10 tests; live leg 7 (staged-during-wait arrival returned at 5.8 s) |
| R17 | U3/U4/U9/U14 tests (strict-headless refusals with foreground unchanged); headed used throughout the dogfood |
| R18 | U12/U14 tests; live leg 5a (frame handle + hosted app/pid from the foreground) |
| R19 | U12 tests; live: same pid across `focus-window`, `list-windows --app`, `snapshot --surface focused` (5a/5b) |
| R20 | U13 test (invert-verified before the change); live leg 6b (nameless positive-area ref resolves) |
| R21 | U11 (A26-11) + §2.15's WinUI3/MSIX bullet (branch B carried) |
| R22 | U11 (A26-12, needle includes cursor) |
| R23 | `scripts/check-win32-ui-shell-exclusion.ps1` over manifest + feature graph; no manifest feature added |
| R24 | `13-ledger-check.ps1` (every UIA row names `uia3-com`); citations gate |
| R25 | `check-phases-ledger-citations.ps1` incl. retired stems |
| R26 | redaction gate self-test: four MUST-CATCH fixtures (one per serialized field) each fail naming their rule |

All gates run for this report are listed with their exact commands and exit codes
in the PR description; the report and its captures pass
`scripts/check-capture-redaction.ps1`.

## Ledger rows naming this sub-phase, disposed

| Row | Disposition |
|---|---|
| A21-8 | disposed — §2.14 carries the recorded not-take decision for by-name/AUMID launch; §2.15 owns the identifier contract; the exclusion gate still passes |
| A24-11 | disposed — fixed in this sub-phase (the zero-identity allocation fix); live-verified at leg 6b: a nameless positive-area content ref resolves where it previously failed closed before any candidate search |
| A24-12 | disposed — superseded by A26-12's fourth staging attempt (corrected host population, third detector source); §2.14's Chromium bullet carries it |
| A23-4 | disposed — narrowed to WinUI3/MSIX and carried: §2.12's residual, §2.14's detector bullet, and §2.15's WinUI3/MSIX evaluation bullet |
| A16-2 | disposed — closed on this host (KTD8); the frame-vs-`CoreWindow` foreground reading is shipped behaviour, verified live at leg 5 |
| C-5 | disposed — the overflow flyout class correction is in the capability map, the P2-O18 row and §2.14's tray bullet |
| C-10 | disposed — the P2-O14 surface vocabulary states the Windows 11 split; §2.14 cites it for the `quick-settings` refusal (live-verified at leg 8) |
| A26-13 | deferral stands, honestly re-attempted and failed again this session (F10) — the row is untouched per its protocol and §2.15 now carries the classification with both failed attempts recorded |

U2's §2.15 writes confirmed present this close-out: the by-name/AUMID launch
identifier bullet, the macOS `open-system-surface` kinds bullet, the
WinUI3/MSIX detector-arm bullet (KTD9 branch B), the UWP-hosted identity-split
bullet, and the error-payload/stale-ref families — plus the three §2.15 writes
this dogfood adds or extends (tray click path, shell-handle rooting,
trace-sanitizer fields, E2E re-baseline, A26-13 carry).

## Verdict

The sub-phase's surfaces were driven against the real shell and real
infrastructure by someone trying to break them. The notification path is the
strong result: every mutation verified against the entries it targeted, the
captured-set semantics observable in the envelope, every failure fail-closed with
the right code. The tray click path is the weak result: it lists but does not
deliver, and that gap is owned, written into §2.15's scope in this PR, with the
exit criteria corrected to say so. Eleven findings, eleven dispositions — eight
owned elsewhere with the receiving §2.15 text landed in this PR, three accepted
with stated reasons, none merely recorded.
