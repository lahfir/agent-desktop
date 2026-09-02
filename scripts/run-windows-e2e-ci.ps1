#Requires -Version 5.1
<#
.SYNOPSIS
    The Windows analog of scripts/run-native-e2e-ci.sh: the one CI entry
    point that builds and runs the exclusive tests/e2e-windows suite.

.DESCRIPTION
    Mirrors run-native-e2e-ci.sh line for line in intent, including the
    refusal guard whose whole purpose is that a stray checkout cannot drive
    somebody's desktop: refuses off Windows, refuses without
    AGENT_DESKTOP_NATIVE_E2E_RUNNER=1, and does both refusals before
    touching cargo, the desktop lease, or any process (R9 test scenario).

    R13b (suite/lib-test serialization): the desktop lease is acquired here,
    before the build, so a concurrent `cargo test -p agent-desktop-windows`
    holding the same lease makes this script wait out Enter-DesktopLease's
    contention loop rather than racing it mid-build.

    R15b (one acquisition per entry path): the lease is handed off to
    Run-E2E.ps1, never re-acquired there - a second exclusive open of a
    path this same process already holds fails with
    ERROR_SHARING_VIOLATION even though it is the same process. The handle
    is made inheritable for exactly the one spawn (AGENT_DESKTOP_E2E_DESKTOP_LEASE_HANDLE),
    then the inheritable flag is cleared immediately after, mirroring
    DesktopLease.psm1's own Invoke-GuardedAgent discipline.

    The U6 self-test tier (Invoke-U6SelfTests.ps1) runs between the fixture
    build and the live suite, gating it: a failing case here fails this
    script before a single live scenario runs. U6's own cases manage their
    own Enter-DesktopLease/Exit-DesktopLease calls and, in one case, open
    the canonical lock file directly as if they were the sole holder - so
    this script's own lease is released for that window and re-acquired
    immediately after, rather than handed off, keeping this process a
    single lease owner at any one time.
#>
[CmdletBinding()]
param()

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot

if ($env:OS -ne 'Windows_NT') {
    Write-Host 'run-windows-e2e-ci.ps1: refusing - the windows E2E runner must be Windows'
    exit 2
}
if ($env:AGENT_DESKTOP_NATIVE_E2E_RUNNER -ne '1') {
    Write-Host 'run-windows-e2e-ci.ps1: refusing native desktop control without AGENT_DESKTOP_NATIVE_E2E_RUNNER=1'
    exit 2
}

Import-Module (Join-Path $repoRoot 'tests\e2e-windows\Native.psm1') -Force
Import-Module (Join-Path $repoRoot 'tests\e2e-windows\BoundedProcess.psm1') -Force
Import-Module (Join-Path $repoRoot 'tests\e2e-windows\DesktopLease.psm1') -Force

$handoffEnvName = 'AGENT_DESKTOP_E2E_DESKTOP_LEASE_HANDLE'
$runE2EPath = Join-Path $repoRoot 'tests\e2e-windows\Run-E2E.ps1'
$releaseBinary = Join-Path $repoRoot 'target\release\agent-desktop.exe'
$fixtureBuildScript = Join-Path $repoRoot 'tests\fixture-app-windows\build.ps1'
$u6SelfTestScript = Join-Path $repoRoot 'tests\e2e-windows\selftest\Invoke-U6SelfTests.ps1'

