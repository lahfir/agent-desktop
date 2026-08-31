#Requires -Version 5.1
<#
.SYNOPSIS
    Area 27 cost-baseline.ps1 - cost-baseline harness for the Windows performance vehicle.

.DESCRIPTION
    Measures read-only release binary commands (snapshot_self, list_apps,
    list_windows, status, list_displays) using the probe corpus cost methodology:
    seven timed runs, first discarded as warm-up, min/median/max reported over
    the remaining six.

    Run: powershell -NoProfile -ExecutionPolicy Bypass -File .\probes\windows\27-cost-baseline.ps1 -Label <devbox|ci>
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'ci')][string]$Label = 'devbox',
    [string]$BinaryPath = 'target\release\agent-desktop.exe'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

. (Join-Path (Split-Path -Parent $PSCommandPath) 'common.ps1')
Initialize-ProbeRedaction

$Probe = '27-cost-baseline'
Register-MandatoryCapture -Name @("cost-baseline-$Label.json")

function Get-HostProcessWindowHandle {
    try {
        $proc = [System.Diagnostics.Process]::GetCurrentProcess()
        if ($proc.MainWindowHandle -ne [IntPtr]::Zero) {
            return $proc.MainWindowHandle
        }
    } catch { }
    try {
        Initialize-ProbeNative
        $fgHwnd = [AgentDesktopProbe.Native]::GetForegroundWindow()
        if ($fgHwnd -ne [IntPtr]::Zero) {
            $fgPid = [AgentDesktopProbe.Native]::GetForegroundProcessId()
            if ($fgPid -eq $PID) {
                return $fgHwnd
            }
        }
    } catch { }
    return [IntPtr]::Zero
}

function Measure-CommandInvocation {
    param(
        [Parameter(Mandatory = $true)][string]$Executable,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [int]$Runs = 7
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
        if (-not $proc.WaitForExit(30000)) {
            try { $proc.Kill() } catch { }
            throw "process timed out after 30s: $Executable"
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
            probe          = $Probe
            methodology    = 'min-of-seven with the warm-up discarded, reported as min with median and max beside it'
            label          = $Label
            binary_present = $false
            snapshot_self  = $null
            list_apps      = $null
            list_windows   = $null
            status         = $null
            list_displays  = $null
        }

        $capturePath = Write-ProbeJson -Probe $Probe -Name "cost-baseline-$Label.json" -InputObject $capture
        Register-MandatoryPass -Capture $capturePath -Result $capture
        Assert-MandatoryMeasurement -Probe $Probe -Label $Label

        Write-ProbeResult -Probe $Probe -Status 'ok' -Message 'release binary must be built first with cargo build --release -p agent-desktop' -Data @{
            binary_present = $false
        }
        exit 0
    }

    $snapshotSelf = $null
    $hostHwnd = Get-HostProcessWindowHandle
    if ($hostHwnd -ne [IntPtr]::Zero -and [long]$hostHwnd -ne 0) {
        try {
            $snapshotSelf = Measure-CommandInvocation -Executable $resolvedBinary -Arguments @('snapshot', '--window-id', ('w-' + [long]$hostHwnd))
        } catch {
            $snapshotSelf = $null
        }
    }

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

    $capture = [ordered]@{
        probe          = $Probe
        methodology    = 'min-of-seven with the warm-up discarded, reported as min with median and max beside it'
        label          = $Label
        binary_present = $true
        snapshot_self  = $snapshotSelf
        list_apps      = $listApps
        list_windows   = $listWindows
        status         = $statusMeasurement
        list_displays  = $listDisplays
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
