#Requires -Version 5.1
<#
.SYNOPSIS
    Hot-path lifecycle cost baseline (A15-13 / A20-6 methodology).

.DESCRIPTION
    Measures launch-to-window, close-to-exit, and window-op round-trip with
    min-of-seven after a discarded warm-up. Writes lifecycle-cost-{Label}.json.
    Absolute milliseconds are environment-sensitive; CI asserts relative shape only.
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'ci')][string]$Label = 'devbox'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

. (Join-Path (Split-Path -Parent $PSCommandPath) '..\common.ps1')
. (Join-Path (Split-Path -Parent $PSCommandPath) 'native.ps1')
Initialize-ProbeRedaction

$script:ProbeDir = Split-Path -Parent $PSCommandPath
$script:CaptureDir = Join-Path $script:ProbeDir 'captures'
if (-not (Test-Path -LiteralPath $script:CaptureDir)) {
    New-Item -ItemType Directory -Path $script:CaptureDir -Force | Out-Null
}
$script:Spawned = New-Object System.Collections.ArrayList

function Write-LifecycleCapture {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Content
    )
    $redacted = Protect-ProbeText -Text $Content
    $path = Join-Path $script:CaptureDir $Name
    $utf8NoBom = New-Object System.Text.UTF8Encoding $false
    [IO.File]::WriteAllText($path, $redacted, $utf8NoBom)
    if (-not (Test-CaptureRedaction -Path $path)) {
        throw "redaction residue in $path"
    }
    return $path
}

function Register-SpawnedPid {
    param([int]$ProcessId)
    if ($ProcessId -gt 0) { [void]$script:Spawned.Add($ProcessId) }
}

function Stop-AllSpawned {
    foreach ($id in @($script:Spawned)) {
        try { Stop-ScratchProcess -ProcessId $id } catch { }
    }
    $script:Spawned.Clear()
}

function Wait-WindowForPid {
    param([int]$ProcessId, [int]$TimeoutMs = 12000)
    $deadline = [Diagnostics.Stopwatch]::StartNew()
    while ($deadline.ElapsedMilliseconds -lt $TimeoutMs) {
        $hwnd = [AgentDesktopProbe.A21.Lifecycle21]::FindVisibleWindowForPid($ProcessId)
        if ($hwnd -ne [IntPtr]::Zero) { return $hwnd }
        Start-Sleep -Milliseconds 50
    }
    return [IntPtr]::Zero
}

function Summarize-Samples {
    param([object[]]$Samples)
    $sorted = @($Samples | Sort-Object)
    $used = @($sorted | Select-Object -Skip 1)
    $medianIdx = [int][Math]::Floor($used.Count / 2)
    return [ordered]@{
        samples_ms       = @($Samples)
        min_ms           = ($used | Measure-Object -Minimum).Minimum
        median_ms        = ($used | Sort-Object)[$medianIdx]
        max_ms           = ($used | Measure-Object -Maximum).Maximum
        n                = $used.Count
        warmup_discarded = $true
    }
}

function Get-HelperPath {
    $root = Get-ProbeRoot
    $helper = Join-Path $root 'scratch\lifecycle-helpers\bin\LifecycleHelpers.exe'
    if (-not (Test-Path -LiteralPath $helper)) {
        $build = Join-Path $root 'scratch\lifecycle-helpers\build.ps1'
        & $build -Force | Out-Null
    }
    if (-not (Test-Path -LiteralPath $helper)) {
        throw "LifecycleHelpers.exe missing"
    }
    return $helper
}

$launchSamples = New-Object System.Collections.ArrayList
$closeSamples = New-Object System.Collections.ArrayList
$windowOpSamples = New-Object System.Collections.ArrayList

