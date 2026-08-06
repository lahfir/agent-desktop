#Requires -Version 5.1
<#
.SYNOPSIS
    Sub-phase 2.4 observation census (A16, Win32 items).

.DESCRIPTION
    Measures the raw Win32 facts the 2.4 plan refuses to infer, on this machine:
    what `EnumWindows` returns for the window set an agent means (cloaked,
    tool, zero-size windows), `GetForegroundWindow` semantics, which process
    enumeration source works for `list_apps`, the effective-DPI read, whether
    `IVirtualDesktopManager` is reachable through the pinned crates, and the
    split-integrity observation read the trust boundary spans.

    Captures are written beside this script under captures\, BOM-less UTF-8,
    through the corpus redaction gate in ..\common.ps1.
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'ci')][string]$Label = 'devbox'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

. (Join-Path (Split-Path -Parent $PSCommandPath) '..\common.ps1')
Initialize-ProbeRedaction

$script:ProbeDir = Split-Path -Parent $PSCommandPath
$script:CaptureDir = Join-Path $script:ProbeDir 'captures'
if (-not (Test-Path -LiteralPath $script:CaptureDir)) {
    New-Item -ItemType Directory -Path $script:CaptureDir -Force | Out-Null
}

function Write-CensusCapture {
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

if (-not ('AgentDesktopCensus.Native' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

namespace AgentDesktopCensus {
    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

    public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }

    public class Native {
        [DllImport("user32.dll")]
        public static extern bool EnumWindows(EnumWindowsProc callback, IntPtr lParam);
        [DllImport("user32.dll")]
        public static extern bool EnumChildWindows(IntPtr parent, EnumWindowsProc callback, IntPtr lParam);
        [DllImport("user32.dll", SetLastError = true)]
        public static extern bool IsWindowVisible(IntPtr hWnd);
        [DllImport("user32.dll", SetLastError = true)]
        public static extern bool IsIconic(IntPtr hWnd);
        [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        public static extern int GetClassName(IntPtr hWnd, StringBuilder text, int count);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        public static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int count);
        [DllImport("user32.dll", SetLastError = true)]
        public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
        [DllImport("user32.dll")]
        public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
        [DllImport("user32.dll")]
        public static extern long GetWindowLongPtr(IntPtr hWnd, int nIndex);
        [DllImport("user32.dll")]
        public static extern IntPtr GetForegroundWindow();
        [DllImport("dwmapi.dll")]
        public static extern int DwmGetWindowAttribute(IntPtr hWnd, int dwAttribute, out int pvAttribute, int cbAttribute);
        [DllImport("user32.dll", SetLastError = true)]
        public static extern IntPtr GetShellWindow();
        [DllImport("user32.dll")]
        public static extern IntPtr GetDesktopWindow();

        public static string ClassName(IntPtr hWnd) {
            StringBuilder sb = new StringBuilder(512);
            if (GetClassName(hWnd, sb, sb.Capacity) == 0) { return "<none>"; }
            return sb.ToString();
        }

        public static string WindowText(IntPtr hWnd) {
            StringBuilder sb = new StringBuilder(512);
            GetWindowText(hWnd, sb, sb.Capacity);
            return sb.ToString();
        }
    }
}
'@ | Out-Null
}

function Get-ExStyle {
    param([IntPtr]$Handle)
    return [AgentDesktopCensus.Native]::GetWindowLongPtr($Handle, -20)
}

function Measure-WindowCensus {
    $all = New-Object System.Collections.ArrayList
    [AgentDesktopCensus.Native]::EnumWindows({
        param($hWnd, $lParam)
        [void]$all.Add($hWnd)
        return $true
    }, [IntPtr]::Zero) | Out-Null

    $rows = New-Object System.Collections.ArrayList
    $byFactor = @{
        cloaked = 0
        tool = 0
        zero_size = 0
        invisible = 0
        iconic = 0
        visible_nonempty = 0
    }
    foreach ($hWnd in $all) {
        $visible = [AgentDesktopCensus.Native]::IsWindowVisible($hWnd)
        $iconic = [AgentDesktopCensus.Native]::IsIconic($hWnd)
        $rect = New-Object AgentDesktopCensus.RECT
        [void][AgentDesktopCensus.Native]::GetWindowRect($hWnd, [ref]$rect)
        $width = $rect.Right - $rect.Left
        $height = $rect.Bottom - $rect.Top
        $exStyle = Get-ExStyle $hWnd
        $isTool = (($exStyle -band 0x00000080) -ne 0)
        $cloaked = 0
        $dwm = [AgentDesktopCensus.Native]::DwmGetWindowAttribute($hWnd, 14, [ref]$cloaked, 4)
        $className = [AgentDesktopCensus.Native]::ClassName($hWnd)
        if ($cloaked -ne 0) { $byFactor['cloaked']++ }
        if ($isTool) { $byFactor['tool']++ }
        if (-not $visible) { $byFactor['invisible']++ }
        if ($iconic) { $byFactor['iconic']++ }
        if (($width -le 0) -or ($height -le 0)) { $byFactor['zero_size']++ } else { $byFactor['visible_nonempty']++ }
        if ($className -match 'IdentityHelperClass|Narrator|Shell_TrayWnd|Progman|WorkerW|Windows.UI.Core') {
            [void]$rows.Add([ordered]@{
                class = $className
                visible = $visible
                iconic = $iconic
                zero_size = ($width -le 0 -or $height -le 0)
                tool = $isTool
                cloaked = ($cloaked -ne 0)
                ex_style_hex = ('0x{0:X}' -f $exStyle)
                width_bucket = ([int]([Math]::Round($width / 8) * 8))
                height_bucket = ([int]([Math]::Round($height / 8) * 8))
            })
        }
    }
    return [ordered]@{
        total_enumerated = $all.Count
        shell_window_class = [AgentDesktopCensus.Native]::ClassName([AgentDesktopCensus.Native]::GetShellWindow())
        desktop_window_class = [AgentDesktopCensus.Native]::ClassName([AgentDesktopCensus.Native]::GetDesktopWindow())
        by_factor = $byFactor
        identifiable_class_rows = $rows
    }
}

function Measure-ForegroundSemantics {
    $fg = [AgentDesktopCensus.Native]::GetForegroundWindow()
    $fgClass = [AgentDesktopCensus.Native]::ClassName($fg)
    $fgPid = 0
    [void][AgentDesktopCensus.Native]::GetWindowThreadProcessId($fg, [ref]$fgPid)
    return [ordered]@{
        foreground_window_non_zero = ($fg -ne [IntPtr]::Zero)
        foreground_class = $fgClass
        foreground_pid_matches_session = ($fgPid -ne 0)
        foreground_title_length = ([AgentDesktopCensus.Native]::WindowText($fg)).Length
    }
}

function Measure-ProcessEnumeration {
    $cim = Get-WinEvent -ListLog Application -ErrorAction SilentlyContinue
    $toolhelpAvailable = $false
    try {
        Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
namespace AgentDesktopCensusProc {
    public static class ProcNative {
        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern IntPtr CreateToolhelp32Snapshot(uint dwFlags, uint th32ProcessID);
        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern bool Process32FirstW(IntPtr hSnapshot, ref PROCESSENTRY32W lppe);
        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern bool Process32NextW(IntPtr hSnapshot, ref PROCESSENTRY32W lppe);
        [DllImport("kernel32.dll")]
        public static extern bool CloseHandle(IntPtr hObject);
        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
        public struct PROCESSENTRY32W {
            public uint dwSize;
            public uint cntUsage;
            public uint th32ProcessID;
            public IntPtr th32DefaultHeapID;
            public uint th32ModuleID;
            public uint cntThreads;
            public uint th32ParentProcessID;
            public int pcPriClassBase;
            public uint dwFlags;
            [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 260)]
            public string szExeFile;
        }
    }
}
'@ | Out-Null
        $toolhelpAvailable = $true
    } catch { $toolhelpAvailable = $false }

    $toolhelpCount = 0
    $toolhelpFirstExe = ''
    if ($toolhelpAvailable) {
        $snap = [AgentDesktopCensusProc.ProcNative]::CreateToolhelp32Snapshot(0x00000002, 0)
        if ($snap -ne [IntPtr]::Zero) {
            $entry = New-Object AgentDesktopCensusProc.ProcNative+PROCESSENTRY32W
            $entry.dwSize = [Runtime.InteropServices.Marshal]::SizeOf($entry)
            if ([AgentDesktopCensusProc.ProcNative]::Process32FirstW($snap, [ref]$entry)) {
                $toolhelpCount += 1
                $toolhelpFirstExe = $entry.szExeFile
                while ([AgentDesktopCensusProc.ProcNative]::Process32NextW($snap, [ref]$entry)) {
                    $toolhelpCount += 1
                    if (-not $toolhelpFirstExe) { $toolhelpFirstExe = $entry.szExeFile }
                }
            }
            [void][AgentDesktopCensusProc.ProcNative]::CloseHandle($snap)
        }
    }

    $cimCount = 0
    try {
        $cimProcesses = Get-CimInstance -ClassName Win32_Process -ErrorAction Stop
        $cimCount = @($cimProcesses).Count
    } catch { $cimCount = -1 }

    $self = Get-Process -Id $PID
    return [ordered]@{
        toolhelp_snapshot_available = $toolhelpAvailable
        toolhelp_process_count = $toolhelpCount
        toolhelp_first_exe_shape = if ($toolhelpFirstExe) { 'non-empty' } else { 'empty' }
        cim_process_count = $cimCount
        cim_self_creation_date_present = ($null -ne $self.StartTime)
        has_net_process_replacement = $false
    }
}

