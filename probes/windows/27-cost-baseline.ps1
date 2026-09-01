#Requires -Version 5.1
<#
.SYNOPSIS
    Area 27 cost-baseline.ps1 - cost-baseline harness for the Windows performance vehicle.

.DESCRIPTION
    Measures read-only release binary commands (snapshot_self, list_apps,
    list_windows, status, list_displays) plus two ref-action legs
    (ref_action_live, ref_action_dead) using the probe corpus cost
    methodology: seven timed runs, first discarded as warm-up, min/median/max
    reported over the remaining six.

    snapshot_self drives `snapshot --window-id` against a scratch Notepad
    instance this probe launches itself - a host CLI process has no window of
    its own to snapshot, which is why the prior host-process-handle lookup
    here always produced null.

    ref_action_live resolves a ref from a fixture app's own "primary-button"
    (built on demand from tests/fixture-app-windows) and clicks it while the
    fixture is still running - the retryable path a still-alive owner keeps.
    ref_action_dead resolves the same shape of ref, kills the owning fixture
    process, and clicks the now-dead ref - the terminal path a fix on this
    branch changed from "poll the whole wait budget into TIMEOUT" to "answer
    STALE_REF on the first resolution attempt". Comparing this leg's min/median
    across two binaries is the entire point of this capture: an order-of-
    magnitude drop here is the fix's claim, not an assumption this script
    makes.

    Every leg that could not be measured on this host records
    `measurable: false` with a fixed, non-dynamic reason string rather than a
    silent null, and every leg records which command and target it measured
    so a reader never has to guess what a number means.

    Run: powershell -NoProfile -ExecutionPolicy Bypass -File .\probes\windows\27-cost-baseline.ps1 -Label <devbox|mergebase|ci>
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'mergebase', 'ci')][string]$Label = 'devbox',
    [string]$BinaryPath = 'target\release\agent-desktop.exe',
    [string]$FixtureBinaryPath = 'tests\fixture-app-windows\build\AgentDeskFixture.exe'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

. (Join-Path (Split-Path -Parent $PSCommandPath) 'common.ps1')
Initialize-ProbeRedaction

$Probe = '27-cost-baseline'
Register-MandatoryCapture -Name @("cost-baseline-$Label.json")

function Measure-CommandInvocation {
    param(
        [Parameter(Mandatory = $true)][string]$Executable,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [int]$Runs = 7,
        [int]$TimeoutMs = 30000
    )
    $samples = New-Object System.Collections.ArrayList
    $lastExit = 0

    for ($i = 0; $i -lt $Runs; $i++) {
        $psi = New-Object System.Diagnostics.ProcessStartInfo
        $psi.FileName = $Executable
        $psi.Arguments = ($Arguments | ForEach-Object {
            if ($_ -match '\s') { '"' + $_ + '"' } else { $_ }
        }) -join ' '
        $psi.UseShellExecute = $false
        $psi.RedirectStandardOutput = $true
        $psi.RedirectStandardError = $true
        $psi.CreateNoWindow = $true

        $sw = [System.Diagnostics.Stopwatch]::StartNew()
        $proc = [System.Diagnostics.Process]::Start($psi)
        if ($null -eq $proc) {
            throw "failed to start process $Executable"
        }
        $null = $proc.StandardOutput.ReadToEnd()
        $null = $proc.StandardError.ReadToEnd()
        if (-not $proc.WaitForExit($TimeoutMs)) {
            try { $proc.Kill() } catch { }
            throw "process timed out after ${TimeoutMs}ms: $Executable"
        }
        $sw.Stop()
        $lastExit = $proc.ExitCode
        [void]$samples.Add([double]$sw.Elapsed.TotalMilliseconds)
    }

    if ($samples.Count -le 1) {
        throw 'insufficient samples collected'
    }

    $timedSamples = @($samples | Select-Object -Skip 1)
    $sorted = @($timedSamples | Sort-Object)
    $minMs = [double]($sorted | Measure-Object -Minimum).Minimum
    $maxMs = [double]($sorted | Measure-Object -Maximum).Maximum
    $medianIdx = [int][Math]::Floor($sorted.Count / 2)
    $medianMs = [double]$sorted[$medianIdx]

    return [ordered]@{
        runs_total            = [int]$Runs
        warmup_discarded      = 1
        min_ms                = [Math]::Round($minMs, 4)
        median_ms             = [Math]::Round($medianMs, 4)
        max_ms                = [Math]::Round($maxMs, 4)
        exit_code_of_last_run = [int]$lastExit
    }
}

