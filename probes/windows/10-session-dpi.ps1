<#
.SYNOPSIS
    Probe 10 (sub-phase 2.0, unit U8): session facts and the DPI-awareness bounds delta.

.DESCRIPTION
    Reads the same probe-launched element's BoundingRectangle from a DPI-aware child
    process and a DPI-unaware child process, and reports the per-edge numeric delta.
    The measurement is taken twice: at the display's recommended scale, and again after
    the probe asks for the next scale step up (125%). The original scale is restored in
    teardown and the restoration is proved by re-reading it back.

    Delta fields are named deltaLeft/deltaTop/deltaWidth/deltaHeight on purpose. The
    KTD9 normalizer buckets any key named exactly x/y/left/top/right/bottom/width/height
    to an 8-pixel bucket to absorb layout jitter; a measured delta must not be bucketed
    away, and "deltaLeft" has no word boundary before "Left", so it survives verbatim
    while the raw rectangles it is derived from are still bucketed.

    Multi-monitor and mixed-DPI behavior is a DEFERRED row: this VM has exactly one
    display. It closes at sub-phase 2.4, which owns list_displays and per-monitor
    scale_factor.
#>
[CmdletBinding()]
param(
    [switch]$Worker,
    [string]$WorkerOut = '',
    [string]$Mode = 'unaware',
    [string]$TargetHwnd = '0',
    [string]$EditHwnd = '0'
)

$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\common.ps1"

$Probe = '10-session-dpi'
$DpiSource = @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

namespace AgentDesktopProbe {
    [StructLayout(LayoutKind.Sequential)]
    public struct DcLuid { public uint LowPart; public int HighPart; }

    [StructLayout(LayoutKind.Sequential)]
    public struct DcHeader { public int type; public uint size; public DcLuid adapterId; public uint id; }

    [StructLayout(LayoutKind.Sequential)]
    public struct DcScaleGet { public DcHeader header; public int minScaleRel; public int curScaleRel; public int maxScaleRel; }

    [StructLayout(LayoutKind.Sequential)]
    public struct DcScaleSet { public DcHeader header; public int scaleRel; }

    [StructLayout(LayoutKind.Sequential, Size = 72)]
    public struct DcPathInfo { public DcLuid srcAdapterId; public uint srcId; }

    [StructLayout(LayoutKind.Sequential, Size = 64)]
    public struct DcModeInfo { public uint infoType; }

    [StructLayout(LayoutKind.Sequential)]
    public struct Rect { public int Left; public int Top; public int Right; public int Bottom; }

    public static class Dpi {
        private delegate bool EnumProc(IntPtr h, IntPtr l);

        [DllImport("user32.dll")] public static extern int GetDisplayConfigBufferSizes(uint flags, out uint numPath, out uint numMode);
        [DllImport("user32.dll")] public static extern int QueryDisplayConfig(uint flags, ref uint numPath, [Out] DcPathInfo[] paths, ref uint numMode, [Out] DcModeInfo[] modes, IntPtr topology);
        [DllImport("user32.dll")] public static extern int DisplayConfigGetDeviceInfo(ref DcScaleGet req);
        [DllImport("user32.dll")] public static extern int DisplayConfigSetDeviceInfo(ref DcScaleSet req);
        [DllImport("user32.dll", SetLastError = true)] public static extern IntPtr SetProcessDpiAwarenessContext(IntPtr context);
        [DllImport("user32.dll")] public static extern int GetSystemMetrics(int index);
        [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out Rect r);
        [DllImport("user32.dll")] public static extern IntPtr MonitorFromPoint(long pt, uint flags);
        [DllImport("user32.dll")] public static extern IntPtr GetDC(IntPtr h);
        [DllImport("user32.dll")] public static extern int ReleaseDC(IntPtr h, IntPtr dc);
        [DllImport("user32.dll")] private static extern bool EnumChildWindows(IntPtr parent, EnumProc cb, IntPtr l);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, StringBuilder b, int max);
        [DllImport("gdi32.dll")] public static extern int GetDeviceCaps(IntPtr dc, int index);
        [DllImport("shcore.dll")] public static extern int GetProcessDpiAwareness(IntPtr proc, out int value);
        [DllImport("shcore.dll")] public static extern int GetDpiForMonitor(IntPtr mon, int type, out uint dx, out uint dy);
        [DllImport("kernel32.dll")] public static extern uint WTSGetActiveConsoleSessionId();
        [DllImport("kernel32.dll")] public static extern uint GetCurrentProcessId();
        [DllImport("kernel32.dll", SetLastError = true)] public static extern bool ProcessIdToSessionId(uint pid, out uint sid);

