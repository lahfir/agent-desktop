# Windows platform exploration probes (sub-phases 2.0 and later)

Evidence-gathering scripts for the Windows adapter. They observe the real OS and write
bounded, redacted captures that the `FINDINGS.md` ledger cites. Nothing here is product
code, and nothing here is a workspace member.

Sub-phase 2.0 (`00-*.ps1` through `13-*.ps1`) is PowerShell only. Later sub-phases extend
the corpus where a question can only be answered against the crate the adapter actually
ships or against a runner this box is not: `14-ci-capability/` adds a standalone Rust
probe, built in a scratch directory outside the workspace, and
`.github/workflows/windows-capability-probe.yml` runs it on `windows-latest`. That
workflow is path-filtered to the probe directory and its own file, so it adds no time to
the required lanes.

## Prerequisites

- Windows with Windows PowerShell 5.1 (`pwsh` is not required and is not used).
- .NET Framework 4.8 — supplies managed UIA (`System.Windows.Automation`) and the in-box
  compiler at `%WINDIR%\Microsoft.NET\Framework64\v4.0.30319\csc.exe`.
- Obsidian installed at `%LOCALAPPDATA%\Programs\Obsidian\Obsidian.exe` — the Electron
  probe target. Its version is read at probe time and recorded in the ledger.
- An interactive console session. Input-synthesis and hit-testing probes need a real
  desktop; they are not valid over a headless/disconnected session.

## Running

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\run-all.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File .\run-all.ps1 -Compare
```

`run-all.ps1` executes every top-level `NN-*.ps1` in lexical order, each in its own
PowerShell process, and prints one summary line per probe. The sub-phase probes that live
in their own `NN-*/` directory are **not** in that set — their scripts are named
`probe.ps1`/`census.ps1` and are driven by `.github/workflows/windows-capability-probe.yml`
or run by hand. The redaction gate below is the exception: it sweeps those directories too,
so "clean" means clean over every capture in the corpus. It exits:

- `0` — every probe produced a capture, the redaction gate is clean, no spawned process survived.
- `1` — probe failure, missing capture, redaction residue, surviving process, or (in
  `-Compare` mode) a non-empty normalized diff.
- `2` — harness error (missing `common.ps1`, unwritable captures directory, probe could
  not be launched). A harness error aborts the run; a probe-level failure is recorded and
  the run continues.

Individual probes are runnable on their own: `powershell -NoProfile -File .\05-interactions.ps1`.

## Capture layout

```
captures/
└── <probe-name>/
    ├── <capture>.json              # redacted capture, BOM-less UTF-8
    └── <capture>.json.normalized   # KTD9 normalized twin, written beside the capture
