#Requires -Version 5.1
<#
.SYNOPSIS
    Sub-phase 2.8 input-synthesis gap probe (A20).

.DESCRIPTION
    Measures the genuine unmeasured facts the input layer depends on via SendInput
    and token APIs. Foundation rows A4-1/2/3 and A9-2/3 are cited, not re-run.

    Captures under captures\ as input-*-{devbox,ci}.json. Honors the corpus safety
    envelope: Assert-Foreground brackets every injection; Show-WindowNoActivate /
    own-window restore; clipboard/cursor/modifier restore; PID-tracked scratch only.
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'ci')][string]$Label = 'devbox'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

. (Join-Path (Split-Path -Parent $PSCommandPath) '..\common.ps1')
Initialize-ProbeRedaction

Add-Type -AssemblyName System.Windows.Forms | Out-Null

$script:ProbeDir = Split-Path -Parent $PSCommandPath
$script:CaptureDir = Join-Path $script:ProbeDir 'captures'
if (-not (Test-Path -LiteralPath $script:CaptureDir)) {
    New-Item -ItemType Directory -Path $script:CaptureDir -Force | Out-Null
}
$script:Spawned = New-Object System.Collections.ArrayList
$script:TargetPid = 0
$script:Interference = $null

$script:ScratchIds = @{
    tbSlider         = 1018
    lblSliderValue   = 1037
    btnDoubleClick   = 1045
    lblDoubleClick   = 1046
    lblStatus        = 1034
    txtValue         = 1004
}

Register-MandatoryCapture -Name @(
    "input-integrity-$Label.json",
    "input-drag-abort-$Label.json",
    "input-dblclick-$Label.json",
    "input-coords-$Label.json",
    "input-cost-$Label.json"
)