        public static int LogPixels() {
            IntPtr dc = GetDC(IntPtr.Zero);
            int v = GetDeviceCaps(dc, 88);
            ReleaseDC(IntPtr.Zero, dc);
            return v;
        }

        public static IntPtr FindChildByClass(IntPtr parent, string wanted) {
            IntPtr hit = IntPtr.Zero;
            EnumChildWindows(parent, delegate(IntPtr h, IntPtr l) {
                StringBuilder b = new StringBuilder(256);
                GetClassNameW(h, b, 256);
                if (String.Equals(b.ToString(), wanted, StringComparison.OrdinalIgnoreCase)) { hit = h; return false; }
                return true;
            }, IntPtr.Zero);
            return hit;
        }

        public static bool GetSourceIds(out DcLuid adapterId, out uint sourceId) {
            adapterId = new DcLuid();
            sourceId = 0;
            uint numPath = 0;
            uint numMode = 0;
            if (GetDisplayConfigBufferSizes(2, out numPath, out numMode) != 0) { return false; }
            DcPathInfo[] paths = new DcPathInfo[numPath];
            DcModeInfo[] modes = new DcModeInfo[numMode];
            if (QueryDisplayConfig(2, ref numPath, paths, ref numMode, modes, IntPtr.Zero) != 0) { return false; }
            if (numPath < 1) { return false; }
            adapterId = paths[0].srcAdapterId;
            sourceId = paths[0].srcId;
            return true;
        }

        public static int[] GetScaleRange() {
            DcLuid adapter;
            uint source;
            if (!GetSourceIds(out adapter, out source)) { return new int[] { -999, -999, -999, -999 }; }
            DcScaleGet g = new DcScaleGet();
            g.header.type = -3;
            g.header.size = (uint)Marshal.SizeOf(typeof(DcScaleGet));
            g.header.adapterId = adapter;
            g.header.id = source;
            int rc = DisplayConfigGetDeviceInfo(ref g);
            return new int[] { rc, g.minScaleRel, g.curScaleRel, g.maxScaleRel };
        }

        public static int SetScaleRelative(int rel) {
            DcLuid adapter;
            uint source;
            if (!GetSourceIds(out adapter, out source)) { return -999; }
            DcScaleSet s = new DcScaleSet();
            s.header.type = -4;
            s.header.size = (uint)Marshal.SizeOf(typeof(DcScaleSet));
            s.header.adapterId = adapter;
            s.header.id = source;
            s.scaleRel = rel;
            return DisplayConfigSetDeviceInfo(ref s);
        }
    }
}
'@

function Get-AwarenessName {
    param([int]$Value)
    switch ($Value) {
        0 { return 'PROCESS_DPI_UNAWARE' }
        1 { return 'PROCESS_SYSTEM_DPI_AWARE' }
        2 { return 'PROCESS_PER_MONITOR_DPI_AWARE' }
        default { return ('unknown(' + $Value + ')') }
    }
}

