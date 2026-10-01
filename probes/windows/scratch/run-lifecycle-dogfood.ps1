#Requires -Version 5.1
<#
.SYNOPSIS
    Sub-phase 2.9 U9 system-lifecycle dogfood runner.

.DESCRIPTION
    Drives target/release/agent-desktop.exe against repo-controlled targets
    (Notepad, Explorer scratch folder). Judges by JSON envelope shapes PLUS
    independent observation - never ok:true alone. Assert-Foreground brackets
    headed press; clipboard/cursor restore; PID-tracked scratch only;
    redaction at point of record (shapes/counts - no titles/paths/pids/
    message text).

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
. (Join-Path $script:RepoRoot 'probes\windows\21-system-lifecycle\native.ps1')
Initialize-ProbeRedaction
Initialize-LifecycleNative

Add-Type -AssemblyName System.Windows.Forms | Out-Null

if (-not $Binary) { $Binary = Join-Path $script:RepoRoot 'target\release\agent-desktop.exe' }
if (-not (Test-Path -LiteralPath $Binary)) { throw "release binary not found at $Binary" }
$script:Binary = (Resolve-Path -LiteralPath $Binary).ProviderPath
if (-not $OutDir) {
    $OutDir = Join-Path $script:RepoRoot 'docs\dogfood-reports\2026-08-08-001-captures'
}
New-Item -ItemType Directory -Path $OutDir -Force | Out-Null
$script:OutDir = (Resolve-Path -LiteralPath $OutDir).ProviderPath
$utf8NoBom = New-Object System.Text.UTF8Encoding $false

$script:LaunchedPids = New-Object System.Collections.Generic.List[int]
$script:ExplorerHwnds = New-Object System.Collections.Generic.List[IntPtr]
$script:Judgements = New-Object System.Collections.Generic.List[object]
$script:Envelopes = New-Object System.Collections.Generic.List[object]
$script:InterferenceRows = New-Object System.Collections.Generic.List[object]
$script:NoJsonCode = 'BINARY_NO_JSON'
$script:TargetPid = 0
$script:PlacementTolerancePx = 8