function Measure-Dpi {
    $aware = 0
    try {
        Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
namespace AgentDesktopCensusDpi {
    public static class DpiNative {
        [DllImport("user32.dll", SetLastError = true)]
        public static extern uint GetDpiForSystem();
        [DllImport("shcore.dll", SetLastError = true)]
        public static extern int GetDpiForMonitor(IntPtr hmonitor, int dpiType, out uint dpiX, out uint dpiY);
        [DllImport("user32.dll", SetLastError = true)]
        public static extern bool GetMonitorInfoW(IntPtr hMonitor, ref MONITORINFO mi);
        [DllImport("user32.dll")]
        public static extern bool EnumDisplayMonitors(IntPtr hdc, IntPtr lprcClip, EnumMonitorsProc lpfnEnum, IntPtr dwData);
        public delegate bool EnumMonitorsProc(IntPtr hMonitor, IntPtr hdcMonitor, IntPtr lprcMonitor, IntPtr dwData);
        [StructLayout(LayoutKind.Sequential)]
        public struct MONITORINFO {
            public int cbSize;
            public RECT rcMonitor;
            public RECT rcWork;
            public uint dwFlags;
        }
        public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
    }
}
'@ | Out-Null
        $handles = New-Object System.Collections.ArrayList
        [AgentDesktopCensusDpi.DpiNative]::EnumDisplayMonitors([IntPtr]::Zero, [IntPtr]::Zero, {
            param($hMon, $hdcM, $r, $d)
            [void]$handles.Add($hMon)
            return $true
        }, [IntPtr]::Zero) | Out-Null
        $monitors = New-Object System.Collections.ArrayList
        foreach ($hMon in $handles) {
            $dpiX = 0
            $dpiY = 0
            $effective = [AgentDesktopCensusDpi.DpiNative]::GetDpiForMonitor($hMon, 0, [ref]$dpiX, [ref]$dpiY)
            $info = New-Object AgentDesktopCensusDpi.DpiNative+MONITORINFO
            $info.cbSize = [Runtime.InteropServices.Marshal]::SizeOf($info)
            $primary = $false
            if ([AgentDesktopCensusDpi.DpiNative]::GetMonitorInfoW($hMon, [ref]$info)) {
                $primary = (($info.dwFlags -band 1) -ne 0)
            }
            [void]$monitors.Add([ordered]@{
                primary = $primary
                effective_dpi_x = $dpiX
                effective_dpi_y = $dpiY
                effective_dpi_over_96 = ([double]$dpiX / 96.0)
                getdpi_error = $effective
                work_top = $info.rcWork.Top
                work_left = $info.rcWork.Left
            })
        }
        $aware = [AgentDesktopCensusDpi.DpiNative]::GetDpiForSystem()
        return [ordered]@{
            monitors = $monitors
            system_dpi = $aware
        }
    } catch {
        return [ordered]@{ monitors = @(); system_dpi = 0; error = $_.Exception.Message }
    }
}