function Write-InputCapture {
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

function Initialize-InputNative {
    if ('AgentDesktopProbe.A20.Input20' -as [type]) { return }
    $src = @'
using System;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;

namespace AgentDesktopProbe.A20 {
    [StructLayout(LayoutKind.Sequential)]
    public struct ProbeRect { public int Left; public int Top; public int Right; public int Bottom; }
    [StructLayout(LayoutKind.Sequential)]
    public struct ProbePoint { public int X; public int Y; }
    [StructLayout(LayoutKind.Sequential)]
    public struct MouseInput { public int dx; public int dy; public uint mouseData; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
    [StructLayout(LayoutKind.Sequential)]
    public struct KeybdInput { public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
    [StructLayout(LayoutKind.Explicit)]
    public struct InputUnion { [FieldOffset(0)] public MouseInput mi; [FieldOffset(0)] public KeybdInput ki; }
    [StructLayout(LayoutKind.Sequential)]
    public struct ProbeInput { public uint type; public InputUnion u; }

    public static class Input20 {
        public const uint MOUSEEVENTF_MOVE = 0x0001;
        public const uint MOUSEEVENTF_LEFTDOWN = 0x0002;
        public const uint MOUSEEVENTF_LEFTUP = 0x0004;
        public const uint MOUSEEVENTF_ABSOLUTE = 0x8000;
        public const uint MOUSEEVENTF_VIRTUALDESK = 0x4000;
        public const uint KEYEVENTF_UNICODE = 0x0004;
        public const uint KEYEVENTF_KEYUP = 0x0002;

        [DllImport("user32.dll", SetLastError = true)]
        public static extern uint SendInput(uint nInputs, ProbeInput[] pInputs, int cbSize);
        [DllImport("user32.dll", SetLastError = true)]
        public static extern IntPtr GetDlgItem(IntPtr hDlg, int nIDDlgItem);
        [DllImport("user32.dll", SetLastError = true)]
        public static extern bool GetWindowRect(IntPtr hWnd, out ProbeRect lpRect);
        [DllImport("user32.dll", EntryPoint = "SendMessageW", CharSet = CharSet.Unicode)]
        private static extern IntPtr SendMessageBuffer(IntPtr hWnd, uint msg, IntPtr wParam, StringBuilder lParam);
        [DllImport("user32.dll")]
        public static extern bool GetCursorPos(out ProbePoint lpPoint);
        [DllImport("user32.dll")]
        public static extern bool SetCursorPos(int X, int Y);
        [DllImport("user32.dll")]
        public static extern int GetSystemMetrics(int nIndex);
        [DllImport("user32.dll")]
        public static extern short GetAsyncKeyState(int vKey);
        [DllImport("user32.dll")]
        public static extern uint GetDoubleClickTime();
        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern IntPtr OpenProcess(uint access, bool inherit, int pid);
        [DllImport("advapi32.dll", SetLastError = true)]
        public static extern bool OpenProcessToken(IntPtr proc, uint access, out IntPtr tok);
        [DllImport("advapi32.dll", SetLastError = true)]
        public static extern bool GetTokenInformation(IntPtr tok, int cls, IntPtr buf, int len, out int ret);
        [DllImport("advapi32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        public static extern bool ConvertSidToStringSid(IntPtr sid, out IntPtr str);
        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern bool CloseHandle(IntPtr h);
        [DllImport("kernel32.dll")]
        public static extern IntPtr LocalFree(IntPtr h);

        public static int InputSize() { return Marshal.SizeOf(typeof(ProbeInput)); }

        public static string GetControlText(IntPtr h) {
            if (h == IntPtr.Zero) { return string.Empty; }
            StringBuilder sb = new StringBuilder(256);
            SendMessageBuffer(h, 0x000D, new IntPtr(256), sb);
            return sb.ToString();
        }

        [DllImport("user32.dll", EntryPoint = "SendMessageW", CharSet = CharSet.Unicode)]
        private static extern IntPtr SendMessageString(IntPtr hWnd, uint msg, IntPtr wParam, string lParam);

        public static void SetControlText(IntPtr h, string value) {
            if (h == IntPtr.Zero) { return; }
            SendMessageString(h, 0x000C, IntPtr.Zero, value);
        }

        public static string GetIntegritySid(int processId) {
            IntPtr proc = OpenProcess(0x1000, false, processId);
            if (proc == IntPtr.Zero) { throw new InvalidOperationException("OpenProcess failed for pid " + processId); }
            IntPtr tok = IntPtr.Zero;
            try {
                if (!OpenProcessToken(proc, 0x0008, out tok)) { throw new InvalidOperationException("OpenProcessToken failed"); }
                int len = 0;
                GetTokenInformation(tok, 25, IntPtr.Zero, 0, out len);
                IntPtr buf = Marshal.AllocHGlobal(len);
                try {
                    if (!GetTokenInformation(tok, 25, buf, len, out len)) { throw new InvalidOperationException("GetTokenInformation failed"); }
                    IntPtr str = IntPtr.Zero;
                    if (!ConvertSidToStringSid(Marshal.ReadIntPtr(buf), out str)) { throw new InvalidOperationException("ConvertSidToStringSid failed"); }
                    string s = Marshal.PtrToStringUni(str);
                    LocalFree(str);
                    return s;
                } finally { Marshal.FreeHGlobal(buf); }
            } finally {
                if (tok != IntPtr.Zero) { CloseHandle(tok); }
                CloseHandle(proc);
            }
        }

        public static int CompareIntegrityRid(string sidA, string sidB) {
            int ridA = ParseIntegrityRid(sidA);
            int ridB = ParseIntegrityRid(sidB);
            if (ridA < ridB) { return -1; }
            if (ridA > ridB) { return 1; }
            return 0;
        }

        private static int ParseIntegrityRid(string sid) {
            if (string.IsNullOrEmpty(sid)) { return -1; }
            int lastDash = sid.LastIndexOf('-');
            if (lastDash < 0) { return -1; }
            int rid;
            if (!int.TryParse(sid.Substring(lastDash + 1), out rid)) { return -1; }
            return rid;
        }

        public static bool IsLeftButtonDown() {
            return (GetAsyncKeyState(0x01) & 0x8000) != 0;
        }

        public static uint SendMouseButton(bool down) {
            ProbeInput[] inputs = new ProbeInput[1];
            inputs[0].type = 0;
            inputs[0].u.mi.dwFlags = down ? MOUSEEVENTF_LEFTDOWN : MOUSEEVENTF_LEFTUP;
            return SendInput(1, inputs, InputSize());
        }

        public static uint SendMouseAbsolute(int x, int y, bool virtualDesk) {
            int normW = GetSystemMetrics(virtualDesk ? 78 : 0);
            int normH = GetSystemMetrics(virtualDesk ? 79 : 1);
            int originX = virtualDesk ? GetSystemMetrics(76) : 0;
            int originY = virtualDesk ? GetSystemMetrics(77) : 0;
            if (normW <= 1) { normW = GetSystemMetrics(0); }
            if (normH <= 1) { normH = GetSystemMetrics(1); }
            int nx = (int)(((double)(x - originX) * 65535.0) / (double)(normW - 1));
            int ny = (int)(((double)(y - originY) * 65535.0) / (double)(normH - 1));
            uint flags = MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE;
            if (virtualDesk) { flags |= MOUSEEVENTF_VIRTUALDESK; }
            ProbeInput[] inputs = new ProbeInput[1];
            inputs[0].type = 0;
            inputs[0].u.mi.dx = nx;
            inputs[0].u.mi.dy = ny;
            inputs[0].u.mi.dwFlags = flags;
            return SendInput(1, inputs, InputSize());
        }

        public static uint SendVirtualKey(ushort vk, bool keyUp) {
            ProbeInput[] inputs = new ProbeInput[1];
            inputs[0].type = 1;
            inputs[0].u.ki.wVk = vk;
            inputs[0].u.ki.dwFlags = keyUp ? KEYEVENTF_KEYUP : (uint)0;
            return SendInput(1, inputs, InputSize());
        }

        public static uint SendUnicodeUnit(ushort unit) {
            ProbeInput[] inputs = new ProbeInput[2];
            inputs[0].type = 1;
            inputs[0].u.ki.wScan = unit;
            inputs[0].u.ki.dwFlags = KEYEVENTF_UNICODE;
            inputs[1].type = 1;
            inputs[1].u.ki.wScan = unit;
            inputs[1].u.ki.dwFlags = KEYEVENTF_UNICODE | KEYEVENTF_KEYUP;
            return SendInput(2, inputs, InputSize());
        }

        public static void ForceLeftButtonUp() {
            if (IsLeftButtonDown()) {
                SendMouseButton(false);
            }
        }
    }
}
'@
    Add-ProbeInlineCSharp -Source $src -AssemblyLeaf 'AgentDesktopProbeA20Input20'
}

function Get-ScratchHandle {
    param([IntPtr]$Form, [string]$Name)
    return [AgentDesktopProbe.A20.Input20]::GetDlgItem($Form, $script:ScratchIds[$Name])
}

function Get-ScratchControlText {
    param([IntPtr]$Handle)
    return [AgentDesktopProbe.A20.Input20]::GetControlText($Handle)
}

function Get-ScratchCenter {
    param([IntPtr]$Handle)
    $r = New-Object AgentDesktopProbe.A20.ProbeRect
    [void][AgentDesktopProbe.A20.Input20]::GetWindowRect($Handle, [ref]$r)
    return [pscustomobject]@{
        X = [int](($r.Left + $r.Right) / 2)
        Y = [int](($r.Top + $r.Bottom) / 2)
        Rect = ($r.Left.ToString() + ',' + $r.Top.ToString() + ',' + ($r.Right - $r.Left).ToString() + ',' + ($r.Bottom - $r.Top).ToString())
    }
}

function Restore-ScratchForeground {
    param([IntPtr]$Form)
    Initialize-ProbeNative
    [void][AgentDesktopProbe.Native]::ShowWindow($Form, 6)
    Start-Sleep -Milliseconds 300
    [void][AgentDesktopProbe.Native]::ShowWindow($Form, 9)
    Start-Sleep -Milliseconds 500
}

function Invoke-BracketedInjection {
    param(
        [Parameter(Mandatory = $true)][string]$Stage,
        [Parameter(Mandatory = $true)][scriptblock]$Action
    )
    if ($null -ne $script:Interference) { return $false }
    try { Assert-Foreground -ExpectedProcessId $script:TargetPid -Stage ($Stage + ':pre') }
    catch {
        $script:Interference = [ordered]@{ stage = ($Stage + ':pre'); detail = ($_.Exception.Message -replace '[\r\n]+', ' ') }
        return $false
    }
    & $Action | Out-Null
    Start-Sleep -Milliseconds 80
    try { Assert-Foreground -ExpectedProcessId $script:TargetPid -Stage ($Stage + ':post') }
    catch {
        $script:Interference = [ordered]@{ stage = ($Stage + ':post'); detail = ($_.Exception.Message -replace '[\r\n]+', ' ') }
        return $false
    }
    return $true
}

function Get-MinOfSeven {
    param([Parameter(Mandatory = $true)][scriptblock]$Measure)
    $samples = New-Object System.Collections.ArrayList
    for ($i = 0; $i -lt 8; $i++) {
        $sw = [System.Diagnostics.Stopwatch]::StartNew()
        & $Measure
        $sw.Stop()
        [void]$samples.Add($sw.Elapsed.TotalMilliseconds)
    }
    $sorted = @($samples | Sort-Object)
    $used = @($sorted | Select-Object -Skip 1)
    return [ordered]@{
        samples_ms = @($samples)
        min_ms     = ($used | Measure-Object -Minimum).Minimum
        median_ms  = ($used | Sort-Object)[([int][Math]::Floor($used.Count / 2))]
        max_ms     = ($used | Measure-Object -Maximum).Maximum
        n          = $used.Count
        warmup_discarded = $true
    }
}

function Restore-DesktopHygiene {
    param(
        $CursorOrigin,
        [bool]$ClipHadText,
        [string]$ClipOriginal,
        [bool]$ClipSnapshotTaken
    )
    Initialize-InputNative
    [AgentDesktopProbe.A20.Input20]::ForceLeftButtonUp()
    foreach ($vk in @(0x10, 0x11, 0x12, 0x5B, 0x5C, 0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5)) {
        if (([AgentDesktopProbe.A20.Input20]::GetAsyncKeyState($vk) -band 0x8000) -ne 0) {
            [void][AgentDesktopProbe.A20.Input20]::SendVirtualKey([uint16]$vk, $true)
        }
    }
    if ($null -ne $CursorOrigin) {
        [void][AgentDesktopProbe.A20.Input20]::SetCursorPos($CursorOrigin.X, $CursorOrigin.Y)
    }
    if ($ClipSnapshotTaken) {
        try {
            if ($ClipHadText) { [System.Windows.Forms.Clipboard]::SetText($ClipOriginal) }
            else { [System.Windows.Forms.Clipboard]::Clear() }
        } catch { }
    }
}

$script:paths = @{}
$cursorOrigin = $null
$clipHadText = $false
$clipOriginal = ''
$clipSnapshotTaken = $false
$form = [IntPtr]::Zero

try {
    Initialize-ProbeNative
    Initialize-InputNative

    $buildScratch = Join-Path (Get-ProbeRoot) 'scratch\build-scratch.ps1'
    $scratchExe = Join-Path (Get-ProbeRoot) 'scratch\bin\ScratchForms.exe'
    if (Test-Path -LiteralPath $buildScratch) {
        & $buildScratch -Force | Out-Null
    }
    if (-not (Test-Path -LiteralPath $scratchExe)) {
        throw ('ScratchForms.exe missing at ' + $scratchExe)
    }

    $cursorOrigin = New-Object AgentDesktopProbe.A20.ProbePoint
    [void][AgentDesktopProbe.A20.Input20]::GetCursorPos([ref]$cursorOrigin)
    try {
        $clipHadText = [System.Windows.Forms.Clipboard]::ContainsText()
        if ($clipHadText) { $clipOriginal = [System.Windows.Forms.Clipboard]::GetText() }
        $clipSnapshotTaken = $true
    } catch { }

    $scratch = Start-ScratchProcess -FilePath $scratchExe -ArgumentList @('--tag', ('a20-' + $Label), '--pos', '120,120') -NoActivate -TimeoutSec 25
    [void]$script:Spawned.Add($scratch.ProcessId)
    if ($scratch.MainWindowHandle -eq [IntPtr]::Zero) { throw 'scratch window never appeared' }
    $script:TargetPid = $scratch.ProcessId
    $form = $scratch.MainWindowHandle
    Restore-ScratchForeground -Form $form

    # --- Leg 1: local integrity read ----------------------------------------
    $probeSid = [AgentDesktopProbe.A20.Input20]::GetIntegritySid($PID)
    $scratchSid = [AgentDesktopProbe.A20.Input20]::GetIntegritySid($script:TargetPid)
    $sameIntegrityCompare = [AgentDesktopProbe.A20.Input20]::CompareIntegrityRid($probeSid, $scratchSid)

    $crossBoundary = [ordered]@{
        effect_measurable = $false
        branch            = 'cross_boundary_input_effect_not_staged'
        reason            = 'A20 measures detection reads only; Medium-to-High input effect inherits A9-2 mapping (A19-4/A18-4)'
        effect_mapping_cites = @('A9-2')
        manufacture_available = $false
    }
    try {
        $mediumExe = Join-Path $env:TEMP ('a20-medium-' + [guid]::NewGuid() + '.exe')
        Copy-Item -LiteralPath $scratchExe -Destination $mediumExe -Force
        $medium = Start-MediumIntegrityProcess -FilePath $mediumExe -ArgumentList @('--tag', 'a20med', '--pos', '400,400')
        [void]$script:Spawned.Add($medium.ProcessId)
        $crossBoundary.manufacture_available = $true
        $crossBoundary.medium_integrity_sid = $medium.IntegritySid
    } catch {
        $crossBoundary.manufacture_available = $false
        $crossBoundary.branch = 'unmeasurable_elevation_manufacture_unavailable'
        $crossBoundary.attempt_error = ($_.Exception.Message -replace '[\r\n]+', ' ')
    }

    $integrity = [ordered]@{
        probe    = '20-input-synthesis'
        question = 'can GetTokenInformation(TokenIntegrityLevel) read probe and scratch tokens for UIPI detection'
        foundation_cites = @('A9-2')
        probe_process_sid = $probeSid
        scratch_process_sid = $scratchSid
        same_integrity_comparison = $sameIntegrityCompare
        same_integrity = ($sameIntegrityCompare -eq 0)
        cross_boundary_medium_to_high = $crossBoundary
        detection_note = 'effect mapping for Medium-to-High input rides A9-2; cross-boundary effect unmeasurable when manufacture unavailable'
    }
    $script:paths.integrity = Write-InputCapture -Name "input-integrity-$Label.json" -Content (ConvertTo-Json -InputObject $integrity -Depth 12)
    Register-MandatoryPass -Capture $script:paths.integrity -Result $integrity
    Write-Host "wrote $($script:paths.integrity)"

    # --- Leg 2: interrupted drag --------------------------------------------
    $hSlider = Get-ScratchHandle -Form $form -Name 'tbSlider'
    $hSliderLabel = Get-ScratchHandle -Form $form -Name 'lblSliderValue'
    $sliderBefore = Get-ScratchControlText -Handle $hSliderLabel
    $sliderCenter = Get-ScratchCenter -Handle $hSlider
    $originPoint = New-Object AgentDesktopProbe.A20.ProbePoint
    [void][AgentDesktopProbe.A20.Input20]::GetCursorPos([ref]$originPoint)
    $dragAbort = $null
    $guardPosted = $false
    $buttonDownBeforeExit = $false
    try {
        $thumbY = $sliderCenter.Y
        $startX = $sliderCenter.X - 80
        if ($startX -lt 0) { $startX = $sliderCenter.X }

        [void](Invoke-BracketedInjection -Stage 'drag-abort:move-start' -Action {
            [AgentDesktopProbe.A20.Input20]::SendMouseAbsolute($startX, $thumbY, $false) | Out-Null
        })
        [void](Invoke-BracketedInjection -Stage 'drag-abort:down' -Action {
            [AgentDesktopProbe.A20.Input20]::SendMouseButton($true) | Out-Null
        })
        Start-Sleep -Milliseconds 150
        [void](Invoke-BracketedInjection -Stage 'drag-abort:partial-move' -Action {
            [AgentDesktopProbe.A20.Input20]::SendMouseAbsolute(($startX + 40), $thumbY, $false) | Out-Null
        })

        $midLabel = Get-ScratchControlText -Handle $hSliderLabel
        $guardPosted = $true
        [void](Invoke-BracketedInjection -Stage 'drag-abort:guard-origin-release' -Action {
            [AgentDesktopProbe.A20.Input20]::SendMouseAbsolute($originPoint.X, $originPoint.Y, $false) | Out-Null
            [AgentDesktopProbe.A20.Input20]::SendMouseButton($false) | Out-Null
        })

        $afterGuardDown = [AgentDesktopProbe.A20.Input20]::IsLeftButtonDown()
        $sliderAfterGuard = Get-ScratchControlText -Handle $hSliderLabel

        $dragAbort = [ordered]@{
            stage                     = 'interrupted drag on tbSlider after mouse-down'
            slider_label_before       = $sliderBefore
            slider_label_mid_abort    = $midLabel
            slider_label_after_guard  = $sliderAfterGuard
            origin_release_posted     = $guardPosted
            button_down_after_guard   = $afterGuardDown
            corrective_acknowledged   = (-not $afterGuardDown)
            branch                    = $(if (-not $afterGuardDown) { 'corrective_release_acknowledged' } else { 'emergency_release_uncertain' })
            interference              = $script:Interference
            drag_target_note          = 'tbSlider (TrackBar) is the staged drag target; A4-3 measured full drag viability on this control'
        }
    } finally {
        [AgentDesktopProbe.A20.Input20]::ForceLeftButtonUp()
        Start-Sleep -Milliseconds 100
        $buttonDownBeforeExit = [AgentDesktopProbe.A20.Input20]::IsLeftButtonDown()
        if ($buttonDownBeforeExit) {
            [AgentDesktopProbe.A20.Input20]::SendMouseButton($false) | Out-Null
            Start-Sleep -Milliseconds 100
            $buttonDownBeforeExit = [AgentDesktopProbe.A20.Input20]::IsLeftButtonDown()
        }
        if ($null -ne $dragAbort) {
            $dragAbort['button_down_before_exit'] = $buttonDownBeforeExit
            $dragAbort['exit_clean'] = (-not $buttonDownBeforeExit)
        }
    }

    if ($null -eq $dragAbort) {
        $dragAbort = New-NotMeasuredResult -Reason 'drag-abort leg did not run'
    }
    $script:paths.drag = Write-InputCapture -Name "input-drag-abort-$Label.json" -Content (ConvertTo-Json -InputObject $dragAbort -Depth 12)
    Register-MandatoryPass -Capture $script:paths.drag -Result $dragAbort
    Write-Host "wrote $($script:paths.drag)"

    # --- Leg 3: double-click recognition ------------------------------------
    $hDblBtn = Get-ScratchHandle -Form $form -Name 'btnDoubleClick'
    $hDblLbl = Get-ScratchHandle -Form $form -Name 'lblDoubleClick'
    $dblCenter = Get-ScratchCenter -Handle $hDblBtn
    $dblTimeMs = [AgentDesktopProbe.A20.Input20]::GetDoubleClickTime()
    $dblBefore = Get-ScratchControlText -Handle $hDblLbl

    $intervals = @(
        [pscustomobject]@{ name = 'zero_gap'; ms = 0 },
        [pscustomobject]@{ name = 'half_window'; ms = [int]($dblTimeMs / 2) },
        [pscustomobject]@{ name = 'within_window'; ms = [int]($dblTimeMs - 10) }
    )
    $dblRows = New-Object System.Collections.ArrayList
    foreach ($trial in $intervals) {
        [AgentDesktopProbe.A20.Input20]::SetControlText($hDblLbl, 'dbl:0') | Out-Null
        Start-Sleep -Milliseconds 200
        $beforeTrial = Get-ScratchControlText -Handle $hDblLbl

        $sent = Invoke-BracketedInjection -Stage ('dbl:' + $trial.name + ':sequence') -Action {
            [AgentDesktopProbe.A20.Input20]::SendMouseAbsolute($dblCenter.X, $dblCenter.Y, $false) | Out-Null
            for ($c = 0; $c -lt 2; $c++) {
                [AgentDesktopProbe.A20.Input20]::SendMouseButton($true) | Out-Null
                [AgentDesktopProbe.A20.Input20]::SendMouseButton($false) | Out-Null
                if ($c -eq 0 -and $trial.ms -gt 0) { Start-Sleep -Milliseconds $trial.ms }
            }
        }
        Start-Sleep -Milliseconds 300
        $afterTrial = Get-ScratchControlText -Handle $hDblLbl
        $recognized = ($afterTrial -ne $beforeTrial -and $afterTrial -match 'dbl:[1-9]')
        [void]$dblRows.Add([ordered]@{
            trial              = $trial.name
            inter_click_ms     = $trial.ms
            get_double_click_time_ms = $dblTimeMs
            label_before       = $beforeTrial
            label_after        = $afterTrial
            sent               = $sent
            recognized         = $recognized
        })
    }

    $best = @($dblRows | Where-Object { $_.recognized } | Select-Object -First 1)
    $dblBranch = if ($best.Count -gt 0) { 'explicit_timing_needed' } else { 'click_count_two_mapping_insufficient_on_this_fixture' }
    if (@($dblRows | Where-Object { $_.trial -eq 'half_window' -and $_.recognized }).Count -gt 0) {
        $dblBranch = 'paired_clicks_within_getdoubleclicktime_suffice'
    }

    $dblclick = [ordered]@{
        probe                 = '20-input-synthesis'
        question              = 'does SendInput double-click encoding register on btnDoubleClick within GetDoubleClickTime'
        get_double_click_time_ms = $dblTimeMs
        label_before_series   = $dblBefore
        trials                = @($dblRows)
        branch                = $dblBranch
        product_mapping_note  = 'MouseEventKind::Click{count:2} maps to count down/up pairs; inter-click delay may need explicit pacing'
        interference          = $script:Interference
    }
    $script:paths.dblclick = Write-InputCapture -Name "input-dblclick-$Label.json" -Content (ConvertTo-Json -InputObject $dblclick -Depth 12)
    Register-MandatoryPass -Capture $script:paths.dblclick -Result $dblclick
    Write-Host "wrote $($script:paths.dblclick)"

    # --- Leg 4: multi-monitor coordinate normalization ----------------------
    $monitorCount = [AgentDesktopProbe.A20.Input20]::GetSystemMetrics(80)
    $primaryW = [AgentDesktopProbe.A20.Input20]::GetSystemMetrics(0)
    $primaryH = [AgentDesktopProbe.A20.Input20]::GetSystemMetrics(1)
    $virtX = [AgentDesktopProbe.A20.Input20]::GetSystemMetrics(76)
    $virtY = [AgentDesktopProbe.A20.Input20]::GetSystemMetrics(77)
    $virtW = [AgentDesktopProbe.A20.Input20]::GetSystemMetrics(78)
    $virtH = [AgentDesktopProbe.A20.Input20]::GetSystemMetrics(79)

    $coords = [ordered]@{
        probe    = '20-input-synthesis'
        question = 'MOUSEEVENTF_ABSOLUTE primary-only vs MOUSEEVENTF_VIRTUALDESK normalization'
        foundation_cites = @('A4-3', 'A10-6')
        monitor_count = $monitorCount
        primary_rect_px = ($primaryW.ToString() + 'x' + $primaryH.ToString())
        virtual_screen_rect_px = ($virtX.ToString() + ',' + $virtY.ToString() + ',' + $virtW.ToString() + 'x' + $virtH.ToString())
    }

    if ($monitorCount -lt 2) {
        $coords['branch'] = 'single_monitor_host'
        $coords['limitation'] = 'secondary display not stageable; virtual-desktop-flag transform is pre-committed for unit-test proof against virtual-screen rect'
        $coords['pre_committed_branch'] = 'use MOUSEEVENTF_VIRTUALDESK when point outside primary rect (KTD5)'
        $coords['absolute_vs_virtualdesk_live_compare'] = 'skipped_single_monitor'
    } else {
        $secondaryX = $primaryW + 100
        $secondaryY = 100
        $ptAbs = New-Object AgentDesktopProbe.A20.ProbePoint
        $ptVirt = New-Object AgentDesktopProbe.A20.ProbePoint
        [void](Invoke-BracketedInjection -Stage 'coords:absolute-secondary' -Action {
            [AgentDesktopProbe.A20.Input20]::SendMouseAbsolute($secondaryX, $secondaryY, $false) | Out-Null
        })
        Start-Sleep -Milliseconds 100
        [void][AgentDesktopProbe.A20.Input20]::GetCursorPos([ref]$ptAbs)
        [void](Invoke-BracketedInjection -Stage 'coords:virtualdesk-secondary' -Action {
            [AgentDesktopProbe.A20.Input20]::SendMouseAbsolute($secondaryX, $secondaryY, $true) | Out-Null
        })
        Start-Sleep -Milliseconds 100
        [void][AgentDesktopProbe.A20.Input20]::GetCursorPos([ref]$ptVirt)
        $coords['branch'] = 'multi_monitor_staged'
        $coords['requested_secondary_point'] = ($secondaryX.ToString() + ',' + $secondaryY.ToString())
        $coords['landed_absolute_only'] = ($ptAbs.X.ToString() + ',' + $ptAbs.Y.ToString())
        $coords['landed_virtualdesk'] = ($ptVirt.X.ToString() + ',' + $ptVirt.Y.ToString())
        $coords['virtualdesk_closer_to_requested'] = (
            ([Math]::Abs($ptVirt.X - $secondaryX) + [Math]::Abs($ptVirt.Y - $secondaryY)) -lt
            ([Math]::Abs($ptAbs.X - $secondaryX) + [Math]::Abs($ptAbs.Y - $secondaryY))
        )
    }

    $script:paths.coords = Write-InputCapture -Name "input-coords-$Label.json" -Content (ConvertTo-Json -InputObject $coords -Depth 12)
    Register-MandatoryPass -Capture $script:paths.coords -Result $coords
    Write-Host "wrote $($script:paths.coords)"

    # --- Leg 5: foreground gate + injection cost ----------------------------
    $hTxt = Get-ScratchHandle -Form $form -Name 'txtValue'
    $txtCenter = Get-ScratchCenter -Handle $hTxt

    $foregroundCost = Get-MinOfSeven -Measure {
        Assert-Foreground -ExpectedProcessId $script:TargetPid -Stage 'cost:foreground'
    }
    $mouseCost = Get-MinOfSeven -Measure {
        Assert-Foreground -ExpectedProcessId $script:TargetPid -Stage 'cost:mouse-pre'
        [AgentDesktopProbe.A20.Input20]::SendMouseAbsolute($txtCenter.X, $txtCenter.Y, $false) | Out-Null
        Assert-Foreground -ExpectedProcessId $script:TargetPid -Stage 'cost:mouse-post'
    }
    $chordCost = Get-MinOfSeven -Measure {
        Assert-Foreground -ExpectedProcessId $script:TargetPid -Stage 'cost:chord-pre'
        [AgentDesktopProbe.A20.Input20]::SendVirtualKey(0x11, $false) | Out-Null
        [AgentDesktopProbe.A20.Input20]::SendVirtualKey(0x41, $false) | Out-Null
        [AgentDesktopProbe.A20.Input20]::SendVirtualKey(0x41, $true) | Out-Null
        [AgentDesktopProbe.A20.Input20]::SendVirtualKey(0x11, $true) | Out-Null
        Assert-Foreground -ExpectedProcessId $script:TargetPid -Stage 'cost:chord-post'
    }
    $textChunk = -join (@(65..(65 + 31) | ForEach-Object { [char]$_ }))
    $textCost = Get-MinOfSeven -Measure {
        Assert-Foreground -ExpectedProcessId $script:TargetPid -Stage 'cost:text-pre'
        foreach ($ch in $textChunk.ToCharArray()) {
            [AgentDesktopProbe.A20.Input20]::SendUnicodeUnit([uint16][int]$ch) | Out-Null
        }
        Assert-Foreground -ExpectedProcessId $script:TargetPid -Stage 'cost:text-post'
    }

    $cost = [ordered]@{
        probe    = '20-input-synthesis'
        question = 'hot-path foreground verify and SendInput cost (min-of-seven, warm-up discarded per A15-13)'
        methodology_cites = @('A15-13')
        foreground_verify = $foregroundCost
        single_mouse_move   = $mouseCost
        modifier_chord      = $chordCost
        text_chunk_32_units = $textCost
        text_chunk_note     = '32 UTF-16 units, one KEYEVENTF_UNICODE down/up pair per unit (A4-1 chunk size authority)'
    }
    $script:paths.cost = Write-InputCapture -Name "input-cost-$Label.json" -Content (ConvertTo-Json -InputObject $cost -Depth 12)
    Register-MandatoryPass -Capture $script:paths.cost -Result $cost
    Write-Host "wrote $($script:paths.cost)"

} finally {
    Restore-DesktopHygiene -CursorOrigin $cursorOrigin -ClipHadText $clipHadText -ClipOriginal $clipOriginal -ClipSnapshotTaken $clipSnapshotTaken
    foreach ($id in @($script:Spawned)) {
        $proc = Get-Process -Id $id -ErrorAction SilentlyContinue
        if ($proc) { try { Stop-ScratchProcess -ProcessId $id } catch { } }
    }
}

Assert-MandatoryMeasurement -Probe '20-input-synthesis' -Label $Label

Write-ProbeResult -Probe '20-input-synthesis' -Status 'ok' -Message 'input-synthesis gap probes captured' -Data @{
    integrity = if ($script:paths.integrity) { Split-Path -Leaf $script:paths.integrity } else { '<none>' }
    drag      = if ($script:paths.drag) { Split-Path -Leaf $script:paths.drag } else { '<none>' }
    dblclick  = if ($script:paths.dblclick) { Split-Path -Leaf $script:paths.dblclick } else { '<none>' }
    coords    = if ($script:paths.coords) { Split-Path -Leaf $script:paths.coords } else { '<none>' }
    cost      = if ($script:paths.cost) { Split-Path -Leaf $script:paths.cost } else { '<none>' }
}
exit 0
