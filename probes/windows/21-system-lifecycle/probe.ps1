#Requires -Version 5.1
<#
.SYNOPSIS
    Sub-phase 2.9 system-lifecycle gap probe (A21).

.DESCRIPTION
    Measures CreateProcessW launch identity, WM_CLOSE/TerminateProcess exit codes,
    IsHungAppWindow vs SendMessageTimeout, SetWindowPos/ShowWindow placement
    tolerance, uncontended SetForegroundWindow budget, cross-integrity focus
    manufacture, and windows-sys/ShellExecuteEx surface decisions.

    Captures under captures\ as lifecycle-*-{devbox,ci}.json. Corpus safety:
    scratch-only windows; shapes/counts/boolean branches; no titles/paths/pids
    in committed JSON (pid presence recorded as booleans only).
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
$script:OpenHandles = New-Object System.Collections.ArrayList

Register-MandatoryCapture -Name @(
    "lifecycle-launch-$Label.json",
    "lifecycle-close-$Label.json",
    "lifecycle-hang-$Label.json",
    "lifecycle-window-op-$Label.json",
    "lifecycle-activation-$Label.json",
    "lifecycle-cross-integrity-$Label.json",
    "lifecycle-manifest-$Label.json",
    "lifecycle-cost-$Label.json"
)

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
    param(
        [int]$ProcessId,
        [int]$TimeoutMs = 10000,
        [switch]$AllowHidden
    )
    $deadline = [Diagnostics.Stopwatch]::StartNew()
    while ($deadline.ElapsedMilliseconds -lt $TimeoutMs) {
        $hwnd = if ($AllowHidden) {
            [AgentDesktopProbe.A21.Lifecycle21]::FindAnyWindowForPid($ProcessId)
        } else {
            [AgentDesktopProbe.A21.Lifecycle21]::FindVisibleWindowForPid($ProcessId)
        }
        if ($hwnd -ne [IntPtr]::Zero) { return $hwnd }
        Start-Sleep -Milliseconds 100
    }
    return [IntPtr]::Zero
}

function Invoke-BuildHelpers {
    $buildHelpers = Join-Path (Get-ProbeRoot) 'scratch\lifecycle-helpers\build.ps1'
    $buildScratch = Join-Path (Get-ProbeRoot) 'scratch\build-scratch.ps1'
    & $buildHelpers -Force | Out-Null
    if (Test-Path -LiteralPath $buildScratch) {
        & $buildScratch -Force | Out-Null
    }
}

function Get-HelperPaths {
    $root = Get-ProbeRoot
    return [ordered]@{
        Helper     = (Join-Path $root 'scratch\lifecycle-helpers\bin\LifecycleHelpers.exe')
        ElevHelper = (Join-Path $root 'scratch\lifecycle-helpers\bin\LifecycleHelpers.elev.exe')
        Scratch    = (Join-Path $root 'scratch\bin\ScratchForms.exe')
        SurfaceDir = (Join-Path $root 'scratch\lifecycle-surface')
        ShellDir   = (Join-Path $root 'scratch\shell-execute-ex')
    }
}

$script:paths = @{}