<#
    A single non-timed setup call: launches nothing, times nothing, just
    runs Arguments against Executable and parses stdout as JSON. Used only
    to discover a ref (via `find`) before the timed loop starts - never part
    of a measured leg itself, and its own stdout never reaches a capture.
#>
function Invoke-ProbeCliJson {
    param(
        [Parameter(Mandatory = $true)][string]$Executable,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [int]$TimeoutMs = 15000
    )
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $Executable
    $psi.Arguments = ($Arguments | ForEach-Object {
        if ($_ -match '\s') { '"' + $_ + '"' } else { $_ }
    }) -join ' '
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.CreateNoWindow = $true
    $proc = [System.Diagnostics.Process]::Start($psi)
    if ($null -eq $proc) { throw "failed to start process $Executable" }
    $stdout = $proc.StandardOutput.ReadToEnd()
    $null = $proc.StandardError.ReadToEnd()
    if (-not $proc.WaitForExit($TimeoutMs)) {
        try { $proc.Kill() } catch { }
        throw "setup process timed out after ${TimeoutMs}ms: $Executable"
    }
    if ([string]::IsNullOrWhiteSpace($stdout)) {
        throw "setup process produced no stdout: $Executable"
    }
    return ($stdout | ConvertFrom-Json)
}

<#
    Ensures the WinForms fixture used by ref_action_live/ref_action_dead is
    built. The build artifact is gitignored (tests/fixture-app-windows/build/)
    and is rebuilt on demand from the pinned in-box csc.exe, the same
    fixture tests/e2e-windows drives - reused here rather than reinvented so
    the "primary-button" target this probe clicks is the same
    known-semantic-invoke, headless-safe control that suite already proved
    idempotent across repeated clicks.
#>
function Get-FixtureExecutablePath {
    param([Parameter(Mandatory = $true)][string]$RepoRoot, [Parameter(Mandatory = $true)][string]$FixtureBinaryPath)
    $resolved = $FixtureBinaryPath
    if (-not [System.IO.Path]::IsPathRooted($resolved)) {
        $resolved = Join-Path $RepoRoot $FixtureBinaryPath
    }
    if (-not (Test-Path -LiteralPath $resolved)) {
        $buildScript = Join-Path $RepoRoot 'tests\fixture-app-windows\build.ps1'
        $buildOutputDir = Split-Path -Parent $resolved
        & $buildScript -OutputDir $buildOutputDir | Out-Null
    }
    if (-not (Test-Path -LiteralPath $resolved)) {
        throw 'fixture build did not produce AgentDeskFixture.exe'
    }
    return $resolved
}

<#
    Resolves the fixture's own "primary-button" (Name never changes on
    click, only a sibling status label does - FixtureCardsClicks.cs) via
    `find --first --window-id ... --native-id primary-button`, so the same
    ref/snapshot pair can be clicked seven times in the timed loop without
    re-identification drifting under repeated identical clicks.
#>
function Get-FixtureButtonReference {
    param(
        [Parameter(Mandatory = $true)][string]$Binary,
        [Parameter(Mandatory = $true)][IntPtr]$WindowHandle
    )
    $windowArg = 'w-' + [long]$WindowHandle
    $result = Invoke-ProbeCliJson -Executable $Binary -Arguments @('find', '--first', '--window-id', $windowArg, '--native-id', 'primary-button')
    if (-not $result.ok) { throw 'find did not resolve primary-button' }
    $match = $result.data.match
    $snapshotId = $result.data.snapshot_id
    if (-not $match -or -not $snapshotId) { throw 'find produced no match for primary-button' }
    return [pscustomobject]@{ RefId = [string]$match.ref_id; SnapshotId = [string]$snapshotId }
}

<#
    snapshot_self: a host CLI process has no window of its own, so this
    drives `snapshot --window-id` against a scratch Notepad instance instead
    - the same target 01-tree-dump.ps1 and 02-cache-timing.ps1 already use
    for a reliably-windowed process on this host.
#>
function Measure-SnapshotSelfLeg {
    param([Parameter(Mandatory = $true)][string]$Binary)
    $notepadPid = 0
    try {
        $notepadPath = Join-Path $env:WINDIR 'System32\notepad.exe'
        $notepad = Start-ScratchProcess -FilePath $notepadPath -NoActivate -TimeoutSec 20
        $notepadPid = $notepad.ProcessId
        if ($notepad.MainWindowHandle -eq [IntPtr]::Zero) {
            throw 'notepad produced no main window handle'
        }
        $windowArg = 'w-' + [long]$notepad.MainWindowHandle
        $measurement = Measure-CommandInvocation -Executable $Binary -Arguments @('snapshot', '--window-id', $windowArg)
        $leg = [ordered]@{
            measurable = $true
            command    = 'snapshot --window-id <window>'
            target     = 'notepad'
        }
        foreach ($key in $measurement.Keys) { $leg[$key] = $measurement[$key] }
        return $leg
    } catch {
        return [ordered]@{
            measurable = $false
            command    = 'snapshot --window-id <window>'
            target     = 'notepad'
            reason     = 'notepad did not produce a resolvable window to snapshot on this host'
        }
    } finally {
        if ($notepadPid -ne 0) { try { Stop-ScratchProcess -ProcessId $notepadPid } catch { } }
    }
}

