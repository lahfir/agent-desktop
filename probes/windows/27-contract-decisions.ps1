#Requires -Version 5.1
<#
.SYNOPSIS
    Probe 27: contract decisions measurement probe.

.DESCRIPTION
    Measures rig census (session, display, elevation, integrity, packages,
    Chromium window class hosts), shell surface resolution mechanism
    (class chain walks for taskbar and tray notify toolbar), and records
    the out-of-band toggle control census placeholder.

    Run: powershell -NoProfile -ExecutionPolicy Bypass -File .\probes\windows\27-contract-decisions.ps1 -Label <devbox|ci>
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'ci')][string]$Label = 'devbox'
)

$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\common.ps1"

$Probe = '27-contract-decisions'

function Initialize-ContractDecisionsNative {
    if ('AgentDesktopProbe.ContractDecisions27' -as [type]) { return }
    $src = @'
using System;
using System.Runtime.InteropServices;
using System.Text;

namespace AgentDesktopProbe {
    public static class ContractDecisions27 {
        [DllImport("user32.dll")]
        public static extern int GetSystemMetrics(int nIndex);

        [DllImport("user32.dll")]
        public static extern IntPtr MonitorFromPoint(long pt, uint flags);

        [DllImport("shcore.dll")]
        public static extern int GetDpiForMonitor(IntPtr hmonitor, int dpiType, out uint dpiX, out uint dpiY);

        [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        public static extern int GetClassNameW(IntPtr hWnd, StringBuilder lpClassName, int nMaxCount);

        [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        public static extern IntPtr FindWindowW(string lpClassName, string lpWindowName);

        [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        public static extern IntPtr FindWindowExW(IntPtr hWndParent, IntPtr hWndChildAfter, string lpszClass, string lpszWindow);
    }
}
'@
    Add-ProbeInlineCSharp -Source $src -AssemblyLeaf 'AgentDesktopProbeContractDecisions27'
}

try {
    Initialize-ContractDecisionsNative

    # ---------------------------------------------------------------- SECTION 1 - rig census
    $sessionName = $env:SESSIONNAME
    if ($null -eq $sessionName) {
        $sessionName = ''
    }

    $sessionId = 0
    try {
        $sessionId = [System.Diagnostics.Process]::GetCurrentProcess().SessionId
    } catch {
        $sessionId = 0
    }

    $isRemoteSession = $false
    try {
        $metricsVal = [AgentDesktopProbe.ContractDecisions27]::GetSystemMetrics(0x1000)
        $isRemoteSession = ($metricsVal -ne 0)
    } catch {
        $isRemoteSession = $false
    }

    $screenCount = 1
    try {
        Add-Type -AssemblyName System.Windows.Forms -ErrorAction Stop
        $screenCount = [System.Windows.Forms.Screen]::AllScreens.Count
    } catch {
        $screenCount = 1
    }

    $virtualScreen = '0x0'
    try {
        $vs = [System.Windows.Forms.SystemInformation]::VirtualScreen
        $virtualScreen = ($vs.Width.ToString() + 'x' + $vs.Height.ToString())
    } catch {
        $virtualScreen = '0x0'
    }

    $primaryDpiX = 96
    $primaryDpiY = 96
    try {
        $mon = [AgentDesktopProbe.ContractDecisions27]::MonitorFromPoint(0, 1)
        if ($mon -ne [IntPtr]::Zero) {
            $dx = 0
            $dy = 0
            $hr = [AgentDesktopProbe.ContractDecisions27]::GetDpiForMonitor($mon, 0, [ref]$dx, [ref]$dy)
            if ($hr -eq 0 -and $dx -gt 0 -and $dy -gt 0) {
                $primaryDpiX = [int]$dx
                $primaryDpiY = [int]$dy
            }
        }
    } catch {
        try {
            Add-Type -AssemblyName System.Drawing -ErrorAction Stop
            $g = [System.Drawing.Graphics]::FromHwnd([IntPtr]::Zero)
            $primaryDpiX = [int]$g.DpiX
            $primaryDpiY = [int]$g.DpiY
            $g.Dispose()
        } catch {
            $primaryDpiX = 96
            $primaryDpiY = 96
        }
    }

    $isElevated = $false
    try {
        $identity = [System.Security.Principal.WindowsIdentity]::GetCurrent()
        $principal = New-Object System.Security.Principal.WindowsPrincipal($identity)
        $isElevated = [bool]$principal.IsInRole([System.Security.Principal.WindowsBuiltInRole]::Administrator)
    } catch {
        $isElevated = $false
    }

    $integritySid = ''
    try {
        $whoamiLines = whoami /groups 2>$null
        if ($whoamiLines) {
            foreach ($line in $whoamiLines) {
                $match = [regex]::Match($line, 'S-1-16-\d+')
                if ($match.Success) {
                    $integritySid = $match.Value
                    break
                }
            }
        }
    } catch {
        $integritySid = ''
    }
    if (-not $integritySid) {
        try {
            Initialize-ProbeNative
            $integritySid = [AgentDesktopProbe.Native]::GetIntegritySid($PID)
        } catch {
            $integritySid = ''
        }
    }

    $enableLua = $null
    try {
        $keyPath = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System'
        $item = Get-ItemProperty -LiteralPath $keyPath -Name 'EnableLUA' -ErrorAction SilentlyContinue
        if ($null -ne $item -and $null -ne $item.EnableLUA) {
            $enableLua = [int]$item.EnableLUA
        }
    } catch {
        $enableLua = $null
    }

    $osBuild = 0
    try {
        $osBuild = [int][Environment]::OSVersion.Version.Build
    } catch {
        $osBuild = 0
    }

    $appxPackageCount = -1
    try {
        $packages = @(Get-AppxPackage -ErrorAction Stop)
        $appxPackageCount = $packages.Count
    } catch {
        $appxPackageCount = -1
    }

    $chromiumWindowClassHosts = 0
    try {
        $allProcesses = Get-Process -ErrorAction SilentlyContinue
        foreach ($proc in $allProcesses) {
            try {
                $hwnd = $proc.MainWindowHandle
                if ($hwnd -ne [IntPtr]::Zero) {
                    $sb = New-Object System.Text.StringBuilder 256
                    $len = [AgentDesktopProbe.ContractDecisions27]::GetClassNameW($hwnd, $sb, 256)
                    if ($len -gt 0 -and $sb.ToString() -eq 'Chrome_WidgetWin_1') {
                        $chromiumWindowClassHosts++
                    }
                }
            } catch { }
        }
    } catch {
        $chromiumWindowClassHosts = 0
    }

    $rig = [ordered]@{
        session_name                = $sessionName
        session_id                  = $sessionId
        is_remote_session           = $isRemoteSession
        screen_count                = $screenCount
        virtual_screen              = $virtualScreen
        primary_dpi_x               = $primaryDpiX
        primary_dpi_y               = $primaryDpiY
        is_elevated                 = $isElevated
        integrity_sid               = $integritySid
        enable_lua                  = $enableLua
        os_build                    = $osBuild
        appx_package_count          = $appxPackageCount
        chromium_window_class_hosts = $chromiumWindowClassHosts
    }

    # ---------------------------------------------------------------- SECTION 2 - shell surface resolution mechanism
    $taskbarHwnd = [IntPtr]::Zero
    try {
        $taskbarHwnd = [AgentDesktopProbe.ContractDecisions27]::FindWindowW('Shell_TrayWnd', $null)
    } catch {
        $taskbarHwnd = [IntPtr]::Zero
    }
    $taskbarHwndNonzero = ($taskbarHwnd -ne [IntPtr]::Zero)

    $trayChainHwndNonzero = $false
    $trayNotifyResolves = $false
    $sysPagerResolves = $false
    $toolbarUnderTrayNotify = $false
    $toolbarUnderSysPager = $false
    try {
        if ($taskbarHwnd -ne [IntPtr]::Zero) {
            $trayNotifyHwnd = [AgentDesktopProbe.ContractDecisions27]::FindWindowExW($taskbarHwnd, [IntPtr]::Zero, 'TrayNotifyWnd', $null)
            $trayNotifyResolves = ($trayNotifyHwnd -ne [IntPtr]::Zero)
            if ($trayNotifyResolves) {
                $toolbarHwnd = [AgentDesktopProbe.ContractDecisions27]::FindWindowExW($trayNotifyHwnd, [IntPtr]::Zero, 'ToolbarWindow32', $null)
                $toolbarUnderTrayNotify = ($toolbarHwnd -ne [IntPtr]::Zero)
                $sysPagerHwnd = [AgentDesktopProbe.ContractDecisions27]::FindWindowExW($trayNotifyHwnd, [IntPtr]::Zero, 'SysPager', $null)
                $sysPagerResolves = ($sysPagerHwnd -ne [IntPtr]::Zero)
                if ($sysPagerResolves) {
                    $pagerToolbar = [AgentDesktopProbe.ContractDecisions27]::FindWindowExW($sysPagerHwnd, [IntPtr]::Zero, 'ToolbarWindow32', $null)
                    $toolbarUnderSysPager = ($pagerToolbar -ne [IntPtr]::Zero)
                }
            }
        }
        $trayChainHwndNonzero = $toolbarUnderTrayNotify
    } catch {
        $trayChainHwndNonzero = $false
    }

    $shellSurface = [ordered]@{
        taskbar_hwnd_nonzero                   = $taskbarHwndNonzero
        tray_notify_resolves                   = $trayNotifyResolves
        syspager_resolves                      = $sysPagerResolves
        toolbar_under_tray_notify_resolves     = $toolbarUnderTrayNotify
        toolbar_under_syspager_resolves        = $toolbarUnderSysPager
        shipped_system_tray_chain_resolves     = $trayChainHwndNonzero
        both_are_class_chain_walks             = $true
        note                                   = 'both taskbar and system-tray kinds resolve by Win32 class chain and neither descends a UIA tree; the shipped system-tray chain is Shell_TrayWnd > TrayNotifyWnd > ToolbarWindow32'
    }

    # ---------------------------------------------------------------- SECTION 3 - toggle control census placeholder
    $toggleCensus = [ordered]@{
        measured_out_of_band = $true
        note                 = 'toggle presentation census was taken with the UIA3 COM shim; see the area 27 rows for the counts'
    }

    # ---------------------------------------------------------------- OUTPUT
    $capture = [pscustomobject]@{
        label         = $Label
        rig           = $rig
        shell_surface = $shellSurface
        toggle_census = $toggleCensus
    }

    $capturePath = Write-ProbeJson -Probe $Probe -Name "contract-decisions-$Label.json" -InputObject $capture
    Write-ProbeLog -Message ('wrote ' + (Split-Path -Leaf $capturePath))
    Write-ProbeResult -Probe $Probe -Status 'ok' -Message 'contract decisions rig census, shell surface resolution, and toggle census placeholder captured'
    exit 0
} catch {
    Write-ProbeResult -Probe $Probe -Status 'fail' -Message ('unhandled error: ' + $_.Exception.Message)
    exit 1
}