function Invoke-DpiWorker {
    Add-Type -TypeDefinition $DpiSource -Language CSharp
    $row = [ordered]@{ mode = $Mode; processId = [int][AgentDesktopProbe.Dpi]::GetCurrentProcessId() }
    $a = 0
    [void][AgentDesktopProbe.Dpi]::GetProcessDpiAwareness([IntPtr]::Zero, [ref]$a)
    $row['awarenessAtStartup'] = Get-AwarenessName -Value $a
    if ($Mode -eq 'aware') {
        $rc = [AgentDesktopProbe.Dpi]::SetProcessDpiAwarenessContext([IntPtr](-4))
        $row['setProcessDpiAwarenessContextPerMonitorV2'] = if ($rc -ne [IntPtr]::Zero) { 'succeeded' } else { 'failed, error ' + [Runtime.InteropServices.Marshal]::GetLastWin32Error() }
    } else {
        $row['setProcessDpiAwarenessContextPerMonitorV2'] = 'not attempted (unaware arm, forced with __COMPAT_LAYER=DPIUNAWARE)'
    }
    [void][AgentDesktopProbe.Dpi]::GetProcessDpiAwareness([IntPtr]::Zero, [ref]$a)
    $row['awarenessEffective'] = Get-AwarenessName -Value $a
    $dx = 0
    $dy = 0
    $mon = [AgentDesktopProbe.Dpi]::MonitorFromPoint(0, 2)
    [void][AgentDesktopProbe.Dpi]::GetDpiForMonitor($mon, 0, [ref]$dx, [ref]$dy)
    $row['monitorEffectiveDpiX'] = [int]$dx
    $row['monitorEffectiveDpiY'] = [int]$dy
    $row['logPixels'] = [AgentDesktopProbe.Dpi]::LogPixels()
    $row['screenMetricsCx'] = [AgentDesktopProbe.Dpi]::GetSystemMetrics(0)
    $row['screenMetricsCy'] = [AgentDesktopProbe.Dpi]::GetSystemMetrics(1)
    $target = [IntPtr][int64]$TargetHwnd
    $edit = [IntPtr][int64]$EditHwnd
    $r = New-Object AgentDesktopProbe.Rect
    [void][AgentDesktopProbe.Dpi]::GetWindowRect($target, [ref]$r)
    $row['getWindowRect'] = [ordered]@{ Left = $r.Left; Top = $r.Top; Right = $r.Right; Bottom = $r.Bottom }
    Add-Type -AssemblyName UIAutomationClient
    Add-Type -AssemblyName UIAutomationTypes
    foreach ($pair in @(@('window', $target), @('editChild', $edit))) {
        $cell = [ordered]@{}
        try {
            $el = [System.Windows.Automation.AutomationElement]::FromHandle([IntPtr]$pair[1])
            $b = $el.Current.BoundingRectangle
            $cell['className'] = $el.Current.ClassName
            $cell['Left'] = [int]$b.Left
            $cell['Top'] = [int]$b.Top
            $cell['Width'] = [int]$b.Width
            $cell['Height'] = [int]$b.Height
        } catch { $cell['error'] = $_.Exception.Message }
        $row[('uiaBounds_' + $pair[0])] = $cell
    }
    [IO.File]::WriteAllText($WorkerOut, (ConvertTo-Json -InputObject $row -Depth 10), (New-Object System.Text.UTF8Encoding $false))
}

if ($Worker) {
    try { Invoke-DpiWorker } catch {
        [IO.File]::WriteAllText($WorkerOut, (ConvertTo-Json -InputObject ([ordered]@{ mode = $Mode; fatal = $_.Exception.Message }) -Depth 6), (New-Object System.Text.UTF8Encoding $false))
    }
    exit 0
}

$status = 'ok'
$message = ''
$summary = [ordered]@{}
$script:Spawned = New-Object System.Collections.ArrayList
$script:OriginalScaleRel = $null
$script:WorkDir = Join-Path $env:TEMP 'agent-desktop-u8'

function Invoke-DpiArm {
    param([string]$ArmMode, [string]$Tag, [IntPtr]$Target, [IntPtr]$Edit)
    $out = Join-Path $script:WorkDir ('dpi-' + $Tag + '-' + $ArmMode + '.json')
    if (Test-Path -LiteralPath $out) { Remove-Item -LiteralPath $out -Force }
    $argv = @('-NoProfile', '-NonInteractive', '-WindowStyle', 'Hidden', '-ExecutionPolicy', 'Bypass',
        '-File', $PSCommandPath, '-Worker', '-WorkerOut', $out, '-Mode', $ArmMode,
        '-TargetHwnd', ([string]$Target.ToInt64()), '-EditHwnd', ([string]$Edit.ToInt64()))
    if ($ArmMode -eq 'unaware') { $env:__COMPAT_LAYER = 'DPIUNAWARE' } else { $env:__COMPAT_LAYER = $null }
    $proc = Start-Process -FilePath (Join-Path $env:WINDIR 'System32\WindowsPowerShell\v1.0\powershell.exe') -ArgumentList $argv -WindowStyle Hidden -PassThru
    Register-ScratchProcessId -ProcessId $proc.Id
    [void]$script:Spawned.Add($proc.Id)
    $env:__COMPAT_LAYER = $null
    $deadline = (Get-Date).AddSeconds(120)
    while ((Get-Date) -lt $deadline -and -not (Test-Path -LiteralPath $out)) { Start-Sleep -Milliseconds 400 }
    try { Stop-ScratchProcess -ProcessId $proc.Id } catch { }
    if (-not (Test-Path -LiteralPath $out)) { return [ordered]@{ mode = $ArmMode; fatal = 'worker produced no output' } }
    return (Get-Content -LiteralPath $out -Raw | ConvertFrom-Json)
}