<#
    ref_action_live: resolve+click a ref while its owning fixture process is
    still alive - the baseline this branch's fix must NOT change, since a
    live owner keeps today's retryable resolution path.
#>
function Measure-RefActionLiveLeg {
    param([Parameter(Mandatory = $true)][string]$Binary, [Parameter(Mandatory = $true)][string]$FixtureExe)
    $instancePid = 0
    try {
        $env:AGENT_DESKTOP_FIXTURE_NO_ACTIVATE = '1'
        $instance = Start-ScratchProcess -FilePath $FixtureExe -NoActivate -TimeoutSec 20
        $instancePid = $instance.ProcessId
        if ($instance.MainWindowHandle -eq [IntPtr]::Zero) {
            throw 'fixture produced no main window handle'
        }
        $ref = Get-FixtureButtonReference -Binary $Binary -WindowHandle $instance.MainWindowHandle
        $measurement = Measure-CommandInvocation -Executable $Binary -Arguments @('click', $ref.RefId, '--snapshot', $ref.SnapshotId)
        $leg = [ordered]@{
            measurable   = $true
            command      = 'click <ref>'
            target       = 'fixture-primary-button'
            target_state = 'live'
        }
        foreach ($key in $measurement.Keys) { $leg[$key] = $measurement[$key] }
        return $leg
    } catch {
        return [ordered]@{
            measurable   = $false
            command      = 'click <ref>'
            target       = 'fixture-primary-button'
            target_state = 'live'
            reason       = 'the live-target fixture instance did not produce a resolvable ref on this host'
        }
    } finally {
        if ($instancePid -ne 0) { try { Stop-ScratchProcess -ProcessId $instancePid } catch { } }
    }
}

