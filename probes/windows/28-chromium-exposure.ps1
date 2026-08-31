#Requires -Version 5.1
<#
.SYNOPSIS
    Probe 28: Chromium exposure measurement probe.

.DESCRIPTION
    Measures whether a Chromium/Electron application on this host exposes
    a UI Automation content tree above the exposure floor (34 nodes).
    Counts nodes beneath the first Chrome_WidgetWin_1 candidate window at
    initial contact, after 15 seconds, and after 30 seconds of settling.

    Run: powershell -NoProfile -ExecutionPolicy Bypass -File .\probes\windows\28-chromium-exposure.ps1 -Label <devbox|ci>
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'ci')][string]$Label = 'devbox'
)

$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\common.ps1"

$Probe = '28-chromium-exposure'

function Initialize-ChromiumExposureNative {
    if ('AgentDesktopProbe.ChromiumExposure28' -as [type]) { return }
    $src = @'
using System;
using System.Runtime.InteropServices;
using System.Text;

namespace AgentDesktopProbe {
    public static class ChromiumExposure28 {
        [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        public static extern int GetClassNameW(IntPtr hWnd, StringBuilder lpClassName, int nMaxCount);
    }
}
'@
    Add-ProbeInlineCSharp -Source $src -AssemblyLeaf 'AgentDesktopProbeChromiumExposure28'
}

function Get-DescendantCount {
    param([IntPtr]$Hwnd)
    if ($Hwnd -eq [IntPtr]::Zero) { return $null }
    try {
        $element = [System.Windows.Automation.AutomationElement]::FromHandle($Hwnd)
        if ($null -eq $element) { return $null }
        $scope = [System.Windows.Automation.TreeScope]::Descendants
        $condition = [System.Windows.Automation.Condition]::TrueCondition
        $all = $element.FindAll($scope, $condition)
        if ($null -eq $all) { return 0 }
        return [int]$all.Count
    } catch {
        return $null
    }
}

try {
    Initialize-ChromiumExposureNative

    try {
        Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes -ErrorAction Stop
    } catch { }

    # ---------------------------------------------------------------- STEP 1 - Find candidate hosts
    $firstCandidateHwnd = [IntPtr]::Zero
    $chromiumHostCount = 0
    try {
        $allProcesses = Get-Process -ErrorAction SilentlyContinue
        foreach ($proc in $allProcesses) {
            try {
                $hwnd = $proc.MainWindowHandle
                if ($hwnd -ne [IntPtr]::Zero) {
                    $sb = New-Object System.Text.StringBuilder 256
                    $len = 0
                    try {
                        $len = [AgentDesktopProbe.ChromiumExposure28]::GetClassNameW($hwnd, $sb, 256)
                    } catch {
                        $len = 0
                    }
                    if ($len -gt 0 -and $sb.ToString() -eq 'Chrome_WidgetWin_1') {
                        $chromiumHostCount++
                        if ($firstCandidateHwnd -eq [IntPtr]::Zero) {
                            $firstCandidateHwnd = $hwnd
                        }
                    }
                }
            } catch { }
        }
    } catch {
        $chromiumHostCount = 0
    }

    if ($chromiumHostCount -eq 0 -or $firstCandidateHwnd -eq [IntPtr]::Zero) {
        $capture = [ordered]@{
            label                      = $Label
            hosts_found                = 0
            chromium_host_count        = 0
            descendant_count_initial   = $null
            descendant_count_after_15s = $null
            descendant_count_after_30s = $null
            exposure_floor             = 34
            exceeded_floor             = $null
        }

        $capturePath = Write-ProbeJson -Probe $Probe -Name "chromium-exposure-$Label.json" -InputObject $capture
        Write-ProbeLog -Message ('wrote ' + (Split-Path -Leaf $capturePath))
        Write-ProbeResult -Probe $Probe -Status 'ok' -Message 'no Chromium host was running'
        exit 0
    }

    # ---------------------------------------------------------------- STEP 2 - Initial observation
    $descendantCountInitial = $null
    try {
        $descendantCountInitial = Get-DescendantCount -Hwnd $firstCandidateHwnd
    } catch {
        $descendantCountInitial = $null
    }

    # ---------------------------------------------------------------- STEP 3 - Settle and re-measure
    Start-Sleep -Seconds 15

    $descendantCountAfter15s = $null
    try {
        $descendantCountAfter15s = Get-DescendantCount -Hwnd $firstCandidateHwnd
    } catch {
        $descendantCountAfter15s = $null
    }

    Start-Sleep -Seconds 15

    $descendantCountAfter30s = $null
    try {
        $descendantCountAfter30s = Get-DescendantCount -Hwnd $firstCandidateHwnd
    } catch {
        $descendantCountAfter30s = $null
    }

    # ---------------------------------------------------------------- STEP 4 - Floor comparison & capture
    $exposureFloor = 34
    $maxCount = -1
    $hasCount = $false
    foreach ($countVal in @($descendantCountInitial, $descendantCountAfter15s, $descendantCountAfter30s)) {
        if ($null -ne $countVal) {
            $hasCount = $true
            if ([int]$countVal -gt $maxCount) {
                $maxCount = [int]$countVal
            }
        }
    }

    $exceededFloor = $null
    if ($hasCount) {
        $exceededFloor = ($maxCount -gt $exposureFloor)
    }

    $capture = [ordered]@{
        label                      = $Label
        hosts_found                = $chromiumHostCount
        chromium_host_count        = $chromiumHostCount
        descendant_count_initial   = $descendantCountInitial
        descendant_count_after_15s = $descendantCountAfter15s
        descendant_count_after_30s = $descendantCountAfter30s
        exposure_floor             = $exposureFloor
        exceeded_floor             = $exceededFloor
    }

    $capturePath = Write-ProbeJson -Probe $Probe -Name "chromium-exposure-$Label.json" -InputObject $capture
    Write-ProbeLog -Message ('wrote ' + (Split-Path -Leaf $capturePath))
    Write-ProbeResult -Probe $Probe -Status 'ok' -Message 'Chromium exposure measured'
    exit 0
} catch {
    Write-ProbeResult -Probe $Probe -Status 'fail' -Message ('unhandled error: ' + $_.Exception.Message)
    exit 1
}