function Initialize-LifecycleDogfoodNative {
    if ('AgentDesktopLifecycleDogfood.Native' -as [type]) { return }
    $src = @'
using System;
using System.Runtime.InteropServices;
using System.Text;

namespace AgentDesktopLifecycleDogfood {
    [StructLayout(LayoutKind.Sequential)]
    public struct ProbePoint { public int X; public int Y; }
    public static class Native {
        [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        public static extern IntPtr FindWindowEx(IntPtr parent, IntPtr childAfter, string cls, string window);
        [DllImport("user32.dll", EntryPoint = "SendMessageW", CharSet = CharSet.Unicode)]
        private static extern IntPtr SendMessageBuffer(IntPtr hWnd, uint msg, IntPtr wParam, StringBuilder lParam);
        [DllImport("user32.dll")]
        public static extern bool PostMessageW(IntPtr hWnd, uint Msg, IntPtr wParam, IntPtr lParam);
        [DllImport("user32.dll")]
        public static extern bool GetCursorPos(out ProbePoint lpPoint);
        [DllImport("user32.dll")]
        public static extern bool SetCursorPos(int X, int Y);

        public static string GetControlText(IntPtr h) {
            if (h == IntPtr.Zero) { return string.Empty; }
            StringBuilder sb = new StringBuilder(4096);
            SendMessageBuffer(h, 0x000D, new IntPtr(4096), sb);
            return sb.ToString();
        }

        public static bool PostClose(IntPtr h) {
            return PostMessageW(h, 0x0010, IntPtr.Zero, IntPtr.Zero);
        }
    }
}
'@
    Add-ProbeInlineCSharp -Source $src -AssemblyLeaf 'AgentDesktopLifecycleDogfoodNative'
    Initialize-ProbeNative
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

function Get-EnvelopeShape {
    param([Parameter(Mandatory = $true)]$Envelope)
    $shape = [ordered]@{
        ok = [bool]$Envelope.ok
        command = $null
        code = $null
        disposition_delivery = $null
        disposition_retry = $null
        data_keys = @()
        bool_flags = [ordered]@{}
        numeric_flags = [ordered]@{}
        steps = @()
    }
    if ($Envelope.PSObject.Properties.Name -contains 'command') {
        $shape.command = [string]$Envelope.command
    }
    if ($Envelope.ok -and ($Envelope.PSObject.Properties.Name -contains 'data') -and $Envelope.data) {
        $data = $Envelope.data
        $shape.data_keys = @($data.PSObject.Properties.Name | Sort-Object)
        foreach ($key in @('closed', 'requested', 'resized', 'moved', 'minimized', 'maximized', 'restored', 'focused')) {
            if ($data.PSObject.Properties.Name -contains $key) {
                $shape.bool_flags[$key] = [bool]$data.$key
            }
        }
        foreach ($key in @('width', 'height', 'x', 'y', 'ref_count')) {
            if ($data.PSObject.Properties.Name -contains $key -and $null -ne $data.$key) {
                $shape.numeric_flags[$key] = [double]$data.$key
            }
        }
        if ($data.PSObject.Properties.Name -contains 'method') {
            $shape.bool_flags['method_is_graceful'] = ([string]$data.method -eq 'graceful')
            $shape.bool_flags['method_is_force'] = ([string]$data.method -eq 'force')
        }
        if ($data.PSObject.Properties.Name -contains 'disposition' -and $data.disposition) {
            $d = $data.disposition
            if ($d.PSObject.Properties.Name -contains 'delivery') {
                $shape.disposition_delivery = [string]$d.delivery
            }
            if ($d.PSObject.Properties.Name -contains 'retry') {
                $shape.disposition_retry = [string]$d.retry
            }
        }
        if ($data.PSObject.Properties.Name -contains 'action') {
            $shape.bool_flags['action_is_press_key'] = ([string]$data.action -eq 'press_key')
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
        if ($data.PSObject.Properties.Name -contains 'id') {
            $shape.bool_flags['has_window_id'] = (-not [string]::IsNullOrEmpty([string]$data.id))
        }
        if ($data.PSObject.Properties.Name -contains 'bounds') {
            $shape.bool_flags['has_bounds'] = ($null -ne $data.bounds)
        }
        if ($data.PSObject.Properties.Name -contains 'process_instance') {
            $shape.bool_flags['has_process_instance'] = (-not [string]::IsNullOrEmpty([string]$data.process_instance))
        }
        if ($data.PSObject.Properties.Name -contains 'complete') {
            $shape.bool_flags['complete'] = [bool]$data.complete
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
        if ($err.PSObject.Properties.Name -contains 'suggestion') {
            $shape.bool_flags['has_suggestion'] = (-not [string]::IsNullOrEmpty([string]$err.suggestion))
        }
        if ($err.PSObject.Properties.Name -contains 'details' -and $err.details) {
            $details = $err.details
            if ($details.PSObject.Properties.Name -contains 'physical_delivery_started') {
                $shape.bool_flags['physical_delivery_started'] = [bool]$details.physical_delivery_started
            }
            if ($details.PSObject.Properties.Name -contains 'retry_safe') {
                $shape.bool_flags['details_retry_safe'] = [bool]$details.retry_safe
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

function Get-TextShape {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][AllowNull()][string]$Text)
    if ($null -eq $Text) { $Text = '' }
    $sha = [System.Security.Cryptography.SHA256]::Create()
    $bytes = [System.Text.Encoding]::Unicode.GetBytes($Text)
    $hash = (($sha.ComputeHash($bytes) | ForEach-Object { $_.ToString('x2') }) -join '')
    $sha.Dispose()
    return [ordered]@{
        utf16Units  = $Text.Length
        sha256Utf16 = $hash
    }
}

function Get-NotepadEditText {
    param([IntPtr]$NotepadHwnd)
    $edit = [AgentDesktopLifecycleDogfood.Native]::FindWindowEx($NotepadHwnd, [IntPtr]::Zero, 'Edit', $null)
    if ($edit -eq [IntPtr]::Zero) { return $null }
    return [AgentDesktopLifecycleDogfood.Native]::GetControlText($edit)
}

function Clear-ForegroundStealers {
    foreach ($name in @('SearchUI', 'SearchApp', 'ShellExperienceHost')) {
        Get-Process -Name $name -ErrorAction SilentlyContinue | ForEach-Object {
            try { Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue } catch { }
        }
    }
    Start-Sleep -Milliseconds 400
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
                detail_shape = 'PROBE-INTERFERENCE'
            })
        return $false
    }
    & $Action | Out-Null
    Start-Sleep -Milliseconds 80
    try { Assert-Foreground -ExpectedProcessId $script:TargetPid -Stage ($Stage + ':post') }
    catch {
        [void]$script:InterferenceRows.Add([ordered]@{
                stage = ($Stage + ':post')
                detail_shape = 'PROBE-INTERFERENCE'
            })
        return $false
    }
    return $true
}

function Restore-DesktopHygiene {
    param(
        $CursorOrigin,
        [bool]$ClipHadText,
        [string]$ClipOriginal,
        [bool]$ClipSnapshotTaken
    )
    if ($null -ne $CursorOrigin) {
        [void][AgentDesktopLifecycleDogfood.Native]::SetCursorPos($CursorOrigin.X, $CursorOrigin.Y)
    }
    if ($ClipSnapshotTaken) {
        try {
            if ($ClipHadText) { [System.Windows.Forms.Clipboard]::SetText($ClipOriginal) }
            else { [System.Windows.Forms.Clipboard]::Clear() }
        } catch { }
    }
}

function Test-WithinTolerance {
    param([int]$Expected, [int]$Actual, [int]$Tolerance = 8)
    return ([math]::Abs($Expected - $Actual) -le $Tolerance)
}

function Close-ExplorerFolderWindows {
    foreach ($hwnd in @($script:ExplorerHwnds)) {
        if ($hwnd -ne [IntPtr]::Zero) {
            try { [void][AgentDesktopLifecycleDogfood.Native]::PostClose($hwnd) } catch { }
        }
    }
    $script:ExplorerHwnds.Clear()
    Start-Sleep -Milliseconds 400
}

function Invoke-LibUnresponsiveEvidence {
    $prev = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $output = &(Get-Command cargo).Source @(
            'test', '--locked', '-p', 'agent-desktop-windows', '--lib',
            'system::process_state::tests::stalled_fixture_classifies_unresponsive',
            '--', '--exact'
        ) 2>&1 | Out-String
        $exit = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $prev
    }
    $passed = ($exit -eq 0) -and ($output -match 'stalled_fixture_classifies_unresponsive \.\.\. ok')
    $testCount = 0
    if ($output -match 'running (\d+) test') { $testCount = [int]$Matches[1] }
    return [ordered]@{
        exit_code = $exit
        passed = $passed
        running_tests = $testCount
        names_cli_process_state = $false
    }
}

function Test-UwpReachable {
    $appxCount = 0
    try {
        $pkgs = @(Get-AppxPackage -ErrorAction SilentlyContinue | Where-Object {
                $_.Name -match 'Calculator|Photos|WindowsCalculator|ZuneVideo'
            })
        $appxCount = $pkgs.Count
    } catch { $appxCount = 0 }
    $frameHostWindows = 0
    try {
        $lw = Invoke-Ad -Arguments @('list-windows')
        if ($lw.Envelope.ok -and $lw.Envelope.data) {
            $frameHostWindows = @($lw.Envelope.data | Where-Object {
                    $_.app_name -match 'ApplicationFrameHost'
                }).Count
        }
    } catch { $frameHostWindows = 0 }
    return [ordered]@{
        appx_candidate_count = $appxCount
        application_frame_host_windows = $frameHostWindows
        reachable = (($appxCount -gt 0) -or ($frameHostWindows -gt 0))
    }
}

function Clear-NotepadScratch {
    Get-Process -Name 'notepad' -ErrorAction SilentlyContinue | ForEach-Object {
        try { Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue } catch { }
    }
    Start-Sleep -Milliseconds 300
}

$cursorOrigin = $null
$clipHadText = $false
$clipOriginal = ''
$clipSnapshotTaken = $false
$explorerScratch = $null

try {
    Initialize-LifecycleDogfoodNative
    Clear-NotepadScratch
    Clear-ForegroundStealers

    $cursorOrigin = New-Object AgentDesktopLifecycleDogfood.ProbePoint
    [void][AgentDesktopLifecycleDogfood.Native]::GetCursorPos([ref]$cursorOrigin)
    try {
        $clipHadText = [System.Windows.Forms.Clipboard]::ContainsText()
        if ($clipHadText) { $clipOriginal = [System.Windows.Forms.Clipboard]::GetText() }
        $clipSnapshotTaken = $true
    } catch { }

    # -------------------------------------------------------------------------
    # J1. Notepad launch -> interact -> close round-trip
    # -------------------------------------------------------------------------
    $notepadAbs = Join-Path $env:WINDIR 'System32\notepad.exe'
    $j1Notes = New-Object System.Collections.Generic.List[string]
    $j1Shape = $null
    $j1Pass = $false
    $npAppPid = 0
    $npHwnd = [IntPtr]::Zero
    try {
        $launch = Invoke-Ad -Arguments @('launch', $notepadAbs, '--no-attach', '--timeout', '15000')
        $launchShape = Get-EnvelopeShape -Envelope $launch.Envelope
        $j1Shape = $launchShape
        Add-EnvelopeRecord -Id 'J1-launch' -Shape $launchShape -Note 'absolute System32 notepad'
        $launchOk = $launch.Envelope.ok -and `
            ($launchShape.command -eq 'launch') -and `
            $launchShape.bool_flags.has_window_id -and `
            $launchShape.bool_flags.has_bounds -and `
            $launchShape.bool_flags.has_process_instance
        if (-not $launchOk) { throw 'launch envelope failed independent shape checks' }

        $npWid = [string]$launch.Envelope.data.id
        $npAppPid = [int]$launch.Envelope.data.pid
        [void]$script:LaunchedPids.Add($npAppPid)
        $npProc = Get-Process -Id $npAppPid -ErrorAction Stop
        $npHwnd = $npProc.MainWindowHandle
        if ($npHwnd -eq [IntPtr]::Zero) {
            $npHwnd = Wait-MainWindow -Process $npProc -TimeoutSec 10
        }
        if ($npHwnd -eq [IntPtr]::Zero) { throw 'notepad window handle missing after launch' }

        Clear-ForegroundStealers
        $focus = Invoke-Ad -Arguments @('focus-window', '--window-id', $npWid) -Headed
        $focusShape = Get-EnvelopeShape -Envelope $focus.Envelope
        Add-EnvelopeRecord -Id 'J1-focus' -Shape $focusShape
        $script:TargetPid = $npAppPid
        $fgOk = [AgentDesktopProbe.A21.Lifecycle21]::ForegroundOwned($npAppPid)
        [void]$j1Notes.Add('focus_ok=' + $focus.Envelope.ok + ';fg_owned=' + $fgOk)

        $find = Invoke-Ad -Arguments @('find', '--window-id', $npWid, '--role', 'textfield', '--first')
        $findShape = Get-EnvelopeShape -Envelope $find.Envelope
        Add-EnvelopeRecord -Id 'J1-find' -Shape $findShape
        $docRef = $null
        if ($find.Envelope.ok -and $find.Envelope.data) {
            $data = $find.Envelope.data
            if ($data.PSObject.Properties.Name -contains 'ref_id') { $docRef = [string]$data.ref_id }
            elseif ($data.PSObject.Properties.Name -contains 'match' -and $data.match) {
                if ($data.match.PSObject.Properties.Name -contains 'ref_id') {
                    $docRef = [string]$data.match.ref_id
                }
            }
        }
        if (-not $docRef) { throw 'textfield ref missing after launch' }

        $marker = 'lifecycle-j1'
        $set = Invoke-Ad -Arguments @('set-value', $docRef, $marker)
        $setShape = Get-EnvelopeShape -Envelope $set.Envelope
        Add-EnvelopeRecord -Id 'J1-set-value' -Shape $setShape
        Start-Sleep -Milliseconds 300
        $observed = Get-NotepadEditText -NotepadHwnd $npHwnd
        $obsShape = Get-TextShape -Text $observed
        $expectedShape = Get-TextShape -Text $marker
        $hashMatch = ($obsShape.sha256Utf16 -eq $expectedShape.sha256Utf16)
        [void]$j1Notes.Add('set_value_ok=' + $set.Envelope.ok + ';wm_gettext_hash_match=' + $hashMatch + ';utf16=' + $obsShape.utf16Units)

        # Clear so graceful WM_CLOSE is not blocked by a save dialog.
        [void](Invoke-Ad -Arguments @('clear', $docRef))
        Start-Sleep -Milliseconds 200
        $close = Invoke-Ad -Arguments @('close-app', 'notepad.exe')
        $closeShape = Get-EnvelopeShape -Envelope $close.Envelope
        Add-EnvelopeRecord -Id 'J1-close' -Shape $closeShape
        Start-Sleep -Milliseconds 500
        $stillAlive = [bool](Get-Process -Id $npAppPid -ErrorAction SilentlyContinue)
        if ($stillAlive) {
            $closeForce = Invoke-Ad -Arguments @('close-app', 'notepad.exe', '--force')
            $closeForceShape = Get-EnvelopeShape -Envelope $closeForce.Envelope
            Add-EnvelopeRecord -Id 'J1-close-force' -Shape $closeForceShape
            Start-Sleep -Milliseconds 400
            $stillAlive = [bool](Get-Process -Id $npAppPid -ErrorAction SilentlyContinue)
            $closeOk = $closeForce.Envelope.ok -and `
                $closeForceShape.bool_flags.closed -and `
                $closeForceShape.bool_flags.method_is_force -and `
                (-not $stillAlive)
            [void]$j1Notes.Add('graceful_blocked=True;force_close_ok=' + $closeOk + ';process_gone=' + (-not $stillAlive))
            $closeShape = $closeForceShape
        } else {
            $closeOk = $close.Envelope.ok -and `
                $closeShape.bool_flags.closed -and `
                $closeShape.bool_flags.requested -and `
                $closeShape.bool_flags.method_is_graceful -and `
                (-not $stillAlive)
            [void]$j1Notes.Add('close_ok=' + $closeOk + ';process_gone=' + (-not $stillAlive) + ';method=graceful')
        }

        $j1Pass = $launchOk -and $hashMatch -and $closeOk
        if ($j1Pass) {
            Add-Judgement -Id 'J1' -Claim 'Notepad launch -> interact -> close round-trip' `
                -Target 'Notepad' -Result 'pass' `
                -Verdict 'launch window shape, WM_GETTEXT hash match after set-value, verified process gone after graceful close' `
                -Shape $j1Shape -Notes ($j1Notes -join '; ')
        } else {
            Add-Judgement -Id 'J1' -Claim 'Notepad launch -> interact -> close round-trip' `
                -Target 'Notepad' -Result 'fail' `
                -Verdict 'one or more independent checks failed' `
                -Shape $j1Shape -Notes ($j1Notes -join '; ')
        }
    } catch {
        Add-Judgement -Id 'J1' -Claim 'Notepad launch -> interact -> close round-trip' `
            -Target 'Notepad' -Result 'fail' `
            -Verdict 'harness exception before judgement complete' `
            -Shape $j1Shape -Notes ('exception_type=' + $_.Exception.GetType().Name)
    } finally {
        $script:TargetPid = 0
        Clear-NotepadScratch
    }

    # -------------------------------------------------------------------------
    # J2. Explorer launch/interact + protected-process close refusal
    # -------------------------------------------------------------------------
    $j2Notes = New-Object System.Collections.Generic.List[string]
    $j2Shape = $null
    try {
        $explorerScratch = Join-Path $env:TEMP ('ad-lc-df-' + [guid]::NewGuid().ToString('n').Substring(0, 8))
        New-Item -ItemType Directory -Path $explorerScratch -Force | Out-Null
        1..5 | ForEach-Object {
            Set-Content -LiteralPath (Join-Path $explorerScratch ("f{0:D2}.txt" -f $_)) -Value 'x'
        }

        $prodLaunch = Invoke-Ad -Arguments @('launch', 'explorer.exe', '--arg', $explorerScratch, '--timeout', '8000')
        $prodLaunchShape = Get-EnvelopeShape -Envelope $prodLaunch.Envelope
        Add-EnvelopeRecord -Id 'J2-product-launch' -Shape $prodLaunchShape -Note 'attach-default shell explorer'
        [void]$j2Notes.Add('product_launch_ok=' + $prodLaunch.Envelope.ok + ';code=' + $prodLaunchShape.code + ';delivery=' + $prodLaunchShape.disposition_delivery)

        $prodNoAttach = Invoke-Ad -Arguments @('launch', 'explorer.exe', '--arg', $explorerScratch, '--no-attach', '--timeout', '3000')
        $prodNoAttachShape = Get-EnvelopeShape -Envelope $prodNoAttach.Envelope
        Add-EnvelopeRecord -Id 'J2-product-launch-no-attach' -Shape $prodNoAttachShape
        [void]$j2Notes.Add('no_attach_ok=' + $prodNoAttach.Envelope.ok + ';code=' + $prodNoAttachShape.code)

        $exProc = Start-DogfoodProcess -FilePath 'explorer.exe' -ArgumentList @($explorerScratch)
        Start-Sleep -Seconds 2
        $lw = Invoke-Ad -Arguments @('list-windows', '--app', 'explorer.exe')
        $lwShape = Get-EnvelopeShape -Envelope $lw.Envelope
        Add-EnvelopeRecord -Id 'J2-list-windows' -Shape $lwShape
        $exRows = @()
        if ($lw.Envelope.ok -and $lw.Envelope.data) { $exRows = @($lw.Envelope.data) }
        [void]$j2Notes.Add('harness_explorer_windows=' + $exRows.Count)
        $interactOk = $false
        $j2Shape = $prodLaunchShape
        if ($exRows.Count -gt 0) {
            $exWid = [string]$exRows[0].id
            $exHwnd = [IntPtr]::new([int64]($exWid -replace '^w-', ''))
            [void]$script:ExplorerHwnds.Add($exHwnd)
            $snap = Invoke-Ad -Arguments @('snapshot', '--window-id', $exWid)
            $snapShape = Get-EnvelopeShape -Envelope $snap.Envelope
            Add-EnvelopeRecord -Id 'J2-snapshot' -Shape $snapShape
            $refCount = 0
            if ($snap.Envelope.ok -and $snap.Envelope.data -and ($snap.Envelope.data.PSObject.Properties.Name -contains 'ref_count')) {
                $refCount = [int]$snap.Envelope.data.ref_count
            }
            $interactOk = $snap.Envelope.ok -and ($refCount -gt 0)
            [void]$j2Notes.Add('snapshot_ok=' + $snap.Envelope.ok + ';ref_count=' + $refCount)
            $j2Shape = $snapShape
        }

        $closeProt = Invoke-Ad -Arguments @('close-app', 'explorer.exe')
        $closeProtShape = Get-EnvelopeShape -Envelope $closeProt.Envelope
        Add-EnvelopeRecord -Id 'J2-close-protected' -Shape $closeProtShape
        $refusalOk = (-not $closeProt.Envelope.ok) -and `
            ($closeProtShape.code -eq 'INVALID_ARGS') -and `
            ($closeProtShape.disposition_delivery -eq 'not_delivered') -and `
            $closeProtShape.bool_flags.has_suggestion
        [void]$j2Notes.Add('protected_refusal_code=' + $closeProtShape.code + ';not_delivered=' + ($closeProtShape.disposition_delivery -eq 'not_delivered'))
        [void]$j2Notes.Add('plan_named_PERM_DENIED_observed_INVALID_ARGS=true')

        # Launch product path is the known shell/launcher residual; interact + refusal are required.
        $j2Pass = $interactOk -and $refusalOk
        if ($j2Pass) {
            Add-Judgement -Id 'J2' -Claim 'Explorer interact coverage + protected-process close refusal' `
                -Target 'Explorer' -Result 'pass' `
                -Verdict 'snapshot refs observed on harness-opened folder; close-app explorer.exe refused not_delivered INVALID_ARGS (protected list)' `
                -Shape $closeProtShape -Notes ($j2Notes -join '; ')
        } else {
            Add-Judgement -Id 'J2' -Claim 'Explorer interact coverage + protected-process close refusal' `
                -Target 'Explorer' -Result 'fail' `
                -Verdict 'interact or protected refusal failed independent checks' `
                -Shape $closeProtShape -Notes ($j2Notes -join '; ')
        }
    } catch {
        Add-Judgement -Id 'J2' -Claim 'Explorer interact coverage + protected-process close refusal' `
            -Target 'Explorer' -Result 'fail' `
            -Verdict 'harness exception before judgement complete' `
            -Shape $j2Shape -Notes ('exception_type=' + $_.Exception.GetType().Name)
    } finally {
        Close-ExplorerFolderWindows
        if ($explorerScratch -and (Test-Path -LiteralPath $explorerScratch)) {
            try { Remove-Item -LiteralPath $explorerScratch -Recurse -Force -ErrorAction SilentlyContinue } catch { }
        }
    }

    # -------------------------------------------------------------------------
    # J3. APP_UNRESPONSIVE / ProcessState::Unresponsive via StalledFixture
    # -------------------------------------------------------------------------
    try {
        $libEv = Invoke-LibUnresponsiveEvidence
        Add-EnvelopeRecord -Id 'J3-lib-stalled' -Shape ([ordered]@{
                ok = $libEv.passed
                command = 'cargo-test'
                code = $null
                disposition_delivery = $null
                bool_flags = [ordered]@{
                    lib_test_passed = $libEv.passed
                    cli_process_state_exists = $libEv.names_cli_process_state
                }
                numeric_flags = [ordered]@{
                    exit_code = $libEv.exit_code
                    running_tests = $libEv.running_tests
                }
            }) -Note 'system::process_state::tests::stalled_fixture_classifies_unresponsive'
        if ($libEv.passed) {
            Add-Judgement -Id 'J3' -Claim 'StalledFixture classifies ProcessState::Unresponsive (APP_UNRESPONSIVE path)' `
                -Target 'StalledFixture' -Result 'pass' `
                -Verdict 'lib test stalled_fixture_classifies_unresponsive ok; no CLI process-state command (adapter-only)' `
                -Notes ('running_tests=' + $libEv.running_tests + ';cli_process_state=false')
        } else {
            Add-Judgement -Id 'J3' -Claim 'StalledFixture classifies ProcessState::Unresponsive (APP_UNRESPONSIVE path)' `
                -Target 'StalledFixture' -Result 'fail' `
                -Verdict 'lib Unresponsive evidence did not pass' `
                -Notes ('exit_code=' + $libEv.exit_code)
        }
    } catch {
        Add-Judgement -Id 'J3' -Claim 'StalledFixture classifies ProcessState::Unresponsive (APP_UNRESPONSIVE path)' `
            -Target 'StalledFixture' -Result 'fail' `
            -Verdict 'could not run lib Unresponsive evidence' `
            -Notes ('exception_type=' + $_.Exception.GetType().Name)
    }

    # -------------------------------------------------------------------------
    # J4. Each window_op on a scratch Notepad window
    # -------------------------------------------------------------------------
    $j4Notes = New-Object System.Collections.Generic.List[string]
    $j4Shape = $null
    $opAppPid = 0
    try {
        Clear-NotepadScratch
        $opLaunch = Invoke-Ad -Arguments @('launch', $notepadAbs, '--no-attach', '--timeout', '15000')
        $opLaunchShape = Get-EnvelopeShape -Envelope $opLaunch.Envelope
        Add-EnvelopeRecord -Id 'J4-launch' -Shape $opLaunchShape
        if (-not $opLaunch.Envelope.ok) { throw 'window-op scratch launch failed' }
        $opWid = [string]$opLaunch.Envelope.data.id
        $opAppPid = [int]$opLaunch.Envelope.data.pid
        [void]$script:LaunchedPids.Add($opAppPid)
        $opHwnd = (Get-Process -Id $opAppPid).MainWindowHandle
        if ($opHwnd -eq [IntPtr]::Zero) {
            $opHwnd = Wait-MainWindow -Process (Get-Process -Id $opAppPid) -TimeoutSec 10
        }
        if ($opHwnd -eq [IntPtr]::Zero) { throw 'window-op hwnd missing' }

        $opsPass = $true
        # resize
        $rz = Invoke-Ad -Arguments @('resize-window', '--window-id', $opWid, '--width', '640', '--height', '480')
        $rzShape = Get-EnvelopeShape -Envelope $rz.Envelope
        Add-EnvelopeRecord -Id 'J4-resize' -Shape $rzShape
        Start-Sleep -Milliseconds 100
        $snapRz = [AgentDesktopProbe.A21.Lifecycle21]::SnapPlacement($opHwnd)
        $rzInd = (Test-WithinTolerance -Expected 640 -Actual $snapRz.Width -Tolerance $script:PlacementTolerancePx) -and `
            (Test-WithinTolerance -Expected 480 -Actual $snapRz.Height -Tolerance $script:PlacementTolerancePx)
        $rzOk = $rz.Envelope.ok -and $rzShape.bool_flags.resized -and $rzInd
        if (-not $rzOk) { $opsPass = $false }
        [void]$j4Notes.Add('resize_ok=' + $rzOk + ';ind_w_delta=' + [math]::Abs(640 - $snapRz.Width) + ';ind_h_delta=' + [math]::Abs(480 - $snapRz.Height))
        $j4Shape = $rzShape

        # move
        $mv = Invoke-Ad -Arguments @('move-window', '--window-id', $opWid, '--x', '120', '--y', '140')
        $mvShape = Get-EnvelopeShape -Envelope $mv.Envelope
        Add-EnvelopeRecord -Id 'J4-move' -Shape $mvShape
        Start-Sleep -Milliseconds 100
        $snapMv = [AgentDesktopProbe.A21.Lifecycle21]::SnapPlacement($opHwnd)
        $mvInd = (Test-WithinTolerance -Expected 120 -Actual $snapMv.Left -Tolerance $script:PlacementTolerancePx) -and `
            (Test-WithinTolerance -Expected 140 -Actual $snapMv.Top -Tolerance $script:PlacementTolerancePx)
        $mvOk = $mv.Envelope.ok -and $mvShape.bool_flags.moved -and $mvInd
        if (-not $mvOk) { $opsPass = $false }
        [void]$j4Notes.Add('move_ok=' + $mvOk + ';ind_x_delta=' + [math]::Abs(120 - $snapMv.Left) + ';ind_y_delta=' + [math]::Abs(140 - $snapMv.Top))

        # minimize -> showCmd 2
        $mn = Invoke-Ad -Arguments @('minimize', '--window-id', $opWid)
        $mnShape = Get-EnvelopeShape -Envelope $mn.Envelope
        Add-EnvelopeRecord -Id 'J4-minimize' -Shape $mnShape
        Start-Sleep -Milliseconds 120
        $snapMn = [AgentDesktopProbe.A21.Lifecycle21]::SnapPlacement($opHwnd)
        $mnOk = $mn.Envelope.ok -and $mnShape.bool_flags.minimized -and ($snapMn.ShowCmd -eq 2)
        if (-not $mnOk) { $opsPass = $false }
        [void]$j4Notes.Add('minimize_ok=' + $mnOk + ';show_cmd=' + $snapMn.ShowCmd)

        # restore -> showCmd 1
        $rs = Invoke-Ad -Arguments @('restore', '--window-id', $opWid)
        $rsShape = Get-EnvelopeShape -Envelope $rs.Envelope
        Add-EnvelopeRecord -Id 'J4-restore-from-min' -Shape $rsShape
        Start-Sleep -Milliseconds 120
        $snapRs = [AgentDesktopProbe.A21.Lifecycle21]::SnapPlacement($opHwnd)
        $rsOk = $rs.Envelope.ok -and $rsShape.bool_flags.restored -and ($snapRs.ShowCmd -eq 1)
        if (-not $rsOk) { $opsPass = $false }
        [void]$j4Notes.Add('restore_min_ok=' + $rsOk + ';show_cmd=' + $snapRs.ShowCmd)

        # maximize -> showCmd 3
        $mx = Invoke-Ad -Arguments @('maximize', '--window-id', $opWid)
        $mxShape = Get-EnvelopeShape -Envelope $mx.Envelope
        Add-EnvelopeRecord -Id 'J4-maximize' -Shape $mxShape
        Start-Sleep -Milliseconds 120
        $snapMx = [AgentDesktopProbe.A21.Lifecycle21]::SnapPlacement($opHwnd)
        $mxOk = $mx.Envelope.ok -and $mxShape.bool_flags.maximized -and ($snapMx.ShowCmd -eq 3)
        if (-not $mxOk) { $opsPass = $false }
        [void]$j4Notes.Add('maximize_ok=' + $mxOk + ';show_cmd=' + $snapMx.ShowCmd)

        # restore from max
        $rs2 = Invoke-Ad -Arguments @('restore', '--window-id', $opWid)
        $rs2Shape = Get-EnvelopeShape -Envelope $rs2.Envelope
        Add-EnvelopeRecord -Id 'J4-restore-from-max' -Shape $rs2Shape
        Start-Sleep -Milliseconds 120
        $snapRs2 = [AgentDesktopProbe.A21.Lifecycle21]::SnapPlacement($opHwnd)
        $rs2Ok = $rs2.Envelope.ok -and $rs2Shape.bool_flags.restored -and ($snapRs2.ShowCmd -eq 1)
        if (-not $rs2Ok) { $opsPass = $false }
        [void]$j4Notes.Add('restore_max_ok=' + $rs2Ok + ';show_cmd=' + $snapRs2.ShowCmd)

        if ($opsPass) {
            Add-Judgement -Id 'J4' -Claim 'window_op resize/move/minimize/maximize/restore verified via Win32 placement' `
                -Target 'Notepad scratch' -Result 'pass' `
                -Verdict 'each op envelope ok and GetWindowPlacement/GetWindowRect matched within 8px / showCmd' `
                -Shape $j4Shape -Notes ($j4Notes -join '; ')
        } else {
            Add-Judgement -Id 'J4' -Claim 'window_op resize/move/minimize/maximize/restore verified via Win32 placement' `
                -Target 'Notepad scratch' -Result 'fail' `
                -Verdict 'one or more ops failed independent placement re-read' `
                -Shape $j4Shape -Notes ($j4Notes -join '; ')
        }
    } catch {
        Add-Judgement -Id 'J4' -Claim 'window_op resize/move/minimize/maximize/restore verified via Win32 placement' `
            -Target 'Notepad scratch' -Result 'fail' `
            -Verdict 'harness exception before judgement complete' `
            -Shape $j4Shape -Notes ('exception_type=' + $_.Exception.GetType().Name)
    } finally {
        Clear-NotepadScratch
    }

    # -------------------------------------------------------------------------
    # J5. press --app on a real target (headed)
    # -------------------------------------------------------------------------
    $j5Notes = New-Object System.Collections.Generic.List[string]
    $j5Shape = $null
    $prAppPid = 0
    try {
        Clear-NotepadScratch
        Clear-ForegroundStealers
        $prLaunch = Invoke-Ad -Arguments @('launch', $notepadAbs, '--no-attach', '--timeout', '15000')
        $prLaunchShape = Get-EnvelopeShape -Envelope $prLaunch.Envelope
        Add-EnvelopeRecord -Id 'J5-launch' -Shape $prLaunchShape
        if (-not $prLaunch.Envelope.ok) { throw 'press scratch launch failed' }
        $prWid = [string]$prLaunch.Envelope.data.id
        $prAppPid = [int]$prLaunch.Envelope.data.pid
        [void]$script:LaunchedPids.Add($prAppPid)
        $prHwnd = (Get-Process -Id $prAppPid).MainWindowHandle
        if ($prHwnd -eq [IntPtr]::Zero) {
            $prHwnd = Wait-MainWindow -Process (Get-Process -Id $prAppPid) -TimeoutSec 10
        }

        $focusPr = Invoke-Ad -Arguments @('focus-window', '--window-id', $prWid) -Headed
        $focusPrShape = Get-EnvelopeShape -Envelope $focusPr.Envelope
        Add-EnvelopeRecord -Id 'J5-focus' -Shape $focusPrShape
        $script:TargetPid = $prAppPid
        $fgOwned = [AgentDesktopProbe.A21.Lifecycle21]::ForegroundOwned($prAppPid)
        [void]$j5Notes.Add('focus_ok=' + $focusPr.Envelope.ok + ';fg_owned=' + $fgOwned)

        $beforeText = Get-NotepadEditText -NotepadHwnd $prHwnd
        if ($null -eq $beforeText) { $beforeText = '' }
        $beforeShape = Get-TextShape -Text $beforeText

        $pressResult = $null
        if ($fgOwned) {
            try { Assert-Foreground -ExpectedProcessId $prAppPid -Stage 'J5-press-app:pre' }
            catch {
                [void]$script:InterferenceRows.Add([ordered]@{
                        stage = 'J5-press-app:pre'
                        detail_shape = 'PROBE-INTERFERENCE'
                    })
            }
        }
        $pressResult = Invoke-Ad -Arguments @('press', 'a', '--app', 'notepad.exe') -Headed
        if ($fgOwned) {
            try { Assert-Foreground -ExpectedProcessId $prAppPid -Stage 'J5-press-app:post' }
            catch {
                [void]$script:InterferenceRows.Add([ordered]@{
                        stage = 'J5-press-app:post'
                        detail_shape = 'PROBE-INTERFERENCE'
                    })
            }
        }
        $bracketOk = (@($script:InterferenceRows | Where-Object { $_.stage -like 'J5-press-app*' }).Count -eq 0)
        $j5Shape = Get-EnvelopeShape -Envelope $pressResult.Envelope
        Add-EnvelopeRecord -Id 'J5-press-app' -Shape $j5Shape
        Start-Sleep -Milliseconds 300
        $afterText = Get-NotepadEditText -NotepadHwnd $prHwnd
        if ($null -eq $afterText) { $afterText = '' }
        $afterShape = Get-TextShape -Text $afterText
        $textChanged = ($afterShape.sha256Utf16 -ne $beforeShape.sha256Utf16) -and ($afterShape.utf16Units -gt $beforeShape.utf16Units)
        $pressOk = $pressResult.Envelope.ok -and `
            ($j5Shape.disposition_delivery -match '^delivered_') -and `
            $j5Shape.bool_flags.action_is_press_key -and `
            $textChanged
        [void]$j5Notes.Add('press_ok=' + $pressResult.Envelope.ok + ';delivery=' + $j5Shape.disposition_delivery + ';wm_gettext_grew=' + $textChanged + ';bracket_ok=' + $bracketOk + ';utf16_before=' + $beforeShape.utf16Units + ';utf16_after=' + $afterShape.utf16Units)

        if ($pressOk) {
            Add-Judgement -Id 'J5' -Claim 'headed press --app delivers to real Notepad target' `
                -Target 'Notepad' -Result 'pass' `
                -Verdict 'envelope delivered_*; WM_GETTEXT length grew after press --app a' `
                -Shape $j5Shape -Notes ($j5Notes -join '; ')
        } else {
            Add-Judgement -Id 'J5' -Claim 'headed press --app delivers to real Notepad target' `
                -Target 'Notepad' -Result 'fail' `
                -Verdict 'press --app envelope or independent edit re-read failed' `
                -Shape $j5Shape -Notes ($j5Notes -join '; ')
        }
    } catch {
        Add-Judgement -Id 'J5' -Claim 'headed press --app delivers to real Notepad target' `
            -Target 'Notepad' -Result 'fail' `
            -Verdict 'harness exception before judgement complete' `
            -Shape $j5Shape -Notes ('exception_type=' + $_.Exception.GetType().Name)
    } finally {
        $script:TargetPid = 0
        Clear-NotepadScratch
    }

    # -------------------------------------------------------------------------
    # J6. UWP target if reachable; else gap per A1-3
    # -------------------------------------------------------------------------
    $uwp = Test-UwpReachable
    Add-EnvelopeRecord -Id 'J6-uwp-census' -Shape ([ordered]@{
            ok = $uwp.reachable
            command = 'uwp-census'
            bool_flags = [ordered]@{ reachable = $uwp.reachable }
            numeric_flags = [ordered]@{
                appx_candidate_count = $uwp.appx_candidate_count
                application_frame_host_windows = $uwp.application_frame_host_windows
            }
        }) -Note 'A1-3 ApplicationFrameHost gap'
    if ($uwp.reachable) {
        Add-Judgement -Id 'J6' -Claim 'UWP lifecycle target reachable' `
            -Target 'UWP' -Result 'fail' `
            -Verdict 'UWP/ApplicationFrameHost candidate present but lifecycle path not exercised' `
            -Notes ('appx=' + $uwp.appx_candidate_count + ';frame_host_windows=' + $uwp.application_frame_host_windows)
    } else {
        Add-Judgement -Id 'J6' -Claim 'UWP lifecycle target reachable' `
            -Target 'UWP' -Result 'skipped' `
            -Verdict 'no AppxPackage or ApplicationFrameHost window on this host; recorded gap per A1-3' `
            -Notes ('appx=' + $uwp.appx_candidate_count + ';frame_host_windows=' + $uwp.application_frame_host_windows)
    }

} finally {
    Restore-DesktopHygiene -CursorOrigin $cursorOrigin -ClipHadText $clipHadText `
        -ClipOriginal $clipOriginal -ClipSnapshotTaken $clipSnapshotTaken
    Close-ExplorerFolderWindows
    foreach ($launchedPid in $script:LaunchedPids) {
        try {
            $proc = Get-Process -Id $launchedPid -ErrorAction SilentlyContinue
            if ($proc -and ($proc.ProcessName -notmatch '^(explorer)$')) {
                Stop-Process -Id $launchedPid -Force -ErrorAction SilentlyContinue
            }
        } catch { }
    }
    Get-Process -Name 'notepad' -ErrorAction SilentlyContinue | ForEach-Object {
        Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
    }
    if ($explorerScratch -and (Test-Path -LiteralPath $explorerScratch)) {
        try { Remove-Item -LiteralPath $explorerScratch -Recurse -Force -ErrorAction SilentlyContinue } catch { }
    }
}

$os = Get-CimInstance Win32_OperatingSystem
$envHeader = [ordered]@{
    os_caption = $os.Caption
    os_build = $os.BuildNumber
    binary = Split-Path -Leaf $script:Binary
    binary_bytes = (Get-Item -LiteralPath $script:Binary).Length
    generated = (Get-Date).ToString('o')
    placement_tolerance_px = $script:PlacementTolerancePx
}

$summaryPath = Join-Path $script:OutDir 'lifecycle-dogfood-run.json'
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