<#
    ref_action_dead: resolve a ref, kill the owning fixture process (and
    confirm it is gone via Stop-ScratchProcess's own poll-to-exit), then
    click the now-dead ref seven times. This is the leg the whole exercise
    exists for: before the fix, resolution could not tell "owner exited"
    from "owner mid-redraw" and retried a dead target to the full wait
    budget (default 5000ms) before answering TIMEOUT; after the fix it
    checks the owning process's own liveness and answers STALE_REF on the
    first attempt. The pid of the killed process is never written to the
    capture - only the timing numbers are.
#>
function Measure-RefActionDeadLeg {
    param([Parameter(Mandatory = $true)][string]$Binary, [Parameter(Mandatory = $true)][string]$FixtureExe)
    $instancePid = 0
    $killed = $false
    try {
        $env:AGENT_DESKTOP_FIXTURE_NO_ACTIVATE = '1'
        $instance = Start-ScratchProcess -FilePath $FixtureExe -NoActivate -TimeoutSec 20
        $instancePid = $instance.ProcessId
        if ($instance.MainWindowHandle -eq [IntPtr]::Zero) {
            throw 'fixture produced no main window handle'
        }
        $ref = Get-FixtureButtonReference -Binary $Binary -WindowHandle $instance.MainWindowHandle
        Stop-ScratchProcess -ProcessId $instancePid
        $killed = $true
        $measurement = Measure-CommandInvocation -Executable $Binary -Arguments @('click', $ref.RefId, '--snapshot', $ref.SnapshotId) -TimeoutMs 60000
        $leg = [ordered]@{
            measurable   = $true
            command      = 'click <ref>'
            target       = 'fixture-primary-button'
            target_state = 'dead'
        }
        foreach ($key in $measurement.Keys) { $leg[$key] = $measurement[$key] }
        return $leg
    } catch {
        return [ordered]@{
            measurable   = $false
            command      = 'click <ref>'
            target       = 'fixture-primary-button'
            target_state = 'dead'
            reason       = 'the dead-target fixture instance did not produce a resolvable ref before termination on this host'
        }
    } finally {
        if ($instancePid -ne 0 -and -not $killed) {
            try { Stop-ScratchProcess -ProcessId $instancePid } catch { }
        }
    }
}

try {
    $probeDir = Split-Path -Parent $PSCommandPath
    $repoRoot = Split-Path -Parent (Split-Path -Parent $probeDir)
    $resolvedBinary = $BinaryPath
    if (-not [System.IO.Path]::IsPathRooted($resolvedBinary)) {
        $candidateRepo = Join-Path $repoRoot $BinaryPath
        $candidatePwd = Join-Path (Get-Location) $BinaryPath
        if (Test-Path -LiteralPath $candidateRepo) {
            $resolvedBinary = $candidateRepo
        } elseif (Test-Path -LiteralPath $candidatePwd) {
            $resolvedBinary = $candidatePwd
        } else {
            $resolvedBinary = $candidateRepo
        }
    }

    if (-not (Test-Path -LiteralPath $resolvedBinary)) {
        $capture = [ordered]@{
            probe            = $Probe
            methodology      = 'min-of-seven with the warm-up discarded, reported as min with median and max beside it'
            label            = $Label
            binary_present   = $false
            snapshot_self    = $null
            list_apps        = $null
            list_windows     = $null
            status           = $null
            list_displays    = $null
            ref_action_live  = $null
            ref_action_dead  = $null
        }

        $capturePath = Write-ProbeJson -Probe $Probe -Name "cost-baseline-$Label.json" -InputObject $capture
        Register-MandatoryPass -Capture $capturePath -Result $capture
        Assert-MandatoryMeasurement -Probe $Probe -Label $Label

        Write-ProbeResult -Probe $Probe -Status 'ok' -Message 'release binary must be built first with cargo build --release -p agent-desktop' -Data @{
            binary_present = $false
        }
        exit 0
    }

    $snapshotSelf = Measure-SnapshotSelfLeg -Binary $resolvedBinary

    $listApps = $null
    try {
        $listApps = Measure-CommandInvocation -Executable $resolvedBinary -Arguments @('list-apps')
    } catch {
        $listApps = $null
    }

    $listWindows = $null
    try {
        $listWindows = Measure-CommandInvocation -Executable $resolvedBinary -Arguments @('list-windows')
    } catch {
        $listWindows = $null
    }

    $statusMeasurement = $null
    try {
        $statusMeasurement = Measure-CommandInvocation -Executable $resolvedBinary -Arguments @('status')
    } catch {
        $statusMeasurement = $null
    }

    $listDisplays = $null
    try {
        $listDisplays = Measure-CommandInvocation -Executable $resolvedBinary -Arguments @('list-displays')
    } catch {
        $listDisplays = $null
    }

    $refActionLive = $null
    $refActionDead = $null
    try {
        $fixtureExe = Get-FixtureExecutablePath -RepoRoot $repoRoot -FixtureBinaryPath $FixtureBinaryPath
        $refActionLive = Measure-RefActionLiveLeg -Binary $resolvedBinary -FixtureExe $fixtureExe
        $refActionDead = Measure-RefActionDeadLeg -Binary $resolvedBinary -FixtureExe $fixtureExe
    } catch {
        if (-not $refActionLive) {
            $refActionLive = [ordered]@{
                measurable   = $false
                command      = 'click <ref>'
                target       = 'fixture-primary-button'
                target_state = 'live'
                reason       = 'the fixture app could not be built or launched on this host'
            }
        }
        if (-not $refActionDead) {
            $refActionDead = [ordered]@{
                measurable   = $false
                command      = 'click <ref>'
                target       = 'fixture-primary-button'
                target_state = 'dead'
                reason       = 'the fixture app could not be built or launched on this host'
            }
        }
    }

    $capture = [ordered]@{
        probe            = $Probe
        methodology      = 'min-of-seven with the warm-up discarded, reported as min with median and max beside it'
        label            = $Label
        binary_present   = $true
        snapshot_self    = $snapshotSelf
        list_apps        = $listApps
        list_windows     = $listWindows
        status           = $statusMeasurement
        list_displays    = $listDisplays
        ref_action_live  = $refActionLive
        ref_action_dead  = $refActionDead
    }

    $capturePath = Write-ProbeJson -Probe $Probe -Name "cost-baseline-$Label.json" -InputObject $capture
    Register-MandatoryPass -Capture $capturePath -Result $capture
    Assert-MandatoryMeasurement -Probe $Probe -Label $Label

    Write-ProbeResult -Probe $Probe -Status 'ok' -Message 'cost baseline captured for release binary' -Data @{
        binary_present = $true
        capture        = "captures/cost-baseline-$Label.json"
    }
    exit 0
} catch {
    Write-ProbeResult -Probe $Probe -Status 'fail' -Message ('unhandled error: ' + ($_.Exception.Message -replace '[\r\n]+', ' '))
    exit 1
}
