#Requires -Version 5.1
<#
.SYNOPSIS
    Area 26 probe.ps1 - rows A26-1 (reach) and A26-2 (open/closed predicate)
    against the UIA3 COM stack.

.DESCRIPTION
    A26-1 pairs a positive and a negative around one mechanism question: when
    an Action Center surface is raised, does the Win32 EnumWindows walk yield
    its handle at all, and does the same walk still yield Shell_TrayWnd (the
    positive control)? The negative only means something next to a true
    control, so a control reading false fails this script outright instead of
    being recorded as evidence. The UIA side of the same states records the
    root's child count open versus closed and whether the surface's own
    CurrentNativeWindowHandle appears among those children - membership, not
    totals, because A26-2 shows the window can survive dismissal.

    A26-2 is the open/closed predicate itself: WS_EX_TOOLWINDOW,
    DWMWA_CLOAKED and GetParent for the Action Center CoreWindow, read in
    both states through the same handle.

    Every UIA fact here comes from the hand-declared CUIAutomation8 shim
    (lib.ps1 compiles shell-probe.cs exactly the way area 8 binds its shim);
    no managed-stack reading participates anywhere in this script.

    Run: powershell -NoProfile -ExecutionPolicy Bypass -File .\probes\windows\26-shell-surfaces\probe.ps1 -Label <devbox|ci>
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

$script:Probe = '26-shell-surfaces/probe'
Register-MandatoryCapture -Name @("shell-reach-$Label.json")

function Get-AcCandidate {
    param([Parameter(Mandatory = $true)]$Scan)
    # ac_candidate itself means "shell-host CoreWindow carrying Action Center
    # landmarks" - content state carries MainListView, empty state carries
    # Microsoft.QuickAction.* buttons.
    foreach ($c in $Scan.children) {
        if ($c.ac_candidate -and $c.nativewindowhandle -ne 0) {
            return $c
        }
    }
    return $null
}

function Test-ShellTrayYield {
    param([Parameter(Mandatory = $true)]$Scan)
    return [bool]$Scan.enum_walk_yields_shell_tray_wnd
}

<#
    This desktop's EnumWindows exposure of Shell_TrayWnd oscillates across
    minutes (FindWindowW keeps finding it throughout, so the taskbar itself
    never leaves - only the enumeration changes), so a single unlucky walk
    would fail the positive control spuriously. A control leg reads FALSE
    here; before failing, re-walk a bounded number of times to distinguish a
    transient phase from a standing one. Only a persistently false control
    fails the script - which is exactly the unit's contract: the negative is
    never recorded next to an untrustworthy control.
#>
function Get-TrustedControlScan {
    param(
        [Parameter(Mandatory = $true)][string[]]$ScanArguments,
        [int]$MaxAttempts = 8,
        [int]$GapMs = 4000
    )
    $lastScan = $null
    for ($attempt = 1; $attempt -le $MaxAttempts; $attempt++) {
        $lastScan = Invoke-ShellProbe -Arguments $ScanArguments
        if (Test-ShellTrayYield -Scan $lastScan) { return @{ scan = $lastScan; attempts = $attempt } }
        Start-Sleep -Milliseconds $GapMs
    }
    return @{ scan = $lastScan; attempts = $MaxAttempts }
}

$result = [ordered]@{
    probe        = $script:Probe
    question     = 'is the immersive Action Center surface reachable by EnumWindows or only through the UIA root children, and what do WS_EX_TOOLWINDOW / DWMWA_CLOAKED / GetParent read for it open versus closed - measured so KTD1''s two-mechanism reach decision rests on committed capture rather than plan prose'
    cites        = @('KTD1')
    label        = $Label
    client_stack = 'uia3-com'
    measurable   = $false
}
$closedScan = $null
$openScan = $null
$predicateOpen = $null
$predicateClosed = $null
$surfaceHandleClosedState = $null
$status = 'ok'
$message = 'shell reach probe captured'

