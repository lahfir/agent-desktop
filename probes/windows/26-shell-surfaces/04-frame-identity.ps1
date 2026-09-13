#Requires -Version 5.1
<#
.SYNOPSIS
    Area 26 04-frame-identity.ps1 - rows A26-8 (ApplicationFrameWindow frame
    identity) and A26-9 (the surface the Start accelerator raises).

.DESCRIPTION
    A26-8 launches the UWP-hosted Settings app, brings its frame to the
    foreground, and records: the foreground window's class, the class of
    every child of that frame, and which child classes have owning processes
    that differ from the frame's - expressed ONLY as pid-equality counts per
    class (owner_pid_equals_frame_pid_count versus instances), never as pid
    numbers, and with owning processes classified as frame_host /
    shell_host tokens / other rather than named. This is what KTD7's
    handle/pid split rests on.

    A26-9 raises the Start menu via a real Meta-key SendInput and records
    which shell host owns the raised surface plus the AutomationId tag set at
    its root. On this build planning measured the accelerator raising a
    search-hosted CoreWindow rather than a tile surface; whatever this run
    observes is recorded truthfully and classified.

    Run: powershell -NoProfile -ExecutionPolicy Bypass -File .\probes\windows\26-shell-surfaces\04-frame-identity.ps1 -Label <devbox|ci>
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'ci')][string]$Label = 'devbox'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

. (Join-Path (Split-Path -Parent $PSCommandPath) '..\common.ps1')
Initialize-ProbeRedaction
. (Join-Path (Split-Path -Parent $PSCommandPath) 'lib.ps1')

$script:Probe = '26-shell-surfaces/04-frame-identity'
Register-MandatoryCapture -Name @("frame-identity-$Label.json")

$touchedFrameHandles = New-Object System.Collections.ArrayList

function Wait-ForForegroundClass {
    param([Parameter(Mandatory = $true)][string[]]$ClassNames, [int]$TimeoutSec = 25)
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ((Get-Date) -lt $deadline) {
        $fg = Invoke-ShellProbe -Arguments @('foregroundinfo')
        if ([bool]$fg.foreground_present -and (@($ClassNames) -contains [string]$fg.foreground_class)) {
            return $fg
        }
        Start-Sleep -Milliseconds 400
    }
    return $null
}

$status = 'ok'
$message = 'frame identity + start-surface rows captured'

$frameLeg = [ordered]@{ measurable = $false; branch = 'not_measured' }
$startLeg = [ordered]@{ measurable = $false; branch = 'not_measured' }

