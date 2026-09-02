# Permissions and Elevation

Windows has no TCC-style permission grant for UI Automation. What controls
access instead is **integrity level**: a Medium-integrity agent can observe and
act on same-integrity applications, and the OS gates what crosses the boundary
upward.

## Same-Integrity Targets Need No Grant

- UIA reads (snapshot, find, get, is, screenshot) and same-integrity input work
  from any terminal with no permission prompt.
- `agent-desktop permissions` probes UIA live rather than consulting a policy
  store: `accessibility` reports `granted` when UIA answers, `automation`
  reports `not_required`, and `screen_recording` reflects capture availability.
- There is no `--request` dialog to trigger; if `accessibility` reports
  `denied`, the cause is an integrity or policy boundary below, not a missing
  checkbox.

## The UIPI Boundary

User Interface Privilege Isolation blocks synthesized input from a
lower-integrity process into a higher-integrity one (Medium → High, e.g. an
elevated Notepad):

- **Reads cross the boundary.** From Medium, UIA resolves an elevated target's
  name, class, control type, pid, bounds, and node count identically to a High
  caller, and `WM_GETTEXT` reads succeed (A9-2). Observation must never be
  refused across the boundary.
- **Writes do not.** `SendInput` events do not land and `PostMessage` returns
  false with `ERROR_ACCESS_DENIED (5)` (A9-2). The adapter maps this to
  `PERM_DENIED` whose `platform_detail` reads
  `COM HRESULT 0x80070005 (E_ACCESSDENIED: Access is denied)`.
- **Fix:** run agent-desktop from a terminal elevated to the target's level,
  or act on elements that do not cross the boundary.

### Detection Is a Token Read, Never a SendInput Verdict

- The adapter decides up front by comparing mandatory-integrity RIDs read from
  process tokens (`GetTokenInformation(TokenIntegrityLevel)`); a strictly
  higher target refuses before synthesis (A20-1).
- `SendInput`'s return value proves nothing: it reported six events accepted
  with `lastError 0` in both arms of the measurement, and a cross-boundary
  `PostMessage` once returned true carrying a stale `lastError 203` (A9-3).
  Only a token comparison or a post-write re-read separates delivered from
  silently dropped.
- An integrity level the process cannot read is never asserted same-or-lower;
  such targets proceed best-effort and are judged by re-reading effect.

## Window Activation Crosses the Boundary

`focus-window` succeeds against a higher-integrity target: measured live, a
confirmed-Medium caller drove the product until a raw `GetForegroundWindow`
re-read — not the command's own envelope — showed the elevated fixture genuinely
foreground (A24-16). `SetForegroundWindow` is gated by Windows'
foreground-lock heuristic (recent input, `AllowSetForegroundWindow`), not by
mandatory integrity. When verification cannot confirm foreground before the
budget runs out on a strictly-higher target, the error is activation-worded
`PERM_DENIED`; on equal integrity it is `ACTION_FAILED`.

## Blocked Key Combos

`press` refuses dangerous shortcuts before synthesis; every modifier order,
key alias (`meta`/`cmd`/`super` → `win`), and modifier superset is caught:

| Blocked | Why |
|---------|-----|
| `alt+f4` | closes the active window |
| `win+l` | locks the session |
| `win+d` | shows the desktop |
| `alt+tab` | steals the foreground mid-run |

Supersets like `alt+shift+tab` are blocked too. `--force` overrides the guard;
the calling agent keeps control.

`ctrl+alt+delete` is deliberately absent: it is the Secure Attention Sequence,
which `SendInput` cannot synthesize at all. Listing it would advertise a guard
this platform does not provide — there is nothing to block.

## Protected Processes

`close-app` refuses session- and shell-critical images by exact,
case-insensitive `.exe` name: `csrss.exe`, `wininit.exe`, `winlogon.exe`,
`services.exe`, `lsass.exe`, `smss.exe`, `lsaiso.exe`, `dwm.exe`,
`explorer.exe`. Near-misses (`explorer++.exe`, a path merely containing
`lsass`) are deliberately unprotected and close normally. A refusal surfaces
as `INVALID_ARGS`; target a regular application.

## Cross-Process Interaction Lease

Interactive work holds an exclusive, token-derived lock file so two concurrent
agent-desktop processes cannot interleave focus changes, clicks, and typing
against the same desktop. An inherited-handle mode
(`AGENT_DESKTOP_INTERACTION_LEASE_HANDLE`) lets a parent pass its lease to a
child after re-verifying identity and exclusivity. If acquisition fails, wait
for the other process to finish rather than bypassing the lease.

## The Cursor Overlay's Control Pipe

The cursor overlay is a detached renderer process driven over a named pipe.
Know its posture before you enable it on a host you share.

**What is protected:**

- **Remote clients are rejected.** The pipe is created with
  `PIPE_REJECT_REMOTE_CLIENTS`, so it is reachable only from this machine.
- **A different user cannot reach it.** Every connection is resolved to its
  peer process and that process's token user SID is compared against this
  one's before any payload is read. Another user's process is refused, in both
  directions.
- **The renderer must be this tool's own image.** Before it writes a control,
  the client resolves the pipe server's process and checks its executable
  name, so controls are not handed to a stranger that happens to answer. A
  mismatch closes the handle and `enable` reports `rendered: false`. This is a
  file-name comparison (`agent-desktop`, case-insensitively, with or without
  `.exe`) — not a signature check.
- **Only one renderer per session.** The name is claimed with
  `FILE_FLAG_FIRST_PIPE_INSTANCE`, so a second renderer loses the race and
  exits before it creates a window.

**What is not protected — a same-user process on the same machine.** The pipe
carries no security descriptor, and its name is a deterministic function of
the state root, the session id and the protocol generation, so any process
running as you can compute it without a lookup file, a registry entry or a
pid. Two consequences follow, and they are different:

- **Squatting.** A same-user process that claims the name first is refused by
  the image check above, so it cannot impersonate the renderer — but it does
  hold the name, and `enable` then answers `rendered: false` for as long as it
  does. The failure mode is a missing overlay, not a fake one.
- **Driving.** A same-user process that connects to a live renderer passes the
  client check, because clients are authenticated by user alone — an FFI host
  legitimately runs under its own image and must still be able to reach the
  renderer it started. Such a process can move the presented cursor and put
  arbitrary text in the topmost label card. A same-user process whose
  executable is itself named `agent-desktop` also passes the server-side image
  check.

This is a deliberate, accepted trade-off for single-user hosts, which is what
every agent host this adapter targets is. The overlay is presentation-only —
it never performs an action, never fails one, and carries no credential — so
the exposure is what is drawn on the screen, not what is done to the desktop.
On a host where other people run processes as your account, do not treat what
the overlay draws as trustworthy, and prefer to leave it disabled.