```

The normalized twin canonicalizes run-varying values so a re-run diffs empty: process ids,
thread ids, window handles, UIA `RuntimeId` arrays, elapsed/duration/timing numbers, GUIDs,
temp-file paths, and timestamps. Rectangle components (`X`/`Y`/`Left`/`Top`/`Right`/
`Bottom`/`Width`/`Height` and `Bounds` arrays) are rounded to an **8-pixel bucket** to
absorb layout jitter. `run-all.ps1 -Compare` regenerates the twin from each committed
capture and reports any difference — a non-empty diff is real platform drift, which is
exactly what a later re-runner needs to see.

## Redaction gate (R11)

Every capture is written through `Protect-ProbeText`, which replaces the operator's user
name, the machine and DNS host names, user-profile paths (`%USERPROFILE%`,
`%LOCALAPPDATA%`, `%APPDATA%`, and any `C:\Users\<name>` in either slash form), and SID
sub-authorities with stable placeholders. `S-1-5-21-a-b-c-RID` becomes
`S-1-5-21-<redacted>-RID`: the RID is load-bearing evidence, the machine-unique part is
not. Well-known SIDs with no domain part (`S-1-5-18`, `S-1-5-32-544`, `S-1-16-12288`, …)
pass through verbatim — the integrity-level SIDs are the point of several probes.

`Protect-ProbeName` reduces a document/content node's `Name` to `<redacted:N chars>`.
`ControlType`, `AutomationId`, `ClassName`, bounds, patterns, and states stay verbatim, and
application-chrome names are kept by the caller simply not calling it.

**`Protect-ProbeName` is a call-site reducer, not part of the gate, and a Rust probe cannot
reach it.** `Protect-ProbeText` and `Test-CaptureRedaction` check operator identity — user
name, machine and DNS names, profile paths, SIDs — and nothing else; neither has any rule
for a content node's `Name`, in any probe language. A `.ps1` probe that reads a content
`Name` calls `Protect-ProbeName` itself; a Rust probe emits its own JSON, which the
orchestrator only passes through `Protect-ProbeText`, so **a Rust probe must reduce a
content `Name` at the point of record** — a length, a presence flag, or a stable digest —
because nothing downstream will do it and nothing downstream will notice. `probe_survival.rs`
is the worked example: markers are paired across captures by an FNV-1a digest of the name,
never by the name.

There is no writer that bypasses the gate. `run-all.ps1` re-asserts the gate over every
capture before exiting and names any offending file — for the identity classes above and for
the echoed-name residue below, which is what "clean" means here.

### Names a container inherits from its own content

A container carrying no label of its own takes its accessible name from the content beneath
it, so a document title lands on a `Group` or a `Button` that no content-control-type test can
see and no window-title comparison catches — the leaked name belongs to a workspace split the
window title does not name. On a target the caller declared to carry user content
(`-RedactRootName`), `01-tree-dump.ps1` therefore also reduces any `Name` that contains, as a
>=4 character substring, the `Name` of a descendant this rule already reduced.

That scope is what separates the rule from evidence destruction, and it was chosen against a
measurement rather than a guess. Unscoped, the same rule reduces the Settings frame's window
title, because a system app's title and its home-page header are both the word `Settings` — and
it reduces it on the frame window and on the core window while leaving the title-bar window
verbatim, dismantling the frame/host split those three nodes exist to demonstrate. Chrome in a
content-carrying app survives for a structural reason rather than a lucky one: an explicit
label owes nothing to the subtree beneath it, and the subtree of a clickable icon is an unnamed
`Image`. The Electron capture keeps every one of its chrome names — `Close`, `Bookmarks`,
`New tab`, the ribbon actions, the view actions — verbatim.

`Test-CaptureNameEcho` re-asserts the rule over a committed capture, where the raw strings are
gone and only lengths survive: inside a pass whose root `Name` is itself reduced — the capture's
own record that the target was declared to carry user content — a kept `Name` is residue when
its own subtree holds a reduced `Name` short enough to fit inside it. Length containment is
weaker than the string containment the reducer applies, so the gate flags a superset and cannot
miss what the reducer would have caught. A residue line names the two node indexes and their
lengths, never the value.

**What no predicate here decides.** This rule catches a title a container *echoes* from content
the corpus has already reduced. A container that carries a document title as its own explicit
label, with nothing reduced beneath it to compare against and no match against the window
title, is indistinguishable from chrome by any structural test and would be published verbatim.
Nothing automatic closes that, so the Electron target's safety is an operator decision, not a
predicate's.

> Committed captures publish a software fingerprint of this VM: OS build, locale, installed
> app versions, window class names, and automation ids. That is a deliberate choice — this
> is a throwaway probe box, and the evidence is worthless if it is redacted to the point of
> being unverifiable. Do not run this corpus on a machine whose software inventory is
> sensitive, and point the Electron probe at a vault whose note titles are not: the corpus
> publishes a tree walked over whatever content that vault holds.

## Safety envelope (KTD5)

Interaction, input-synthesis, and elevation probes act only on scratch windows the probe
itself launched.

- `Start-ScratchProcess` records every spawned pid in a run-scoped ledger (a temp file
  named by `AGENT_DESKTOP_PROBE_PIDS`, set by `run-all.ps1`).
- `Stop-ScratchProcess` terminates and then **confirms** termination by re-reading the
  process list, throwing if the pid survives.
- `run-all.ps1` exits nonzero if any tracked pid is still alive at the end of the run.
- `Assert-Foreground` must bracket every `SendInput` call. It reads
  `GetForegroundWindow` + `GetWindowThreadProcessId` and throws a `PROBE-INTERFERENCE:`
  error when the foreground process is not the scratch app. A probe that catches it files
  an interference ledger row — it never re-injects.
- `Show-WindowNoActivate` places a scratch window on the visible desktop with
  `ShowWindow(SW_SHOWNOACTIVATE)` plus an optional `SetWindowPos(SWP_NOACTIVATE|SWP_NOZORDER)`.
  `SetForegroundWindow` is never called.
- The clipboard is snapshotted before and restored after any chord probe.

### Medium-integrity processes

This box runs as the built-in Administrator at High integrity with AAM off, so
`Start-Process -Verb RunAs` yields High-vs-High and produces no integrity boundary.
`Start-MediumIntegrityProcess` manufactures a real Medium-IL process by duplicating the
current primary token, lowering its mandatory label to `S-1-16-8192` with
`SetTokenInformation(TokenIntegrityLevel)`, and launching through `CreateProcessAsUser`.
It then reads the spawned process's token label back with
`GetTokenInformation(TokenIntegrityLevel)` and throws unless it is exactly `S-1-16-8192`.
A UIPI observation taken without a real boundary is an environment artifact and must never
reach the product contract, so this assertion is not optional.

`runas.exe /trustlevel:0x20000` (`SAFER_LEVELID_NORMALUSER`) was tried first and is **not**
sufficient here: measured on this VM, it restricts the token (BUILTIN\Administrators becomes
deny-only) but leaves the mandatory label at `S-1-16-12288` (High). That is itself a probe
finding, not just a harness detail — a UIPI probe built on `runas /trustlevel` would have
measured High-vs-High and reported it as an integrity boundary.

## Source encoding (R12)

- **Probe sources (`.ps1`, `.cs`, `.md`) are saved UTF-8 _with_ BOM.** PowerShell 5.1 parses
  a BOM-less file as ANSI, and this VM's code page is 1252 — non-ASCII literals corrupt
  before they are ever typed.
- **Captures are written BOM-less UTF-8**, only through `Write-ProbeCapture` /
  `Write-ProbeJson`, which use
  `[IO.File]::WriteAllText($path, $text, (New-Object System.Text.UTF8Encoding $false))`.
  Never `Out-File`, `Set-Content`, or `>` for a capture: 5.1 defaults to UTF-16LE, which git
  treats as binary and makes the committed evidence unreviewable.
- Non-ASCII input payloads are built with `[char]::ConvertFromUtf32(...)` so payload
  integrity does not depend on source encoding at all.

To re-save a source with a BOM after editing with a tool that strips it:

```powershell
$p = '.\common.ps1'
$c = [IO.File]::ReadAllText($p)
[IO.File]::WriteAllText($p, $c, (New-Object System.Text.UTF8Encoding $true))
```

## C# 5 ceiling

The in-box compiler is the pre-Roslyn .NET Framework 4.8 `csc.exe` (4.8.3761.0). Every C#
source in this corpus — `csc.exe` shims, scratch apps, and `Add-Type` sources — is capped at
C# 5: no string interpolation (`$"..."`), no null-conditional operators (`?.`), no
expression-bodied members, no `nameof`. Write it plainly and it compiles everywhere.

## `common.ps1` API

| Function | Purpose |
| --- | --- |
| `Get-ProbeRoot` | absolute path of `probes/windows` |
| `Get-CaptureDir -Probe` | `captures/<Probe>`, created if absent |
| `Protect-ProbeText -Text` | R11 redaction over free text |
| `Protect-ProbeName -Name` | `<redacted:N chars>` content-node reducer |
| `Write-ProbeCapture -Probe -Name -Content` | redacted BOM-less UTF-8 capture + normalized twin |
| `Write-ProbeJson -Probe -Name -InputObject` | `ConvertTo-Json -Depth 25`, then `Write-ProbeCapture` |
| `Get-NormalizedCapture -Text` | KTD9 canonicalization |
| `Test-CaptureRedaction -Path` | `$true` when clean; logs each residue reason |
| `Test-CaptureNameEcho -Path` | `$true` when no kept `Name` echoes reduced content beneath it |
| `Read-ProbeNodeRecords -Path` | node records in a capture, keyed to the pass each belongs to |
| `ConvertFrom-ProbeJsonEscape -Text` | undo the JSON escaping a committed capture applied |
| `Start-ScratchProcess -FilePath [-ArgumentList] [-NoActivate] [-TimeoutSec]` | tracked launch |
| `Stop-ScratchProcess -ProcessId` | terminate and confirm gone |
| `Get-ScratchProcessIds` | tracked pids still alive |
| `Assert-Foreground -ExpectedProcessId -Stage` | KTD5 injection guard |
| `Show-WindowNoActivate -WindowHandle [-X -Y -Width -Height]` | place without stealing focus |
| `Start-MediumIntegrityProcess -FilePath [-ArgumentList]` | Medium-IL process, label-asserted |
| `Write-ProbeResult -Probe -Status -Message [-Data]` | `PROBE-RESULT\|...` harness summary line |
| `Write-ProbeLog -Message [-Level]` | `PROBE-LOG [level] ...` |

A probe script dot-sources `common.ps1`, does its work in a `try`, tears down scratch
processes in a `finally`, writes at least one capture, and ends with exactly one
`Write-ProbeResult` whose status is `ok`, `fail`, or `skip`.

## Reading the Windows live suite (A28-6, re-confirmed by A30-4)

`cargo test -p agent-desktop-windows --lib` drives a real desktop, and its
live legs stage windows, menus and toasts that outlive the test that created
them. Three rules follow, and getting them backwards costs the same either
way - a real regression dismissed as flake, or a clean branch blocked by one.

- **Quiesce the desktop first.** Concurrent builds, probe processes and a
  second test run all contend for the same windows.
- **A single red run is not a regression, and a single green run is not
  proof.** Re-run before believing either. Failures wander between unrelated
  modules rather than repeating: while landing this sub-phase they appeared
  in `surface_inventory`, then `frame_identity`, then `notifications` and
  `tree::observe` - four different sets across five runs, every one passing
  in isolation, with the suite green twice on the same tree.
- **Never attribute a one-test difference to a change through a stash A/B.**
  Stashing a change here reads as a one-test regression whether or not the
  change caused anything; A28-6 measured that directly, and this sub-phase
  met it again and nearly went hunting a defect that did not exist.

If a run fails under load, check disk headroom too: A28-6 records a full
volume surfacing as wandering test failures rather than as a write error.
