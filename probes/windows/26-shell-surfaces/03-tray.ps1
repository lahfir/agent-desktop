#Requires -Version 5.1
<#
.SYNOPSIS
    Area 26 03-tray.ps1 - rows A26-5/A26-6/A26-7 for the notification area,
    all UIA facts on the COM stack.

.DESCRIPTION
    A26-5 records the child count of the promoted notification-area
    ToolbarWindow32 and of the overflow toolbar, each read through the
    CUIAutomation8 shim, BESIDE a managed System.Windows.Automation count for
    the same window which is committed only as a labelled non-authoritative
    cross-check (KTD3: the managed client is the one that miscounts these
    classic toolbars). A26-6 records which overflow window class is present -
    NotifyIconOverflowWindow on this build corroborating C-5 first-party.
    A26-7 records per child of the promoted toolbar: control type, whether an
    AutomationId is present and non-empty (the flag only; the ids themselves
    are machine-local GUIDs KTD14 keeps out of captures), the
    pattern-availability set, whether bounds are positive-area, and one
    within-session re-read whose per-index booleans are what R8's stability
    premise rests on.

    Run: powershell -NoProfile -ExecutionPolicy Bypass -File .\probes\windows\26-shell-surfaces\03-tray.ps1 -Label <devbox|ci>
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

$script:Probe = '26-shell-surfaces/03-tray'
Register-MandatoryCapture -Name @("tray-$Label.json")

function Get-ManagedChildCount {
    <# Labelled non-authoritative cross-check ONLY (KTD3). The managed client
       reads these classic toolbars through its own proxy layer and reported
       zero children during planning where COM reported three; whatever it
       says here is recorded with authoritative=$false. #>
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$HandleDecimal)
    Add-Type -AssemblyName UIAutomationClient -ErrorAction SilentlyContinue
    Add-Type -AssemblyName UIAutomationTypes -ErrorAction SilentlyContinue
    $row = [ordered]@{ stack = 'managed'; authoritative = $false; child_count = $null }
    try {
        $handle = [IntPtr][long]$HandleDecimal
        if ($handle -eq [IntPtr]::Zero) { return $row }
        $element = [System.Windows.Automation.AutomationElement]::FromHandle($handle)
        $children = $element.FindAll([System.Windows.Automation.TreeScope]::Children, [System.Windows.Automation.Condition]::TrueCondition)
        $row['child_count'] = [int]$children.Count
    } catch {
        $row['child_count'] = $null
        $row['error_class'] = $_.Exception.GetType().Name
    }
    return $row
}

$status = 'ok'
$message = 'tray rows captured on the COM stack'
$trayscan = $null
$analysisResult = $null

try {
    Initialize-ShellProbe | Out-Null
    $trayscan = Invoke-ShellProbe -Arguments @('trayscan')

    $promotedRows = @()
    foreach ($tb in $trayscan.toolbars) {
        if (-not $tb.found) { continue }
        $btns = @($tb.buttons)
        $allPositive = ($btns.Count -gt 0)
        $anyAid = $false
        foreach ($b in $btns) {
            if (-not $b.bounds_positive_area) { $allPositive = $false }
            if ($b.automation_id_present_nonempty) { $anyAid = $true }
        }
        $isPromotedShape = ($allPositive -and $anyAid)
        $managedRow = $null
        if ($tb.PSObject.Properties['nativewindowhandle']) {
            $managedRow = Get-ManagedChildCount -HandleDecimal ([string]$tb.nativewindowhandle)
        }
        $promotedRows += [ordered]@{
            label                     = [string]$tb.label
            com_direct_children       = [int]$tb.com_direct_children
            stack_com                 = 'uia3-com'
            managed_cross_check       = $managedRow
            button_shapes_recorded    = [int]$tb.button_shapes_recorded
            buttons                   = $btns
            stability_reread          = @($tb.stability_reread)
            fits_promoted_shape_flags = [ordered]@{
                every_button_positive_area      = $allPositive
                any_automation_id_present_nonempty = $anyAid
                classified_as_promoted_candidate = $isPromotedShape
            }
        }
    }

    $promotedCandidates = @($promotedRows | Where-Object { $_.fits_promoted_shape_flags.classified_as_promoted_candidate })
    $overflowToolbarRow = @($promotedRows | Where-Object { $_.label -eq 'overflow' }) | Select-Object -First 1

    $analysisResult = [ordered]@{
        overflow_window_class_notify_icon_overflow_window_present = [bool]$trayscan.overflow_window_class_notify_icon_overflow_window_present
        overflow_window_visible                                  = [bool]$trayscan.overflow_window_visible
        overflow_inner_toolbar_found                             = [bool]$trayscan.overflow_inner_toolbar_found
        tray_notify_wnd_child_present                            = [bool]$trayscan.tray_notify_wnd_child_present
        promoted_toolbar_candidates_labels                       = @($promotedCandidates | ForEach-Object { $_.label })
        toolbars                                                 = @($promotedRows)
    }

    if ($overflowToolbarRow) {
        if ([int]$overflowToolbarRow.com_direct_children -ge 0 -and -not $trayscan.overflow_inner_toolbar_found) {
            $analysisResult['overflow_children_count_measurable'] = $false
            $analysisResult['overflow_children_count_reason'] = 'no inner ToolbarWindow32 exists while the overflow window stays closed on this build'
        } else {
            $analysisResult['overflow_children_count_measurable'] = $true
            $analysisResult['overflow_children_count_com'] = [int]$overflowToolbarRow.com_direct_children
            $analysisResult['stack_of_overflow_children_count'] = 'uia3-com'
        }
    }

    if (@($analysisResult.toolbars).Count -eq 0) {
        throw 'no_taskbar_toolbar_windows_reported_by_shell_probe'
    }
} catch {
    $status = 'fail'
    $message = $_.Exception.Message -replace '[\r\n]+', ' '
}

$content = ConvertTo-Json -InputObject ([ordered]@{
        probe         = $script:Probe
        label         = $Label
        measurable    = ($status -eq 'ok')
        error_message = $(if ($status -eq 'fail') { $message } else { $null })
        result        = ([ordered]@{
                shell_probe   = $trayscan
                client_stack  = 'uia3-com'
                analysis      = $analysisResult
            })
    }) -Depth 24

if ($status -ne 'ok') {
    $placeholder = New-NotMeasuredResult -Reason $message
    $content = ConvertTo-Json -InputObject ([ordered]@{
            probe         = $script:Probe
            label         = $Label
            not_measured  = $placeholder.not_measured
            skipped       = $placeholder.skipped
        }) -Depth 12
}

try {
    $capturePath = Write-Shell26Capture -Name "tray-$Label.json" -Content $content
    Register-MandatoryPass -Capture $capturePath -Result @{ measured_placeholder_written = $false; status_ok = ($status -eq 'ok'); not_measured = ($status -ne 'ok') }
} catch {
    $status = 'fail'
    $message = ('capture write failed: ' + $_.Exception.Message)
}

Write-ProbeResult -Probe $script:Probe -Status $status -Message $message -Data @{
    capture   = "captures/tray-$Label.json"
    rows      = @('A26-5', 'A26-6', 'A26-7')
    stack     = 'uia3-com'
}
if ($status -eq 'fail') { exit 1 }

Assert-MandatoryMeasurement -Probe $script:Probe -Label $Label
exit 0