$exitCode = 1
$leaseHeld = $false
Push-Location $repoRoot
try {
    Write-Host 'run-windows-e2e-ci.ps1: acquiring the desktop lease before build (R13b)'
    $lease = Enter-DesktopLease
    $leaseHeld = $true
    Write-Host ('run-windows-e2e-ci.ps1: desktop lease acquired (contention rounds: ' + $lease.ContentionCount + ')')

    Write-Host 'run-windows-e2e-ci.ps1: building agent-desktop --release'
    & cargo build -p agent-desktop --locked --release
    if ($LASTEXITCODE -ne 0) { throw "cargo build -p agent-desktop --locked --release failed with exit code $LASTEXITCODE" }

    Write-Host 'run-windows-e2e-ci.ps1: building the E2E fixture'
    $fixtureOutput = @(& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $fixtureBuildScript 2>&1)
    $fixtureExitCode = $LASTEXITCODE
    $fixtureOutput | ForEach-Object { Write-Host $_ }
    if ($fixtureExitCode -ne 0) { throw "the E2E fixture build failed with exit code $fixtureExitCode" }

    # The cursor-overlay teardown tests need a composited desktop they can read
    # back with BitBlt, which a hosted CI runner does not have: it has a window
    # station enough for UI Automation, so a WPF fixture stages fine there while
    # a layered overlay reads as zero pixels. They therefore belong to this
    # operator-run suite, under the same exclusive lease, rather than to a lane
    # that would fail for a reason that says nothing about the renderer.
    Write-Host 'run-windows-e2e-ci.ps1: running the cursor-overlay teardown tests under the lease'
    $env:AGENT_DESKTOP_LIVE_WPF = '1'
    & cargo test -p agent-desktop --locked --release --test windows_cursor_overlay_teardown -- --test-threads=1
    $overlayExitCode = $LASTEXITCODE
    Remove-Item Env:\AGENT_DESKTOP_LIVE_WPF -ErrorAction SilentlyContinue
    if ($overlayExitCode -ne 0) { throw "the cursor-overlay teardown tests failed with exit code $overlayExitCode" }
    Write-Host 'run-windows-e2e-ci.ps1: cursor-overlay teardown tests passed'

    Write-Host 'run-windows-e2e-ci.ps1: releasing the desktop lease so the U6 self-test tier can manage its own lease lifecycle'
    Exit-DesktopLease
    $leaseHeld = $false

    # The cursor-overlay frame budgets are wall-clock numbers, and a wall-clock
    # budget describes hardware. Asserting them on every lane was tried and a
    # hosted ARM64 runner failed on timing alone, which is how a real budget
    # gets loosened into one that can no longer fire. They are enforced here
    # instead - the quiesced, operator-controlled host this suite already
    # requires - and reported without assertion everywhere else. Release,
    # because the numbers state what a shipped binary does; and outside the
    # desktop lease, because these compose pixels into memory and need no
    # desktop at all, so holding the lease would only lengthen it.
    Write-Host 'run-windows-e2e-ci.ps1: enforcing the cursor-overlay frame budgets'
    $env:AGENT_DESKTOP_PERF_BUDGET = '1'
    & cargo test -p agent-desktop-windows --locked --release --lib cursor_overlay::render -- --nocapture
    $frameBudgetExitCode = $LASTEXITCODE
    Remove-Item Env:\AGENT_DESKTOP_PERF_BUDGET -ErrorAction SilentlyContinue
    if ($frameBudgetExitCode -ne 0) { throw "the cursor-overlay frame budgets failed with exit code $frameBudgetExitCode" }
    Write-Host 'run-windows-e2e-ci.ps1: cursor-overlay frame budgets passed'

    Write-Host 'run-windows-e2e-ci.ps1: running the U6 harness-core self-test tier before the live suite'
    $u6Result = Invoke-BoundedProcess -FilePath 'powershell.exe' `
        -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $u6SelfTestScript, '-AgentDesktopBinary', $releaseBinary) `
        -TimeoutSeconds 600 `
        -MaxCaptureBytes 52428800
    if ($u6Result.StdOut) { Write-Host $u6Result.StdOut }
    if ($u6Result.StdErr) { Write-Host $u6Result.StdErr }
    if ($u6Result.TerminationNote) { Write-Host ('run-windows-e2e-ci.ps1: ' + $u6Result.TerminationNote) }
    if ($u6Result.TimedOut) { throw 'run-windows-e2e-ci.ps1: the U6 harness-core self-test tier timed out' }
    if ($u6Result.ExitCode -ne 0) { throw "run-windows-e2e-ci.ps1: the U6 harness-core self-test tier failed with exit code $($u6Result.ExitCode)" }
    Write-Host 'run-windows-e2e-ci.ps1: U6 harness-core self-test tier passed'

    Write-Host 'run-windows-e2e-ci.ps1: re-acquiring the desktop lease before the live suite (R13b)'
    $lease = Enter-DesktopLease
    $leaseHeld = $true

    $leaseValue = Get-DesktopLeaseHandleValue
    if ($null -eq $leaseValue) { throw 'run-windows-e2e-ci.ps1: no desktop lease handle held after Enter-DesktopLease' }
    $handle = [IntPtr]$leaseValue

    $childEnvironment = @{}
    foreach ($entry in [System.Environment]::GetEnvironmentVariables().GetEnumerator()) {
        $childEnvironment[[string]$entry.Key] = [string]$entry.Value
    }
    $childEnvironment[$handoffEnvName] = [string]$leaseValue

    Set-NativeHandleInheritable -Handle $handle -Enabled $true
    $result = $null
    try {
        Write-Host 'run-windows-e2e-ci.ps1: handing the desktop lease off to Run-E2E.ps1 (R15b - one acquisition per entry path)'
        $result = Invoke-BoundedProcess -FilePath 'powershell.exe' `
            -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $runE2EPath, '-AgentDesktopBinary', $releaseBinary) `
            -Environment $childEnvironment `
            -TimeoutSeconds 2700 `
            -MaxCaptureBytes 52428800
    } finally {
        Set-NativeHandleInheritable -Handle $handle -Enabled $false
    }

    if ($result.StdOut) { Write-Host $result.StdOut }
    if ($result.StdErr) { Write-Host $result.StdErr }
    if ($result.TerminationNote) { Write-Host ('run-windows-e2e-ci.ps1: ' + $result.TerminationNote) }
    if ($result.TimedOut) { Write-Host 'run-windows-e2e-ci.ps1: Run-E2E.ps1 timed out' }
    if ($result.OutputLimited) { Write-Host 'run-windows-e2e-ci.ps1: Run-E2E.ps1 output exceeded the capture cap' }

    $exitCode = $result.ExitCode
} catch {
    Write-Host "run-windows-e2e-ci.ps1: aborting - $($_.Exception.Message)"
    $exitCode = 1
} finally {
    if ($leaseHeld) { try { Exit-DesktopLease } catch { Write-Warning "lease teardown: $($_.Exception.Message)" } }
    Pop-Location
}

exit $exitCode