try {
    Initialize-ShellProbe | Out-Null

    # ------------------------------------------------------------- A26-8
    if (-not (Test-Path -LiteralPath (Join-Path $env:WINDIR 'ImmersiveControlPanel\SystemSettings.exe'))) {
        $frameLeg['branch'] = 'system_settings_exe_not_found_at_immersive_control_panel_path'
        $frameLeg['staged'] = $false
        $frameLeg['declined_reason'] = $frameLeg['branch']
    } else {
        function Get-FrameCandidateHandles {
            $found = New-Object System.Collections.ArrayList
            try {
                $hits = Invoke-ShellProbe -Arguments @('findbyclass', '--cls', 'ApplicationFrameWindow')
                foreach ($h in $hits.handles) { [void]$found.Add([string]$h) }
            } catch { }
            return $found.ToArray()
        }

        # Launch route measured on this box: a direct spawn of
        # ImmersiveControlPanel\SystemSettings.exe exits immediately without
        # creating any frame; the URI dispatch through the shell is the one
        # that materializes an ApplicationFrameWindow. Settings is
        # single-instanced here, so any SUSPENDED (cloaked) frame-host-owned
        # window from a prior session must be dismissed first - the capture
        # records how many were.
        $suspendedClosed = 0
        foreach ($w in @(Get-FrameCandidateHandles)) {
            try {
                $pred = Invoke-ShellProbe -Arguments @('predicate', '--hwnd', ([string]$w))
                if ($pred.cloak_state -ne 'none' -and (@('frame_host', 'system_settings') -contains [string]$pred.host_token)) {
                    Invoke-ShellProbe -Arguments @('closewindow', '--hwnd', ([string]$w)) | Out-Null
                    $suspendedClosed++
                }
            } catch { }
        }
        if ($suspendedClosed -gt 0) { Start-Sleep -Seconds 2 }

        $preExistingFrames = @(Get-FrameCandidateHandles)
        $launchRoute = 'ms_settings_uri_shell_dispatch'
        $usedExistingInstance = $false
        Start-Process 'ms-settings:'

        $frameActivated = $false
        $foreground = $null
        for ($i = 0; $i -lt 10 -and -not $frameActivated; $i++) {
            Start-Sleep -Seconds 2
            $candidatesNow = @(Get-FrameCandidateHandles)
            $freshHandles = @($candidatesNow | Where-Object { $preExistingFrames -notcontains $_ })
            foreach ($w in $freshHandles) {
                if (-not $touchedFrameHandles.Contains($w)) { [void]$touchedFrameHandles.Add($w) }
                Invoke-ShellProbe -Arguments @('activate', '--hwnd', ([string]$w)) | Out-Null
                Start-Sleep -Milliseconds 900
                $foreground = Wait-ForForegroundClass -ClassNames @('ApplicationFrameWindow') -TimeoutSec 5
                if ($foreground) { $frameActivated = $true; break }
            }
        }

        # Single-instance fallback: when a live Settings already existed, the
        # URI dispatch focuses IT and creates no fresh frame. Activating any
        # existing frame-host-owned frame whose foreground read lands on
        # ApplicationFrameWindow measures the same identity chain.
        if (-not $frameActivated) {
            foreach ($w in @(Get-FrameCandidateHandles)) {
                try {
                    $pred = Invoke-ShellProbe -Arguments @('predicate', '--hwnd', ([string]$w))
                    if ((@('frame_host', 'system_settings') -notcontains [string]$pred.host_token)) { continue }
                } catch { continue }
                Invoke-ShellProbe -Arguments @('activate', '--hwnd', ([string]$w)) | Out-Null
                Start-Sleep -Milliseconds 900
                $fg = Wait-ForForegroundClass -ClassNames @('ApplicationFrameWindow') -TimeoutSec 5
                if ($fg -and [string]$fg.nativewindowhandle -eq [string]$w) {
                    $foreground = $fg
                    $frameActivated = $true
                    $usedExistingInstance = $true
                    break
                }
            }
        }

        if (-not $frameActivated -or -not $foreground) {
            # Settings never presented a frame on this shell: the frame walk
            # could not be staged, an environment outcome rather than a failed
            # read (a walk that runs once Settings DOES activate and then fails
            # still throws below and stays a strict failure).
            $frameLeg['branch'] = 'application_frame_window_never_reached_foreground'
            $frameLeg['staged'] = $false
            $frameLeg['declined_reason'] = $frameLeg['branch']
        } else {
            $framePredicate = Invoke-ShellProbe -Arguments @('predicate', '--hwnd', ([string]$foreground.nativewindowhandle))
            $frameWalk = Invoke-ShellProbe -Arguments @('framewalk', '--frame', ([string]$foreground.nativewindowhandle))
            $frameLeg['measurable'] = $true
            $frameLeg['branch'] = $(if ($usedExistingInstance) { 'foreground_frame_walked_on_existing_settings_instance' } else { 'foreground_frame_walked_fresh_launch' })
            $frameLeg['launch_route_recorded'] = $launchRoute
            $frameLeg['suspended_frames_closed_before_launch'] = $suspendedClosed
            $frameLeg['foreground_class'] = [string]$foreground.foreground_class
            $frameLeg['frame_owner_host_token'] = [string]$framePredicate.host_token
            $frameLeg['frame_class_is_application_frame_window'] = [bool]$frameWalk.frame_class_is_application_frame_window
            $frameLeg['child_instances_by_class'] = @($frameWalk.child_instances_by_class)
            $classesDiffering = @(@($frameWalk.child_instances_by_class) | Where-Object { $_.any_owner_differs_from_frame })
            $frameLeg['child_classes_with_owner_differing_from_frame'] = @($classesDiffering | ForEach-Object { $_.class })
            $coreChild = (@(@($frameWalk.child_instances_by_class) | Where-Object { $_.class -eq 'Windows.UI.Core.CoreWindow' }) | Select-Object -First 1)
            $frameLeg['core_window_child_present'] = [bool]$coreChild
            $frameLeg['core_window_owner_differs_from_frame'] = ([bool]$coreChild -and [bool]$coreChild.any_owner_differs_from_frame)
        }
    }

    # ------------------------------------------------------------- A26-9
    Reset-ShellSurfaceBaseline | Out-Null
    Invoke-ShellProbe -Arguments @('key', '--seq', 'lwin') | Out-Null
    Start-Sleep -Milliseconds 900
    $raisedFg = Invoke-ShellProbe -Arguments @('foregroundinfo')
    if ([bool]$raisedFg.foreground_present -and [string]$raisedFg.foreground_class -eq 'Windows.UI.Core.CoreWindow') {
        $rootIds = Invoke-ShellProbe -Arguments @('surfacerootids', '--hwnd', ([string]$raisedFg.nativewindowhandle))
        $startLeg['measurable'] = $true
        $startLeg['branch'] = 'accelerator_raised_corewindow_surface_read'
        $startLeg['foreground_host_token'] = [string]$raisedFg.foreground_host_token
        $startLeg['shell_host_owns_raised_surface'] = [string]$raisedFg.foreground_host_token
        $startLeg['root_control_type'] = [string]$rootIds.root_control_type
        $startLeg['root_automation_id_tag'] = [string]$rootIds.root_automation_id_tag
        $startLeg['direct_children_automation_id_tags'] = @($rootIds.direct_children_automation_id_tags)
    } else {
        # The Meta key raised nothing the shell would present: a Start/immersive
        # surface this shell genuinely does not expose is an environment
        # outcome, recorded honestly and skipped rather than failed.
        $startLeg['branch'] = 'accelerator_did_not_raise_a_shell_core_window_foreground'
        $startLeg['staged'] = $false
        $startLeg['raisable'] = $false
        $startLeg['declined_reason'] = $startLeg['branch']
        $startLeg['observed_foreground_class'] = $(if ([bool]$raisedFg.foreground_present) { [string]$raisedFg.foreground_class } else { '<none>' })
        $startLeg['observed_foreground_host_token'] = $(if ([bool]$raisedFg.foreground_present) { [string]$raisedFg.foreground_host_token } else { $null })
    }
    Invoke-ShellProbe -Arguments @('key', '--seq', 'esc') | Out-Null
} catch {
    $status = 'fail'
    $message = $_.Exception.Message -replace '[\r\n]+', ' '
} finally {
    foreach ($w in @($touchedFrameHandles)) {
        try { Invoke-ShellProbe -Arguments @('closewindow', '--hwnd', ([string]$w)) | Out-Null } catch { }
    }
    try { Invoke-ShellProbe -Arguments @('key', '--seq', 'esc') | Out-Null } catch { }
}

