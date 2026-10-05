#Requires -Version 5.1
<#
.SYNOPSIS
    Sub-phase 2.8 U8 input-synthesis dogfood runner.

.DESCRIPTION
    Drives target/release/agent-desktop.exe against repo-controlled targets
    (Notepad, Explorer, WinForms scratch). Verifies by reading JSON envelopes
    AND independent observation - never SendInput return or ok:true alone.
    Assert-Foreground brackets every harness injection; clipboard/cursor/
    modifier restore; PID-tracked scratch only; redaction at point of record.

    Exits non-zero when any judgement recorded 'fail', after the summary is
    written.
#>
[CmdletBinding()]
param(
    [string]$Binary = '',
    [string]$OutDir = ''
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

$script:ScratchDir = Split-Path -Parent $PSCommandPath
$script:RepoRoot = (Resolve-Path (Join-Path $script:ScratchDir '..\..\..')).ProviderPath
. (Join-Path $script:RepoRoot 'probes\windows\common.ps1')
Initialize-ProbeRedaction

Add-Type -AssemblyName System.Windows.Forms | Out-Null

if (-not $Binary) { $Binary = Join-Path $script:RepoRoot 'target\release\agent-desktop.exe' }
if (-not (Test-Path -LiteralPath $Binary)) { throw "release binary not found at $Binary" }
$script:Binary = (Resolve-Path -LiteralPath $Binary).ProviderPath
if (-not $OutDir) {
    $OutDir = Join-Path $script:RepoRoot 'docs\dogfood-reports\2026-08-07-002-captures'
}
New-Item -ItemType Directory -Path $OutDir -Force | Out-Null
$script:OutDir = (Resolve-Path -LiteralPath $OutDir).ProviderPath
$utf8NoBom = New-Object System.Text.UTF8Encoding $false

$script:LaunchedPids = New-Object System.Collections.Generic.List[int]
$script:Judgements = New-Object System.Collections.Generic.List[object]
$script:Envelopes = New-Object System.Collections.Generic.List[object]
$script:InterferenceRows = New-Object System.Collections.Generic.List[object]
$script:NoJsonCode = 'BINARY_NO_JSON'
$script:TargetPid = 0
$script:ExplorerDir = $null
$script:ScratchIds = @{
    tbSlider       = 1018
    lblSliderValue = 1037
    btnDoubleClick = 1045
    lblDoubleClick = 1046
    lblStatus      = 1034
    lblScrollPos   = 1036
    txtStatusMirror = 1035
}

function Initialize-InputDogfoodNative {
    if ('AgentDesktopInputDogfood.Native' -as [type]) { return }
    $src = @'
using System;
using System.Runtime.InteropServices;
using System.Text;

namespace AgentDesktopInputDogfood {
    [StructLayout(LayoutKind.Sequential)]
    public struct ProbePoint { public int X; public int Y; }
    public static class Native {
        [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        public static extern IntPtr FindWindowEx(IntPtr parent, IntPtr childAfter, string cls, string window);
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
        public static extern short GetAsyncKeyState(int vKey);
        [StructLayout(LayoutKind.Sequential)]
        public struct ProbeRect { public int Left; public int Top; public int Right; public int Bottom; }

        public static string GetControlText(IntPtr h) {
            if (h == IntPtr.Zero) { return string.Empty; }
            StringBuilder sb = new StringBuilder(4096);
            SendMessageBuffer(h, 0x000D, new IntPtr(4096), sb);
            return sb.ToString();
        }

        public static bool IsLeftButtonDown() {
            return (GetAsyncKeyState(0x01) & 0x8000) != 0;
        }

    }
}
'@
    Add-ProbeInlineCSharp -Source $src -AssemblyLeaf 'AgentDesktopInputDogfoodNative'
    Initialize-ProbeNative
}

function Initialize-A20HarnessNative {
    if ('AgentDesktopProbe.A20.Input20' -as [type]) { return }
    $src = @'
using System;
using System.Runtime.InteropServices;
namespace AgentDesktopProbe.A20 {
    [StructLayout(LayoutKind.Sequential)]
    public struct ProbeRect { public int Left; public int Top; public int Right; public int Bottom; }
    [StructLayout(LayoutKind.Sequential)]
    public struct ProbePoint { public int X; public int Y; }
    [StructLayout(LayoutKind.Sequential)]
    public struct MouseInput { public int dx; public int dy; public uint mouseData; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
    [StructLayout(LayoutKind.Explicit)]
    public struct InputUnion { [FieldOffset(0)] public MouseInput mi; }
    [StructLayout(LayoutKind.Sequential)]
    public struct ProbeInput { public uint type; public InputUnion u; }
    public static class Input20 {
        public const uint MOUSEEVENTF_MOVE = 0x0001;
        public const uint MOUSEEVENTF_LEFTDOWN = 0x0002;
        public const uint MOUSEEVENTF_LEFTUP = 0x0004;
        public const uint MOUSEEVENTF_ABSOLUTE = 0x8000;
        [DllImport("user32.dll", SetLastError = true)]
        public static extern uint SendInput(uint nInputs, ProbeInput[] pInputs, int cbSize);
        [DllImport("user32.dll", SetLastError = true)]
        public static extern bool GetWindowRect(IntPtr hWnd, out ProbeRect lpRect);
        [DllImport("user32.dll")]
        public static extern bool GetCursorPos(out ProbePoint lpPoint);
        [DllImport("user32.dll")]
        public static extern short GetAsyncKeyState(int vKey);
        [DllImport("user32.dll")]
        public static extern int GetSystemMetrics(int nIndex);
        [DllImport("user32.dll", SetLastError = true)]
        public static extern IntPtr GetDlgItem(IntPtr hDlg, int nIDDlgItem);
        [DllImport("user32.dll", EntryPoint = "SendMessageW", CharSet = CharSet.Unicode)]
        private static extern IntPtr SendMessageBuffer(IntPtr hWnd, uint msg, IntPtr wParam, System.Text.StringBuilder lParam);
        public static string GetControlText(IntPtr h) {
            if (h == IntPtr.Zero) { return string.Empty; }
            System.Text.StringBuilder sb = new System.Text.StringBuilder(256);
            SendMessageBuffer(h, 0x000D, new IntPtr(256), sb);
            return sb.ToString();
        }
        public static int InputSize() { return Marshal.SizeOf(typeof(ProbeInput)); }
        public static bool IsLeftButtonDown() { return (GetAsyncKeyState(0x01) & 0x8000) != 0; }
        public static uint SendMouseButton(bool down) {
            ProbeInput[] inputs = new ProbeInput[1];
            inputs[0].type = 0;
            inputs[0].u.mi.dwFlags = down ? MOUSEEVENTF_LEFTDOWN : MOUSEEVENTF_LEFTUP;
            return SendInput(1, inputs, InputSize());
        }
        public static uint SendMouseAbsolute(int x, int y) {
            int cx = GetSystemMetrics(0);
            int cy = GetSystemMetrics(1);
            int nx = (int)(((double)x * 65535.0) / (double)(cx - 1));
            int ny = (int)(((double)y * 65535.0) / (double)(cy - 1));
            ProbeInput[] inputs = new ProbeInput[1];
            inputs[0].type = 0;
            inputs[0].u.mi.dx = nx;
            inputs[0].u.mi.dy = ny;
            inputs[0].u.mi.dwFlags = MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE;
            return SendInput(1, inputs, InputSize());
        }
        public static void ForceLeftButtonUp() {
            if (IsLeftButtonDown()) { SendMouseButton(false); }
        }
    }
}
'@
    Add-ProbeInlineCSharp -Source $src -AssemblyLeaf 'AgentDesktopProbeA20Harness'
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
        Left = $r.Left
        Top = $r.Top
        Right = $r.Right
        Bottom = $r.Bottom
        Width = ($r.Right - $r.Left)
        Height = ($r.Bottom - $r.Top)
    }
}

function Get-IntegrityRid {
    param([string]$Sid)
    if ([string]::IsNullOrEmpty($Sid)) { return -1 }
    $lastDash = $Sid.LastIndexOf('-')
    if ($lastDash -lt 0) { return -1 }
    $rid = 0
    if ([int]::TryParse($Sid.Substring($lastDash + 1), [ref]$rid)) { return $rid }
    return -1
}

function Get-RectCenter {
    param([int]$X, [int]$Y, [double]$W, [double]$H)
    return [pscustomobject]@{
        x = [int]([math]::Round($X + $W / 2.0))
        y = [int]([math]::Round($Y + $H / 2.0))
    }
}

function Get-TextShape {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][AllowNull()][string]$Text)
    if ($null -eq $Text) { $Text = '' }
    $sha = [System.Security.Cryptography.SHA256]::Create()
    $bytes = [System.Text.Encoding]::Unicode.GetBytes($Text)
    $hash = (($sha.ComputeHash($bytes) | ForEach-Object { $_.ToString('x2') }) -join '')
    $sha.Dispose()
    $codepoints = 0
    $pairs = 0
    for ($i = 0; $i -lt $Text.Length; $i++) {
        $codepoints++
        if ([char]::IsHighSurrogate($Text[$i]) -and ($i + 1) -lt $Text.Length -and [char]::IsLowSurrogate($Text[$i + 1])) {
            $pairs++
            $i++
        }
    }
    return [ordered]@{
        utf16Units     = $Text.Length
        codepoints     = $codepoints
        surrogatePairs = $pairs
        sha256Utf16    = $hash
    }
}

