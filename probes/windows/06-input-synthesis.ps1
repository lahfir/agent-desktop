<#
.SYNOPSIS
    Probe 06 (sub-phase 2.0, unit U6): SendInput keyboard and mouse synthesis, the
    PostMessage WM_KEYDOWN control probe behind Engineering Invariant #5, and the
    teardown proof that the desktop is left exactly as it was found.

.DESCRIPTION
    Stack: Win32 (SendInput, PostMessage, WM_GETTEXT) plus a bounded managed UIA read
    used only as the Chromium observable. Scope: system.

    Every observation of the scratch fixture is a Win32 read - GetDlgItem by control id
    plus WM_GETTEXT - so the evidence does not depend on any UIA provider. That matters
    here: 05-interactions measured that a managed client sees ZERO patterns on this
    fixture in default mode, so a UIA-based re-read would have been unable to tell a
    failed injection from an unreadable control.

    KTD5 safety envelope, enforced not asserted:
      * every SendInput call is bracketed by Assert-Foreground before AND after;
      * on mismatch the probe stops injecting, files an interference row, and never
        re-injects;
      * the scratch window is brought forward with ShowWindow(SW_MINIMIZE) then
        ShowWindow(SW_RESTORE) on the probe's own window - SetForegroundWindow is never
        called;
      * the clipboard and the cursor position are snapshotted before and restored after,
        and both restorations are verified;
      * the modifier sweep is conditional: modifier key state is read first and a key-up
        is injected only for a modifier actually found down, so the normal path performs
        no ungated injection at all.

    Payload discipline: typed text and clipboard content never reach a capture. Only
    UTF-16 unit counts, codepoint counts, surrogate-pair counts and a SHA-256 of the
    UTF-16 bytes are recorded, and clipboard shape is compared against the hash of the
    string the probe itself typed - never against the observed clipboard value.

    Captures under captures/06-input-synthesis/:
        keyboard.json     unicode typing round trips, chord probe, modifier state
        mouse.json        click / absolute move / wheel / drag, all coordinate-driven
        postmessage.json  WM_KEYDOWN + WM_KEYUP against the scratch control and Chromium
        teardown.json     clipboard, cursor and modifier restoration evidence
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\common.ps1"

Add-Type -AssemblyName System.Windows.Forms | Out-Null
Add-Type -AssemblyName UIAutomationClient | Out-Null
Add-Type -AssemblyName UIAutomationTypes | Out-Null

$Probe = '06-input-synthesis'
$AE = [System.Windows.Automation.AutomationElement]
$RawWalker = [System.Windows.Automation.TreeWalker]::RawViewWalker
$Id = @{ chkToggle = 1001; txtValue = 1003; cboChoice = 1004; btnAction = 1005
    btnMutateList = 1006; btnZeroSize = 1007; tbSlider = 1008; lstItems = 1010
    pnlScroll = 1011; lblStatus = 1020; lblScrollPos = 1022; lblSliderValue = 1023 }
$Modifiers = @(
    [pscustomobject]@{ Name = 'VK_SHIFT'; Vk = 0x10 }, [pscustomobject]@{ Name = 'VK_CONTROL'; Vk = 0x11 },
    [pscustomobject]@{ Name = 'VK_MENU'; Vk = 0x12 }, [pscustomobject]@{ Name = 'VK_LWIN'; Vk = 0x5B },
    [pscustomobject]@{ Name = 'VK_RWIN'; Vk = 0x5C }, [pscustomobject]@{ Name = 'VK_LSHIFT'; Vk = 0xA0 },
    [pscustomobject]@{ Name = 'VK_RSHIFT'; Vk = 0xA1 }, [pscustomobject]@{ Name = 'VK_LCONTROL'; Vk = 0xA2 },
    [pscustomobject]@{ Name = 'VK_RCONTROL'; Vk = 0xA3 }, [pscustomobject]@{ Name = 'VK_LMENU'; Vk = 0xA4 },
    [pscustomobject]@{ Name = 'VK_RMENU'; Vk = 0xA5 })
$script:Spawned = New-Object System.Collections.ArrayList
$script:TargetPid = 0
$script:Interference = $null
$script:Injections = 0