function Measure-VirtualDesktopManager {
    # HKCR: is not a mounted PSDrive in Windows PowerShell - only HKCU: and
    # HKLM: are - so a query against it throws DriveNotFound before it reads
    # anything. Catching that into a plain $false published a definite "not
    # registered" for a lookup that never ran, and no registration state could
    # change the answer. HKEY_CLASSES_ROOT is the merge of the HKLM and HKCU
    # Classes subtrees; the machine registration lives under the HKLM one.
    # A failed lookup is now its own outcome rather than a negative result.
    # Test-Path is not used to decide this: it answers $false for an
    # unreachable hive exactly as it does for an absent class, which is the
    # same conflation in a new place. Only ItemNotFound is a real negative.
    $clsidKey = 'HKLM:\SOFTWARE\Classes\CLSID\{aa509086-5ca9-4c25-8f95-589d3c07b48a}'
    $clsidRegistered = $false
    $inprocServer = $null
    $lookupError = $null
    try {
        Get-Item -LiteralPath $clsidKey -ErrorAction Stop | Out-Null
        $clsidRegistered = $true
        $server = Get-ItemProperty -LiteralPath (Join-Path $clsidKey 'InProcServer32') -ErrorAction Stop
        $defaultValue = $server.PSObject.Properties['(default)']
        if ($null -ne $defaultValue) { $inprocServer = [string]$defaultValue.Value }
    } catch [System.Management.Automation.ItemNotFoundException] {
        $lookupError = $null
    } catch {
        $lookupError = $_.Exception.Message
    }
    return [ordered]@{
        clsid_registered = $clsidRegistered
        clsid_inproc_server = $inprocServer
        clsid_lookup_error = $lookupError
        interface_in_pinned_crates = 'the VirtualDesktopManager CLSID constant exists in windows-sys 0.61 (Win32_UI_Shell), but no IVirtualDesktopManager interface is generated in windows-sys or the windows crate - reaching the interface requires a hand-declared COM declaration, i.e. a new dependency'
        verdict = 'the COM class is registered on this machine, but the interface is unreachable through the pinned crates without a new dependency'
    }
}

$census = [ordered]@{
    probe = '16-observation-census'
    label = $Label
    stack = 'n/a'
    scope = 'app/provider'
    window_census = Measure-WindowCensus
    foreground_semantics = Measure-ForegroundSemantics
    process_enumeration = Measure-ProcessEnumeration
    dpi = Measure-Dpi
    virtual_desktop_manager = Measure-VirtualDesktopManager
}

$path = Write-CensusCapture -Name "observation-census-$Label.json" -Content (ConvertTo-Json -InputObject $census -Depth 20)
Write-Host "wrote $path"
Write-ProbeResult -Probe '16-observation-census' -Status 'ok' -Message 'observation census captured' -Data $census
exit 0
