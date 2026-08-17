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
