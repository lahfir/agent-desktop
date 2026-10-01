#Requires -Version 5.1
<#
.SYNOPSIS
    Probe 28: tray read diagnosis measurement probe.

.DESCRIPTION
    Measures and compares UI Automation tree reads rooted at the taskbar
    handle (Shell_TrayWnd) versus the notification area toolbar handle
    (ToolbarWindow32 under TrayNotifyWnd).

    Run: powershell -NoProfile -ExecutionPolicy Bypass -File .\probes\windows\28-tray-read-diagnosis.ps1 -Label <devbox|ci>
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'ci')][string]$Label = 'devbox'
)

$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\common.ps1"

$Probe = '28-tray-read-diagnosis'

function Initialize-TrayReadDiagnosisNative {
    if ('AgentDesktopProbe.TrayReadDiagnosis28' -as [type]) { return }
    $src = @'
using System;
using System.Runtime.InteropServices;
using System.Text;

namespace AgentDesktopProbe {
    public static class TrayReadDiagnosis28 {
        public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

        [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        public static extern IntPtr FindWindowW(string lpClassName, string lpWindowName);

        [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        public static extern IntPtr FindWindowExW(IntPtr hWndParent, IntPtr hWndChildAfter, string lpszClass, string lpszWindow);

        [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        public static extern int GetClassNameW(IntPtr hWnd, StringBuilder lpClassName, int nMaxCount);

        [DllImport("user32.dll", SetLastError = true)]
        public static extern IntPtr GetParent(IntPtr hWnd);

        [DllImport("user32.dll", SetLastError = true)]
        public static extern bool EnumChildWindows(IntPtr hWndParent, EnumWindowsProc lpEnumFunc, IntPtr lParam);

        public static IntPtr FindToolbarInTaskbar(IntPtr taskbarHwnd) {
            if (taskbarHwnd == IntPtr.Zero) return IntPtr.Zero;
            IntPtr hit = IntPtr.Zero;
            try {
                EnumChildWindows(taskbarHwnd, delegate(IntPtr h, IntPtr l) {
                    try {
                        StringBuilder b = new StringBuilder(256);
                        int len = GetClassNameW(h, b, 256);
                        if (len > 0 && string.Equals(b.ToString(), "ToolbarWindow32", StringComparison.Ordinal)) {
                            IntPtr parent = GetParent(h);
                            if (parent != IntPtr.Zero) {
                                StringBuilder pb = new StringBuilder(256);
                                int plen = GetClassNameW(parent, pb, 256);
                                if (plen > 0 && string.Equals(pb.ToString(), "TrayNotifyWnd", StringComparison.Ordinal)) {
                                    hit = h;
                                    return false;
                                }
                            }
                        }
                    } catch { }
                    return true;
                }, IntPtr.Zero);
            } catch { }
            return hit;
        }
    }
}
'@
    Add-ProbeInlineCSharp -Source $src -AssemblyLeaf 'AgentDesktopProbeTrayReadDiagnosis28'
}

try {
    Initialize-TrayReadDiagnosisNative

    try {
        Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes -ErrorAction Stop
    } catch { }

    # ---------------------------------------------------------------- STEP 1 - Resolve window handles
    $taskbarHwnd = [IntPtr]::Zero
    try {
        $taskbarHwnd = [AgentDesktopProbe.TrayReadDiagnosis28]::FindWindowW('Shell_TrayWnd', $null)
    } catch {
        $taskbarHwnd = [IntPtr]::Zero
    }
    $taskbar_resolved = ($taskbarHwnd -ne [IntPtr]::Zero)

    $trayNotifyHwnd = [IntPtr]::Zero
    try {
        if ($taskbar_resolved) {
            $trayNotifyHwnd = [AgentDesktopProbe.TrayReadDiagnosis28]::FindWindowExW($taskbarHwnd, [IntPtr]::Zero, 'TrayNotifyWnd', $null)
        }
    } catch {
        $trayNotifyHwnd = [IntPtr]::Zero
    }
    $tray_notify_resolved = ($trayNotifyHwnd -ne [IntPtr]::Zero)

    $toolbarHwnd = [IntPtr]::Zero
    try {
        if ($tray_notify_resolved) {
            $toolbarHwnd = [AgentDesktopProbe.TrayReadDiagnosis28]::FindWindowExW($trayNotifyHwnd, [IntPtr]::Zero, 'ToolbarWindow32', $null)
        }
    } catch {
        $toolbarHwnd = [IntPtr]::Zero
    }
    $toolbar_resolved = ($toolbarHwnd -ne [IntPtr]::Zero)

    # ---------------------------------------------------------------- STEP 2 - Fallback enumeration
    $toolbar_found_by_enum = $false
    if ((-not $toolbar_resolved) -and $taskbar_resolved) {
        try {
            $enumHwnd = [AgentDesktopProbe.TrayReadDiagnosis28]::FindToolbarInTaskbar($taskbarHwnd)
            if ($enumHwnd -ne [IntPtr]::Zero) {
                $toolbarHwnd = $enumHwnd
                $toolbar_found_by_enum = $true
            }
        } catch {
            $toolbar_found_by_enum = $false
        }
    }

    # ---------------------------------------------------------------- STEP 3 - Measure roots
    $taskbar_descendant_count = $null
    $taskbar_child_count = $null
    $taskbar_control_type_id = $null
    $taskbar_is_offscreen = $null
    $taskbar_provider_description_length = $null
    $taskbar_provider_names_uiautomationcore = $null
    $taskbar_provider_names_a_pid_marker = $null

    if ($taskbarHwnd -ne [IntPtr]::Zero) {
        try {
            $taskbarElement = [System.Windows.Automation.AutomationElement]::FromHandle($taskbarHwnd)
            if ($null -ne $taskbarElement) {
                try {
                    $ct = $taskbarElement.Current.ControlType
                    if ($null -ne $ct) {
                        $taskbar_control_type_id = [int]$ct.Id
                    }
                } catch { }

                try {
                    $taskbar_is_offscreen = [bool]$taskbarElement.Current.IsOffscreen
                } catch { }

                try {
                    $prop = [System.Windows.Automation.AutomationProperty]::LookupById(30107)
                    if ($null -ne $prop) {
                        $desc = $taskbarElement.GetCurrentPropertyValue($prop)
                        if ($null -ne $desc -and $desc -is [string]) {
                            $taskbar_provider_description_length = [int]$desc.Length
                            $taskbar_provider_names_uiautomationcore = [bool]($desc.IndexOf('uiautomationcore', [System.StringComparison]::OrdinalIgnoreCase) -ge 0)
                            $taskbar_provider_names_a_pid_marker = [bool]($desc.IndexOf('pid:', [System.StringComparison]::OrdinalIgnoreCase) -ge 0)
                        }
                    }
                } catch { }

                try {
                    $taskbarDescendants = $taskbarElement.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
                    if ($null -ne $taskbarDescendants) {
                        $taskbar_descendant_count = [int]$taskbarDescendants.Count
                    }
                } catch { }

                try {
                    $taskbarChildren = $taskbarElement.FindAll([System.Windows.Automation.TreeScope]::Children, [System.Windows.Automation.Condition]::TrueCondition)
                    if ($null -ne $taskbarChildren) {
                        $taskbar_child_count = [int]$taskbarChildren.Count
                    }
                } catch { }
            }
        } catch { }
    }

    $toolbar_descendant_count = $null
    $toolbar_child_count = $null
    $toolbar_control_type_id = $null
    $toolbar_is_offscreen = $null
    $toolbar_provider_description_length = $null
    $toolbar_provider_names_uiautomationcore = $null
    $toolbar_provider_names_a_pid_marker = $null
    $toolbar_button_count = $null

    if ($toolbarHwnd -ne [IntPtr]::Zero) {
        try {
            $toolbarElement = [System.Windows.Automation.AutomationElement]::FromHandle($toolbarHwnd)
            if ($null -ne $toolbarElement) {
                try {
                    $ct = $toolbarElement.Current.ControlType
                    if ($null -ne $ct) {
                        $toolbar_control_type_id = [int]$ct.Id
                    }
                } catch { }

                try {
                    $toolbar_is_offscreen = [bool]$toolbarElement.Current.IsOffscreen
                } catch { }

                try {
                    $prop = [System.Windows.Automation.AutomationProperty]::LookupById(30107)
                    if ($null -ne $prop) {
                        $desc = $toolbarElement.GetCurrentPropertyValue($prop)
                        if ($null -ne $desc -and $desc -is [string]) {
                            $toolbar_provider_description_length = [int]$desc.Length
                            $toolbar_provider_names_uiautomationcore = [bool]($desc.IndexOf('uiautomationcore', [System.StringComparison]::OrdinalIgnoreCase) -ge 0)
                            $toolbar_provider_names_a_pid_marker = [bool]($desc.IndexOf('pid:', [System.StringComparison]::OrdinalIgnoreCase) -ge 0)
                        }
                    }
                } catch { }

                $toolbarDescendants = $null
                try {
                    $toolbarDescendants = $toolbarElement.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
                    if ($null -ne $toolbarDescendants) {
                        $toolbar_descendant_count = [int]$toolbarDescendants.Count
                    }
                } catch { }

                try {
                    $toolbarChildren = $toolbarElement.FindAll([System.Windows.Automation.TreeScope]::Children, [System.Windows.Automation.Condition]::TrueCondition)
                    if ($null -ne $toolbarChildren) {
                        $toolbar_child_count = [int]$toolbarChildren.Count
                    }
                } catch { }

                # ------------------------------------------------------------ STEP 4 - Toolbar button count
                try {
                    if ($null -ne $toolbarDescendants) {
                        $btnCount = 0
                        foreach ($d in $toolbarDescendants) {
                            try {
                                $dct = $d.Current.ControlType
                                if ($null -ne $dct -and $dct.Id -eq 50000) {
                                    $btnCount++
                                }
                            } catch { }
                        }
                        $toolbar_button_count = [int]$btnCount
                    }
                } catch { }
            }
        } catch { }
    }

    # ---------------------------------------------------------------- CAPTURE
    $capture = [ordered]@{
        label                                   = $Label
        taskbar_resolved                        = $taskbar_resolved
        tray_notify_resolved                    = $tray_notify_resolved
        toolbar_resolved                        = $toolbar_resolved
        toolbar_found_by_enum                   = $toolbar_found_by_enum
        taskbar_descendant_count                = $taskbar_descendant_count
        taskbar_child_count                     = $taskbar_child_count
        taskbar_control_type_id                 = $taskbar_control_type_id
        taskbar_is_offscreen                    = $taskbar_is_offscreen
        taskbar_provider_description_length     = $taskbar_provider_description_length
        taskbar_provider_names_uiautomationcore = $taskbar_provider_names_uiautomationcore
        taskbar_provider_names_a_pid_marker     = $taskbar_provider_names_a_pid_marker
        toolbar_descendant_count                = $toolbar_descendant_count
        toolbar_child_count                     = $toolbar_child_count
        toolbar_control_type_id                 = $toolbar_control_type_id
        toolbar_is_offscreen                    = $toolbar_is_offscreen
        toolbar_provider_description_length     = $toolbar_provider_description_length
        toolbar_provider_names_uiautomationcore = $toolbar_provider_names_uiautomationcore
        toolbar_provider_names_a_pid_marker     = $toolbar_provider_names_a_pid_marker
        toolbar_button_count                    = $toolbar_button_count
    }

    $capturePath = Write-ProbeJson -Probe $Probe -Name "tray-read-diagnosis-$Label.json" -InputObject $capture
    Write-ProbeLog -Message ('wrote ' + (Split-Path -Leaf $capturePath))
    Write-ProbeResult -Probe $Probe -Status 'ok' -Message 'tray read diagnosis measured'
    exit 0
} catch {
    Write-ProbeResult -Probe $Probe -Status 'fail' -Message ('unhandled error: ' + $_.Exception.Message)
    exit 1
}