try {
    Initialize-ProbeNative
    Initialize-LifecycleNative
    Invoke-BuildHelpers
    $paths = Get-HelperPaths
    foreach ($required in @($paths.Helper, $paths.ElevHelper, $paths.Scratch)) {
        if (-not (Test-Path -LiteralPath $required)) {
            throw ("required helper missing: " + (Split-Path -Leaf $required))
        }
    }

    # =========================================================================
    # Leg 1: Launch
    # =========================================================================
    $helperLeaf = 'LifecycleHelpers.exe'
    $baselineCount = [AgentDesktopProbe.A21.Lifecycle21]::CountRunningImage($helperLeaf)

    $launch1 = [AgentDesktopProbe.A21.Lifecycle21]::Launch(
        $paths.Helper,
        ('"' + $paths.Helper + '" --mode window'),
        (Split-Path -Parent $paths.Helper),
        [AgentDesktopProbe.A21.Lifecycle21]::CREATE_NEW_CONSOLE
    )
    Register-SpawnedPid -ProcessId $launch1.ProcessId
    $hwnd1 = Wait-WindowForPid -ProcessId $launch1.ProcessId -TimeoutMs 12000
    $windowAppeared = ($hwnd1 -ne [IntPtr]::Zero)

    $afterCount = [AgentDesktopProbe.A21.Lifecycle21]::CountRunningImage($helperLeaf)
    $attachDetected = ($afterCount -ge ($baselineCount + 1))

    # Second launch while first still running (attach_if_running detection shape)
    $runningMatchCount = [AgentDesktopProbe.A21.Lifecycle21]::CountRunningImage($helperLeaf)
    $secondWouldAttach = ($runningMatchCount -eq 1)
    $ambiguousIfAttach = ($runningMatchCount -ge 2)

    # Child-window spawner: parent pid from CreateProcessW; window owned by child
    $childLaunch = [AgentDesktopProbe.A21.Lifecycle21]::Launch(
        $paths.Helper,
        ('"' + $paths.Helper + '" --mode child-spawner --child "' + $paths.Scratch + '" --child-args "--tag a21child --pos 200,200" --sleep-ms 20000'),
        (Split-Path -Parent $paths.Helper),
        [AgentDesktopProbe.A21.Lifecycle21]::CREATE_NEW_CONSOLE
    )
    Register-SpawnedPid -ProcessId $childLaunch.ProcessId
    Start-Sleep -Milliseconds 1500
    $childPid = [AgentDesktopProbe.A21.Lifecycle21]::FindChildProcessId($childLaunch.ProcessId)
    if ($childPid -gt 0) { Register-SpawnedPid -ProcessId $childPid }
    $childHwnd = [IntPtr]::Zero
    if ($childPid -gt 0) { $childHwnd = Wait-WindowForPid -ProcessId $childPid -TimeoutMs 8000 }
    $parentHasWindow = ([AgentDesktopProbe.A21.Lifecycle21]::FindVisibleWindowForPid($childLaunch.ProcessId) -ne [IntPtr]::Zero)
    $childWindowAtDifferentPid = (
        ($childPid -gt 0) -and
        ($childHwnd -ne [IntPtr]::Zero) -and
        ($childPid -ne $childLaunch.ProcessId) -and
        (-not $parentHasWindow)
    )

    # Elevation-required manufacture
    $elevation = [ordered]@{
        measurable = $false
        branch     = 'elevation_required_not_observed'
    }
    $elevLaunch = [AgentDesktopProbe.A21.Lifecycle21]::Launch(
        $paths.ElevHelper,
        ('"' + $paths.ElevHelper + '" --mode clean-exit'),
        (Split-Path -Parent $paths.ElevHelper),
        0
    )
    if (-not $elevLaunch.Ok -and $elevLaunch.LastError -eq [AgentDesktopProbe.A21.Lifecycle21]::ERROR_ELEVATION_REQUIRED) {
        $elevation.measurable = $true
        $elevation.branch = 'error_elevation_required_740'
        $elevation.win32_error = 740
        $elevation.hresult_from_win32 = ('0x{0:X8}' -f [AgentDesktopProbe.A21.Lifecycle21]::HresultFromWin32(740))
    } else {
        if ($elevLaunch.Ok) {
            Register-SpawnedPid -ProcessId $elevLaunch.ProcessId
            [void][AgentDesktopProbe.A21.Lifecycle21]::ReadExit($elevLaunch.ProcessHandle, 5000)
            [AgentDesktopProbe.A21.Lifecycle21]::CloseLaunchHandles($elevLaunch)
            $elevation.branch = 'host_already_elevated_createprocess_succeeds'
            $elevation.note = 'ERROR_ELEVATION_REQUIRED 740 not observed from High-IL host; Medium manufacture tried next'
        } else {
            $elevation.branch = 'elev_launch_failed_other'
            $elevation.win32_error = $elevLaunch.LastError
        }
        try {
            $mediumCopy = Join-Path $env:TEMP ('a21-elev-' + [guid]::NewGuid().ToString('N').Substring(0, 8) + '.exe')
            Copy-Item -LiteralPath $paths.ElevHelper -Destination $mediumCopy -Force
            # Medium process launching requireAdministrator should yield 740; we can only
            # stage Medium when Start-MediumIntegrityProcess works. Capture manufacture gate.
            $medium = Start-MediumIntegrityProcess -FilePath $paths.Helper -ArgumentList @('--mode', 'clean-exit')
            Register-SpawnedPid -ProcessId $medium.ProcessId
            $elevation.medium_manufacture_available = $true
            $elevation.branch = 'medium_available_but_nested_elev_launch_not_instrumented'
            $elevation.measurable = $false
            $elevation.cite = @('A18-4', 'A19-4', 'A20-2')
        } catch {
            $elevation.medium_manufacture_available = $false
            $elevation.measurable = $false
            $elevation.branch = 'unmeasurable_elevation_manufacture_unavailable'
            $elevation.cite = @('A18-4', 'A19-4', 'A20-2')
            $elevation.attempt_error_kind = 'privilege_or_token_gate'
        }
    }

    $launchCapture = [ordered]@{
        probe    = '21-system-lifecycle'
        question = 'CreateProcessW identity, attach_if_running detection, child-window pid split, elevation-required'
        create_process_ok            = [bool]$launch1.Ok
        process_id_nonzero           = ($launch1.ProcessId -gt 0)
        process_handle_nonzero       = [bool]$launch1.ProcessHandleNonZero
        thread_handle_nonzero        = [bool]$launch1.ThreadHandleNonZero
        window_appeared_for_pid      = $windowAppeared
        attach_if_running_detection  = [ordered]@{
            image_count_after_first = $runningMatchCount
            single_match_attachable = $secondWouldAttach
            two_plus_is_ambiguous   = $ambiguousIfAttach
            toolhelp_detects_running = $attachDetected
        }
        child_window_spawner = [ordered]@{
            parent_launch_ok              = [bool]$childLaunch.Ok
            child_pid_nonzero             = ($childPid -gt 0)
            child_window_appeared         = ($childHwnd -ne [IntPtr]::Zero)
            parent_has_visible_window     = $parentHasWindow
            window_at_different_pid       = $childWindowAtDifferentPid
            branch = $(if ($childWindowAtDifferentPid) { 'launcher_style_child_pid_window' } else { 'child_window_not_staged' })
        }
        elevation_required = $elevation
    }
    $script:paths.launch = Write-LifecycleCapture -Name "lifecycle-launch-$Label.json" -Content (ConvertTo-Json -InputObject $launchCapture -Depth 12)
    Register-MandatoryPass -Capture $script:paths.launch -Result $launchCapture
    Write-Host "wrote $($script:paths.launch)"

    # Keep first window for later legs; tear down child spawner now
    if ($childPid -gt 0) { try { Stop-ScratchProcess -ProcessId $childPid } catch { } }
    if ($childLaunch.ProcessId -gt 0) {
        try { Stop-ScratchProcess -ProcessId $childLaunch.ProcessId } catch { }
        [AgentDesktopProbe.A21.Lifecycle21]::CloseLaunchHandles($childLaunch)
    }

    # =========================================================================
    # Leg 2: Close / exit-code boundary
    # =========================================================================
    # Clean exit
    $clean = [AgentDesktopProbe.A21.Lifecycle21]::Launch(
        $paths.Helper,
        ('"' + $paths.Helper + '" --mode clean-exit'),
        (Split-Path -Parent $paths.Helper),
        [AgentDesktopProbe.A21.Lifecycle21]::CREATE_NO_WINDOW
    )
    Register-SpawnedPid -ProcessId $clean.ProcessId
    $cleanRead = [AgentDesktopProbe.A21.Lifecycle21]::ReadExit($clean.ProcessHandle, 8000)
    [AgentDesktopProbe.A21.Lifecycle21]::CloseLaunchHandles($clean)

    # Crash-shaped exit (NTSTATUS high nibble 0xC)
    $crash = [AgentDesktopProbe.A21.Lifecycle21]::Launch(
        $paths.Helper,
        ('"' + $paths.Helper + '" --mode crash-exit'),
        (Split-Path -Parent $paths.Helper),
        [AgentDesktopProbe.A21.Lifecycle21]::CREATE_NO_WINDOW
    )
    Register-SpawnedPid -ProcessId $crash.ProcessId
    $crashRead = [AgentDesktopProbe.A21.Lifecycle21]::ReadExit($crash.ProcessHandle, 8000)
    [AgentDesktopProbe.A21.Lifecycle21]::CloseLaunchHandles($crash)

    # WM_CLOSE graceful
    $grace = [AgentDesktopProbe.A21.Lifecycle21]::Launch(
        $paths.Helper,
        ('"' + $paths.Helper + '" --mode window'),
        (Split-Path -Parent $paths.Helper),
        [AgentDesktopProbe.A21.Lifecycle21]::CREATE_NEW_CONSOLE
    )
    Register-SpawnedPid -ProcessId $grace.ProcessId
    $graceHwnd = Wait-WindowForPid -ProcessId $grace.ProcessId -TimeoutMs 10000
    $wmClosePosted = $false
    $wmCloseSignaled = $false
    if ($graceHwnd -ne [IntPtr]::Zero) {
        $wmClosePosted = [AgentDesktopProbe.A21.Lifecycle21]::PostClose($graceHwnd)
        $graceRead = [AgentDesktopProbe.A21.Lifecycle21]::ReadExit($grace.ProcessHandle, 8000)
        $wmCloseSignaled = $graceRead.WaitSignaled
    }
    if (-not $wmCloseSignaled -and $grace.ProcessId -gt 0) {
        try { Stop-ScratchProcess -ProcessId $grace.ProcessId } catch { }
    }
    [AgentDesktopProbe.A21.Lifecycle21]::CloseLaunchHandles($grace)

    # TerminateProcess force
    $force = [AgentDesktopProbe.A21.Lifecycle21]::Launch(
        $paths.Helper,
        ('"' + $paths.Helper + '" --mode wait --sleep-ms 30000'),
        (Split-Path -Parent $paths.Helper),
        [AgentDesktopProbe.A21.Lifecycle21]::CREATE_NO_WINDOW
    )
    Register-SpawnedPid -ProcessId $force.ProcessId
    Start-Sleep -Milliseconds 200
    $termOk = [AgentDesktopProbe.A21.Lifecycle21]::TerminateProcess($force.ProcessHandle, 1)
    $forceRead = [AgentDesktopProbe.A21.Lifecycle21]::ReadExit($force.ProcessHandle, 8000)
    [AgentDesktopProbe.A21.Lifecycle21]::CloseLaunchHandles($force)

    $stillActiveGuard = [ordered]@{
        still_active_constant = 259
        note = 'WaitForSingleObject must gate GetExitCodeProcess; a live process can also read 259'
    }

    $closeCapture = [ordered]@{
        probe    = '21-system-lifecycle'
        question = 'WM_CLOSE vs TerminateProcess; WaitForSingleObject+GetExitCodeProcess; Exited vs Crashed boundary'
        clean_exit = [ordered]@{
            wait_signaled     = [bool]$cleanRead.WaitSignaled
            exit_code_is_zero = ($cleanRead.ExitCode -eq 0)
            high_nibble_is_c  = [bool]$cleanRead.HighNibbleIsC
            classification    = $cleanRead.Classification
        }
        crash_shaped_exit = [ordered]@{
            wait_signaled            = [bool]$crashRead.WaitSignaled
            exit_code_hex            = ('0x{0:X8}' -f $crashRead.ExitCode)
            high_nibble_is_c         = [bool]$crashRead.HighNibbleIsC
            classification           = $crashRead.Classification
            crashed_boundary_rule    = 'high_nibble_0xC'
            example_status           = '0xC0000005'
        }
        wm_close = [ordered]@{
            posted   = $wmClosePosted
            signaled = $wmCloseSignaled
            branch   = $(if ($wmCloseSignaled) { 'wm_close_verified_exit' } else { 'wm_close_did_not_exit_in_budget' })
        }
        terminate_process = [ordered]@{
            api_ok        = [bool]$termOk
            wait_signaled = [bool]$forceRead.WaitSignaled
            exit_code     = [int]$forceRead.ExitCode
            classification = $forceRead.Classification
        }
        still_active_guard = $stillActiveGuard
    }
    $script:paths.close = Write-LifecycleCapture -Name "lifecycle-close-$Label.json" -Content (ConvertTo-Json -InputObject $closeCapture -Depth 12)
    Register-MandatoryPass -Capture $script:paths.close -Result $closeCapture
    Write-Host "wrote $($script:paths.close)"

    # =========================================================================
    # Leg 3: Hang — IsHungAppWindow vs SendMessageTimeout
    # =========================================================================
    $stalled = [AgentDesktopProbe.A21.Lifecycle21]::Launch(
        $paths.Helper,
        ('"' + $paths.Helper + '" --mode stalled --sleep-ms 25000'),
        (Split-Path -Parent $paths.Helper),
        [AgentDesktopProbe.A21.Lifecycle21]::CREATE_NEW_CONSOLE
    )
    Register-SpawnedPid -ProcessId $stalled.ProcessId
    # Non-pumping windows may not report IsWindowVisible; accept any top-level HWND.
    $stalledHwnd = Wait-WindowForPid -ProcessId $stalled.ProcessId -TimeoutMs 8000 -AllowHidden
    # Give the non-pumping window time to become "hung" for IsHungAppWindow (~5s heuristic)
    Start-Sleep -Milliseconds 5500

    $hangSamples = New-Object System.Collections.ArrayList
    $agreeCount = 0
    if ($stalledHwnd -ne [IntPtr]::Zero) {
        for ($i = 0; $i -lt 3; $i++) {
            $sample = [AgentDesktopProbe.A21.Lifecycle21]::ProbeHang($stalledHwnd, 500)
            [void]$hangSamples.Add([ordered]@{
                is_hung_app_window     = [bool]$sample.IsHung
                is_hung_elapsed_ms     = [int]$sample.IsHungElapsedMs
                sendmessage_timed_out  = [bool]$sample.TimeoutTimedOut
                sendmessage_elapsed_ms = [int]$sample.TimeoutElapsedMs
                agree_hung             = [bool]$sample.AgreeHung
            })
            if ($sample.AgreeHung) { $agreeCount++ }
            Start-Sleep -Milliseconds 200
        }
    }

    $control = [AgentDesktopProbe.A21.Lifecycle21]::Launch(
        $paths.Helper,
        ('"' + $paths.Helper + '" --mode window'),
        (Split-Path -Parent $paths.Helper),
        [AgentDesktopProbe.A21.Lifecycle21]::CREATE_NEW_CONSOLE
    )
    Register-SpawnedPid -ProcessId $control.ProcessId
    $controlHwnd = Wait-WindowForPid -ProcessId $control.ProcessId -TimeoutMs 8000
    $controlProbe = $null
    if ($controlHwnd -ne [IntPtr]::Zero) {
        $cp = [AgentDesktopProbe.A21.Lifecycle21]::ProbeHang($controlHwnd, 500)
        $controlProbe = [ordered]@{
            is_hung_app_window     = [bool]$cp.IsHung
            sendmessage_timed_out  = [bool]$cp.TimeoutTimedOut
            agree_responsive       = (-not $cp.IsHung -and -not $cp.TimeoutTimedOut)
            sendmessage_elapsed_ms = [int]$cp.TimeoutElapsedMs
            is_hung_elapsed_ms     = [int]$cp.IsHungElapsedMs
        }
    }

    $hangAgree = ($hangSamples.Count -gt 0 -and $agreeCount -eq $hangSamples.Count)
    $ishungCheaper = $false
    if ($hangSamples.Count -gt 0) {
        $medianHung = ($hangSamples | ForEach-Object { $_.is_hung_elapsed_ms } | Sort-Object)[[int][Math]::Floor(($hangSamples.Count - 1) / 2)]
        $medianTimeout = ($hangSamples | ForEach-Object { $_.sendmessage_elapsed_ms } | Sort-Object)[[int][Math]::Floor(($hangSamples.Count - 1) / 2)]
        $ishungCheaper = ($medianHung -lt $medianTimeout)
    }

    $hangCapture = [ordered]@{
        probe    = '21-system-lifecycle'
        question = 'IsHungAppWindow vs SendMessageTimeout(WM_NULL, SMTO_ABORTIFHUNG) agreement and latency'
        foundation_cites = @('A14-11')
        stalled_window_ready = ($stalledHwnd -ne [IntPtr]::Zero)
        samples = @($hangSamples)
        agreement_rate = $(if ($hangSamples.Count -gt 0) { [math]::Round($agreeCount / $hangSamples.Count, 3) } else { 0 })
        all_samples_agree = $hangAgree
        ishung_cheaper_precheck = $ishungCheaper
        median_is_hung_ms = $(if ($hangSamples.Count -gt 0) { $medianHung } else { $null })
        median_sendmessage_timeout_ms = $(if ($hangSamples.Count -gt 0) { $medianTimeout } else { $null })
        pumping_control = $controlProbe
        ktd3_conclusion = $(if ($hangAgree) {
            'reuse_SendMessageTimeout_as_authoritative_IsHungAppWindow_cheap_precheck'
        } else {
            'disagreement_recorded_prefer_SendMessageTimeout_authoritative'
        })
        branch = $(if ($stalledHwnd -eq [IntPtr]::Zero) { 'stalled_window_missing' } else { 'stalled_non_pumping_measured' })
    }
    $script:paths.hang = Write-LifecycleCapture -Name "lifecycle-hang-$Label.json" -Content (ConvertTo-Json -InputObject $hangCapture -Depth 12)
    Register-MandatoryPass -Capture $script:paths.hang -Result $hangCapture
    Write-Host "wrote $($script:paths.hang)"

    try { Stop-ScratchProcess -ProcessId $stalled.ProcessId } catch { }
    [AgentDesktopProbe.A21.Lifecycle21]::CloseLaunchHandles($stalled)

    # =========================================================================
    # Leg 4: Window ops + placement tolerance
    # =========================================================================
    $winHwnd = $controlHwnd
    if ($winHwnd -eq [IntPtr]::Zero) {
        $winLaunch = [AgentDesktopProbe.A21.Lifecycle21]::Launch(
            $paths.Helper,
            ('"' + $paths.Helper + '" --mode window'),
            (Split-Path -Parent $paths.Helper),
            [AgentDesktopProbe.A21.Lifecycle21]::CREATE_NEW_CONSOLE
        )
        Register-SpawnedPid -ProcessId $winLaunch.ProcessId
        $winHwnd = Wait-WindowForPid -ProcessId $winLaunch.ProcessId -TimeoutMs 10000
        $control = $winLaunch
    }

    $immediateDeltas = New-Object System.Collections.ArrayList
    $waitedDeltas = New-Object System.Collections.ArrayList
    $targets = @(
        @{ X = 160; Y = 160; W = 440; H = 300 },
        @{ X = 180; Y = 170; W = 460; H = 320 },
        @{ X = 200; Y = 190; W = 480; H = 340 }
    )
    foreach ($t in $targets) {
        [void][AgentDesktopProbe.A21.Lifecycle21]::MoveResize($winHwnd, $t.X, $t.Y, $t.W, $t.H)
        $imm = [AgentDesktopProbe.A21.Lifecycle21]::SnapPlacement($winHwnd)
        $dImm = [Math]::Max(
            [Math]::Abs($imm.Left - $t.X),
            [Math]::Max(
                [Math]::Abs($imm.Top - $t.Y),
                [Math]::Max([Math]::Abs($imm.Width - $t.W), [Math]::Abs($imm.Height - $t.H))
            )
        )
        [void]$immediateDeltas.Add($dImm)
        Start-Sleep -Milliseconds 80
        $waited = [AgentDesktopProbe.A21.Lifecycle21]::SnapPlacement($winHwnd)
        $dWait = [Math]::Max(
            [Math]::Abs($waited.Left - $t.X),
            [Math]::Max(
                [Math]::Abs($waited.Top - $t.Y),
                [Math]::Max([Math]::Abs($waited.Width - $t.W), [Math]::Abs($waited.Height - $t.H))
            )
        )
        [void]$waitedDeltas.Add($dWait)
    }

    [void][AgentDesktopProbe.A21.Lifecycle21]::ShowWindow($winHwnd, [AgentDesktopProbe.A21.Lifecycle21]::SW_MINIMIZE)
    Start-Sleep -Milliseconds 80
    $minSnap = [AgentDesktopProbe.A21.Lifecycle21]::SnapPlacement($winHwnd)
    [void][AgentDesktopProbe.A21.Lifecycle21]::ShowWindow($winHwnd, [AgentDesktopProbe.A21.Lifecycle21]::SW_MAXIMIZE)
    Start-Sleep -Milliseconds 80
    $maxSnap = [AgentDesktopProbe.A21.Lifecycle21]::SnapPlacement($winHwnd)
    [void][AgentDesktopProbe.A21.Lifecycle21]::ShowWindow($winHwnd, [AgentDesktopProbe.A21.Lifecycle21]::SW_RESTORE)
    Start-Sleep -Milliseconds 80
    $restSnap = [AgentDesktopProbe.A21.Lifecycle21]::SnapPlacement($winHwnd)

    $maxImm = ($immediateDeltas | Measure-Object -Maximum).Maximum
    $maxWait = ($waitedDeltas | Measure-Object -Maximum).Maximum
    # Product tolerance: measured max waited delta, floored at 8 px for DWM animation headroom
    # (plan default pending measurement; this host measured 0 on immediate and 80 ms re-read).
    $recommended = [Math]::Max(8, [int]([Math]::Ceiling($maxWait) + 2))

    $windowCapture = [ordered]@{
        probe    = '21-system-lifecycle'
        question = 'SetWindowPos/ShowWindow round-trip; -32000 sentinel; re-read delta tolerance'
        foundation_cites = @('A1-2', 'A5-3', 'A14-8')
        move_resize = [ordered]@{
            trials                 = $targets.Count
            immediate_deltas_px    = @($immediateDeltas)
            waited_80ms_deltas_px  = @($waitedDeltas)
            max_immediate_delta_px = [int]$maxImm
            max_waited_delta_px    = [int]$maxWait
            recommended_tolerance_px = [int]$recommended
            wait_then_reread_ms    = 80
        }
        minimize = [ordered]@{
            show_cmd             = [int]$minSnap.ShowCmd
            minimized_sentinel   = [bool]$minSnap.MinimizedSentinel
            show_cmd_is_minimized = ($minSnap.ShowCmd -eq 2)
        }
        maximize = [ordered]@{
            show_cmd              = [int]$maxSnap.ShowCmd
            show_cmd_is_maximized = ($maxSnap.ShowCmd -eq 3)
        }
        restore = [ordered]@{
            show_cmd            = [int]$restSnap.ShowCmd
            show_cmd_is_normal  = ($restSnap.ShowCmd -eq 1)
            sentinel_cleared    = (-not $restSnap.MinimizedSentinel)
        }
        branch = 'placement_round_trip_measured'
    }
    $script:paths.window = Write-LifecycleCapture -Name "lifecycle-window-op-$Label.json" -Content (ConvertTo-Json -InputObject $windowCapture -Depth 12)
    Register-MandatoryPass -Capture $script:paths.window -Result $windowCapture
    Write-Host "wrote $($script:paths.window)"

    # =========================================================================
    # Leg 5: Activation budget (uncontended)
    # =========================================================================
    # Contender window so the target is not already foreground before ActivateOnce.
    $contender = [AgentDesktopProbe.A21.Lifecycle21]::Launch(
        $paths.Helper,
        ('"' + $paths.Helper + '" --mode window'),
        (Split-Path -Parent $paths.Helper),
        [AgentDesktopProbe.A21.Lifecycle21]::CREATE_NEW_CONSOLE
    )
    Register-SpawnedPid -ProcessId $contender.ProcessId
    $contenderHwnd = Wait-WindowForPid -ProcessId $contender.ProcessId -TimeoutMs 8000

    $activationTrials = New-Object System.Collections.ArrayList
    $firstAttemptSuccesses = 0
    $trialN = 5
    $winOwnerPid = 0
    if ($null -ne $control -and $control.ProcessId -gt 0) { $winOwnerPid = [int]$control.ProcessId }
    for ($i = 0; $i -lt $trialN; $i++) {
        if ($contenderHwnd -ne [IntPtr]::Zero) {
            [void][AgentDesktopProbe.A21.Lifecycle21]::ActivateOnce($contenderHwnd)
            Start-Sleep -Milliseconds 80
        }
        [void][AgentDesktopProbe.A21.Lifecycle21]::ShowWindow($winHwnd, [AgentDesktopProbe.A21.Lifecycle21]::SW_SHOWNOACTIVATE)
        Start-Sleep -Milliseconds 80
        $beforeOwned = [AgentDesktopProbe.A21.Lifecycle21]::ForegroundOwned($winOwnerPid)
        $apiOk = [AgentDesktopProbe.A21.Lifecycle21]::ActivateOnce($winHwnd)
        Start-Sleep -Milliseconds 50
        $after1 = [AgentDesktopProbe.A21.Lifecycle21]::ForegroundOwned($winOwnerPid)
        $secondNeeded = $false
        $after2 = $after1
        if (-not $after1) {
            $secondNeeded = $true
            [void][AgentDesktopProbe.A21.Lifecycle21]::ActivateOnce($winHwnd)
            Start-Sleep -Milliseconds 50
            $after2 = [AgentDesktopProbe.A21.Lifecycle21]::ForegroundOwned($winOwnerPid)
        }
        if ($after1) { $firstAttemptSuccesses++ }
        [void]$activationTrials.Add([ordered]@{
            api_returned_true     = [bool]$apiOk
            owned_after_first     = [bool]$after1
            second_attempt_needed = $secondNeeded
            owned_after_second    = [bool]$after2
            was_owned_before      = [bool]$beforeOwned
        })
    }
    $firstRate = [math]::Round($firstAttemptSuccesses / $trialN, 3)

    $activationCapture = [ordered]@{
        probe    = '21-system-lifecycle'
        question = 'SetForegroundWindow uncontended success; whether bounded retry budget is needed'
        contention_staged = $false
        contention_note   = 'repo-controlled scratch cannot stage foreground contention; contended re-measure owned by section 2.12 split-integrity runner'
        trials = @($activationTrials)
        first_attempt_success_rate = $firstRate
        first_attempt_always_lands = ($firstAttemptSuccesses -eq $trialN)
        recommended_retry_budget   = 2
        branch = $(if ($firstAttemptSuccesses -eq $trialN) {
            'uncontended_first_attempt_always_lands'
        } else {
            'uncontended_second_attempt_sometimes_needed'
        })
    }
    $script:paths.activation = Write-LifecycleCapture -Name "lifecycle-activation-$Label.json" -Content (ConvertTo-Json -InputObject $activationCapture -Depth 12)
    Register-MandatoryPass -Capture $script:paths.activation -Result $activationCapture
    Write-Host "wrote $($script:paths.activation)"

    # =========================================================================
    # Leg 6: Cross-integrity focus
    # =========================================================================
    $cross = [ordered]@{
        probe    = '21-system-lifecycle'
        question = 'cross-integrity SetForegroundWindow effect via A9-1 token-lowering'
        foundation_cites = @('A9-1', 'A18-4', 'A19-4', 'A20-2')
        measurable = $false
        branch     = 'unmeasurable_elevation_manufacture_unavailable'
    }
    try {
        $mediumExe = Join-Path $env:TEMP ('a21-med-' + [guid]::NewGuid().ToString('N').Substring(0, 8) + '.exe')
        Copy-Item -LiteralPath $paths.Helper -Destination $mediumExe -Force
        $medium = Start-MediumIntegrityProcess -FilePath $mediumExe -ArgumentList @('--mode', 'window')
        Register-SpawnedPid -ProcessId $medium.ProcessId
        $medHwnd = Wait-WindowForPid -ProcessId $medium.ProcessId -TimeoutMs 10000
        if ($medHwnd -eq [IntPtr]::Zero) {
            $cross.branch = 'medium_launched_but_window_missing'
            $cross.manufacture_available = $true
        } else {
            # High probe activates Medium window (same-user High→Medium usually succeeds);
            # the interesting UIPI refusal is Medium→High, which needs a Medium client we do not run here.
            $cross.manufacture_available = $true
            $cross.medium_integrity_sid = $medium.IntegritySid
            $cross.measurable = $false
            $cross.branch = 'medium_manufactured_cross_direction_effect_not_instrumented'
            $cross.note = 'A9-1 manufacture path available; Medium-to-High focus effect remains with section 2.12 (same gate as A18-4/A19-4/A20-2)'
            $cross.cite = @('A18-4', 'A19-4', 'A20-2')
        }
    } catch {
        $cross.manufacture_available = $false
        $cross.measurable = $false
        $cross.branch = 'unmeasurable_elevation_manufacture_unavailable'
        $cross.cite = @('A18-4', 'A19-4', 'A20-2')
        $cross.attempt_error_kind = 'privilege_or_token_gate'
    }
    $script:paths.cross = Write-LifecycleCapture -Name "lifecycle-cross-integrity-$Label.json" -Content (ConvertTo-Json -InputObject $cross -Depth 12)
    Register-MandatoryPass -Capture $script:paths.cross -Result $cross
    Write-Host "wrote $($script:paths.cross)"

    # =========================================================================
    # Leg 7: Manifest / scan / ShellExecuteEx
    # =========================================================================
    function Invoke-CargoLines {
        param([string[]]$CargoArgs)
        $prev = $ErrorActionPreference
        $ErrorActionPreference = 'Continue'
        $lines = @(& cargo @CargoArgs 2>&1 | ForEach-Object { "$_" })
        $code = $LASTEXITCODE
        $ErrorActionPreference = $prev
        return [pscustomobject]@{ Lines = $lines; ExitCode = $code }
    }

    $surfaceOk = $false
    $surfaceJson = $null
    $surfaceError = $null
    $createProcessNeedsSecurity = $true
    Push-Location $paths.SurfaceDir
    try {
        $build = Invoke-CargoLines -CargoArgs @('build', '--offline')
        if ($build.ExitCode -ne 0) {
            $build = Invoke-CargoLines -CargoArgs @('build')
        }
        if ($build.ExitCode -ne 0) {
            throw ("lifecycle-surface build failed: " + (($build.Lines | Select-Object -Last 12) -join ' | '))
        }
        $run = Invoke-CargoLines -CargoArgs @('run', '--quiet')
        if ($run.ExitCode -ne 0) {
            throw ("lifecycle-surface run failed: " + (($run.Lines | Select-Object -Last 12) -join ' | '))
        }
        $surfaceJson = ($run.Lines | Where-Object { $_ -match 'surface_compiles' } | Select-Object -Last 1)
        if ([string]::IsNullOrEmpty($surfaceJson)) { $surfaceJson = ($run.Lines | Select-Object -Last 1) }
        $parsed = $surfaceJson | ConvertFrom-Json
        $surfaceOk = [bool]$parsed.surface_compiles
    } catch {
        $surfaceError = ($_.Exception.Message -replace '[\r\n]+', ' ')
    } finally {
        Pop-Location
    }

    $shellOk = $false
    $shellJson = $null
    $shellError = $null
    $shellMeasurable = $true
    $shellNeedsRegistry = $true
    Push-Location $paths.ShellDir
    try {
        $build = Invoke-CargoLines -CargoArgs @('build', '--offline')
        if ($build.ExitCode -ne 0) {
            $build = Invoke-CargoLines -CargoArgs @('build')
        }
        if ($build.ExitCode -ne 0) {
            $shellMeasurable = $false
            throw ("shell-execute-ex build failed: " + (($build.Lines | Select-Object -Last 12) -join ' | '))
        }
        $comspec = $env:COMSPEC
        if (-not $comspec) { $comspec = (Join-Path $env:SystemRoot 'System32\cmd.exe') }
        $run = Invoke-CargoLines -CargoArgs @('run', '--quiet', '--', $comspec)
        if ($run.ExitCode -ne 0) {
            throw ("shell-execute-ex run failed: " + (($run.Lines | Select-Object -Last 12) -join ' | '))
        }
        $shellJson = ($run.Lines | Where-Object { $_ -match 'binding_ok' } | Select-Object -Last 1)
        if ([string]::IsNullOrEmpty($shellJson)) { $shellJson = ($run.Lines | Select-Object -Last 1) }
        $parsedShell = $shellJson | ConvertFrom-Json
        $shellOk = [bool]$parsedShell.binding_ok -and [bool]$parsedShell.launch_ok
    } catch {
        $shellError = ($_.Exception.Message -replace '[\r\n]+', ' ')
        if ($shellError -match 'offline|network|index') {
            $shellMeasurable = $false
        }
    } finally {
        Pop-Location
    }

    # Win32 error classification decision: sample known codes through HRESULT_FROM_WIN32
    $win32Samples = @(
        @{ name = 'ERROR_FILE_NOT_FOUND'; code = 2 },
        @{ name = 'ERROR_ACCESS_DENIED'; code = 5 },
        @{ name = 'ERROR_ELEVATION_REQUIRED'; code = 740 },
        @{ name = 'ERROR_BAD_EXE_FORMAT'; code = 193 }
    )
    $mapped = @()
    foreach ($s in $win32Samples) {
        $hr = [AgentDesktopProbe.A21.Lifecycle21]::HresultFromWin32([uint32]$s.code)
        $mapped += [ordered]@{
            name = $s.name
            win32 = [int]$s.code
            hresult = ('0x{0:X8}' -f $hr)
            unique_facility_win32 = ((([uint32]$hr -shr 16) -band 0x1FFF) -eq 7)
        }
    }
    # E_ACCESSDENIED already in hresult.rs is 0x80070005 = HRESULT_FROM_WIN32(5)
    $win32Decision = [ordered]@{
        decision = 'HRESULT_FROM_WIN32_into_existing_one_record_per_code_table'
        rationale = 'GetLastError codes wrap to FACILITY_WIN32 (0x8007xxxx); E_ACCESSDENIED/E_INVALIDARG already live in that shape in hresult.rs; keep one record per code, never a parallel ad-hoc match'
        samples = $mapped
        parallel_table_rejected = $true
    }

    # Banned-call scan governance: lifecycle APIs are Win32, not UIA get_pattern family
    $scanDecision = [ordered]@{
        existing_hit_test_scan_covers_lifecycle = $false
        reason = 'hit_test_scan_tests and write-path bans target UIA get_pattern/get_children/UIAutomation::new; CreateProcessW/TerminateProcess/SetWindowPos/SetForegroundWindow are Win32 and not in those needles'
        new_lifecycle_files_need_registration = $true
        needle_style = 'concat_split_required'
        note = 'when U2-U7 add system/launch.rs close.rs process_state.rs window_op.rs key_dispatch.rs, extend any new lifecycle-specific scan with concat!-split needles; do not assume hit_test_scan_tests auto-covers them'
    }

    $manifestCapture = [ordered]@{
        probe    = '21-system-lifecycle'
        question = 'windows-sys surface compile; Win32 error table; banned-call scan; ShellExecuteExW standalone'
        surface_compile = [ordered]@{
            ok = $surfaceOk
            feature_set = 'crates_windows_current'
            create_process_w_requires_win32_security = $createProcessNeedsSecurity
            error = $surfaceError
            stdout_shape_present = (-not [string]::IsNullOrEmpty($surfaceJson))
        }
        win32_error_classification = $win32Decision
        banned_call_scan = $scanDecision
        shell_execute_ex = [ordered]@{
            measurable = $shellMeasurable
            binding_and_launch_ok = $shellOk
            win32_ui_shell_in_crates_windows = $false
            also_requires_win32_system_registry = $shellNeedsRegistry
            ownership = 'section_2_14_owns_by_name_aumid'
            expand_2_9 = $false
            error = $shellError
            branch = $(if (-not $shellMeasurable) {
                'declined_unmeasurable_environment'
            } elseif ($shellOk) {
                'positive_binding_validated_no_2_9_expansion'
            } else {
                'binding_failed'
            })
        }
    }
    $script:paths.manifest = Write-LifecycleCapture -Name "lifecycle-manifest-$Label.json" -Content (ConvertTo-Json -InputObject $manifestCapture -Depth 14)
    Register-MandatoryPass -Capture $script:paths.manifest -Result $manifestCapture
    Write-Host "wrote $($script:paths.manifest)"

    # --- Cost baseline (A15-13 / A20-6): delegated to measure-cost.ps1 ----------
    $measureCost = Join-Path $script:ProbeDir 'measure-cost.ps1'
    & $measureCost -Label $Label
    $costPath = Join-Path $script:CaptureDir "lifecycle-cost-$Label.json"
    if (-not (Test-Path -LiteralPath $costPath)) {
        throw "lifecycle-cost-$Label.json was not written by measure-cost.ps1"
    }
    $costObj = Get-Content -LiteralPath $costPath -Raw | ConvertFrom-Json
    $script:paths.cost = $costPath
    Register-MandatoryPass -Capture $script:paths.cost -Result $costObj
    Write-Host "wrote $($script:paths.cost)"

} finally {
    Stop-AllSpawned
}

Assert-MandatoryMeasurement -Probe '21-system-lifecycle' -Label $Label

Write-ProbeResult -Probe '21-system-lifecycle' -Status 'ok' -Message 'system-lifecycle gap probes captured' -Data @{
    launch     = if ($script:paths.launch) { Split-Path -Leaf $script:paths.launch } else { '<none>' }
    close      = if ($script:paths.close) { Split-Path -Leaf $script:paths.close } else { '<none>' }
    hang       = if ($script:paths.hang) { Split-Path -Leaf $script:paths.hang } else { '<none>' }
    window_op  = if ($script:paths.window) { Split-Path -Leaf $script:paths.window } else { '<none>' }
    activation = if ($script:paths.activation) { Split-Path -Leaf $script:paths.activation } else { '<none>' }
    cross      = if ($script:paths.cross) { Split-Path -Leaf $script:paths.cross } else { '<none>' }
    manifest   = if ($script:paths.manifest) { Split-Path -Leaf $script:paths.manifest } else { '<none>' }
    cost       = if ($script:paths.cost) { Split-Path -Leaf $script:paths.cost } else { '<none>' }
}
exit 0
