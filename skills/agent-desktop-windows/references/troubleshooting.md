# Troubleshooting

Symptom-to-cause map for the Windows adapter. Each entry names the observable
symptom first; the fix assumes the observe-act loop and JSON envelope are
already familiar.

## Empty or Tiny Tree

| Symptom | Cause | Fix |
|---------|-------|-----|
| Chromium/Electron app returns far fewer nodes than the UI shows | First-contact read taken before the renderer's accessibility build settled (13 vs 172 measured, A1-5) | Re-run `snapshot --timeout-ms 30000` — the settle can take 10–25 s cold (A16-11) |
| Node count frozen at a small number no matter how long you wait | Target window fully covered by other windows during settle (A1-6) | Bring the target to the foreground or minimize the covering windows, then re-snapshot |
| WPF app: one node, `ClassName` like `HwndWrapper[...]`, zero children | Tree read before the automation peer existed bound the generic HWND provider permanently (A1-7) | Take a fresh snapshot so resolution starts from a new handle; in-process retries never recover |
| Custom-rendered UI or games return nothing meaningful | No accessibility exposure at all | Out of scope for accessibility automation; use screenshots for verification only |

## PERM_DENIED with an E_ACCESSDENIED Detail

```json
"code": "PERM_DENIED",
"platform_detail": "COM HRESULT 0x80070005 (E_ACCESSDENIED: Access is denied)"
```

Cause: the target process runs at a strictly higher integrity level (typically
an elevated app driven from a Medium terminal); UIPI blocks synthesized input
across that boundary while reads still succeed.

Fix: relaunch your terminal elevated to the target's level. Detection is a
token-integrity comparison made before synthesis — `SendInput`'s own return
value cannot be trusted as evidence either way (A9-3). Window activation is
the measured exception that does cross the boundary (A24-16).

## COM Apartment Error

```json
"message": "... COM multithreaded apartment ..."
```

Cause: UIA requires the calling thread to belong to a COM apartment. The CLI
joins the multithreaded apartment itself; seeing this error means an embedding
host's threading conflicts with it.

Fix: in FFI consumers, initialize through the library's bootstrap entrypoints
rather than calling UIA from arbitrary threads; the error's `suggestion` names
joining the multithreaded apartment. An uninitialized apartment is a defect of
the calling process, not of the target application.

## DPI-Scaled Coordinates

Symptom: headed coordinate actions (`mouse-click --xy`, drag endpoints) land
offset from the visible element on scaled or multi-monitor setups.

Cause: without per-monitor awareness, Windows lies to the process about
coordinates. The adapter applies `Per-Monitor V2` at startup; coordinates in
bounds and `--xy` values are physical pixels.

Notes:

- A host that already fixed an incompatible awareness context is tolerated,
  not fatal (`ERROR_ACCESS_DENIED` on the second call means already set).
- If startup reports `INTERNAL` with a `SetProcessDpiAwarenessContext` Win32
  error detail, the hosting process locked the context first — rerun from a
  plain terminal rather than an embedded host.
- Prefer refs over raw coordinates; bounds read through UIA are already in
  physical pixels after awareness is applied.

## The Binary Will Not Start

agent-desktop ships unsigned on Windows, which three execution controls treat
differently:

- **SmartScreen (browser downloads):** a browser attaches Mark-of-the-Web, so
  double-clicking a downloaded binary can show the "Windows protected your
  PC" warning — choose More info → Run anyway. The npm install path is
  different by measurement: System32 curl writes with plain file I/O and
  attaches **no** mark (a real HTTP fetch landed with
  `zone_identifier_present: false`, A25-5), so no mark-gated prompt can fire
  during npm installation.
- **Antivirus quarantine:** Defender or third-party scanners may quarantine a
  freshly downloaded unsigned executable regardless of how it was obtained.
  Restore/allow-list it or verify the published SHA-256 checksum and rebuild
  trust at your organization's proxy.
- **Smart App Control:** on a clean Windows 11 install it blocks unknown
  unsigned executables at process creation — including command-line launches,
  the path this CLI actually uses. If enabled, neither the download path nor
  the launch mode avoids it; disable Smart App Control or use a signed build.

## POLICY_DENIED from a Shell-Surface or Notification Command

```json
"code": "POLICY_DENIED"
```

Symptom: `open-system-surface`, `list-notifications`, `wait --notification`
or a notification mutation refuses without touching the desktop.

Cause: the command raises shell chrome — a shell surface or the Action
Center — and takes the foreground by definition, so a strict-headless call is
refused before anything is raised.