try {
    Initialize-ProbeNative
    Initialize-LifecycleNative
    $helper = Get-HelperPath
    $helperDir = Split-Path -Parent $helper

    for ($i = 0; $i -lt 8; $i++) {
        $swLaunch = [Diagnostics.Stopwatch]::StartNew()
        $launch = [AgentDesktopProbe.A21.Lifecycle21]::Launch(
            $helper,
            ('"' + $helper + '" --mode window'),
            $helperDir,
            [AgentDesktopProbe.A21.Lifecycle21]::CREATE_NEW_CONSOLE
        )
        if (-not $launch.Ok) { throw "launch failed win32=$($launch.LastError)" }
        Register-SpawnedPid -ProcessId $launch.ProcessId
        $hwnd = Wait-WindowForPid -ProcessId $launch.ProcessId
        $swLaunch.Stop()
        if ($hwnd -eq [IntPtr]::Zero) { throw 'window did not appear' }
        [void]$launchSamples.Add([Math]::Round($swLaunch.Elapsed.TotalMilliseconds, 4))

        $swClose = [Diagnostics.Stopwatch]::StartNew()
        [void][AgentDesktopProbe.A21.Lifecycle21]::PostClose($hwnd)
        $exit = [AgentDesktopProbe.A21.Lifecycle21]::ReadExit($launch.ProcessHandle, 8000)
        $swClose.Stop()
        if (-not $exit.WaitSignaled) { throw 'close did not signal exit' }
        [void]$closeSamples.Add([Math]::Round($swClose.Elapsed.TotalMilliseconds, 4))
        [AgentDesktopProbe.A21.Lifecycle21]::CloseLaunchHandles($launch)
        try { Stop-ScratchProcess -ProcessId $launch.ProcessId } catch { }
    }

    $opLaunch = [AgentDesktopProbe.A21.Lifecycle21]::Launch(
        $helper,
        ('"' + $helper + '" --mode window'),
        $helperDir,
        [AgentDesktopProbe.A21.Lifecycle21]::CREATE_NEW_CONSOLE
    )
    if (-not $opLaunch.Ok) { throw "window-op launch failed" }
    Register-SpawnedPid -ProcessId $opLaunch.ProcessId
    $opHwnd = Wait-WindowForPid -ProcessId $opLaunch.ProcessId
    if ($opHwnd -eq [IntPtr]::Zero) { throw 'window-op window missing' }

    for ($i = 0; $i -lt 8; $i++) {
        $sw = [Diagnostics.Stopwatch]::StartNew()
        $before = [AgentDesktopProbe.A21.Lifecycle21]::SnapPlacement($opHwnd)
        $cx = [Math]::Max(200, $before.Width + 8)
        $cy = [Math]::Max(160, $before.Height + 8)
        [void][AgentDesktopProbe.A21.Lifecycle21]::MoveResize($opHwnd, $before.Left, $before.Top, $cx, $cy)
        Start-Sleep -Milliseconds 80
        $after = [AgentDesktopProbe.A21.Lifecycle21]::SnapPlacement($opHwnd)
        $sw.Stop()
        if ($after.Width -le 0) { throw 'window-op placement unread' }
        [void]$windowOpSamples.Add([Math]::Round($sw.Elapsed.TotalMilliseconds, 4))
    }

    [void][AgentDesktopProbe.A21.Lifecycle21]::PostClose($opHwnd)
    [void][AgentDesktopProbe.A21.Lifecycle21]::ReadExit($opLaunch.ProcessHandle, 8000)
    [AgentDesktopProbe.A21.Lifecycle21]::CloseLaunchHandles($opLaunch)
    try { Stop-ScratchProcess -ProcessId $opLaunch.ProcessId } catch { }

    $cost = [ordered]@{
        probe                = '21-system-lifecycle'
        question             = 'hot-path launch-to-window, close-to-exit, window-op round-trip (min-of-seven, warm-up discarded per A15-13)'
        methodology_cites    = @('A15-13', 'A20-6')
        launch_to_window     = (Summarize-Samples -Samples @($launchSamples))
        close_to_exit        = (Summarize-Samples -Samples @($closeSamples))
        window_op_round_trip = (Summarize-Samples -Samples @($windowOpSamples))
        note                 = 'absolute ms are environment-sensitive; CI asserts min<=median<=max, n=7, warmup_discarded only'
    }
    $path = Write-LifecycleCapture -Name "lifecycle-cost-$Label.json" -Content (ConvertTo-Json -InputObject $cost -Depth 12)
    Write-Host "wrote $path"
} finally {
    Stop-AllSpawned
}
