# Windows live E2E harness

PowerShell 5.1, no Python, no POSIX shell. Drives the real, staged, hashed
`agent-desktop.exe` against the real WinForms fixture (`tests/fixture-app-windows`)
and asserts every effect by independent re-observation, never by a command's
own `ok:true`.

## Modules

| File | Owns |
|---|---|
| `NativeTypes.psm1` | The Win32 P/Invoke type surface (structs, DllImport declarations) loaded once via `Add-Type`. |
| `Native.psm1` | File/handle/job-object/token-SID primitives built on `NativeTypes.psm1`. |
| `NativeDesktop.psm1` | Raw foreground-window/cursor/GUI-thread reads, also built on `NativeTypes.psm1`. |
| `BoundedProcess.psm1` | `Invoke-BoundedProcess` - job-object-bounded child spawn with a wall-clock deadline and a captured-output byte cap. |
| `Harness.psm1` | Isolated environment (`HOME`/`TEMP`/`CARGO_TARGET_DIR` redirection), recoverable delete, immutable binary staging + hash re-verification, fixture process identity, and `ConvertFrom-AgentJson`. |
| `DesktopLease.psm1` | The desktop-exclusivity lease and its CI adoption protocol, and the two guarded spawn entry points (`Invoke-Guarded`, `Invoke-GuardedAgent`). |
| `Lib.psm1` | Target objects that always carry their snapshot (`Find-Target`, `Get-Target`, `Test-Target`, `Wait-Target`, `Invoke-Target`), the independent-observation assertions (`Assert-Effect`, `Assert-NoEffect`, `Assert-Envelope`), lock ordering (`Enter-Stage`), and the verdict/skip ledger (`Register-Legs`, `Add-Pass`/`Add-Fail`/`Add-Skip`, `Write-Verdict`). |
| `Run-E2E.ps1` | The one process-terminating call in the whole tree. Scenario files (`scenarios/*.ps1`) return; only this script exits. |

`Harness.psm1`/`Native.psm1` and their self-tests were split (and `DesktopLease.psm1`/`NativeTypes.psm1`/`NativeDesktop.psm1` introduced) purely to keep every file under the 400-line structural cap `scripts/check-e2e-windows-contract.ps1` enforces (rule 13); no exported function moved, only which file owns it. Every `selftest/*.ps1` entry point is likewise paired with a `*Cases*.ps1` file it dot-sources for the same reason.

## Data files

- `skip-allowlist.psd1` - every capability token a scenario may pass to
  `Add-Skip`, with a reason. An undeclared token fails the run (a skip must
  never read as a pass).
- `latency-baseline.psd1` - the committed per-leg p99 store `Assert-NoEffect`'s
  window is derived from. Starts with only a conservative bootstrap value;
  the cost run appends one entry per positive `Assert-Effect` leg.

## Running

```powershell
# U7's self-tests: no desktop, no fixture, no staged binary - hosted-runner-runnable.
powershell -NoProfile -File tests\e2e-windows\selftest\Invoke-U7SelfTests.ps1

# U6's self-tests: needs a real desktop and target\release\agent-desktop.exe already
# built - acquires the real desktop lease, not hosted-runner-runnable.
powershell -NoProfile -File tests\e2e-windows\selftest\Invoke-U6SelfTests.ps1

# The seeded-failure entry path (proves a failing leg reaches a non-zero
# process exit code, run on the hosted CI lane on every PR):
powershell -NoProfile -File tests\e2e-windows\Run-E2E.ps1 -SelfTestSeedFailure

# The live suite (local box only - acquires the desktop-exclusivity lease):
powershell -NoProfile -File tests\e2e-windows\Run-E2E.ps1
```

## Conventions a scenario file must follow

- Every ref is a target object from `Find-Target`/`Find-TargetById`, carrying
  both `RefId` and `SnapshotId`. No helper accepts or returns a bare `@eN`.
- Effect assertions go through `Assert-Effect`/`Assert-NoEffect`; envelope
  assertions go through `Assert-Envelope`. Touching `.ok`/`.error`/
  `.disposition`/`.data` on a command result directly is a `Lib.psm1`-only
  privilege.
- Every desktop-touching leg is lexically wrapped in `Enter-Stage`, declaring
  locks in the fixed order `DesktopLease` -> `ForegroundStage` -> `MenuStage`.
- Every scenario calls `Register-Legs` with its leg names up front; every
  registered leg ends the run with a disposition (`Add-Pass`/`Add-Fail`/
  `Add-Skip`).