function Get-BoundsDelta {
    param($Aware, $Unaware, [string]$Field)
    $a = $Aware.$Field
    $u = $Unaware.$Field
    if ($null -eq $a -or $null -eq $u -or $a.error -or $u.error) { return [ordered]@{ measurable = $false } }
    return [ordered]@{
        measurable  = $true
        deltaLeft   = ([int]$a.Left - [int]$u.Left)
        deltaTop    = ([int]$a.Top - [int]$u.Top)
        deltaWidth  = ([int]$a.Width - [int]$u.Width)
        deltaHeight = ([int]$a.Height - [int]$u.Height)
    }
}

try {
    Initialize-ProbeNative
    Add-Type -TypeDefinition $DpiSource -Language CSharp
    if (-not (Test-Path -LiteralPath $script:WorkDir)) { New-Item -ItemType Directory -Path $script:WorkDir -Force | Out-Null }

    $sid = 0
    [void][AgentDesktopProbe.Dpi]::ProcessIdToSessionId([AgentDesktopProbe.Dpi]::GetCurrentProcessId(), [ref]$sid)
    $session = [ordered]@{
        sessionId               = [int]$sid
        activeConsoleSessionId  = [int][AgentDesktopProbe.Dpi]::WTSGetActiveConsoleSessionId()
        isRemoteSession         = ([AgentDesktopProbe.Dpi]::GetSystemMetrics(0x1000) -ne 0)
        windowStation           = ([System.Environment]::GetEnvironmentVariable('SESSIONNAME'))
        note                    = 'RDP session-transition behavior is not measurable here (physical console session); it closes at sub-phase 2.12 runner registration'
    }

    Add-Type -AssemblyName System.Windows.Forms
    $displays = @()
    foreach ($s in [System.Windows.Forms.Screen]::AllScreens) {
        $displays += [ordered]@{
            primary = $s.Primary
            Left    = $s.Bounds.Left
            Top     = $s.Bounds.Top
            Width   = $s.Bounds.Width
            Height  = $s.Bounds.Height
        }
    }
    $monitorKeys = @(Get-ChildItem -LiteralPath 'HKCU:\Control Panel\Desktop\PerMonitorSettings' -ErrorAction SilentlyContinue | ForEach-Object { $_.PSChildName })
    $range = [AgentDesktopProbe.Dpi]::GetScaleRange()
    $script:OriginalScaleRel = $range[2]
    $scale = [ordered]@{
        displayConfigGetResult = $range[0]
        scaleRelMin            = $range[1]
        scaleRelCurrent        = $range[2]
        scaleRelMax            = $range[3]
        monitorRegistryKeys    = $monitorKeys
        stepsAvailableAboveRecommended = ($range[3] - $range[1])
    }

    $notepad = Start-ScratchProcess -FilePath (Join-Path $env:WINDIR 'System32\notepad.exe') -TimeoutSec 20
    [void]$script:Spawned.Add($notepad.ProcessId)
    if ($notepad.MainWindowHandle -eq [IntPtr]::Zero) { throw 'dpi target notepad window never appeared' }
    $editHandle = [AgentDesktopProbe.Dpi]::FindChildByClass($notepad.MainWindowHandle, 'Edit')
    Start-Sleep -Seconds 1

    $passes = @()
    foreach ($step in @(
            [pscustomobject]@{ Tag = 'recommended'; Rel = $script:OriginalScaleRel; Wanted = 'display recommended scale (100%)' },
            [pscustomobject]@{ Tag = 'plus-one-step'; Rel = ($script:OriginalScaleRel + 1); Wanted = 'one scale step above recommended (125%)' })) {
        $setResult = $null
        if ($step.Rel -ne $script:OriginalScaleRel) {
            $setResult = [AgentDesktopProbe.Dpi]::SetScaleRelative($step.Rel)
            Start-Sleep -Milliseconds 1500
        }
        $after = [AgentDesktopProbe.Dpi]::GetScaleRange()
        $regValue = $null
        if ($monitorKeys.Count -gt 0) {
            $regValue = (Get-ItemProperty -Path ('HKCU:\Control Panel\Desktop\PerMonitorSettings\' + $monitorKeys[0]) -Name 'DpiValue' -ErrorAction SilentlyContinue).DpiValue
        }
        $aware = Invoke-DpiArm -ArmMode 'aware' -Tag $step.Tag -Target $notepad.MainWindowHandle -Edit $editHandle
        $unaware = Invoke-DpiArm -ArmMode 'unaware' -Tag $step.Tag -Target $notepad.MainWindowHandle -Edit $editHandle
        $passes += [ordered]@{
            pass                       = $step.Tag
            requested                  = $step.Wanted
            requestedScaleRel          = $step.Rel
            displayConfigSetResult     = $setResult
            scaleRelReportedAfterwards = $after[2]
            registryDpiValueAfterwards = $regValue
            effectiveDpiSeenByAwareArm = $aware.monitorEffectiveDpiX
            armsMeasured               = ($null -eq $aware.fatal -and $null -eq $unaware.fatal -and $null -ne $aware.monitorEffectiveDpiX -and $null -ne $unaware.monitorEffectiveDpiX)
            scaleActuallyApplied       = ($null -ne $aware.monitorEffectiveDpiX -and $aware.monitorEffectiveDpiX -ne 96)
            awareArm                   = $aware
            unawareArm                 = $unaware
            windowBoundsDelta          = (Get-BoundsDelta -Aware $aware -Unaware $unaware -Field 'uiaBounds_window')
            editChildBoundsDelta       = (Get-BoundsDelta -Aware $aware -Unaware $unaware -Field 'uiaBounds_editChild')
        }
    }

    $restoreResult = [AgentDesktopProbe.Dpi]::SetScaleRelative($script:OriginalScaleRel)
    Start-Sleep -Milliseconds 1500
    $restored = [AgentDesktopProbe.Dpi]::GetScaleRange()
    $restoredReg = $null
    if ($monitorKeys.Count -gt 0) {
        $restoredReg = (Get-ItemProperty -Path ('HKCU:\Control Panel\Desktop\PerMonitorSettings\' + $monitorKeys[0]) -Name 'DpiValue' -ErrorAction SilentlyContinue).DpiValue
    }
    $teardown = [ordered]@{
        originalScaleRel           = $script:OriginalScaleRel
        displayConfigSetResult     = $restoreResult
        scaleRelReadBackAfterwards = $restored[2]
        registryDpiValueAfterwards = $restoredReg
        scaleQueryUsable           = ($range[0] -eq 0 -and $restored[0] -eq 0)
        restored                   = ($range[0] -eq 0 -and $restored[0] -eq 0 -and $restored[2] -eq $script:OriginalScaleRel)
    }

    $capture = [ordered]@{
        question   = 'what bounds delta does DPI awareness produce between two processes reading the same element, and what scale can this environment actually apply'
        stack      = 'managed UIA for BoundingRectangle; raw Win32 for awareness, DisplayConfig and metrics'
        scope      = 'api-contract for the awareness APIs; app/provider and environment-specific for the achievable scale'
        session    = $session
        displays   = $displays
        displayScaleCapability = $scale
        method     = 'two sibling powershell children read the same probe-launched notepad window and its Edit child; the unaware arm is forced with __COMPAT_LAYER=DPIUNAWARE, the aware arm calls SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)'
        normalizationNote = 'raw rectangles are bucketed to 8px by the KTD9 normalizer; delta fields are named deltaLeft/deltaTop/deltaWidth/deltaHeight, which the bucketing regex does not match, so the measured delta survives the normalized twin verbatim'
        passes     = $passes
        teardown   = $teardown
        deferred   = [ordered]@{
            row          = 'multi-monitor and mixed-DPI bounds behavior, and any non-zero aware-vs-unaware delta'
            reason       = 'single display, and that display reports no EDID so Windows offers exactly one scale step (min=cur=max); the 125% request is accepted by DisplayConfigSetDeviceInfo and persisted to the registry but never takes effect'
            closesAt     = 'sub-phase 2.4 (owns list_displays and per-monitor scale_factor) on a runner with a scalable display'
            neverLeaves  = 'Phase 2'
        }
    }
    Write-ProbeJson -Probe $Probe -Name 'session-dpi.json' -InputObject $capture | Out-Null

    $unmeasuredPasses = @($passes | Where-Object { -not $_.armsMeasured })
    if ($unmeasuredPasses.Count -gt 0) {
        throw ('PROBE-HARNESS: refusing to record a DPI verdict - the aware and unaware arms did not both report a measurement in pass(es): ' +
            (($unmeasuredPasses | ForEach-Object { $_.pass }) -join ', '))
    }
    if (-not $teardown.restored) {
        throw ('PROBE-HARNESS: display scale restoration is unverified - DisplayConfigGetDeviceInfo returned ' + $range[0] +
            ' at entry and ' + $restored[0] + ' on read-back, relative scale read back as ' + $restored[2] +
            ' against the original ' + $script:OriginalScaleRel)
    }

    $summary['sessionId'] = $session.sessionId
    $summary['isRemoteSession'] = $session.isRemoteSession
    $summary['displayCount'] = $displays.Count
    $summary['scaleRelMinCurMax'] = ('' + $scale.scaleRelMin + '/' + $scale.scaleRelCurrent + '/' + $scale.scaleRelMax)
    $summary['awarenessPerMonitorV2'] = $passes[0].awareArm.setProcessDpiAwarenessContextPerMonitorV2
    $summary['unawareArmAwareness'] = $passes[0].unawareArm.awarenessEffective
    $summary['awareArmAwareness'] = $passes[0].awareArm.awarenessEffective
    $summary['windowDeltaAtRecommended'] = ('' + $passes[0].windowBoundsDelta.deltaLeft + ',' + $passes[0].windowBoundsDelta.deltaTop + ',' + $passes[0].windowBoundsDelta.deltaWidth + ',' + $passes[0].windowBoundsDelta.deltaHeight)
    $summary['windowDeltaAt125Request'] = ('' + $passes[1].windowBoundsDelta.deltaLeft + ',' + $passes[1].windowBoundsDelta.deltaTop + ',' + $passes[1].windowBoundsDelta.deltaWidth + ',' + $passes[1].windowBoundsDelta.deltaHeight)
    $summary['scaleActuallyAppliedAt125Request'] = $passes[1].scaleActuallyApplied
    $summary['scaleRestored'] = $teardown.restored
    $message = 'session and dpi measured; scale request and restoration both verified by read-back'
} catch {
    $status = 'fail'
    $message = ($_.Exception.Message -replace '[\r\n]+', ' ')
    Write-ProbeLog -Message ('probe failed: ' + $message) -Level 'error'
} finally {
    if ($null -ne $script:OriginalScaleRel) {
        try { [void][AgentDesktopProbe.Dpi]::SetScaleRelative($script:OriginalScaleRel) } catch { }
    }
    $env:__COMPAT_LAYER = $null
    foreach ($id in @($script:Spawned)) {
        try { Stop-ScratchProcess -ProcessId $id } catch { Write-ProbeLog -Message ('teardown: ' + $_.Exception.Message) -Level 'warn' }
    }
    foreach ($f in @(Get-ChildItem -LiteralPath (Get-CaptureDir -Probe $Probe) -File -ErrorAction SilentlyContinue)) {
        if ($f.Name -like '*.normalized') { continue }
        if (-not (Test-CaptureRedaction -Path $f.FullName)) { $status = 'fail'; $message = ('redaction residue in ' + $f.Name) }
    }
}

Write-ProbeResult -Probe $Probe -Status $status -Message $message -Data $summary
if ($status -eq 'fail') { exit 1 }
exit 0