Fix: pass global `--headed`, or put the surface up yourself first.
`snapshot --surface <kind>` reads an already-present shell surface headless,
and `list-notifications` / `wait --notification` adopt an already-open Action
Center without raising it.

## wait --notification Times Out While Toasts Are Being Posted

Cause: on this shell the Action Center collects a toast only while it is
open, and a center that closes evicts its entries (A26-3). The wait holds no
long-lived session — it opens and closes the center per poll, adopting one
that is already present and restoring the entry state afterwards, so the
window between polls is exactly where a staged toast is lost.

Fix: hold the center open yourself across the staging window — open it
before the toasts are posted and the wait's polls will adopt it without
closing it — or confirm the toast landed with a `list-notifications` taken
while the center is open.

## A Hosted (UWP) App Reads as ApplicationFrameHost

Symptom: `focused_window` or `list-windows` reports a hosted UWP application
(Settings and peers) under the frame host's identity instead of the app's
own, or an identity that was verified against the app's pid stops resolving.

Cause: the app is suspended. Suspension drops the app's
`Windows.UI.Core.CoreWindow` while its `ApplicationFrameWindow` frame
survives, and the frame is what the listing reports (A26-8).

Fix: activate the app so it resumes, then re-list — the hosted identity
returns. When the app is live, the documented shape is a deliberate split:
`id` is the frame's handle (the handle every window operation targets) while
`app` and `pid` name the hosted application read one level down.

## INTERNAL: "private file parent is owned by a foreign principal"

```json
"code": "INTERNAL",
"message": "private file parent is owned by a foreign principal, not this
process's token owner"
```

Cause: the data directory under your home (`~/.agent-desktop`) was created by
a process with a different token owner than the one now writing into it — on
Windows this is almost always an elevated (Run as administrator) shell using
a directory a non-elevated session created, or the reverse. The refusal is
the private-file ownership check working as designed across an integrity
boundary; it protects refmaps and trace segments from a pre-planted path.

Fix: run consistently from one elevation level, or point `HOME` at a fresh
directory for the elevated session so the tool creates its own data tree.
Do not take ownership of or ACL-open the existing directory to work around
it — that defeats the check the error exists for.

## Action Verification

Every stateful action — `set-value`, `clear`, `type`, `toggle`, `check`,
`uncheck`, `expand`, `collapse` — is performed **exactly once** and then
verified by re-reading live state. The settle loop only re-reads; it never
re-performs. It polls at 50 ms up to a 400 ms cap and stops as soon as the
postcondition holds, so an action that lands immediately pays nothing.

Three outcomes, and they are not interchangeable:

- The readback matches the request: `disposition.delivery` is
  `delivered_verified`. A value write also carries
  `details.verification_scope: "element_value"` with
  `application_commit: "not_verified"` — the control holds your value; whether
  the application has committed it is a separate question the control cannot
  answer.
- The readback contradicts the request: `ACTION_FAILED` with
  `details.kind: "post_action_verification"` and `details.post_state`. **Read
  `post_state` before doing anything else.** The action was delivered, so
  `disposition.retry` is `unsafe` and repeating it may act twice.
- The evidence was never observed: the action succeeds as
  `delivered_unverified` with `details.verification_scope: "unavailable"`.
  This is not a failure and not proof the change did not happen — it means the
  UIA property the verifier needs was not readable on that element. A control
  whose TogglePattern or ExpandCollapsePattern is absent, or whose property
  read failed, reports this rather than a false verdict. **`type` always lands
  here on Windows today**: deriving what a field should contain after an
  insertion needs the selection range, and the adapter does not yet read one.
  A `type` is therefore correct and unverified, never falsely failed — confirm
  it with a fresh `get` if the value matters.

**A `set-value` that the application reshapes is reported as failed.** If the
app trims whitespace, reformats a date, or normalizes a number, the readback no
longer equals what you wrote and the envelope says `ACTION_FAILED` even though
the write landed. The `SetValue` comparison captures no *before* state, so it
cannot separate "nothing happened" from "accepted and reshaped" — only the
first proves non-delivery. When the failure surprises you, compare
`details.post_state.value` against what you sent: if it holds your value in the
application's own formatting, the write succeeded and it is this limitation you
are seeing, not a broken control. Numeric tolerance is role-scoped — only
`slider`, `incrementor`, `scrollbar` and `handle` compare numerically, so a
`textfield` an app stores as `42.0` after you wrote `42` fails this way.

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