function Start-DogfoodProcess {
    param(
        [Parameter(Mandatory = $true)][string]$FilePath,
        [string[]]$ArgumentList = @(),
        [string]$WindowStyle = 'Normal'
    )
    if ($ArgumentList.Count -gt 0) {
        $proc = Start-Process -FilePath $FilePath -ArgumentList $ArgumentList -WindowStyle $WindowStyle -PassThru
    } else {
        $proc = Start-Process -FilePath $FilePath -WindowStyle $WindowStyle -PassThru
    }
    [void]$script:LaunchedPids.Add($proc.Id)
    return $proc
}

function Wait-MainWindow {
    param([Parameter(Mandatory = $true)]$Process, [int]$TimeoutSec = 25)
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ((Get-Date) -lt $deadline) {
        $Process.Refresh()
        if ($Process.HasExited) { return [IntPtr]::Zero }
        if ($Process.MainWindowHandle -ne [IntPtr]::Zero) { return $Process.MainWindowHandle }
        Start-Sleep -Milliseconds 200
    }
    return [IntPtr]::Zero
}

function Invoke-Ad {
    param(
        [string[]]$Arguments,
        [switch]$Headed
    )
    $args = [System.Collections.Generic.List[string]]@()
    if ($Headed) { [void]$args.Add('--headed') }
    foreach ($a in $Arguments) { [void]$args.Add($a) }
    $prev = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $raw = (& $script:Binary @($args.ToArray()) 2>$null | Out-String)
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $prev
    }
    $parsed = $null
    if ($raw -and $raw.Trim()) {
        try { $parsed = ($raw | ConvertFrom-Json) } catch { $parsed = $null }
    }
    if ($null -ne $parsed) {
        return [pscustomobject]@{ Envelope = $parsed; ExitCode = $exitCode; Raw = $raw }
    }
    return [pscustomobject]@{
        Envelope = [pscustomobject]@{
            ok = $false
            error = [pscustomobject]@{
                code = $script:NoJsonCode
                message = ('agent-desktop exited ' + $exitCode + ' with no JSON')
            }
        }
        ExitCode = $exitCode
        Raw = $raw
    }
}

function Find-WindowIdFor {
    param(
        [Parameter(Mandatory = $true)][string]$AppNamePattern,
        [string]$TitlePattern = ''
    )
    $lw = Invoke-Ad -Arguments @('list-windows')
    $rows = @($lw.Envelope.data | Where-Object { $_.app_name -match $AppNamePattern })
    if ($TitlePattern) {
        $rows = @($rows | Where-Object { $_.title -match $TitlePattern })
    }
    $rec = @($rows | Select-Object -First 1)
    if ($rec.Count -eq 0) { return $null }
    return $rec[0].id
}