try {
    Initialize-ShellProbe | Out-Null

    $baselineClean = Reset-ShellSurfaceBaseline
    if (-not $baselineClean) {
        throw 'shell_baseline_not_clean_after_escape_and_wm_close'
    }
    $closedScanResult = Get-TrustedControlScan -ScanArguments @('reachscan')
    $closedScan = $closedScanResult.scan
    Write-ProbeLog -Message ('control walk for the closed state needed ' + $closedScanResult.attempts + ' attempt(s)') -Level 'info'
    $closedScan | Add-Member -NotePropertyName taken_after_baseline_reset -NotePropertyValue ([bool]$baselineClean)

    # --- raise -----------------------------------------------------------
    $raised = $false
    $attemptsUsed = 0
    for ($attempt = 1; $attempt -le 3 -and -not $raised; $attempt++) {
        $attemptsUsed = $attempt
        Invoke-ShellProbe -Arguments @('key', '--seq', 'lwin_a') | Out-Null
        Start-Sleep -Milliseconds 900
        $openScanProbe = Invoke-ShellProbe -Arguments @('reachscan')
        if (Get-AcCandidate -Scan $openScanProbe) {
            $openScan = $openScanProbe
            $raised = $true
        }
        elseif ($attempt -lt 3) {
            Invoke-ShellProbe -Arguments @('key', '--seq', 'esc') | Out-Null
            Start-Sleep -Milliseconds 600
        }
    }

    if (-not ($raised -and $null -ne $openScan)) {
        if (Test-ShellTrayYield -Scan $closedScan) { throw 'action_center_not_raisable_by_lwin_a_accelerator' }
        # Never distinguish which leg failed while the control is untrusted.
        throw 'control_leg_shell_tray_wnd_not_yielded_by_enum_windows'
    }
    $openScanResult = Get-TrustedControlScan -ScanArguments @('reachscan')
    $openScan = $openScanResult.scan
    Write-ProbeLog -Message ('control walk for the open state needed ' + $openScanResult.attempts + ' attempt(s)') -Level 'info'

    $candidate = Get-AcCandidate -Scan $openScan
    $surfaceHandle = [string]$candidate.nativewindowhandle
    $predicateOpen = Invoke-ShellProbe -Arguments @('predicate', '--hwnd', $surfaceHandle)

    # --- dismiss and re-read ---------------------------------------------
    Invoke-ShellProbe -Arguments @('key', '--seq', 'esc') | Out-Null
    Start-Sleep -Milliseconds 700
    $afterCloseScan = Invoke-ShellProbe -Arguments @('reachscan')
    $survivor = $null
    foreach ($c in $afterCloseScan.children) {
        if ($c.ac_candidate -and $c.nativewindowhandle -eq [long]$candidate.nativewindowhandle) {
            $survivor = $c
        }
    }
    $closedAgainScan = $afterCloseScan
    $surfaceHandleClosedState = ([bool]$survivor)
    if ($survivor) {
        try {
            $predicateClosed = Invoke-ShellProbe -Arguments @('predicate', '--hwnd', ([string]$survivor.nativewindowhandle))
        } catch {
            $predicateClosed = [ordered]@{ readable_again = $false; error_class = $_.Exception.GetType().Name }
        }
    } else {
        # The surface left the UIA root entirely on dismissal; test whether the
        # HWND itself survived so its state can still be read.
        try {
            $predicateClosed = Invoke-ShellProbe -Arguments @('predicate', '--hwnd', $surfaceHandle)
            $surfaceHandleClosedState = $true
        } catch {
            $predicateClosed = [ordered]@{ readable_again = $false; error_class = $_.Exception.GetType().Name }
        }
    }
    $classificationClosedViaFindWindow = $null
    try {
        $coreWindowsLeft = Invoke-ShellProbe -Arguments @('findbyclass', '--cls', 'Windows.UI.Core.CoreWindow')
        $classificationClosedViaFindWindow = [int]$coreWindowsLeft.match_count
    } catch { }

    $controlClosed = Test-ShellTrayYield -Scan $closedScan
    $controlOpen = Test-ShellTrayYield -Scan $openScan

    $classification = [ordered]@{
        control_leg_shell_tray_wnd_yielded_in_both_walks = ($controlClosed -and $controlOpen)
        shell_tray_yielded_closed                        = $controlClosed
        shell_tray_yielded_open                          = $controlOpen
        surface_among_enum_walk_open                     = [bool]$openScan.surface_present_in_enum_walk
        surface_among_uia_root_children_open             = $true
        surface_among_uia_root_children_closed           = $surfaceHandleClosedState
        uia_root_child_count_closed                      = [int]$closedScan.uia_root_child_count
        uia_root_child_count_open                        = [int]$openScan.uia_root_child_count
        uia_root_child_count_delta_open_minus_closed     = ([int]$openScan.uia_root_child_count - [int]$closedScan.uia_root_child_count)
        find_window_route_finds_core_window_class        = $null
        core_window_class_top_level_matches_after_dismissal = $classificationClosedViaFindWindow
        window_survives_dismissal                        = ([bool]$surfaceHandleClosedState)
        window_among_root_children_after_dismissal       = ([bool]$survivor)
    }
    if ($predicateOpen) {
        $classification['cloak_state_open'] = $predicateOpen.cloak_state
    }
    if ($predicateClosed -and ($predicateClosed.PSObject.Properties['cloak_state'] -and $null -ne $predicateClosed.cloak_state)) {
        $classification['cloak_state_closed'] = $predicateClosed.cloak_state
    }

    $result['measurable'] = $true
    $result['branch'] = 'measured_open_and_closed_with_predicate_reads'
    $result['raise_attempts_used'] = $attemptsUsed
    $result['classification'] = $classification
    $result['closed_scan'] = $closedScan
    $result['open_scan'] = $openScan
    $result['predicate_open'] = $predicateOpen
    $result['predicate_closed'] = $predicateClosed

    if (-not ($controlClosed -and $controlOpen)) {
        # The unit's contract is explicit: a false positive control means the
        # negative legs mean nothing, so the run FAILS instead of recording
        # them. Everything observed up to here stays inside the capture -
        # failing loudly must not be failing silently.
        $message = 'positive control failed: EnumWindows walk did not yield Shell_TrayWnd'
        $status = 'fail'
        $result['branch'] = 'control_leg_shell_tray_wnd_not_yielded_by_enum_windows'
        $result['measurable'] = $false
        $result['control_leg_failed'] = $true
    }
} catch {
    $status = 'fail'
    $message = $_.Exception.Message -replace '\r?\n+', ' '
    $result['measurable'] = $false
    $result['error'] = ($_.Exception.Message -replace '[\r\n]+', ' ')
    $result['not_measured_reason'] = $result['error']
    if ($closedScan) { $result['closed_scan_partial'] = $closedScan }
} finally {
    try { Invoke-ShellProbe -Arguments @('key', '--seq', 'esc') | Out-Null } catch { }
}

$content = ConvertTo-Json -InputObject $result -Depth 24
if ($status -eq 'fail') {
    $placeholder = New-NotMeasuredResult -Reason $message
    $content = ConvertTo-Json -InputObject ([ordered]@{
            probe              = $script:Probe
            label              = $Label
            not_measured       = $placeholder.not_measured
            skipped            = $placeholder.skipped
            partial_facts      = $result
        }) -Depth 24
}

try {
    $capturePath = Write-Shell26Capture -Name "shell-reach-$Label.json" -Content $content
    Register-MandatoryPass -Capture $capturePath -Result (@{ not_measured_placeholder_written = ($status -eq 'fail') })
} catch {
    $status = 'fail'
    $message = ('capture write failed: ' + $_.Exception.Message)
}

Write-ProbeResult -Probe $script:Probe -Status $status -Message $message -Data @{
    capture   = "captures/shell-reach-$Label.json"
    rows      = @('A26-1', 'A26-2')
    stack     = 'uia3-com'
}
if ($status -eq 'fail') { exit 1 }

Assert-MandatoryMeasurement -Probe $script:Probe -Label $Label
exit 0