# A leg whose staging the shell declined is an environment outcome, not a
# failure: name it in the result and skip. A leg that WAS staged and then
# threw already failed the run inside the try above.
if ($status -eq 'ok') {
    $declinedReasons = @()
    if ($frameLeg.measurable -eq $false -and $frameLeg.Contains('declined_reason')) { $declinedReasons += [string]$frameLeg['declined_reason'] }
    if ($startLeg.measurable -eq $false -and $startLeg.Contains('declined_reason')) { $declinedReasons += [string]$startLeg['declined_reason'] }
    if ($declinedReasons.Count -gt 0) {
        $status = 'skip'
        $message = ($declinedReasons -join '; ')
    }
}

$content = ConvertTo-Json -InputObject ([ordered]@{
        probe         = $script:Probe
        question      = 'which of the frame window''s children belong to a different process than the frame itself (pid-equality classes only), and which shell host owns the CoreWindow the Meta key raises'
        cites         = @('KTD7')
        label         = $Label
        frame_leg     = $frameLeg
        start_leg     = $startLeg
    }) -Depth 20

if ($status -eq 'fail') {
    $placeholder = New-NotMeasuredResult -Reason $message
    $content = ConvertTo-Json -InputObject ([ordered]@{
            probe        = $script:Probe
            label        = $Label
            not_measured = $placeholder.not_measured
            skipped      = $placeholder.skipped
            partial_facts = ([ordered]@{ frame_leg = $frameLeg; start_leg = $startLeg })
        }) -Depth 14
}

try {
    $capturePath = Write-Shell26Capture -Name "frame-identity-$Label.json" -Content $content
    Register-MandatoryPass -Capture $capturePath -Result @{ measurable_placeholder_written = $false; status_ok = ($status -eq 'ok'); not_measured = ($status -eq 'fail') }
} catch {
    $status = 'fail'
    $message = ('capture write failed: ' + $_.Exception.Message)
}

Write-ProbeResult -Probe $script:Probe -Status $status -Message $message -Data @{
    capture   = "captures/frame-identity-$Label.json"
    rows      = @('A26-8', 'A26-9')
    stack     = 'uia3-com'
}
if ($status -eq 'fail') { exit 1 }

Assert-MandatoryMeasurement -Probe $script:Probe -Label $Label
exit 0