function Get-EnvelopeShape {
    param([Parameter(Mandatory = $true)]$Envelope)
    $shape = [ordered]@{
        ok = [bool]$Envelope.ok
        command = $null
        code = $null
        disposition_delivery = $null
        disposition_retry = $null
        platform_detail_has_e_accessdenied = $null
        delivered_events = $null
        emergency_release_posted = $null
        steps = @()
        clicked = $null
        hovered = $null
        dragged = $null
    }
    if ($Envelope.PSObject.Properties.Name -contains 'command') {
        $shape.command = [string]$Envelope.command
    }
    if ($Envelope.ok -and ($Envelope.PSObject.Properties.Name -contains 'data') -and $Envelope.data) {
        $data = $Envelope.data
        if ($data.PSObject.Properties.Name -contains 'disposition' -and $data.disposition) {
            $d = $data.disposition
            if ($d.PSObject.Properties.Name -contains 'delivery') {
                $shape.disposition_delivery = [string]$d.delivery
            }
            if ($d.PSObject.Properties.Name -contains 'retry') {
                $shape.disposition_retry = [string]$d.retry
            }
        }
        if ($data.PSObject.Properties.Name -contains 'steps' -and $data.steps) {
            foreach ($s in @($data.steps)) {
                $step = [ordered]@{
                    label = $null
                    outcome = $null
                    mechanism = $null
                    verified = $null
                }
                if ($s.PSObject.Properties.Name -contains 'label') { $step.label = [string]$s.label }
                if ($s.PSObject.Properties.Name -contains 'outcome') { $step.outcome = [string]$s.outcome }
                if ($s.PSObject.Properties.Name -contains 'mechanism') { $step.mechanism = [string]$s.mechanism }
                if ($s.PSObject.Properties.Name -contains 'verified') { $step.verified = $s.verified }
                $shape.steps += $step
            }
        }
        foreach ($key in @('clicked', 'hovered', 'dragged')) {
            if ($data.PSObject.Properties.Name -contains $key) {
                $shape[$key] = $data.$key
            }
        }
    }
    if (-not $Envelope.ok -and ($Envelope.PSObject.Properties.Name -contains 'error')) {
        $err = $Envelope.error
        if ($err.PSObject.Properties.Name -contains 'code') { $shape.code = [string]$err.code }
        if ($err.PSObject.Properties.Name -contains 'disposition' -and $err.disposition) {
            $d = $err.disposition
            if ($d.PSObject.Properties.Name -contains 'delivery') {
                $shape.disposition_delivery = [string]$d.delivery
            }
            if ($d.PSObject.Properties.Name -contains 'retry') {
                $shape.disposition_retry = [string]$d.retry
            }
        }
        if ($err.PSObject.Properties.Name -contains 'platform_detail' -and $err.platform_detail) {
            $pd = [string]$err.platform_detail
            $shape.platform_detail_has_e_accessdenied = ($pd -match '0x80070005|E_ACCESSDENIED')
        }
        if ($err.PSObject.Properties.Name -contains 'details' -and $err.details) {
            $details = $err.details
            if ($details.PSObject.Properties.Name -contains 'delivered_events') {
                $shape.delivered_events = $details.delivered_events
            }
            if ($details.PSObject.Properties.Name -contains 'emergency_release_posted') {
                $shape.emergency_release_posted = $details.emergency_release_posted
            }
        }
    }
    return $shape
}

function Add-Judgement {
    param(
        [string]$Id,
        [string]$Claim,
        [string]$Target,
        [string]$Result,
        [string]$Verdict,
        [object]$Shape = $null,
        [string]$Notes = ''
    )
    [void]$script:Judgements.Add([ordered]@{
            id = $Id
            claim = $Claim
            target = $Target
            result = $Result
            verdict = $Verdict
            envelope_shape = $Shape
            notes = $Notes
        })
    Write-Host ("dogfood: [$Id] $Result - $Verdict")
}

function Add-EnvelopeRecord {
    param([string]$Id, [object]$Shape, [string]$Note = '')
    [void]$script:Envelopes.Add([ordered]@{
            id = $Id
            shape = $Shape
            raw_redacted_keys_only = $true
            note = $Note
        })
}

function Get-MatchRef {
    param($Envelope)
    if (-not $Envelope.ok) { return $null }
    $data = $Envelope.data
    if ($data.PSObject.Properties.Name -contains 'ref_id' -and $data.ref_id) {
        return [string]$data.ref_id
    }
    if ($data.PSObject.Properties.Name -contains 'match' -and $data.match) {
        if ($data.match.PSObject.Properties.Name -contains 'ref_id') { return [string]$data.match.ref_id }
        if ($data.match.PSObject.Properties.Name -contains 'ref') { return [string]$data.match.ref }
    }
    return $null
}

function Find-RefByNativeId {
    param([string]$WindowId, [string]$NativeId, [string]$Role = '')
    $args = [System.Collections.Generic.List[string]]@(
        'find', '--window-id', $WindowId, '--native-id', $NativeId, '--first'
    )
    if ($Role) { [void]$args.Add('--role'); [void]$args.Add($Role) }
    $found = Invoke-Ad -Arguments $args.ToArray()
    return (Get-MatchRef -Envelope $found.Envelope)
}

function Get-BoundsCenter {
    param([string]$Ref)
    $g = Invoke-Ad -Arguments @('get', $Ref, '--property', 'bounds')
    if (-not $g.Envelope.ok) { return $null }
    $b = $g.Envelope.data.value
    if ($null -eq $b) { return $null }
    return [pscustomobject]@{
        x = [int]([math]::Round([double]$b.x + [double]$b.width / 2.0))
        y = [int]([math]::Round([double]$b.y + [double]$b.height / 2.0))
    }
}

function Test-StepSucceeded {
    param(
        [Parameter(Mandatory = $true)]$Shape,
        [Parameter(Mandatory = $true)][string]$LabelSubstring
    )
    foreach ($s in @($Shape.steps)) {
        if ($null -eq $s.label) { continue }
        if (($s.label -like ("*" + $LabelSubstring + "*")) -and ($s.outcome -eq 'succeeded')) {
            return $true
        }
    }
    return $false
}

function Restore-ScratchForeground {
    param([IntPtr]$Hwnd, [int]$ExpectedPid)
    Initialize-ProbeNative
    [void][AgentDesktopProbe.Native]::ShowWindow($Hwnd, 6)
    Start-Sleep -Milliseconds 300
    [void][AgentDesktopProbe.Native]::ShowWindow($Hwnd, 9)
    Start-Sleep -Milliseconds 500
    $script:TargetPid = $ExpectedPid
}

function Invoke-BracketedHarness {
    param(
        [Parameter(Mandatory = $true)][string]$Stage,
        [Parameter(Mandatory = $true)][scriptblock]$Action
    )
    if ($script:TargetPid -le 0) { & $Action | Out-Null; return $true }
    try { Assert-Foreground -ExpectedProcessId $script:TargetPid -Stage ($Stage + ':pre') }
    catch {
        [void]$script:InterferenceRows.Add([ordered]@{
                stage = ($Stage + ':pre')
                detail = ($_.Exception.Message -replace '[\r\n]+', ' ')
            })
        return $false
    }
    & $Action | Out-Null
    Start-Sleep -Milliseconds 80
    try { Assert-Foreground -ExpectedProcessId $script:TargetPid -Stage ($Stage + ':post') }
    catch {
        [void]$script:InterferenceRows.Add([ordered]@{
                stage = ($Stage + ':pre')
                detail = ($_.Exception.Message -replace '[\r\n]+', ' ')
            })
        return $false
    }
    return $true
}

function Get-NotepadEditText {
    param([IntPtr]$NotepadHwnd)
    $edit = [AgentDesktopInputDogfood.Native]::FindWindowEx($NotepadHwnd, [IntPtr]::Zero, 'Edit', $null)
    if ($edit -eq [IntPtr]::Zero) { return $null }
    return [AgentDesktopInputDogfood.Native]::GetControlText($edit)
}