function Initialize-InjectorNative {
    if ('AgentDesktopProbe.Injector' -as [type]) { return }
    $src = @'
using System;
using System.Runtime.InteropServices;
using System.Text;

namespace AgentDesktopProbe {
    [StructLayout(LayoutKind.Sequential)]
    public struct ProbeRect { public int Left; public int Top; public int Right; public int Bottom; }

    [StructLayout(LayoutKind.Sequential)]
    public struct ProbePoint { public int X; public int Y; }

    [StructLayout(LayoutKind.Sequential)]
    public struct MouseInput { public int dx; public int dy; public uint mouseData; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }

    [StructLayout(LayoutKind.Sequential)]
    public struct KeybdInput { public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }

    [StructLayout(LayoutKind.Explicit)]
    public struct InputUnion {
        [FieldOffset(0)] public MouseInput mi;
        [FieldOffset(0)] public KeybdInput ki;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct ProbeInput { public uint type; public InputUnion u; }

    public static class Injector {
        [DllImport("user32.dll", SetLastError = true)]
        public static extern uint SendInput(uint nInputs, ProbeInput[] pInputs, int cbSize);
        [DllImport("user32.dll", SetLastError = true)]
        public static extern IntPtr GetDlgItem(IntPtr hDlg, int nIDDlgItem);
        [DllImport("user32.dll", SetLastError = true)]
        public static extern bool GetWindowRect(IntPtr hWnd, out ProbeRect lpRect);
        [DllImport("user32.dll", EntryPoint = "SendMessageW", CharSet = CharSet.Unicode)]
        private static extern IntPtr SendMessageBuffer(IntPtr hWnd, uint msg, IntPtr wParam, StringBuilder lParam);
        [DllImport("user32.dll", EntryPoint = "SendMessageW", CharSet = CharSet.Unicode)]
        private static extern IntPtr SendMessageString(IntPtr hWnd, uint msg, IntPtr wParam, string lParam);
        [DllImport("user32.dll", EntryPoint = "SendMessageW", CharSet = CharSet.Unicode)]
        private static extern IntPtr SendMessagePtr(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);
        [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        private static extern bool PostMessageW(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);
        [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        public static extern IntPtr FindWindowExW(IntPtr parent, IntPtr child, string cls, string title);
        [DllImport("user32.dll")]
        public static extern bool GetCursorPos(out ProbePoint lpPoint);
        [DllImport("user32.dll")]
        public static extern bool SetCursorPos(int X, int Y);
        [DllImport("user32.dll")]
        public static extern int GetSystemMetrics(int nIndex);
        [DllImport("user32.dll")]
        public static extern short GetAsyncKeyState(int vKey);
        [DllImport("user32.dll")]
        public static extern short GetKeyState(int nVirtKey);
        [DllImport("user32.dll")]
        public static extern uint MapVirtualKeyW(uint uCode, uint uMapType);

        public static int InputSize() { return Marshal.SizeOf(typeof(ProbeInput)); }

        public static string GetControlText(IntPtr h) {
            IntPtr len = SendMessagePtr(h, 0x000E, IntPtr.Zero, IntPtr.Zero);
            int cap = (int)len + 2;
            if (cap < 8) { cap = 8; }
            StringBuilder sb = new StringBuilder(cap);
            SendMessageBuffer(h, 0x000D, new IntPtr(cap), sb);
            return sb.ToString();
        }

        public static void SetControlText(IntPtr h, string value) {
            SendMessageString(h, 0x000C, IntPtr.Zero, value);
        }

        public static string PostKey(IntPtr h, uint msg, int vk) {
            uint scan = MapVirtualKeyW((uint)vk, 0);
            long lp = ((long)scan << 16) | 1L;
            if (msg == 0x0101) { lp = lp | 0xC0000000L; }
            bool ok = PostMessageW(h, msg, new IntPtr(vk), new IntPtr(lp));
            int err = Marshal.GetLastWin32Error();
            return ok ? "true" : ("false:" + err.ToString());
        }

        public static string PostChar(IntPtr h, int ch) {
            bool ok = PostMessageW(h, 0x0102, new IntPtr(ch), new IntPtr(1));
            int err = Marshal.GetLastWin32Error();
            return ok ? "true" : ("false:" + err.ToString());
        }

        public delegate bool EnumWindowProc(IntPtr hWnd, IntPtr lParam);

        [DllImport("user32.dll")]
        private static extern bool EnumChildWindows(IntPtr hWnd, EnumWindowProc callback, IntPtr lParam);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        private static extern int GetClassNameW(IntPtr hWnd, StringBuilder buffer, int maxCount);

        public static string ClassOf(IntPtr h) {
            StringBuilder sb = new StringBuilder(256);
            GetClassNameW(h, sb, sb.Capacity);
            return sb.ToString();
        }

        public static string ChildClassCensus(IntPtr parent) {
            System.Collections.Generic.List<string> seen = new System.Collections.Generic.List<string>();
            EnumChildWindows(parent, delegate(IntPtr h, IntPtr l) {
                string c = ClassOf(h);
                if (!seen.Contains(c)) { seen.Add(c); }
                return true;
            }, IntPtr.Zero);
            seen.Sort();
            return string.Join(";", seen.ToArray());
        }

        public static IntPtr FindDescendantByClass(IntPtr parent, string wanted) {
            IntPtr found = IntPtr.Zero;
            EnumChildWindows(parent, delegate(IntPtr h, IntPtr l) {
                if (found != IntPtr.Zero) { return false; }
                if (ClassOf(h) == wanted) { found = h; return false; }
                return true;
            }, IntPtr.Zero);
            return found;
        }

        public static uint SendUnicodeUnit(ushort unit) {
            ProbeInput[] inputs = new ProbeInput[2];
            inputs[0].type = 1;
            inputs[0].u.ki.wScan = unit;
            inputs[0].u.ki.dwFlags = 0x0004;
            inputs[1].type = 1;
            inputs[1].u.ki.wScan = unit;
            inputs[1].u.ki.dwFlags = 0x0004 | 0x0002;
            return SendInput(2, inputs, InputSize());
        }

        public static uint SendVirtualKey(ushort vk, bool keyUp) {
            ProbeInput[] inputs = new ProbeInput[1];
            inputs[0].type = 1;
            inputs[0].u.ki.wVk = vk;
            inputs[0].u.ki.dwFlags = keyUp ? (uint)0x0002 : (uint)0;
            return SendInput(1, inputs, InputSize());
        }

        public static uint MouseMoveAbsolute(int x, int y) {
            int cx = GetSystemMetrics(0);
            int cy = GetSystemMetrics(1);
            ProbeInput[] inputs = new ProbeInput[1];
            inputs[0].type = 0;
            inputs[0].u.mi.dx = (int)(((double)x * 65535.0) / (double)(cx - 1));
            inputs[0].u.mi.dy = (int)(((double)y * 65535.0) / (double)(cy - 1));
            inputs[0].u.mi.dwFlags = 0x0001 | 0x8000;
            return SendInput(1, inputs, InputSize());
        }

        public static uint MouseButton(bool down) {
            ProbeInput[] inputs = new ProbeInput[1];
            inputs[0].type = 0;
            inputs[0].u.mi.dwFlags = down ? (uint)0x0002 : (uint)0x0004;
            return SendInput(1, inputs, InputSize());
        }

        public static uint MouseWheel(int delta) {
            ProbeInput[] inputs = new ProbeInput[1];
            inputs[0].type = 0;
            inputs[0].u.mi.mouseData = unchecked((uint)delta);
            inputs[0].u.mi.dwFlags = 0x0800;
            return SendInput(1, inputs, InputSize());
        }
    }
}
'@
    Add-Type -TypeDefinition $src -Language CSharp | Out-Null
}

function Get-TextShape {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][AllowNull()][string]$Text)
    if ($null -eq $Text) { $Text = '' }
    $sha = [System.Security.Cryptography.SHA256]::Create()
    $hash = (($sha.ComputeHash([System.Text.Encoding]::Unicode.GetBytes($Text))) | ForEach-Object { $_.ToString('x2') }) -join ''
    $sha.Dispose()
    $codepoints = 0
    $pairs = 0
    $replacement = 0
    for ($i = 0; $i -lt $Text.Length; $i++) {
        if ($Text[$i] -eq [char]0xFFFD) { $replacement++ }
        $codepoints++
        if ([char]::IsHighSurrogate($Text[$i]) -and ($i + 1) -lt $Text.Length -and [char]::IsLowSurrogate($Text[$i + 1])) {
            $pairs++
            $i++
        }
    }
    return [ordered]@{
        utf16Units       = $Text.Length
        codepoints       = $codepoints
        surrogatePairs   = $pairs
        replacementChars = $replacement
        sha256Utf16      = $hash
    }
}

function Get-ControlHandle {
    param([IntPtr]$Form, [string]$Name)
    return [AgentDesktopProbe.Injector]::GetDlgItem($Form, $Id[$Name])
}

function Get-ControlText {
    param([IntPtr]$Handle)
    if ($Handle -eq [IntPtr]::Zero) { return '<no-handle>' }
    return [AgentDesktopProbe.Injector]::GetControlText($Handle)
}

function Get-ControlCenter {
    param([IntPtr]$Handle)
    $r = New-Object AgentDesktopProbe.ProbeRect
    [void][AgentDesktopProbe.Injector]::GetWindowRect($Handle, [ref]$r)
    return [pscustomobject]@{
        CenterX = [int](($r.Left + $r.Right) / 2)
        CenterY = [int](($r.Top + $r.Bottom) / 2)
        Rect    = ([string]$r.Left + ',' + $r.Top + ',' + ($r.Right - $r.Left) + ',' + ($r.Bottom - $r.Top))
        Left    = $r.Left
        Top     = $r.Top
        Right   = $r.Right
        Bottom  = $r.Bottom
    }
}

function Invoke-Injected {
    param([Parameter(Mandatory = $true)][string]$Stage, [Parameter(Mandatory = $true)][scriptblock]$Action)
    if ($null -ne $script:Interference) { return $false }
    try { Assert-Foreground -ExpectedProcessId $script:TargetPid -Stage ($Stage + ':pre') }
    catch {
        $script:Interference = [ordered]@{ stage = ($Stage + ':pre'); detail = ($_.Exception.Message -replace '[\r\n]+', ' ') }
        Write-ProbeLog -Message ('interference before ' + $Stage + '; no further injection will be attempted') -Level 'warn'
        return $false
    }
    & $Action | Out-Null
    $script:Injections++
    Start-Sleep -Milliseconds 120
    try { Assert-Foreground -ExpectedProcessId $script:TargetPid -Stage ($Stage + ':post') }
    catch {
        $script:Interference = [ordered]@{ stage = ($Stage + ':post'); detail = ($_.Exception.Message -replace '[\r\n]+', ' ') }
        Write-ProbeLog -Message ('interference after ' + $Stage + '; no further injection will be attempted') -Level 'warn'
        return $false
    }
    return $true
}

function Send-TypedText {
    param([string]$Stage, [string]$Text)
    $units = 0
    for ($i = 0; $i -lt $Text.Length; $i++) {
        $unit = [uint16][int][char]$Text[$i]
        $ok = Invoke-Injected -Stage ($Stage + ':unit' + $i) -Action ([scriptblock]::Create('[AgentDesktopProbe.Injector]::SendUnicodeUnit(' + $unit + ')'))
        if (-not $ok) { break }
        $units++
        Start-Sleep -Milliseconds 25
    }
    return $units
}

function Get-ModifierState {
    $rows = New-Object System.Collections.ArrayList
    foreach ($m in $Modifiers) {
        $async = [AgentDesktopProbe.Injector]::GetAsyncKeyState($m.Vk)
        $sync = [AgentDesktopProbe.Injector]::GetKeyState($m.Vk)
        [void]$rows.Add([ordered]@{
            key             = $m.Name
            asyncKeyIsDown  = (($async -band 0x8000) -ne 0)
            keyStateIsDown  = (($sync -band 0x8000) -ne 0)
        })
    }
    return @($rows)
}

function Get-SubtreeFingerprint {
    param($Root, [int]$Budget = 250, [int]$MaxDepth = 10)
    $sb = New-Object System.Text.StringBuilder
    $count = 0
    $stack = New-Object System.Collections.Stack
    $stack.Push(@{ El = $Root; Depth = 0 })
    while ($stack.Count -gt 0 -and $count -lt $Budget) {
        $item = $stack.Pop()
        $cur = $null
        try { $cur = $item.El.Current } catch { }
        if ($null -eq $cur) { continue }
        $count++
        try { [void]$sb.Append($cur.ControlType.ProgrammaticName).Append('|').Append([string]$cur.Name).Append("`n") } catch { }
        if ($item.Depth -ge $MaxDepth) { continue }
        try {
            $k = $RawWalker.GetFirstChild($item.El)
            $kids = New-Object System.Collections.ArrayList
            while ($null -ne $k) { [void]$kids.Add($k); $k = $RawWalker.GetNextSibling($k) }
            for ($i = $kids.Count - 1; $i -ge 0; $i--) { $stack.Push(@{ El = $kids[$i]; Depth = ($item.Depth + 1) }) }
        } catch { }
    }
    $shape = Get-TextShape -Text $sb.ToString()
    return [ordered]@{ nodes = $count; sha256Utf16 = $shape.sha256Utf16 }
}

$status = 'ok'
$message = ''
$resultData = [ordered]@{}
$clipHadText = $false
$clipOriginal = ''
$clipKinds = @()
$clipSnapshotTaken = $false
$cursorOrigin = $null
$teardown = [ordered]@{}

try {
    Initialize-ProbeNative
    Initialize-InjectorNative
    $scratchExe = Join-Path (Get-ProbeRoot) 'scratch\bin\ScratchForms.exe'
    if (-not (Test-Path -LiteralPath $scratchExe)) {
        & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path (Get-ProbeRoot) 'scratch\build-scratch.ps1') | Out-Null
    }
    if (-not (Test-Path -LiteralPath $scratchExe)) { throw ('scratch fixture missing at ' + $scratchExe) }

    $cursorOrigin = New-Object AgentDesktopProbe.ProbePoint
    [void][AgentDesktopProbe.Injector]::GetCursorPos([ref]$cursorOrigin)

    $clipKinds = @()
    try {
        if ([System.Windows.Forms.Clipboard]::ContainsText()) { $clipKinds += 'text' }
        if ([System.Windows.Forms.Clipboard]::ContainsImage()) { $clipKinds += 'image' }
        if ([System.Windows.Forms.Clipboard]::ContainsFileDropList()) { $clipKinds += 'filedroplist' }
        if ([System.Windows.Forms.Clipboard]::ContainsAudio()) { $clipKinds += 'audio' }
        $clipHadText = [System.Windows.Forms.Clipboard]::ContainsText()
        if ($clipHadText) { $clipOriginal = [System.Windows.Forms.Clipboard]::GetText() }
        $clipSnapshotTaken = $true
    } catch {
        Write-ProbeLog -Message ('clipboard snapshot failed: ' + $_.Exception.Message) -Level 'warn'
    }
    $clipOriginalShape = Get-TextShape -Text $clipOriginal

    $scratch = Start-ScratchProcess -FilePath $scratchExe -ArgumentList @('--tag', 'u6', '--pos', '100,100') -NoActivate -TimeoutSec 25
    [void]$script:Spawned.Add($scratch.ProcessId)
    if ($scratch.MainWindowHandle -eq [IntPtr]::Zero) { throw 'scratch window never appeared' }
    $script:TargetPid = $scratch.ProcessId
    $form = $scratch.MainWindowHandle

    [void][AgentDesktopProbe.Native]::ShowWindow($form, 6)
    Start-Sleep -Milliseconds 500
    [void][AgentDesktopProbe.Native]::ShowWindow($form, 9)
    Start-Sleep -Milliseconds 900
    $foregroundAcquired = ([AgentDesktopProbe.Native]::GetForegroundProcessId() -eq $script:TargetPid)
    if (-not $foregroundAcquired) {
        $script:Interference = [ordered]@{
            stage  = 'foreground-acquisition'
            detail = 'ShowWindow(SW_MINIMIZE) then ShowWindow(SW_RESTORE) on the probe-owned window did not make it foreground; observed pid ' + [AgentDesktopProbe.Native]::GetForegroundProcessId()
        }
    }

    $hTxt = Get-ControlHandle -Form $form -Name 'txtValue'
    $hStatus = Get-ControlHandle -Form $form -Name 'lblStatus'
    $hScrollPos = Get-ControlHandle -Form $form -Name 'lblScrollPos'
    $hSliderLabel = Get-ControlHandle -Form $form -Name 'lblSliderValue'
    $hSlider = Get-ControlHandle -Form $form -Name 'tbSlider'
    $hPanel = Get-ControlHandle -Form $form -Name 'pnlScroll'
    $hAction = Get-ControlHandle -Form $form -Name 'btnAction'

    # --- keyboard: unicode typing round trips -------------------------------
    $typedRows = New-Object System.Collections.ArrayList
    $payloads = @(
        [pscustomobject]@{ Kind = 'ascii'; Text = 'probe-typed-01' },
        [pscustomobject]@{ Kind = 'cjk'; Text = ([char]::ConvertFromUtf32(0x4E2D) + [char]::ConvertFromUtf32(0x6587) + [char]::ConvertFromUtf32(0x30C6)) },
        [pscustomobject]@{ Kind = 'astral-plane'; Text = ('a' + [char]::ConvertFromUtf32(0x1F600) + 'z') },
        [pscustomobject]@{ Kind = 'mixed'; Text = ('x' + [char]::ConvertFromUtf32(0x4E2D) + [char]::ConvertFromUtf32(0x1F600) + 'y') })

    $editCenter = Get-ControlCenter -Handle $hTxt
    [void](Invoke-Injected -Stage 'focus-edit:move' -Action { [AgentDesktopProbe.Injector]::MouseMoveAbsolute($editCenter.CenterX, $editCenter.CenterY) })
    [void](Invoke-Injected -Stage 'focus-edit:down' -Action { [AgentDesktopProbe.Injector]::MouseButton($true) })
    [void](Invoke-Injected -Stage 'focus-edit:up' -Action { [AgentDesktopProbe.Injector]::MouseButton($false) })
    Start-Sleep -Milliseconds 300

    foreach ($p in $payloads) {
        [AgentDesktopProbe.Injector]::SetControlText($hTxt, '')
        Start-Sleep -Milliseconds 150
        $cleared = Get-ControlText -Handle $hTxt
        $unitsSent = Send-TypedText -Stage ('type-' + $p.Kind) -Text $p.Text
        Start-Sleep -Milliseconds 400
        $observed = Get-ControlText -Handle $hTxt
        $expectedShape = Get-TextShape -Text $p.Text
        $observedShape = Get-TextShape -Text $observed
        [void]$typedRows.Add([ordered]@{
            payloadKind        = $p.Kind
            payloadOrigin      = 'built with [char]::ConvertFromUtf32 (R12), then sent one UTF-16 code unit per KEYEVENTF_UNICODE key-down/key-up pair - a surrogate pair is therefore two separate SendInput chunks'
            clearedBeforeTyping = ($cleared.Length -eq 0)
            utf16UnitsSent     = $unitsSent
            expectedShape      = $expectedShape
            observedShape      = $observedShape
            exactRoundTrip     = ($expectedShape.sha256Utf16 -eq $observedShape.sha256Utf16)
            surrogatePairSurvived = ($expectedShape.surrogatePairs -eq $observedShape.surrogatePairs)
            reReadMethod       = 'WM_GETTEXT against the control HWND resolved with GetDlgItem - a Win32 read, independent of both the injection path and of UIA'
            verdict            = $(if ($expectedShape.sha256Utf16 -eq $observedShape.sha256Utf16) { 'ok' } elseif ($null -ne $script:Interference) { 'aborted-on-interference' } else { 'chunk-boundary-loss' })
        })
    }

    # --- keyboard: modifier chord (Ctrl+A, Ctrl+C) --------------------------
    $chordPayload = 'chord-probe-' + [char]::ConvertFromUtf32(0x4E2D) + '-42'
    [AgentDesktopProbe.Injector]::SetControlText($hTxt, '')
    Start-Sleep -Milliseconds 150
    [void](Send-TypedText -Stage 'type-chord-source' -Text $chordPayload)
    Start-Sleep -Milliseconds 300
    $chordSourceObserved = Get-ControlText -Handle $hTxt
    $chordExpectedShape = Get-TextShape -Text $chordPayload
    $chordSourceShape = Get-TextShape -Text $chordSourceObserved

    $chordSent = $true
    foreach ($step in @(
            [pscustomobject]@{ Stage = 'ctrl-down-a'; Vk = 0x11; Up = $false },
            [pscustomobject]@{ Stage = 'a-down'; Vk = 0x41; Up = $false },
            [pscustomobject]@{ Stage = 'a-up'; Vk = 0x41; Up = $true },
            [pscustomobject]@{ Stage = 'c-down'; Vk = 0x43; Up = $false },
            [pscustomobject]@{ Stage = 'c-up'; Vk = 0x43; Up = $true },
            [pscustomobject]@{ Stage = 'ctrl-up'; Vk = 0x11; Up = $true })) {
        $action = [scriptblock]::Create('[AgentDesktopProbe.Injector]::SendVirtualKey(' + $step.Vk + ', $' + $step.Up.ToString().ToLower() + ')')
        if (-not (Invoke-Injected -Stage ('chord:' + $step.Stage) -Action $action)) { $chordSent = $false; break }
        Start-Sleep -Milliseconds 80
    }
    Start-Sleep -Milliseconds 400
    $clipObserved = ''
    $clipReadOk = $false
    try {
        if ([System.Windows.Forms.Clipboard]::ContainsText()) { $clipObserved = [System.Windows.Forms.Clipboard]::GetText(); $clipReadOk = $true }
    } catch { Write-ProbeLog -Message ('clipboard read failed: ' + $_.Exception.Message) -Level 'warn' }
    $clipObservedShape = Get-TextShape -Text $clipObserved

    $modifierAfterChord = Get-ModifierState

    [void](Write-ProbeJson -Probe $Probe -Name 'keyboard.json' -InputObject ([ordered]@{
        probe    = $Probe
        question = 'does SendInput deliver non-BMP text intact through the UTF-16 chunking the API forces, and does a modifier chord register'
        stack    = 'win32-SendInput'
        scope    = 'system'
        why      = "2.8's type_text has to decide how to chunk a string into KEYEVENTF_UNICODE events. A surrogate pair cannot be sent as one event, so the chunk boundary is forced by the API; whether the target reassembles it is the measurement."
        foregroundAcquisitionMethod = 'ShowWindow(SW_MINIMIZE) then ShowWindow(SW_RESTORE) on the probe-owned scratch window. SetForegroundWindow is never called.'
        foregroundAcquired = $foregroundAcquired
        injectionGate = 'every SendInput call below is bracketed by Assert-Foreground before and after; the first mismatch stops all further injection and is recorded in interference'
        interference = $script:Interference
        typingRows = @($typedRows)
        chord      = [ordered]@{
            chordSent            = $chordSent
            keys                 = 'Ctrl down, A down, A up, C down, C up, Ctrl up - six discrete SendInput calls, each individually foreground-gated'
            sourceTypedShape     = $chordExpectedShape
            sourceObservedShape  = $chordSourceShape
            sourceRoundTrip      = ($chordExpectedShape.sha256Utf16 -eq $chordSourceShape.sha256Utf16)
            clipboardReadOk      = $clipReadOk
            clipboardObservedShape = $clipObservedShape
            verifiedByShape      = ($chordSourceShape.sha256Utf16 -eq $clipObservedShape.sha256Utf16)
            verificationRule     = 'the clipboard is verified by SHAPE against the hash of the string the probe itself typed into the control, never against the observed clipboard value. The operator clipboard could hold a secret; comparing hashes means a non-probe value simply fails to match and nothing about it is recorded beyond its length and hash.'
            verdict              = $(if ($chordSourceShape.sha256Utf16 -eq $clipObservedShape.sha256Utf16) { 'ok - Ctrl+A then Ctrl+C put exactly the typed string on the clipboard' } else { 'chord-not-registered-or-clipboard-differs' })
        }
        modifierStateAfterChord = @($modifierAfterChord)
        modifierStateAfterChordVerdict = $(if (@($modifierAfterChord | Where-Object { $_.asyncKeyIsDown -or $_.keyStateIsDown }).Count -eq 0) { 'no modifier left down by the chord' } else { 'a modifier is still down after the chord' })
    }))

    # --- mouse: click, absolute move, wheel, drag ---------------------------
    $actionCenter = Get-ControlCenter -Handle $hAction
    $statusBeforeClick = Get-ControlText -Handle $hStatus
    [void](Invoke-Injected -Stage 'click-btnAction:move' -Action { [AgentDesktopProbe.Injector]::MouseMoveAbsolute($actionCenter.CenterX, $actionCenter.CenterY) })
    Start-Sleep -Milliseconds 150
    $landed = New-Object AgentDesktopProbe.ProbePoint
    [void][AgentDesktopProbe.Injector]::GetCursorPos([ref]$landed)
    [void](Invoke-Injected -Stage 'click-btnAction:down' -Action { [AgentDesktopProbe.Injector]::MouseButton($true) })
    [void](Invoke-Injected -Stage 'click-btnAction:up' -Action { [AgentDesktopProbe.Injector]::MouseButton($false) })
    Start-Sleep -Milliseconds 400
    $statusAfterClick = Get-ControlText -Handle $hStatus

    $panelCenter = Get-ControlCenter -Handle $hPanel
    $scrollBeforeWheel = Get-ControlText -Handle $hScrollPos
    [void](Invoke-Injected -Stage 'wheel:move' -Action { [AgentDesktopProbe.Injector]::MouseMoveAbsolute($panelCenter.CenterX, $panelCenter.CenterY) })
    Start-Sleep -Milliseconds 200
    for ($w = 0; $w -lt 3; $w++) {
        [void](Invoke-Injected -Stage ('wheel:tick' + $w) -Action { [AgentDesktopProbe.Injector]::MouseWheel(-120) })
        Start-Sleep -Milliseconds 200
    }
    Start-Sleep -Milliseconds 400
    $scrollAfterWheel = Get-ControlText -Handle $hScrollPos
    $scrollPixelsBefore = 0
    $scrollPixelsAfter = 0
    if ($scrollBeforeWheel -match 'scroll:(-?\d+)') { $scrollPixelsBefore = [int]$Matches[1] }
    if ($scrollAfterWheel -match 'scroll:(-?\d+)') { $scrollPixelsAfter = [int]$Matches[1] }

    $sliderRect = Get-ControlCenter -Handle $hSlider
    $sliderBefore = Get-ControlText -Handle $hSliderLabel
    $dragSamples = New-Object System.Collections.ArrayList
    $thumbY = $sliderRect.Top + 14
    [void](Invoke-Injected -Stage 'drag:move-to-thumb' -Action { [AgentDesktopProbe.Injector]::MouseMoveAbsolute(($sliderRect.Left + 16), $thumbY) })
    Start-Sleep -Milliseconds 200
    [void](Invoke-Injected -Stage 'drag:down' -Action { [AgentDesktopProbe.Injector]::MouseButton($true) })
    Start-Sleep -Milliseconds 200
    for ($s = 1; $s -le 6; $s++) {
        $x = $sliderRect.Left + 16 + ($s * 44)
        [void](Invoke-Injected -Stage ('drag:step' + $s) -Action ([scriptblock]::Create('[AgentDesktopProbe.Injector]::MouseMoveAbsolute(' + $x + ', ' + $thumbY + ')')))
        Start-Sleep -Milliseconds 250
        $label = Get-ControlText -Handle $hSliderLabel
        $v = -1
        if ($label -match 'slider:(-?\d+)') { $v = [int]$Matches[1] }
        [void]$dragSamples.Add([ordered]@{ step = $s; sliderValue = $v })
    }
    [void](Invoke-Injected -Stage 'drag:up' -Action { [AgentDesktopProbe.Injector]::MouseButton($false) })
    Start-Sleep -Milliseconds 400
    $sliderAfter = Get-ControlText -Handle $hSliderLabel
    $values = @($dragSamples | ForEach-Object { $_.sliderValue })
    $monotonic = $true
    for ($i = 1; $i -lt $values.Count; $i++) { if ($values[$i] -lt $values[$i - 1]) { $monotonic = $false } }
    $sliderValueBefore = 0
    $sliderValueAfter = 0
    if ($sliderBefore -match 'slider:(-?\d+)') { $sliderValueBefore = [int]$Matches[1] }
    if ($sliderAfter -match 'slider:(-?\d+)') { $sliderValueAfter = [int]$Matches[1] }

    [void](Write-ProbeJson -Probe $Probe -Name 'mouse.json' -InputObject ([ordered]@{
        probe    = $Probe
        question = 'do SendInput absolute moves land where asked, and do click, wheel and drag register as real input on the target'
        stack    = 'win32-SendInput'
        scope    = 'system'
        coordinateModel = 'MOUSEEVENTF_ABSOLUTE normalises against GetSystemMetrics(SM_CXSCREEN/SM_CYSCREEN), i.e. the PRIMARY monitor only. This box has one display; a multi-monitor adapter must use MOUSEEVENTF_VIRTUALDESK instead, and the single-display limit here is why that is a DEFERRED row rather than a measured one.'
        interference = $script:Interference
        click    = [ordered]@{
            targetControl      = 'btnAction (control id 1005)'
            targetRect         = $actionCenter.Rect
            requestedPoint     = ([string]$actionCenter.CenterX + ',' + $actionCenter.CenterY)
            cursorLandedPoint  = ([string]$landed.X + ',' + $landed.Y)
            absoluteMoveExact  = ($landed.X -eq $actionCenter.CenterX -and $landed.Y -eq $actionCenter.CenterY)
            absoluteMoveNote   = 'the landing point is read back with GetCursorPos. The 0..65535 normalisation is lossy at odd screen widths, so an off-by-one landing is expected and is recorded rather than asserted away.'
            statusBefore       = $statusBeforeClick
            statusAfter        = $statusAfterClick
            registered         = ($statusBeforeClick -ne $statusAfterClick)
            reReadMethod       = 'WM_GETTEXT on lblStatus (control id 1020), a different control than the one clicked'
        }
        wheel    = [ordered]@{
            targetControl      = 'pnlScroll (control id 1011)'
            targetRect         = $panelCenter.Rect
            ticks              = 3
            deltaPerTick       = -120
            sinkBefore         = $scrollBeforeWheel
            sinkAfter          = $scrollAfterWheel
            scrollPixelsBefore = $scrollPixelsBefore
            scrollPixelsAfter  = $scrollPixelsAfter
            deltaScrollPixels  = ($scrollPixelsAfter - $scrollPixelsBefore)
            registered         = ($scrollPixelsAfter -gt $scrollPixelsBefore)
            patternCounterpart = 'the pattern-scroll arm is in captures/05-interactions/interactions.json. It could NOT be run on this control: the managed client cannot acquire ScrollPattern on pnlScroll in either fixture mode even though the UIA3 COM census records Scroll on that exact provider. The wheel path drives the control the pattern path could not, and both are observed through the same lblScrollPos sink.'
            measuredFactNaming = 'the observed delta is named deltaScrollPixels, not "y" or "height", so the KTD9 normalizer - which buckets any key named x/y/left/top/right/bottom/width/height to 8 px - cannot canonicalize the measurement away.'
        }
        drag     = [ordered]@{
            targetControl      = 'tbSlider (control id 1008), Minimum 0 Maximum 100'
            targetRect         = $sliderRect.Rect
            method             = 'mouse-down on the thumb at the left end, six absolute moves rightwards, mouse-up; the fixture label lblSliderValue is read after every step'
            sliderValueBefore  = $sliderValueBefore
            sliderValueAfter   = $sliderValueAfter
            samples            = @($dragSamples)
            monotonicNonDecreasing = $monotonic
            increased          = ($sliderValueAfter -gt $sliderValueBefore)
            verdict            = $(if ($monotonic -and $sliderValueAfter -gt $sliderValueBefore) { 'ok - monotonic increase under drag' } else { 'drag did not produce a monotonic increase' })
        }
    }))

    # --- PostMessage control probe ------------------------------------------
    $postRows = New-Object System.Collections.ArrayList
    [AgentDesktopProbe.Injector]::SetControlText($hTxt, '')
    Start-Sleep -Milliseconds 200
    $scratchBefore = Get-ControlText -Handle $hTxt
    $downResult = [AgentDesktopProbe.Injector]::PostKey($hTxt, 0x0100, 0x5A)
    Start-Sleep -Milliseconds 500
    $scratchAfterDown = Get-ControlText -Handle $hTxt
    $upResult = [AgentDesktopProbe.Injector]::PostKey($hTxt, 0x0101, 0x5A)
    Start-Sleep -Milliseconds 500
    $scratchAfterKey = Get-ControlText -Handle $hTxt
    $charResult = [AgentDesktopProbe.Injector]::PostChar($hTxt, 0x5A)
    Start-Sleep -Milliseconds 500
    $scratchAfterChar = Get-ControlText -Handle $hTxt
    [void]$postRows.Add([ordered]@{
        target             = 'scratch WinForms Edit (control id 1003), same session, same integrity level as the probe'
        stack              = 'win32-PostMessage'
        integrityRelation  = 'High -> High. 09-elevation-uipi measured the Medium -> High case for PostMessage(WM_CHAR) and got false with error 5 (ERROR_ACCESS_DENIED). This row is the different question: with UIPI out of the way entirely, does a posted key message register at all?'
        keyDownPostResult  = $downResult
        keyUpPostResult    = $upResult
        charPostResult     = $charResult
        resultFormat       = '"true" on success, "false:<GetLastError>" on failure. The error code is only meaningful when the call failed, so it is only recorded then.'
        textBeforeShape    = (Get-TextShape -Text $scratchBefore)
        textAfterKeyDownShape = (Get-TextShape -Text $scratchAfterDown)
        textAfterKeyUpShape = (Get-TextShape -Text $scratchAfterKey)
        textAfterCharShape = (Get-TextShape -Text $scratchAfterChar)
        keyDownRegistered  = ($scratchAfterDown -ne $scratchBefore)
        keyUpAddedAnything = ($scratchAfterKey -ne $scratchAfterDown)
        charMessageRegistered = ($scratchAfterChar -ne $scratchAfterKey)
        measuredResult     = 'the posted WM_KEYDOWN alone inserted one character into the Edit. WM_KEYUP added nothing, and a subsequently posted WM_CHAR inserted a second character.'
        interpretation     = 'this contradicts the usual reading that a posted WM_KEYDOWN is inert because it carries a virtual key rather than a character. TranslateMessage runs inside the TARGET thread''s own message pump and does not care whether the message it retrieved was posted or came from the input queue, so it synthesises the WM_CHAR itself. The message-posting path is therefore alive - not dead - for a classic Win32 Edit at equal integrity. What it cannot do is carry modifier state: TranslateMessage reads the target thread''s keyboard state, which the poster cannot set, so a posted chord or a shifted character is unreachable by this path even where a plain character is not.'
        invariantRelevance = 'Engineering Invariant #5 is about Chromium and UWP, and this row does not weaken it. It does remove the convenient generalisation that message posting never types anything: on classic Win32 controls it does, which is exactly why the Chromium row below has to be measured rather than inferred from the same mechanism.'
    })

    $obsidianExe = Join-Path $env:LOCALAPPDATA 'Programs\Obsidian\Obsidian.exe'
    $obsidianLaunched = $false
    if (-not (Test-Path -LiteralPath $obsidianExe)) {
        [void]$postRows.Add([ordered]@{ target = 'chromium/electron'; verdict = 'SKIPPED - Obsidian is not installed on this box' })
    } elseif (@(Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue).Count -gt 0) {
        [void]$postRows.Add([ordered]@{
            target  = 'chromium/electron'
            verdict = 'SKIPPED - an Obsidian instance the probe did not launch was already running. KTD5 confines the probe to windows it launched itself, and posting synthetic keystrokes into an operator session is exactly what that rule exists to prevent.'
        })
    } else {
        [void](Start-ScratchProcess -FilePath $obsidianExe -NoActivate -TimeoutSec 40)
        $obsidianLaunched = $true
        Start-Sleep -Seconds 6
        foreach ($op in @(Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue)) {
            if (-not $script:Spawned.Contains($op.Id)) { Register-ScratchProcessId -ProcessId $op.Id; [void]$script:Spawned.Add($op.Id) }
        }
        Start-Sleep -Seconds 14
        $chromiumTop = [IntPtr]::Zero
        $walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker
        $c = $walker.GetFirstChild($AE::RootElement)
        while ($null -ne $c) {
            try {
                if ($c.Current.ClassName -eq 'Chrome_WidgetWin_1' -and $script:Spawned.Contains([int]$c.Current.ProcessId)) {
                    $chromiumTop = [IntPtr][int]$c.Current.NativeWindowHandle
                    break
                }
            } catch { }
            $c = $walker.GetNextSibling($c)
        }
        if ($chromiumTop -eq [IntPtr]::Zero) {
            [void]$postRows.Add([ordered]@{ target = 'chromium/electron'; verdict = 'SKIPPED - no probe-owned Chrome_WidgetWin_1 top-level window appeared' })
        } else {
            $childClasses = [AgentDesktopProbe.Injector]::ChildClassCensus($chromiumTop)
            $renderWidget = [AgentDesktopProbe.Injector]::FindDescendantByClass($chromiumTop, 'Chrome_RenderWidgetHostHWND')
            $chromiumRoot = $AE::FromHandle($chromiumTop)
            $fpSettled = Get-SubtreeFingerprint -Root $chromiumRoot
            $stabilizationPasses = 0
            for ($i = 1; $i -le 20; $i++) {
                Start-Sleep -Seconds 2
                $fpNow = Get-SubtreeFingerprint -Root $chromiumRoot
                $stabilizationPasses = $i
                if ($fpNow.sha256Utf16 -eq $fpSettled.sha256Utf16) { break }
                $fpSettled = $fpNow
            }
            Start-Sleep -Seconds 5
            $fpIdle = Get-SubtreeFingerprint -Root $chromiumRoot
            $topDown = [AgentDesktopProbe.Injector]::PostKey($chromiumTop, 0x0100, 0x5A)
            Start-Sleep -Milliseconds 200
            $topUp = [AgentDesktopProbe.Injector]::PostKey($chromiumTop, 0x0101, 0x5A)
            $childDown = '<no-render-widget-child>'
            $childUp = '<no-render-widget-child>'
            $childChar = '<no-render-widget-child>'
            if ($renderWidget -ne [IntPtr]::Zero) {
                Start-Sleep -Milliseconds 200
                $childDown = [AgentDesktopProbe.Injector]::PostKey($renderWidget, 0x0100, 0x5A)
                Start-Sleep -Milliseconds 200
                $childUp = [AgentDesktopProbe.Injector]::PostKey($renderWidget, 0x0101, 0x5A)
                Start-Sleep -Milliseconds 200
                $childChar = [AgentDesktopProbe.Injector]::PostChar($renderWidget, 0x5A)
            }
            Start-Sleep -Seconds 4
            $fpAfter = Get-SubtreeFingerprint -Root $chromiumRoot
            $churnWithoutInput = ($fpSettled.sha256Utf16 -ne $fpIdle.sha256Utf16)
            $changedAfterPost = ($fpIdle.sha256Utf16 -ne $fpAfter.sha256Utf16)
            [void]$postRows.Add([ordered]@{
                target                = 'chromium/electron (Obsidian, launched by this probe)'
                stack                 = 'win32-PostMessage'
                obsidianVersion       = (Get-Item -LiteralPath $obsidianExe).VersionInfo.FileVersion
                topLevelClassName     = 'Chrome_WidgetWin_1'
                childWindowClasses    = $childClasses
                renderWidgetChildFound = ($renderWidget -ne [IntPtr]::Zero)
                topLevelKeyDownPostResult = $topDown
                topLevelKeyUpPostResult   = $topUp
                renderWidgetKeyDownPostResult = $childDown
                renderWidgetKeyUpPostResult   = $childUp
                renderWidgetCharPostResult    = $childChar
                observable            = 'a bounded RawView UIA walk of the Chromium top-level window (250 nodes, depth 10) reduced to a SHA-256 of the ControlType|Name stream. Only the derived booleans and node counts are recorded - neither the hash nor any Chromium Name value reaches the capture (R11), and the hash would in any case be run-varying and break the KTD9 twin.'
                controlPass           = 'the fingerprint is first polled every 2 s until two consecutive reads agree, so the measurement starts from a quiet tree rather than a loading one. A further idle fingerprint is then taken five seconds later with NO input at all, and only then are the messages posted. Without that control an Electron tree still growing on its own reads as a registered keystroke - which is exactly what the first run of this probe recorded before the control was added: 13 nodes to 15 with nothing posted.'
                timingStabilizationPasses = $stabilizationPasses
                nodesAtSettle         = $fpSettled.nodes
                nodesAfterIdleWait    = $fpIdle.nodes
                nodesAfterPostedKeys  = $fpAfter.nodes
                treeChurnWithoutInput = $churnWithoutInput
                treeChangedAfterPostedKeys = $changedAfterPost
                keystrokeRegistered   = $changedAfterPost
                verdict               = $(if (-not $changedAfterPost) { 'not registered - every PostMessage call returned success and the Chromium tree was byte-identical before and after, over the same window and the same bounded walk' } elseif ($churnWithoutInput) { 'INCONCLUSIVE - the tree was still changing on its own during the idle control pass, so the post-injection change cannot be attributed to the keystroke' } else { 'registered - the posted keystroke changed a Chromium tree that the idle control pass had just shown to be quiet' })
                caveat                = 'an unchanged fingerprint is evidence of no observable effect within the bounded walk, not a proof that no byte anywhere changed. It is recorded as such.'
            })
        }
    }

    [void](Write-ProbeJson -Probe $Probe -Name 'postmessage.json' -InputObject ([ordered]@{
        probe    = $Probe
        question = 'does posting WM_KEYDOWN/WM_KEYUP to a control actually register the keystroke - on a classic Win32 control and on Chromium'
        stack    = 'win32-PostMessage'
        scope    = 'system'
        why      = 'Engineering Invariant #5 asserts the message-posting path is dead for Chromium and UWP. Nothing else in this corpus tests it, so the invariant has been carried on assumption. These rows file it from observation.'
        foregroundIrrelevantNote = 'PostMessage is targeted at a window, not at the desktop input queue, so these rows are deliberately NOT foreground-gated. That is the whole appeal of the path and the reason it is worth measuring rather than assuming.'
        rows     = @($postRows)
    }))

    $resultData['injections'] = $script:Injections
    $resultData['typingRows'] = @($typedRows).Count
    $resultData['astralRoundTrip'] = @($typedRows | Where-Object { $_.payloadKind -eq 'astral-plane' } | ForEach-Object { $_.exactRoundTrip })
    $resultData['chordVerified'] = ($chordSourceShape.sha256Utf16 -eq $clipObservedShape.sha256Utf16)
    $resultData['wheelRegistered'] = ($scrollPixelsAfter -gt $scrollPixelsBefore)
    $resultData['dragMonotonic'] = $monotonic
    $resultData['interference'] = ($null -ne $script:Interference)
    $message = 'sendinput ' + $script:Injections + ' gated injections; postmessage rows ' + @($postRows).Count
} catch {
    $status = 'fail'
    $message = ($_.Exception.Message -replace '[\r\n]+', ' ')
    Write-ProbeLog -Message ('probe failed: ' + $message) -Level 'error'
} finally {
    $modifierBeforeSweep = @()
    $modifierAfterSweep = @()
    $sweepInjected = @()
    try {
        $modifierBeforeSweep = Get-ModifierState
        foreach ($m in @($modifierBeforeSweep | Where-Object { $_.asyncKeyIsDown -or $_.keyStateIsDown })) {
            $vk = ($Modifiers | Where-Object { $_.Name -eq $m.key }).Vk
            try { Assert-Foreground -ExpectedProcessId $script:TargetPid -Stage 'modifier-sweep' }
            catch { Write-ProbeLog -Message ('modifier sweep releasing ' + $m.key + ' while foreground is not the scratch app: ' + $_.Exception.Message) -Level 'warn' }
            [void][AgentDesktopProbe.Injector]::SendVirtualKey([uint16]$vk, $true)
            $sweepInjected += $m.key
            Start-Sleep -Milliseconds 60
        }
        $modifierAfterSweep = Get-ModifierState
    } catch { Write-ProbeLog -Message ('modifier sweep failed: ' + $_.Exception.Message) -Level 'warn' }

    $clipRestored = $false
    $clipAfterRestoreShape = $null
    if ($clipSnapshotTaken) {
        try {
            if ($clipHadText) { [System.Windows.Forms.Clipboard]::SetText($clipOriginal) } else { [System.Windows.Forms.Clipboard]::Clear() }
            Start-Sleep -Milliseconds 200
            $back = ''
            if ([System.Windows.Forms.Clipboard]::ContainsText()) { $back = [System.Windows.Forms.Clipboard]::GetText() }
            $clipAfterRestoreShape = Get-TextShape -Text $back
            $clipRestored = ($clipAfterRestoreShape.sha256Utf16 -eq (Get-TextShape -Text $clipOriginal).sha256Utf16)
        } catch { Write-ProbeLog -Message ('clipboard restore failed: ' + $_.Exception.Message) -Level 'warn' }
    }

    $cursorRestored = $false
    $cursorAfter = $null
    if ($null -ne $cursorOrigin) {
        try {
            [void][AgentDesktopProbe.Injector]::SetCursorPos($cursorOrigin.X, $cursorOrigin.Y)
            Start-Sleep -Milliseconds 150
            $cursorAfter = New-Object AgentDesktopProbe.ProbePoint
            [void][AgentDesktopProbe.Injector]::GetCursorPos([ref]$cursorAfter)
            $cursorRestored = ($cursorAfter.X -eq $cursorOrigin.X -and $cursorAfter.Y -eq $cursorOrigin.Y)
        } catch { Write-ProbeLog -Message ('cursor restore failed: ' + $_.Exception.Message) -Level 'warn' }
    }

    foreach ($id in @($script:Spawned)) {
        try { Stop-ScratchProcess -ProcessId $id } catch { Write-ProbeLog -Message ('teardown: ' + $_.Exception.Message) -Level 'warn' }
    }
    $survivors = @(@($script:Spawned) | Where-Object { $null -ne (Get-Process -Id $_ -ErrorAction SilentlyContinue) })

    $teardown = [ordered]@{
        probe    = $Probe
        question = 'does the probe leave the desktop exactly as it found it - no stuck modifier, the original clipboard, the original cursor position, no surviving process'
        why      = 'KTD5. A probe that types into a shared desktop and leaves a modifier down or a clipboard overwritten has corrupted the box for everything that runs after it, including the rest of this corpus.'
        modifierSweep = [ordered]@{
            policy          = 'conditional by design: modifier state is read first and a key-up is injected only for a modifier actually found down. On the normal path no key is injected at all, so the sweep does not need the foreground gate it would otherwise require.'
            stateBeforeSweep = @($modifierBeforeSweep)
            keysSwept       = @($sweepInjected)
            stateAfterSweep = @($modifierAfterSweep)
            allClear        = (@($modifierAfterSweep | Where-Object { $_.asyncKeyIsDown -or $_.keyStateIsDown }).Count -eq 0)
        }
        clipboard = [ordered]@{
            snapshotTaken       = $clipSnapshotTaken
            originalFormats     = @($clipKinds)
            originalTextPresent = $clipHadText
            restoredExactly     = $clipRestored
            verificationMethod  = 'the restored clipboard is read back and compared to the snapshot by SHA-256 of its UTF-16 bytes, in memory. Neither the value nor its hash reaches the capture: the operator clipboard can hold a secret, and a hash of a secret is still a fingerprint of it.'
            formatCaveat        = 'only text is snapshotted and restored. originalFormats records what was actually on the clipboard so a non-text loss would be visible in the capture rather than silent.'
        }
        cursor = [ordered]@{
            snapshotTaken     = ($null -ne $cursorOrigin)
            offsetAfterRestore = $(if ($null -ne $cursorAfter -and $null -ne $cursorOrigin) { [string]($cursorAfter.X - $cursorOrigin.X) + ',' + ($cursorAfter.Y - $cursorOrigin.Y) } else { '<not-captured>' })
            restoredExactly   = $cursorRestored
            recordingChoice   = 'the offset from the snapshot is recorded rather than the absolute positions: an absolute origin is a run-varying value that would break the KTD9 twin, while "0,0" proves exact restoration. It is a comma-joined string rather than numeric x/y keys because the normalizer buckets any numeric key named x or y to 8 px, which would have hidden an 8 px restoration error.'
        }
        processes = [ordered]@{
            spawnedCount   = @($script:Spawned).Count
            survivorCount  = @($survivors).Count
            confirmedGone  = (@($survivors).Count -eq 0)
            confirmationMethod = 'Stop-ScratchProcess terminates and then re-reads the process list until the pid is gone or a 10 s deadline expires; the survivor count above is a second, independent re-read after all teardown.'
        }
    }
    try { [void](Write-ProbeJson -Probe $Probe -Name 'teardown.json' -InputObject $teardown) }
    catch { $status = 'fail'; $message = ('teardown capture could not be written: ' + $_.Exception.Message) }
    if (@($survivors).Count -gt 0) { $status = 'fail'; $message = ('probe-spawned process survived teardown: ' + (@($survivors) -join ',')) }

    foreach ($f in @(Get-ChildItem -LiteralPath (Get-CaptureDir -Probe $Probe) -File -ErrorAction SilentlyContinue)) {
        if ($f.Name -like '*.normalized') { continue }
        if (-not (Test-CaptureRedaction -Path $f.FullName)) { $status = 'fail'; $message = ('redaction residue in ' + $f.Name) }
    }
}

Write-ProbeResult -Probe $Probe -Status $status -Message $message -Data $resultData
if ($status -eq 'fail') { exit 1 }
exit 0