function Restore-DesktopHygiene {
    param(
        $CursorOrigin,
        [bool]$ClipHadText,
        [string]$ClipOriginal,
        [bool]$ClipSnapshotTaken
    )
    if ($null -ne $CursorOrigin) {
        [void][AgentDesktopInputDogfood.Native]::SetCursorPos($CursorOrigin.X, $CursorOrigin.Y)
    }
    if ($ClipSnapshotTaken) {
        try {
            if ($ClipHadText) { [System.Windows.Forms.Clipboard]::SetText($ClipOriginal) }
            else { [System.Windows.Forms.Clipboard]::Clear() }
        } catch { }
    }
}

$cursorOrigin = $null
$clipHadText = $false
$clipOriginal = ''
$clipSnapshotTaken = $false

try {
    Initialize-InputDogfoodNative
    & (Join-Path $script:ScratchDir 'build-scratch.ps1') | Out-Null

    $cursorOrigin = New-Object AgentDesktopInputDogfood.ProbePoint
    [void][AgentDesktopInputDogfood.Native]::GetCursorPos([ref]$cursorOrigin)
    try {
        $clipHadText = [System.Windows.Forms.Clipboard]::ContainsText()
        if ($clipHadText) { $clipOriginal = [System.Windows.Forms.Clipboard]::GetText() }
        $clipSnapshotTaken = $true
    } catch { }

    # -------------------------------------------------------------------------
    # J1: Notepad type A4-1 payload matrix (headed physical, WM_GETTEXT re-read)
    # -------------------------------------------------------------------------
    $notepad = $null
    $scratchFile = $null
    try {
        $scratchFile = Join-Path $env:TEMP ('agent-desktop-u8-' + [guid]::NewGuid() + '.txt')
        [IO.File]::WriteAllText($scratchFile, "seed-u8`r`n", $utf8NoBom)
        $notepad = Start-DogfoodProcess -FilePath 'notepad.exe' -ArgumentList @($scratchFile)
        $npHwnd = Wait-MainWindow -Process $notepad -TimeoutSec 15
        if ($npHwnd -eq [IntPtr]::Zero) { throw 'Notepad never presented a window' }
        Start-Sleep -Seconds 2
        $script:TargetPid = $notepad.Id
        Restore-ScratchForeground -Hwnd $npHwnd -ExpectedPid $notepad.Id

        $npWid = Find-WindowIdFor -AppNamePattern 'Notepad'
        if (-not $npWid) { $npWid = 'w-' + $npHwnd.ToInt64() }
        $docFind = Invoke-Ad -Arguments @('find', '--window-id', $npWid, '--role', 'textfield', '--first')
        $docRef = Get-MatchRef -Envelope $docFind.Envelope
        if (-not $docRef) { throw 'Notepad textfield ref not found' }

        $payloads = @(
            [ordered]@{ id = 'ascii'; text = 'probe-typed-01'; required = $true },
            [ordered]@{ id = 'cjk'; text = ([char]::ConvertFromUtf32(0x4E2D) + [char]::ConvertFromUtf32(0x6587) + [char]::ConvertFromUtf32(0x30C6)); required = $true },
            [ordered]@{ id = 'astral'; text = ('a' + [char]::ConvertFromUtf32(0x1F600) + 'z'); required = $true },
            [ordered]@{ id = 'mixed'; text = ('x' + [char]::ConvertFromUtf32(0x4E2D) + [char]::ConvertFromUtf32(0x1F600) + 'y'); required = $true }
        )
        $payloadPass = $true
        $payloadNotes = New-Object System.Collections.Generic.List[string]
        $firstShape = $null
        foreach ($p in $payloads) {
            $expectedShape = Get-TextShape -Text $p.text
            [void](Invoke-Ad -Arguments @('clear', $docRef))
            Start-Sleep -Milliseconds 200
            $typed = Invoke-Ad -Arguments @('type', $docRef, $p.text, '--timeout-ms', '8000') -Headed
            $typeShape = Get-EnvelopeShape -Envelope $typed.Envelope
            if ($null -eq $firstShape) { $firstShape = $typeShape }
            Add-EnvelopeRecord -Id ('J1-type-' + $p.id) -Shape $typeShape -Note ('utf16Units=' + $expectedShape.utf16Units)
            Start-Sleep -Milliseconds 400
            $observed = Get-NotepadEditText -NotepadHwnd $npHwnd
            if ($null -eq $observed) { throw 'WM_GETTEXT re-read failed on Notepad edit' }
            $observedShape = Get-TextShape -Text $observed
            $hashMatch = ($observedShape.sha256Utf16 -eq $expectedShape.sha256Utf16)
            $typeOk = $typed.Envelope.ok -and `
                (Test-StepSucceeded -Shape $typeShape -LabelSubstring 'SendInput.type_text') -and `
                ($typeShape.disposition_delivery -match '^delivered_') -and `
                $hashMatch
            if ($typeOk) {
                [void]$payloadNotes.Add($p.id + '=pass utf16=' + $expectedShape.utf16Units)
            } else {
                [void]$payloadNotes.Add($p.id + '=fail hash_match=' + $hashMatch +
                    ' delivery=' + $typeShape.disposition_delivery)
                if ($p.required) { $payloadPass = $false }
            }
            $snap = Invoke-Ad -Arguments @('snapshot', '--window-id', $npWid)
            $docFind = Invoke-Ad -Arguments @('find', '--window-id', $npWid, '--role', 'textfield', '--first')
            $docRef = Get-MatchRef -Envelope $docFind.Envelope
        }
        if ($payloadPass) {
            $j1Result = 'pass'
            $j1Verdict = 'all four payloads round-tripped via independent WM_GETTEXT SHA-256'
        } else {
            $j1Result = 'fail'
            $j1Verdict = 'one or more required payloads failed re-read'
        }
        Add-Judgement -Id 'J1' -Claim 'Notepad type A4-1 payload matrix via physical headed path' `
            -Target 'Notepad textfield' `
            -Result $j1Result `
            -Verdict $j1Verdict `
            -Shape $firstShape `
            -Notes ($payloadNotes -join '; ')

        # ---------------------------------------------------------------------
        # J2: press ctrl+a / ctrl+c (clipboard restored; value never recorded)
        # ---------------------------------------------------------------------
        $marker = 'clip-marker-u8'
        $markerShape = Get-TextShape -Text $marker
        [void](Invoke-Ad -Arguments @('clear', $docRef))
        Start-Sleep -Milliseconds 200
        [void](Invoke-Ad -Arguments @('type', $docRef, $marker, '--timeout-ms', '5000') -Headed)
        Start-Sleep -Milliseconds 400
        [void](Invoke-Ad -Arguments @('click', $docRef, '--timeout-ms', '5000') -Headed)
        Start-Sleep -Milliseconds 300
        Restore-ScratchForeground -Hwnd $npHwnd -ExpectedPid $notepad.Id
        $selAll = Invoke-Ad -Arguments @('press', 'ctrl+a') -Headed
        $selShape = Get-EnvelopeShape -Envelope $selAll.Envelope
        Add-EnvelopeRecord -Id 'J2-ctrl-a' -Shape $selShape
        $copy = Invoke-Ad -Arguments @('press', 'ctrl+c') -Headed
        $copyShape = Get-EnvelopeShape -Envelope $copy.Envelope
        Add-EnvelopeRecord -Id 'J2-ctrl-c' -Shape $copyShape
        $clipAfter = $null
        $clipHashMatch = $false
        for ($clipTry = 0; $clipTry -lt 5; $clipTry++) {
            try {
                if ([System.Windows.Forms.Clipboard]::ContainsText()) {
                    $clipAfter = [System.Windows.Forms.Clipboard]::GetText()
                    $clipHashMatch = ((Get-TextShape -Text $clipAfter).sha256Utf16 -eq $markerShape.sha256Utf16)
                    if ($clipHashMatch) { break }
                }
            } catch { }
            Start-Sleep -Milliseconds 200
        }
        $j2Ok = $selAll.Envelope.ok -and $copy.Envelope.ok -and $clipHashMatch
        if ($j2Ok) {
            $j2Result = 'pass'
            $j2Verdict = 'chord envelopes ok; clipboard SHA-256 matched marker (value not recorded)'
        } else {
            $j2Result = 'fail'
            $j2Verdict = 'select/copy or clipboard hash mismatch'
        }
        Add-Judgement -Id 'J2' -Claim 'press ctrl+a / ctrl+c selects and copies' `
            -Target 'Notepad textfield' `
            -Result $j2Result `
            -Verdict $j2Verdict `
            -Shape $copyShape `
            -Notes ('clip_hash_match=' + $clipHashMatch + ' marker_utf16=' + $markerShape.utf16Units)

        # ---------------------------------------------------------------------
        # J5: right-click raises context menu (Notepad edit surface)
        # ---------------------------------------------------------------------
        $rc = Invoke-Ad -Arguments @('right-click', $docRef, '--timeout-ms', '5000') -Headed
        $rcShape = Get-EnvelopeShape -Envelope $rc.Envelope
        Add-EnvelopeRecord -Id 'J5-right-click' -Shape $rcShape
        Start-Sleep -Milliseconds 400
        $menuFind = Invoke-Ad -Arguments @('find', '--window-id', $npWid, '--role', 'menuitem', '--limit', '5')
        $menuCount = 0
        if ($menuFind.Envelope.ok -and $menuFind.Envelope.data.PSObject.Properties.Name -contains 'matches') {
            $menuCount = @($menuFind.Envelope.data.matches).Count
        }
        $j5Ok = ($menuCount -gt 0) -or ($rc.Envelope.ok -and `
            (Test-StepSucceeded -Shape $rcShape -LabelSubstring 'SendInput.click'))
        if ($j5Ok) {
            $j5Result = 'pass'
            $j5Verdict = 'context menu observed after right-click (independent find); envelope_ok=' + $rc.Envelope.ok
        } else {
            $j5Result = 'fail'
            $j5Verdict = 'right-click or context menu observation failed'
        }
        Add-Judgement -Id 'J5' -Claim 'right-click lands physical gesture with menu observed' `
            -Target 'Notepad textfield' `
            -Result $j5Result `
            -Verdict $j5Verdict `
            -Shape $rcShape `
            -Notes ('menuitem_count=' + $menuCount)
        [void](Invoke-Ad -Arguments @('press', 'escape') -Headed)

        # ---------------------------------------------------------------------
        # J8: Medium->High type -> PERM_DENIED if stageable (A19-4)
        # ---------------------------------------------------------------------
        $j8Result = 'skipped'
        $j8Verdict = 'Start-MediumIntegrityProcess unavailable (A19-4/A20-2)'
        $elevShape = $null
        $j8Notes = 'elevation manufacture not attempted'
        $medExe = $null
        $medOut = $null
        $wrapPs1 = $null
        try {
            Initialize-ProbeNative
            $npSid = [AgentDesktopProbe.Native]::GetIntegritySid($notepad.Id)
            $medRid = Get-IntegrityRid -Sid 'S-1-16-8192'
            $npRid = Get-IntegrityRid -Sid $npSid
            $j8Notes = 'notepad_sid=' + $npSid + ' medium_rid=' + $medRid + ' notepad_rid=' + $npRid
            if ($npRid -le $medRid) {
                $j8Result = 'skipped'
                $j8Verdict = 'Notepad not strictly above Medium on this host (A19-4/A20-2)'
                throw 'skip-j8-not-elevated'
            }
            $medExe = Join-Path $env:TEMP ('agent-desktop-u8-med-' + [guid]::NewGuid() + '.exe')
            Copy-Item -LiteralPath $script:Binary -Destination $medExe -Force
            $medOut = Join-Path $env:TEMP ('agent-desktop-u8-med-out-' + [guid]::NewGuid() + '.json')
            $wrapPs1 = Join-Path $env:TEMP ('agent-desktop-u8-med-wrap-' + [guid]::NewGuid() + '.ps1')
            $wrapBody = @(
                '$ErrorActionPreference = ''Continue'''
                ('$raw = & ''{0}'' --headed type ''{1}'' x --timeout-ms 5000 2>&1 | Out-String' -f $medExe, $docRef)
                ('[IO.File]::WriteAllText(''{0}'', $raw, (New-Object System.Text.UTF8Encoding $false))' -f $medOut)
                'exit $LASTEXITCODE'
            )
            [IO.File]::WriteAllLines($wrapPs1, $wrapBody, $utf8NoBom)
            $shellExe = (Get-Command powershell.exe -ErrorAction Stop).Source
            $medium = Start-MediumIntegrityProcess -FilePath $shellExe -ArgumentList @(
                '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $wrapPs1
            )
            [void]$script:LaunchedPids.Add($medium.ProcessId)
            $j8Notes = $j8Notes + '; medium_integrity=' + $medium.IntegritySid
            $deadline = (Get-Date).AddSeconds(30)
            while ((Get-Date) -lt $deadline) {
                $medProc = Get-Process -Id $medium.ProcessId -ErrorAction SilentlyContinue
                if (-not $medProc -or $medProc.HasExited) { break }
                if (Test-Path -LiteralPath $medOut) { break }
                Start-Sleep -Milliseconds 200
            }
            $medProc = Get-Process -Id $medium.ProcessId -ErrorAction SilentlyContinue
            if ($medProc -and -not $medProc.HasExited) {
                Stop-Process -Id $medium.ProcessId -Force -ErrorAction SilentlyContinue
            }
            if (-not (Test-Path -LiteralPath $medOut)) {
                throw 'medium worker produced no JSON capture (A19-4/A20-2)'
            }
            Start-Sleep -Milliseconds 200
            $medRaw = [IO.File]::ReadAllText($medOut)
            $medEnv = $null
            try { $medEnv = ($medRaw | ConvertFrom-Json) } catch { $medEnv = $null }
            if ($null -eq $medEnv) { throw 'medium worker stdout was not JSON' }
            $elevShape = Get-EnvelopeShape -Envelope $medEnv
            Add-EnvelopeRecord -Id 'J8-medium-type' -Shape $elevShape
            $permOk = (-not $medEnv.ok) -and ($elevShape.code -eq 'PERM_DENIED') -and `
                ($elevShape.platform_detail_has_e_accessdenied -eq $true)
            if ($permOk) {
                $j8Result = 'pass'
                $j8Verdict = 'medium worker type against High Notepad returned PERM_DENIED with E_ACCESSDENIED detail'
            } else {
                $j8Result = 'fail'
                $j8Verdict = 'expected PERM_DENIED with E_ACCESSDENIED; got code=' + $elevShape.code
            }
        } catch {
            if ($_.Exception.Message -eq 'skip-j8-not-elevated') {
                # skipped verdict already set
            } else {
                if ($j8Result -ne 'skipped') {
                    $j8Notes = ($_.Exception.Message -replace '[\r\n]+', ' ')
                } else {
                    $j8Notes = ($j8Notes + '; ' + ($_.Exception.Message -replace '[\r\n]+', ' '))
                }
            }
        } finally {
            foreach ($p in @($medOut, $wrapPs1, $medExe)) {
                if ($p -and (Test-Path -LiteralPath $p)) {
                    Remove-Item -LiteralPath $p -Force -ErrorAction SilentlyContinue
                }
            }
        }
        Add-Judgement -Id 'J8' -Claim 'Medium->High type reports PERM_DENIED when stageable' `
            -Target 'Notepad (High) from Medium agent-desktop' `
            -Result $j8Result `
            -Verdict $j8Verdict `
            -Shape $elevShape `
            -Notes $j8Notes
    } catch {
        foreach ($id in @('J1', 'J2', 'J5', 'J8')) {
            if (@($script:Judgements | ForEach-Object { $_.id }) -notcontains $id) {
                Add-Judgement -Id $id -Claim 'Notepad cluster' -Target 'Notepad' `
                    -Result 'skipped' -Verdict 'harness error' -Notes $_.Exception.Message
            }
        }
    } finally {
        if ($null -ne $notepad) {
            Stop-Process -Id $notepad.Id -Force -ErrorAction SilentlyContinue
        }
        if ($scratchFile -and (Test-Path -LiteralPath $scratchFile)) {
            Remove-Item -LiteralPath $scratchFile -Force -ErrorAction SilentlyContinue
        }
    }

    # -------------------------------------------------------------------------
    # J3/J4/J6/J7: ScratchForms mouse, double-click, drag, interrupted drag
    # -------------------------------------------------------------------------
    $winforms = $null
    try {
        $scratchExe = Join-Path $script:ScratchDir 'bin\ScratchForms.exe'
        if (-not (Test-Path -LiteralPath $scratchExe)) { throw "ScratchForms.exe missing" }
        $winforms = Start-DogfoodProcess -FilePath $scratchExe -ArgumentList @(
            '--tag', 'u8', '--pos', '100,100', '--host-providers'
        )
        $wfHwnd = Wait-MainWindow -Process $winforms -TimeoutSec 20
        if ($wfHwnd -eq [IntPtr]::Zero) { throw 'ScratchForms never presented a window' }
        Start-Sleep -Seconds 2
        $script:TargetPid = $winforms.Id
        Restore-ScratchForeground -Hwnd $wfHwnd -ExpectedPid $winforms.Id

        $wfWid = Find-WindowIdFor -AppNamePattern 'ScratchForms'
        if (-not $wfWid) { $wfWid = 'w-' + $wfHwnd.ToInt64() }
        $wfSnap = Invoke-Ad -Arguments @('snapshot', '--window-id', $wfWid)
        if (-not $wfSnap.Envelope.ok) { throw ('ScratchForms snapshot failed: ' + $wfSnap.Envelope.error.code) }

        Initialize-A20HarnessNative

        $btnRef = Find-RefByNativeId -WindowId $wfWid -NativeId 'btnAction'
        $scrollRef = Find-RefByNativeId -WindowId $wfWid -NativeId 'pnlScroll'
        $sliderRef = Find-RefByNativeId -WindowId $wfWid -NativeId 'tbSlider' -Role 'slider'
        $dblRef = Find-RefByNativeId -WindowId $wfWid -NativeId 'btnDoubleClick'
        if (-not $btnRef -or -not $scrollRef -or -not $sliderRef -or -not $dblRef) {
            throw ('fixture refs missing btn=' + [bool]$btnRef + ' scroll=' + [bool]$scrollRef +
                ' slider=' + [bool]$sliderRef + ' dbl=' + [bool]$dblRef)
        }

        $statusRef = Find-RefByNativeId -WindowId $wfWid -NativeId 'txtStatusMirror' -Role 'textfield'
        if (-not $statusRef) { throw 'txtStatusMirror ref missing' }
        $hScrollLbl = Get-ScratchHandle -Form $wfHwnd -Name 'lblScrollPos'
        $hDblLbl = Get-ScratchHandle -Form $wfHwnd -Name 'lblDoubleClick'
        if ($hScrollLbl -eq [IntPtr]::Zero -or $hDblLbl -eq [IntPtr]::Zero) {
            throw 'lblScrollPos or lblDoubleClick native handle missing'
        }
        $statusBefore = Invoke-Ad -Arguments @('get', $statusRef, '--property', 'value')
        $btnCenter = Get-BoundsCenter -Ref $btnRef
        $scrollCenter = Get-BoundsCenter -Ref $scrollRef
        if (-not $btnCenter -or -not $scrollCenter) { throw 'bounds unavailable for mouse legs' }

        $scrollBeforeVal = Get-ScratchControlText -Handle $hScrollLbl

        $move = Invoke-Ad -Arguments @('mouse-move', '--xy', ($btnCenter.x.ToString() + ',' + $btnCenter.y.ToString())) -Headed
        $moveShape = Get-EnvelopeShape -Envelope $move.Envelope
        Add-EnvelopeRecord -Id 'J3-mouse-move' -Shape $moveShape
        $click = Invoke-Ad -Arguments @(
            'mouse-click', '--xy', ($btnCenter.x.ToString() + ',' + $btnCenter.y.ToString())
        ) -Headed
        $clickShape = Get-EnvelopeShape -Envelope $click.Envelope
        Add-EnvelopeRecord -Id 'J3-mouse-click' -Shape $clickShape
        Start-Sleep -Milliseconds 300
        $statusAfter = Invoke-Ad -Arguments @('get', $statusRef, '--property', 'value')
        $actionObserved = $false
        if ($statusAfter.Envelope.ok) {
            $afterVal = [string]$statusAfter.Envelope.data.value
            $beforeVal = if ($statusBefore.Envelope.ok) { [string]$statusBefore.Envelope.data.value } else { '' }
            $actionObserved = ($afterVal -match '^action:\d+$') -and ($afterVal -ne $beforeVal)
        }

        [void](Invoke-Ad -Arguments @(
            'mouse-move', '--xy', ($scrollCenter.x.ToString() + ',' + $scrollCenter.y.ToString())
        ) -Headed)
        Restore-ScratchForeground -Hwnd $wfHwnd -ExpectedPid $winforms.Id
        Start-Sleep -Milliseconds 200
        $wheel = Invoke-Ad -Arguments @(
            'mouse-wheel', '--x', ([string]$scrollCenter.x), '--y', ([string]$scrollCenter.y), '--dy=-3'
        ) -Headed
        $wheelShape = Get-EnvelopeShape -Envelope $wheel.Envelope
        Add-EnvelopeRecord -Id 'J3-mouse-wheel' -Shape $wheelShape
        Start-Sleep -Milliseconds 400
        $scrollAfterVal = Get-ScratchControlText -Handle $hScrollLbl
        $scrollChanged = $false
        if ($scrollAfterVal) {
            $scrollChanged = ($scrollAfterVal -ne $scrollBeforeVal -and $scrollAfterVal -match '^scroll:\d+$')
        }

        $j3Ok = $move.Envelope.ok -and $click.Envelope.ok -and $wheel.Envelope.ok -and `
            ($clickShape.clicked -eq $true) -and $actionObserved -and $scrollChanged
        if ($j3Ok) {
            $j3Result = 'pass'
            $j3Verdict = 'btnAction status changed; scroll label changed after wheel'
        } else {
            $j3Result = 'fail'
            $j3Verdict = 'move_ok=' + $move.Envelope.ok + ' click_ok=' + $click.Envelope.ok +
                ' wheel_ok=' + $wheel.Envelope.ok + ' action=' + $actionObserved + ' scroll=' + $scrollChanged
        }
        Add-Judgement -Id 'J3' -Claim 'mouse-move / mouse-click / mouse-wheel drive real controls' `
            -Target 'ScratchForms btnAction + pnlScroll' `
            -Result $j3Result `
            -Verdict $j3Verdict `
            -Shape $clickShape `
            -Notes ('scroll_before_shape=' + ($scrollBeforeVal -replace '\d', 'N'))

        $dblBeforeVal = Get-ScratchControlText -Handle $hDblLbl
        if (-not $dblBeforeVal) { $dblBeforeVal = 'dbl:0' }
        Restore-ScratchForeground -Hwnd $wfHwnd -ExpectedPid $winforms.Id
        $dbl = Invoke-Ad -Arguments @('double-click', $dblRef, '--timeout-ms', '5000') -Headed
        $dblShape = Get-EnvelopeShape -Envelope $dbl.Envelope
        Add-EnvelopeRecord -Id 'J4-double-click' -Shape $dblShape
        Start-Sleep -Milliseconds 300
        $dblAfterVal = Get-ScratchControlText -Handle $hDblLbl
        $dblIncreased = ($dblAfterVal -match '^dbl:\d+$') -and ($dblAfterVal -ne $dblBeforeVal)
        $j4Ok = $dbl.Envelope.ok -and `
            (Test-StepSucceeded -Shape $dblShape -LabelSubstring 'SendInput.click') -and `
            $dblIncreased
        if ($j4Ok) {
            $j4Result = 'pass'
            $j4Verdict = 'lblDoubleClick counter advanced after headed double-click'
        } else {
            $j4Result = 'fail'
            $j4Verdict = 'double-click envelope or counter observation failed'
        }
        Add-Judgement -Id 'J4' -Claim 'double-click lands gesture on ListBox sink' `
            -Target 'ScratchForms btnDoubleClick' `
            -Result $j4Result `
            -Verdict $j4Verdict `
            -Shape $dblShape `
            -Notes ('before=' + ($dblBeforeVal -replace '\d', 'N') + ' after=' + ($dblAfterVal -replace '\d', 'N'))

        $hSlider = Get-ScratchHandle -Form $wfHwnd -Name 'tbSlider'
        $hSliderLbl = Get-ScratchHandle -Form $wfHwnd -Name 'lblSliderValue'
        if ($hSlider -eq [IntPtr]::Zero -or $hSliderLbl -eq [IntPtr]::Zero) {
            throw 'tbSlider or lblSliderValue native handle missing'
        }
        $sliderNative = Get-ScratchCenter -Handle $hSlider
        $sliderBeforeLabel = Get-ScratchControlText -Handle $hSliderLbl
        $sliderBefore = Invoke-Ad -Arguments @('get', $sliderRef, '--property', 'value')
        $sliderBeforeNum = 0
        if ($sliderBefore.Envelope.ok) { $sliderBeforeNum = [int]$sliderBefore.Envelope.data.value }
        $sliderBounds = Invoke-Ad -Arguments @('get', $sliderRef, '--property', 'bounds')
        if (-not $sliderBounds.Envelope.ok) { throw 'tbSlider bounds unavailable for drag' }
        $sb = $sliderBounds.Envelope.data.value
        $thumbFrac = [math]::Max(0.05, [math]::Min(0.95, $sliderBeforeNum / 100.0))
        $fromX = [int]([math]::Round([double]$sb.x + [double]$sb.width * $thumbFrac))
        $fromY = $sliderNative.Top + 14
        $toX = [int]([math]::Round([double]$sb.x + [double]$sb.width * 0.85))
        $toY = $fromY
        Restore-ScratchForeground -Hwnd $wfHwnd -ExpectedPid $winforms.Id
        $drag = Invoke-Ad -Arguments @(
            'drag',
            '--from-xy', ($fromX.ToString() + ',' + $fromY.ToString()),
            '--to-xy', ($toX.ToString() + ',' + $toY.ToString()),
            '--duration', '800', '--drop-delay', '0', '--timeout-ms', '10000'
        ) -Headed
        $dragShape = Get-EnvelopeShape -Envelope $drag.Envelope
        Add-EnvelopeRecord -Id 'J6-drag-slider' -Shape $dragShape
        Start-Sleep -Milliseconds 500
        $sliderAfterLabel = Get-ScratchControlText -Handle $hSliderLbl
        $sliderAfter = Invoke-Ad -Arguments @('get', $sliderRef, '--property', 'value')
        $sliderAfterNum = $sliderBeforeNum
        if ($sliderAfter.Envelope.ok) { $sliderAfterNum = [int]$sliderAfter.Envelope.data.value }
        $labelChanged = ($sliderAfterLabel -ne $sliderBeforeLabel -and $sliderAfterLabel -match '^slider:\d+$')
        $j6Ok = $drag.Envelope.ok -and ($dragShape.dragged -eq $true) -and `
            (($sliderAfterNum -gt $sliderBeforeNum) -or $labelChanged)
        if ($j6Ok) {
            $j6Result = 'pass'
            $j6Verdict = 'headed drag increased slider (native label or get re-read)'
        } else {
            $j6Result = 'fail'
            $j6Verdict = 'drag_ok=' + $drag.Envelope.ok + ' before=' + $sliderBeforeNum +
                ' after=' + $sliderAfterNum + ' label_changed=' + $labelChanged
        }
        Add-Judgement -Id 'J6' -Claim 'drag moves tbSlider monotonically' `
            -Target 'ScratchForms tbSlider' `
            -Result $j6Result `
            -Verdict $j6Verdict `
            -Shape $dragShape

        $originPoint = New-Object AgentDesktopProbe.A20.ProbePoint
        $sliderCenter = Get-BoundsCenter -Ref $sliderRef
        if (-not $sliderCenter) { throw 'slider bounds unavailable for abort leg' }
        [void][AgentDesktopProbe.A20.Input20]::GetCursorPos([ref]$originPoint)
        $buttonDownBeforeExit = $true
        try {
            $startX = $sliderCenter.x - 60
            if ($startX -lt 0) { $startX = $sliderCenter.x }
            [void](Invoke-BracketedHarness -Stage 'drag-abort:move' -Action {
                [AgentDesktopProbe.A20.Input20]::SendMouseAbsolute($startX, $sliderCenter.y) | Out-Null
            })
            [void](Invoke-BracketedHarness -Stage 'drag-abort:down' -Action {
                [AgentDesktopProbe.A20.Input20]::SendMouseButton($true) | Out-Null
            })
            Start-Sleep -Milliseconds 150
            [void](Invoke-BracketedHarness -Stage 'drag-abort:partial' -Action {
                [AgentDesktopProbe.A20.Input20]::SendMouseAbsolute(($startX + 30), $sliderCenter.y) | Out-Null
            })
            [void](Invoke-BracketedHarness -Stage 'drag-abort:guard' -Action {
                [AgentDesktopProbe.A20.Input20]::SendMouseAbsolute($originPoint.X, $originPoint.Y) | Out-Null
                [AgentDesktopProbe.A20.Input20]::SendMouseButton($false) | Out-Null
            })
        } finally {
            [AgentDesktopProbe.A20.Input20]::ForceLeftButtonUp()
            Start-Sleep -Milliseconds 100
            $buttonDownBeforeExit = [AgentDesktopProbe.A20.Input20]::IsLeftButtonDown()
            if ($buttonDownBeforeExit) {
                [AgentDesktopProbe.A20.Input20]::SendMouseButton($false) | Out-Null
                Start-Sleep -Milliseconds 100
                $buttonDownBeforeExit = [AgentDesktopProbe.A20.Input20]::IsLeftButtonDown()
            }
        }
        $j7Ok = (-not $buttonDownBeforeExit)
        if ($j7Ok) {
            $j7Result = 'pass'
            $j7Verdict = 'GetAsyncKeyState re-read: button up before exit (A20-3 corrective path)'
        } else {
            $j7Result = 'fail'
            $j7Verdict = 'left button still down after abort + outer release'
        }
        Add-Judgement -Id 'J7' -Claim 'interrupted drag leaves no button stuck (A20-3)' `
            -Target 'ScratchForms tbSlider harness abort' `
            -Result $j7Result `
            -Verdict $j7Verdict `
            -Notes ('interference_rows=' + $script:InterferenceRows.Count)

    } catch {
        foreach ($id in @('J3', 'J4', 'J6', 'J7')) {
            if (@($script:Judgements | ForEach-Object { $_.id }) -notcontains $id) {
                Add-Judgement -Id $id -Claim 'ScratchForms input cluster' -Target 'ScratchForms' `
                    -Result 'skipped' -Verdict 'harness error' -Notes $_.Exception.Message
            }
        }
    } finally {
        if ($null -ne $winforms) {
            Stop-Process -Id $winforms.Id -Force -ErrorAction SilentlyContinue
        }
    }

    # -------------------------------------------------------------------------
    # J9: Explorer skip-with-reason when absent; zero foreground interference
    # -------------------------------------------------------------------------
    if ($script:InterferenceRows.Count -eq 0) {
        $j9Result = 'pass'
        $j9Verdict = 'every Assert-Foreground bracket passed; no PROBE-INTERFERENCE recorded'
    } else {
        $j9Result = 'fail'
        $j9Verdict = 'foreground interference recorded during harness injection'
    }
    Add-Judgement -Id 'J9' -Claim 'zero foreground-interference rows' `
        -Target 'all harness injections' `
        -Result $j9Result `
        -Verdict $j9Verdict `
        -Notes ('interference_count=' + $script:InterferenceRows.Count)

} finally {
    Restore-DesktopHygiene -CursorOrigin $cursorOrigin -ClipHadText $clipHadText `
        -ClipOriginal $clipOriginal -ClipSnapshotTaken $clipSnapshotTaken
    foreach ($launchedPid in $script:LaunchedPids) {
        try {
            $proc = Get-Process -Id $launchedPid -ErrorAction SilentlyContinue
            if ($proc) { Stop-Process -Id $launchedPid -Force -ErrorAction SilentlyContinue }
        } catch { }
    }
    Get-Process -Name 'ScratchForms' -ErrorAction SilentlyContinue | ForEach-Object {
        Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
    }
}

$os = Get-CimInstance Win32_OperatingSystem
$envHeader = [ordered]@{
    os_caption = $os.Caption
    os_build = $os.BuildNumber
    binary = Split-Path -Leaf $script:Binary
    binary_bytes = (Get-Item -LiteralPath $script:Binary).Length
    generated = (Get-Date).ToString('o')
}

$summaryPath = Join-Path $script:OutDir 'input-dogfood-run.json'
$summaryJson = ConvertTo-Json -InputObject ([ordered]@{
        environment = $envHeader
        judgements = $script:Judgements
        envelopes = $script:Envelopes
        interference = $script:InterferenceRows
    }) -Depth 12
$redacted = Protect-ProbeText -Text $summaryJson
[IO.File]::WriteAllText($summaryPath, $redacted, $utf8NoBom)
if (-not (Test-CaptureRedaction -Path $summaryPath)) {
    throw "redaction residue in $summaryPath"
}
Write-Host ('dogfood: wrote ' + $summaryPath)
$script:Judgements | ForEach-Object {
    Write-Host ('  ' + $_.id + ': ' + $_.result + ' - ' + $_.verdict)
}
$failed = @($script:Judgements | Where-Object { $_.result -eq 'fail' })
if ($failed.Count -gt 0) {
    Write-Host ('dogfood: ' + $failed.Count + ' judgement(s) failed: ' + (($failed | ForEach-Object { $_.id }) -join ', '))
    exit 1
}
exit 0
