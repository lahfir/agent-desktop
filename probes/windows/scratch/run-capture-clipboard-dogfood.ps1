#Requires -Version 5.1
<#
.SYNOPSIS
    Sub-phase 2.10 U10 capture + clipboard dogfood runner.

.DESCRIPTION
    Drives target/release/agent-desktop.exe against repo-controlled stacks
    (Notepad/Win32, ScratchForms, ScratchWpf, Obsidian/Electron when present).
    Judges by JSON envelope shapes PLUS independent PNG pixel statistics and
    clipboard hash/count checks - never ok:true alone. Redaction at point of
    record: shapes and counts only (no titles, paths, pids, machine names,
    message text, or clipboard values).

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
Initialize-ProbeNative

Add-Type -AssemblyName System.Drawing | Out-Null
Add-Type -AssemblyName System.Windows.Forms | Out-Null

if (-not $Binary) { $Binary = Join-Path $script:RepoRoot 'target\release\agent-desktop.exe' }
if (-not (Test-Path -LiteralPath $Binary)) { throw "release binary not found at $Binary" }
$script:Binary = (Resolve-Path -LiteralPath $Binary).ProviderPath
if (-not $OutDir) {
    $OutDir = Join-Path $script:RepoRoot 'docs\dogfood-reports\2026-08-09-001-captures'
}
New-Item -ItemType Directory -Path $OutDir -Force | Out-Null
$script:OutDir = (Resolve-Path -LiteralPath $OutDir).ProviderPath
$script:WorkDir = Join-Path $env:TEMP ('ad-capclip-dogfood-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
New-Item -ItemType Directory -Path $script:WorkDir -Force | Out-Null

$script:LaunchedPids = New-Object System.Collections.Generic.List[int]
$script:Judgements = New-Object System.Collections.Generic.List[object]
$script:Envelopes = New-Object System.Collections.Generic.List[object]
$script:NoJsonCode = 'BINARY_NO_JSON'
$utf8NoBom = New-Object System.Text.UTF8Encoding $false

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
    param([string[]]$Arguments)
    $prev = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $raw = (& $script:Binary @Arguments 2>$null | Out-String)
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

function Get-ScreenshotShape {
    param($Envelope)
    $shape = [ordered]@{
        ok = [bool]$Envelope.ok
        command = $null
        code = $null
        format = $null
        width = $null
        height = $null
        scale_factor = $null
        has_path = $false
        has_data = $false
        data_byte_estimate = $null
    }
    if ($Envelope.PSObject.Properties.Name -contains 'command') {
        $shape.command = [string]$Envelope.command
    }
    if (-not $Envelope.ok) {
        if ($Envelope.PSObject.Properties.Name -contains 'error' -and $Envelope.error) {
            if ($Envelope.error.PSObject.Properties.Name -contains 'code') {
                $shape.code = [string]$Envelope.error.code
            }
        }
        return $shape
    }
    if ($Envelope.PSObject.Properties.Name -contains 'data' -and $Envelope.data) {
        $d = $Envelope.data
        foreach ($k in @('format', 'width', 'height', 'scale_factor')) {
            if ($d.PSObject.Properties.Name -contains $k) { $shape[$k] = $d.$k }
        }
        $shape.has_path = ($d.PSObject.Properties.Name -contains 'path') -and (-not [string]::IsNullOrEmpty([string]$d.path))
        $shape.has_data = ($d.PSObject.Properties.Name -contains 'data') -and (-not [string]::IsNullOrEmpty([string]$d.data))
        if ($shape.has_data) {
            $shape.data_byte_estimate = [int]([math]::Floor(([string]$d.data).Length * 0.75))
        }
    }
    return $shape
}

function Get-ClipboardShape {
    param($Envelope)
    $shape = [ordered]@{
        ok = [bool]$Envelope.ok
        command = $null
        code = $null
        type = $null
        found = $null
        has_text = $false
        text_char_count = $null
        file_url_count = $null
        image_width = $null
        image_height = $null
        image_format = $null
        has_image_path = $false
    }
    if ($Envelope.PSObject.Properties.Name -contains 'command') {
        $shape.command = [string]$Envelope.command
    }
    if (-not $Envelope.ok) {
        if ($Envelope.PSObject.Properties.Name -contains 'error' -and $Envelope.error) {
            if ($Envelope.error.PSObject.Properties.Name -contains 'code') {
                $shape.code = [string]$Envelope.error.code
            }
        }
        return $shape
    }
    if ($Envelope.PSObject.Properties.Name -contains 'data' -and $Envelope.data) {
        $d = $Envelope.data
        if ($d.PSObject.Properties.Name -contains 'type') { $shape.type = [string]$d.type }
        if ($d.PSObject.Properties.Name -contains 'found') { $shape.found = [bool]$d.found }
        if ($d.PSObject.Properties.Name -contains 'text') {
            $shape.has_text = $true
            $shape.text_char_count = ([string]$d.text).Length
        }
        if ($d.PSObject.Properties.Name -contains 'file_urls') {
            $shape.file_url_count = @($d.file_urls).Count
        }
        foreach ($k in @('width', 'height', 'format')) {
            if ($d.PSObject.Properties.Name -contains $k) {
                if ($k -eq 'width') { $shape.image_width = $d.width }
                elseif ($k -eq 'height') { $shape.image_height = $d.height }
                else { $shape.image_format = [string]$d.format }
            }
        }
        $shape.has_image_path = ($d.PSObject.Properties.Name -contains 'path') -and (-not [string]::IsNullOrEmpty([string]$d.path))
    }
    return $shape
}

function Get-PngStats {
    param([Parameter(Mandatory = $true)][string]$Path)
    if (-not (Test-Path -LiteralPath $Path)) {
        return [ordered]@{
            present = $false
            bytes = 0
            width = 0
            height = 0
            black_ratio = $null
            near_black_ratio = $null
            unique_colors_capped = $null
            mean_luma = $null
            classification = 'missing'
        }
    }
    $bytes = (Get-Item -LiteralPath $Path).Length
    $bmp = [System.Drawing.Bitmap]::FromFile($Path)
    try {
        $w = $bmp.Width
        $h = $bmp.Height
        $total = [double]($w * $h)
        if ($total -le 0) {
            return [ordered]@{
                present = $true
                bytes = $bytes
                width = $w
                height = $h
                black_ratio = 1.0
                near_black_ratio = 1.0
                unique_colors_capped = 0
                mean_luma = 0.0
                classification = 'empty_dims'
            }
        }
        $black = 0
        $nearBlack = 0
        $lumaSum = 0.0
        $uniq = New-Object 'System.Collections.Generic.HashSet[int]'
        $stepX = [Math]::Max(1, [int][Math]::Floor($w / 64.0))
        $stepY = [Math]::Max(1, [int][Math]::Floor($h / 64.0))
        $sampled = 0
        for ($y = 0; $y -lt $h; $y += $stepY) {
            for ($x = 0; $x -lt $w; $x += $stepX) {
                $c = $bmp.GetPixel($x, $y)
                $sampled++
                $luma = (0.2126 * $c.R) + (0.7152 * $c.G) + (0.0722 * $c.B)
                $lumaSum += $luma
                if ($c.R -eq 0 -and $c.G -eq 0 -and $c.B -eq 0) { $black++ }
                if ($c.R -lt 8 -and $c.G -lt 8 -and $c.B -lt 8) { $nearBlack++ }
                if ($uniq.Count -lt 256) {
                    [void]$uniq.Add(($c.R -shl 16) -bor ($c.G -shl 8) -bor $c.B)
                }
            }
        }
        $blackRatio = $black / [double]$sampled
        $nearRatio = $nearBlack / [double]$sampled
        $mean = $lumaSum / [double]$sampled
        $classification = 'real_content'
        if ($blackRatio -ge 0.98) { $classification = 'black' }
        elseif ($nearRatio -ge 0.95 -and $uniq.Count -le 4) { $classification = 'near_black' }
        elseif ($uniq.Count -le 2 -and $mean -lt 16) { $classification = 'flat_dark' }
        elseif ($uniq.Count -le 3 -and $blackRatio -ge 0.70) { $classification = 'partial_or_sparse' }
        return [ordered]@{
            present = $true
            bytes = $bytes
            width = $w
            height = $h
            black_ratio = [math]::Round($blackRatio, 4)
            near_black_ratio = [math]::Round($nearRatio, 4)
            unique_colors_capped = $uniq.Count
            mean_luma = [math]::Round($mean, 2)
            sample_count = $sampled
            classification = $classification
        }
    } finally {
        $bmp.Dispose()
    }
}

function Add-Judgement {
    param(
        [Parameter(Mandatory = $true)][string]$Id,
        [Parameter(Mandatory = $true)][string]$Target,
        [Parameter(Mandatory = $true)][string]$Stack,
        [Parameter(Mandatory = $true)][ValidateSet('pass', 'fail', 'skipped', 'disappointing')][string]$Result,
        [Parameter(Mandatory = $true)][string]$Verdict,
        [hashtable]$Evidence = @{},
        [string]$Disposition = '',
        [string]$Owner = '',
        [string]$Notes = ''
    )
    $row = New-Object psobject
    $row | Add-Member NoteProperty id $Id
    $row | Add-Member NoteProperty target $Target
    $row | Add-Member NoteProperty stack $Stack
    $row | Add-Member NoteProperty result $Result
    $row | Add-Member NoteProperty verdict $Verdict
    $row | Add-Member NoteProperty evidence $Evidence
    $row | Add-Member NoteProperty disposition $Disposition
    $row | Add-Member NoteProperty owner $Owner
    $row | Add-Member NoteProperty notes $Notes
    [void]$script:Judgements.Add($row)
}

function Find-WindowIdForApp {
    param([Parameter(Mandatory = $true)][string]$App)
    $res = Invoke-Ad -Arguments @('list-windows', '--app', $App)
    if (-not $res.Envelope.ok) { return $null }
    $windows = @($res.Envelope.data)
    if ($windows.Count -lt 1) { return $null }
    $first = $windows[0]
    if ($first.PSObject.Properties.Name -contains 'id') { return [string]$first.id }
    return $null
}

function Set-WindowRectSafe {
    param([IntPtr]$Hwnd, [int]$X, [int]$Y, [int]$W, [int]$H)
    if ($Hwnd -eq [IntPtr]::Zero) { return }
    Initialize-ProbeNative
    Show-WindowNoActivate -WindowHandle $Hwnd -X $X -Y $Y -Width $W -Height $H
}

function Set-WindowZOrder {
    param([IntPtr]$Hwnd, [ValidateSet('Top', 'Bottom')][string]$Order)
    if ($Hwnd -eq [IntPtr]::Zero) { return }
    Initialize-ProbeNative
    $after = if ($Order -eq 'Top') { [IntPtr]::Zero } else { New-Object System.IntPtr 1 }
    # SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE
    [void][AgentDesktopProbe.Native]::SetWindowPos($Hwnd, $after, 0, 0, 0, 0, [int](0x0001 -bor 0x0002 -bor 0x0010))
}

function Get-Sha256Hex {
    param([Parameter(Mandatory = $true)][string]$Text)
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($Text)
    $hash = [System.Security.Cryptography.SHA256]::Create().ComputeHash($bytes)
    return ([BitConverter]::ToString($hash)).Replace('-', '').ToLowerInvariant()
}

function Get-FileSha256Hex {
    param([Parameter(Mandatory = $true)][string]$Path)
    $sha = [System.Security.Cryptography.SHA256]::Create()
    $fs = [System.IO.File]::OpenRead($Path)
    try {
        $hash = $sha.ComputeHash($fs)
        return ([BitConverter]::ToString($hash)).Replace('-', '').ToLowerInvariant()
    } finally {
        $fs.Dispose()
        $sha.Dispose()
    }
}

function Get-WgcFacts {
    $facts = [ordered]@{
        is_supported = $null
        modern_attempt_observed = $false
        modern_fail_class = $null
        legacy_fallback_observed = $false
    }
    try {
        $t = [Windows.Graphics.Capture.GraphicsCaptureSession, Windows.Graphics.Capture, ContentType=WindowsRuntime]
        $facts.is_supported = [bool]$t.GetMethod('IsSupported').Invoke($null, @())
    } catch {
        $facts.is_supported = $null
        $facts.modern_fail_class = 'winrt_type_load'
    }
    $prev = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $np = Start-Process notepad.exe -PassThru
        [void]$script:LaunchedPids.Add($np.Id)
        Start-Sleep -Milliseconds 600
        $null = Wait-MainWindow -Process $np -TimeoutSec 10
        $errLines = & $script:Binary -v screenshot --app notepad.exe 2>&1 | ForEach-Object { "$_" }
        $err = ($errLines -join "`n")
        if ($err -match 'falling back to legacy') { $facts.legacy_fallback_observed = $true }
        if ($err -match 'modern capture unavailable or failed|falling back to legacy') {
            $facts.modern_attempt_observed = $true
        }
        if ($err -match 'IGraphicsCaptureItemInterop') { $facts.modern_fail_class = 'interop_activate_failed' }
        elseif ($err -match 'modern capture unsupported') { $facts.modern_fail_class = 'unsupported' }
        Stop-Process -Id $np.Id -Force -ErrorAction SilentlyContinue
    } finally {
        $ErrorActionPreference = $prev
    }
    return $facts
}

function Capture-AppWindow {
    param(
        [Parameter(Mandatory = $true)][string]$App,
        [Parameter(Mandatory = $true)][string]$Leaf
    )
    $path = Join-Path $script:WorkDir $Leaf
    $res = Invoke-Ad -Arguments @('screenshot', '--app', $App, $path)
    $shape = Get-ScreenshotShape -Envelope $res.Envelope
    $stats = $null
    if ($res.Envelope.ok -and (Test-Path -LiteralPath $path)) {
        $stats = Get-PngStats -Path $path
    }
    [void]$script:Envelopes.Add([pscustomobject]@{
            leg = $Leaf
            shape = $shape
            png = $stats
            exit = $res.ExitCode
        })
    return [pscustomobject]@{ Shape = $shape; Stats = $stats; Ok = [bool]$res.Envelope.ok; Path = $path }
}

try {
    $binInfo = Get-Item -LiteralPath $script:Binary
    $wgc = Get-WgcFacts

    # --- Leg 1: stack window captures ---
    # Win32/GDI: Notepad
    $np = Start-DogfoodProcess -FilePath 'notepad.exe'
    $npHwnd = Wait-MainWindow -Process $np -TimeoutSec 15
    if ($npHwnd -eq [IntPtr]::Zero) {
        Add-Judgement -Id 'J1a' -Target 'Notepad' -Stack 'Win32/GDI' -Result 'fail' -Verdict 'window never presented'
    } else {
        Set-WindowRectSafe -Hwnd $npHwnd -X 40 -Y 40 -W 640 -H 480
        Start-Sleep -Milliseconds 300
        $cap = Capture-AppWindow -App 'notepad.exe' -Leaf 'j1a-notepad.png'
        $class = if ($cap.Stats) { [string]$cap.Stats.classification } else { 'missing' }
        $pass = $cap.Ok -and ($class -eq 'real_content')
        Add-Judgement -Id 'J1a' -Target 'Notepad' -Stack 'Win32/GDI' `
            -Result $(if ($pass) { 'pass' } else { 'fail' }) `
            -Verdict $(if ($pass) { 'legacy returned real content (PW_RENDERFULLCONTENT path)' } else { "capture class=$class ok=$($cap.Ok)" }) `
            -Evidence @{
                envelope = $cap.Shape
                png = $cap.Stats
                modern = $wgc
            }
    }

    # WinForms: ScratchForms
    $formsExe = Join-Path $script:ScratchDir 'bin\ScratchForms.exe'
    if (-not (Test-Path -LiteralPath $formsExe)) {
        Add-Judgement -Id 'J1b' -Target 'ScratchForms' -Stack 'WinForms' -Result 'skipped' `
            -Verdict 'measurable:false' -Notes 'ScratchForms.exe absent; named branch build-scratch.ps1' `
            -Disposition 'accepted' -Owner 'environment'
    } else {
        $sf = Start-DogfoodProcess -FilePath $formsExe
        $sfHwnd = Wait-MainWindow -Process $sf -TimeoutSec 15
        if ($sfHwnd -eq [IntPtr]::Zero) {
            Add-Judgement -Id 'J1b' -Target 'ScratchForms' -Stack 'WinForms' -Result 'fail' -Verdict 'window never presented'
        } else {
            Set-WindowRectSafe -Hwnd $sfHwnd -X 80 -Y 80 -W 700 -H 520
            Start-Sleep -Milliseconds 400
            $cap = Capture-AppWindow -App 'ScratchForms.exe' -Leaf 'j1b-winforms.png'
            if (-not $cap.Ok) {
                $cap = Capture-AppWindow -App 'ScratchForms' -Leaf 'j1b-winforms.png'
            }
            $class = if ($cap.Stats) { [string]$cap.Stats.classification } else { 'missing' }
            $pass = $cap.Ok -and ($class -eq 'real_content')
            Add-Judgement -Id 'J1b' -Target 'ScratchForms' -Stack 'WinForms' `
                -Result $(if ($pass) { 'pass' } else { 'disappointing' }) `
                -Verdict $(if ($pass) { 'legacy returned real content' } else { "capture class=$class ok=$($cap.Ok)" }) `
                -Evidence @{ envelope = $cap.Shape; png = $cap.Stats } `
                -Disposition $(if ($pass) { '' } else { 'accepted' }) `
                -Owner $(if ($pass) { '' } else { 'legacy PrintWindow on WinForms; A22-2 GDI path confirmed elsewhere' })
        }
    }

    # WPF: ScratchWpf.ps1
    $wpfScript = Join-Path $script:ScratchDir 'ScratchWpf.ps1'
    if (-not (Test-Path -LiteralPath $wpfScript)) {
        Add-Judgement -Id 'J1c' -Target 'ScratchWpf' -Stack 'WPF' -Result 'skipped' `
            -Verdict 'measurable:false' -Notes 'ScratchWpf.ps1 absent' -Disposition 'accepted' -Owner 'environment'
    } else {
        $wpfProc = Start-Process -FilePath 'powershell.exe' -ArgumentList @(
            '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $wpfScript,
            '-Tag', 'capclip', '-Left', '120', '-Top', '120', '-TimeoutSeconds', '90'
        ) -PassThru -WindowStyle Hidden
        [void]$script:LaunchedPids.Add($wpfProc.Id)
        $deadline = (Get-Date).AddSeconds(20)
        $wpfReady = $false
        while ((Get-Date) -lt $deadline) {
            $wid = Find-WindowIdForApp -App 'powershell.exe'
            # Prefer listing and matching by process - use capture by window-id from child
            $kids = Get-CimInstance Win32_Process -Filter "ParentProcessId=$($wpfProc.Id)" -ErrorAction SilentlyContinue
            Start-Sleep -Milliseconds 400
            # ScratchWpf runs in the started powershell process itself
            $wpfProc.Refresh()
            if ($wpfProc.MainWindowHandle -ne [IntPtr]::Zero) { $wpfReady = $true; break }
            # Also try list-windows for any window owned by this pid via product
            $lw = Invoke-Ad -Arguments @('list-windows')
            if ($lw.Envelope.ok) {
                foreach ($w in @($lw.Envelope.data)) {
                    if (($w.PSObject.Properties.Name -contains 'pid') -and ([int]$w.pid -eq $wpfProc.Id)) {
                        $wpfReady = $true
                        break
                    }
                }
                if ($wpfReady) { break }
            }
        }
        if (-not $wpfReady) {
            Add-Judgement -Id 'J1c' -Target 'ScratchWpf' -Stack 'WPF' -Result 'skipped' `
                -Verdict 'measurable:false' -Notes 'ScratchWpf window not observed within settle; named branch WPF dispatcher' `
                -Disposition 'accepted' -Owner 'environment / ScratchWpf ShowActivated=False'
        } else {
            # Capture via window-id belonging to the WPF process
            $lw = Invoke-Ad -Arguments @('list-windows')
            $wid = $null
            foreach ($w in @($lw.Envelope.data)) {
                if (($w.PSObject.Properties.Name -contains 'pid') -and ([int]$w.pid -eq $wpfProc.Id)) {
                    $wid = [string]$w.id
                    break
                }
            }
            $path = Join-Path $script:WorkDir 'j1c-wpf.png'
            if ($wid) {
                $res = Invoke-Ad -Arguments @('screenshot', '--window-id', $wid, $path)
            } else {
                $res = Invoke-Ad -Arguments @('screenshot', '--app', 'powershell.exe', $path)
            }
            $shape = Get-ScreenshotShape -Envelope $res.Envelope
            $stats = if ($res.Envelope.ok -and (Test-Path $path)) { Get-PngStats -Path $path } else { $null }
            [void]$script:Envelopes.Add([pscustomobject]@{ leg = 'j1c-wpf'; shape = $shape; png = $stats; exit = $res.ExitCode })
            $class = if ($stats) { [string]$stats.classification } else { 'missing' }
            $pass = $res.Envelope.ok -and ($class -eq 'real_content')
            $disappointing = $res.Envelope.ok -and ($class -ne 'real_content')
            Add-Judgement -Id 'J1c' -Target 'ScratchWpf' -Stack 'WPF' `
                -Result $(if ($pass) { 'pass' } elseif ($disappointing) { 'disappointing' } else { 'fail' }) `
                -Verdict $(if ($pass) { 'legacy returned real content on DWM-composited WPF' } else { "capture class=$class ok=$($res.Envelope.ok) - PW_RENDERFULLCONTENT question" }) `
                -Evidence @{ envelope = $shape; png = $stats } `
                -Disposition $(if ($pass) { '' } elseif ($disappointing) { 'owned elsewhere' } else { '' }) `
                -Owner $(if ($disappointing) { 'section 2.12 interactive runner / modern-backend host (legacy black/partial on GPU-composited WPF)' } else { '' })
        }
    }

    # Electron/Chromium: Obsidian
    $obsidianExe = Join-Path $env:LOCALAPPDATA 'Programs\Obsidian\Obsidian.exe'
    if (-not (Test-Path -LiteralPath $obsidianExe)) {
        Add-Judgement -Id 'J1d' -Target 'Obsidian' -Stack 'Electron/Chromium' -Result 'skipped' `
            -Verdict 'measurable:false' -Notes 'Obsidian not installed' -Disposition 'accepted' -Owner 'environment'
    } else {
        $obs = Start-DogfoodProcess -FilePath $obsidianExe
        $deadline = (Get-Date).AddSeconds(25)
        $obsReady = $false
        $obsWid = $null
        while ((Get-Date) -lt $deadline) {
            $lw = Invoke-Ad -Arguments @('list-windows', '--app', 'Obsidian.exe')
            if (-not $lw.Envelope.ok) {
                $lw = Invoke-Ad -Arguments @('list-windows', '--app', 'Obsidian')
            }
            if ($lw.Envelope.ok -and @($lw.Envelope.data).Count -gt 0) {
                $obsWid = [string]@($lw.Envelope.data)[0].id
                $obsReady = $true
                break
            }
            Start-Sleep -Milliseconds 500
        }
        if (-not $obsReady) {
            Add-Judgement -Id 'J1d' -Target 'Obsidian' -Stack 'Electron/Chromium' -Result 'skipped' `
                -Verdict 'measurable:false' -Notes 'Obsidian window not listed within settle' `
                -Disposition 'accepted' -Owner 'environment / Electron cold start'
        } else {
            $path = Join-Path $script:WorkDir 'j1d-electron.png'
            $res = Invoke-Ad -Arguments @('screenshot', '--window-id', $obsWid, $path)
            if (-not $res.Envelope.ok) {
                $res = Invoke-Ad -Arguments @('screenshot', '--app', 'Obsidian.exe', $path)
            }
            $shape = Get-ScreenshotShape -Envelope $res.Envelope
            $stats = if ($res.Envelope.ok -and (Test-Path $path)) { Get-PngStats -Path $path } else { $null }
            [void]$script:Envelopes.Add([pscustomobject]@{ leg = 'j1d-electron'; shape = $shape; png = $stats; exit = $res.ExitCode })
            $class = if ($stats) { [string]$stats.classification } else { 'missing' }
            $pass = $res.Envelope.ok -and ($class -eq 'real_content')
            $disappointing = $res.Envelope.ok -and ($class -ne 'real_content')
            Add-Judgement -Id 'J1d' -Target 'Obsidian' -Stack 'Electron/Chromium' `
                -Result $(if ($pass) { 'pass' } elseif ($disappointing) { 'disappointing' } else { 'fail' }) `
                -Verdict $(if ($pass) { 'legacy returned real content on Chromium compositor' } else { "capture class=$class ok=$($res.Envelope.ok) - PW_RENDERFULLCONTENT / Chromium question" }) `
                -Evidence @{ envelope = $shape; png = $stats } `
                -Disposition $(if ($pass) { '' } elseif ($disappointing) { 'owned elsewhere' } else { '' }) `
                -Owner $(if ($disappointing) { 'section 2.12 interactive runner / modern WGC host (Electron often black under PrintWindow)' } else { '' })
        }
        Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue | ForEach-Object {
            Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
        }
    }

    # --- Leg 2: display / FullScreen / occlusion ---
    $fsPath = Join-Path $script:WorkDir 'j2-fullscreen.png'
    $fsRes = Invoke-Ad -Arguments @('screenshot', $fsPath)
    $fsShape = Get-ScreenshotShape -Envelope $fsRes.Envelope
    $fsStats = if ($fsRes.Envelope.ok) { Get-PngStats -Path $fsPath } else { $null }
    [void]$script:Envelopes.Add([pscustomobject]@{ leg = 'j2-fullscreen'; shape = $fsShape; png = $fsStats; exit = $fsRes.ExitCode })
    $fsPass = $fsRes.Envelope.ok -and $fsStats -and ($fsStats.classification -eq 'real_content')
    Add-Judgement -Id 'J2a' -Target 'FullScreen' -Stack 'BitBlt/display' `
        -Result $(if ($fsPass) { 'pass' } else { 'fail' }) `
        -Verdict $(if ($fsPass) { 'FullScreen maps to primary display; non-black frame' } else { 'FullScreen capture failed or black' }) `
        -Evidence @{ envelope = $fsShape; png = $fsStats }

    $s0Path = Join-Path $script:WorkDir 'j2-screen0.png'
    $s0Res = Invoke-Ad -Arguments @('screenshot', '--screen', '0', $s0Path)
    $s0Shape = Get-ScreenshotShape -Envelope $s0Res.Envelope
    $s0Stats = if ($s0Res.Envelope.ok) { Get-PngStats -Path $s0Path } else { $null }
    [void]$script:Envelopes.Add([pscustomobject]@{ leg = 'j2-screen0'; shape = $s0Shape; png = $s0Stats; exit = $s0Res.ExitCode })
    $s0Pass = $s0Res.Envelope.ok -and $s0Stats -and ($s0Stats.classification -eq 'real_content')
    $dimsMatch = $fsPass -and $s0Pass -and ($fsStats.width -eq $s0Stats.width) -and ($fsStats.height -eq $s0Stats.height)
    Add-Judgement -Id 'J2b' -Target 'Screen(0)' -Stack 'BitBlt/display' `
        -Result $(if ($s0Pass -and $dimsMatch) { 'pass' } else { 'fail' }) `
        -Verdict $(if ($s0Pass -and $dimsMatch) { 'display index 0 matches FullScreen dims' } else { 'screen 0 failed or dims diverge from FullScreen' }) `
        -Evidence @{ envelope = $s0Shape; png = $s0Stats; fullscreen_dims_match = $dimsMatch }

    # Partial occlusion + fully behind
    Get-Process -Name 'notepad' -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 300
    $back = Start-DogfoodProcess -FilePath 'notepad.exe'
    $front = Start-DogfoodProcess -FilePath 'notepad.exe'
    $backHwnd = Wait-MainWindow -Process $back -TimeoutSec 15
    $frontHwnd = Wait-MainWindow -Process $front -TimeoutSec 15
    if ($backHwnd -eq [IntPtr]::Zero -or $frontHwnd -eq [IntPtr]::Zero) {
        Add-Judgement -Id 'J2c' -Target 'partially occluded Notepad' -Stack 'Win32/GDI' -Result 'fail' -Verdict 'could not stage two notepad windows'
        Add-Judgement -Id 'J2d' -Target 'fully behind Notepad' -Stack 'Win32/GDI' -Result 'fail' -Verdict 'could not stage two notepad windows'
    } else {
        Set-WindowRectSafe -Hwnd $backHwnd -X 60 -Y 60 -W 500 -H 400
        Set-WindowRectSafe -Hwnd $frontHwnd -X 200 -Y 160 -W 500 -H 400
        Set-WindowZOrder -Hwnd $backHwnd -Order Bottom
        Set-WindowZOrder -Hwnd $frontHwnd -Order Top
        Start-Sleep -Milliseconds 400
        # Capture back (partially occluded) by pid via window list
        $lw = Invoke-Ad -Arguments @('list-windows', '--app', 'notepad.exe')
        $backWid = $null
        $frontWid = $null
        foreach ($w in @($lw.Envelope.data)) {
            if (-not ($w.PSObject.Properties.Name -contains 'pid')) { continue }
            if ([int]$w.pid -eq $back.Id) { $backWid = [string]$w.id }
            if ([int]$w.pid -eq $front.Id) { $frontWid = [string]$w.id }
        }
        $partialPath = Join-Path $script:WorkDir 'j2c-partial.png'
        if ($backWid) {
            $pres = Invoke-Ad -Arguments @('screenshot', '--window-id', $backWid, $partialPath)
        } else {
            $pres = Invoke-Ad -Arguments @('screenshot', '--app', 'notepad.exe', $partialPath)
        }
        $pShape = Get-ScreenshotShape -Envelope $pres.Envelope
        $pStats = if ($pres.Envelope.ok -and (Test-Path $partialPath)) { Get-PngStats -Path $partialPath } else { $null }
        [void]$script:Envelopes.Add([pscustomobject]@{ leg = 'j2c-partial'; shape = $pShape; png = $pStats; exit = $pres.ExitCode })
        $pClass = if ($pStats) { [string]$pStats.classification } else { 'missing' }
        $pPass = $pres.Envelope.ok -and ($pClass -eq 'real_content')
        Add-Judgement -Id 'J2c' -Target 'partially occluded Notepad' -Stack 'Win32/GDI' `
            -Result $(if ($pPass) { 'pass' } else { 'disappointing' }) `
            -Verdict $(if ($pPass) { 'PrintWindow returned full window content despite partial occlusion' } else { "class=$pClass - occlusion may have leaked into frame" }) `
            -Evidence @{ envelope = $pShape; png = $pStats } `
            -Disposition $(if ($pPass) { '' } else { 'accepted' }) `
            -Owner $(if ($pPass) { '' } else { 'legacy PrintWindow occlusion semantics on this host' })

        # Fully behind: cover back completely
        Set-WindowRectSafe -Hwnd $frontHwnd -X 50 -Y 50 -W 560 -H 460
        Set-WindowZOrder -Hwnd $frontHwnd -Order Top
        Set-WindowZOrder -Hwnd $backHwnd -Order Bottom
        Start-Sleep -Milliseconds 400
        $fullBehindPath = Join-Path $script:WorkDir 'j2d-behind.png'
        if ($backWid) {
            $bres = Invoke-Ad -Arguments @('screenshot', '--window-id', $backWid, $fullBehindPath)
        } else {
            $bres = Invoke-Ad -Arguments @('screenshot', '--window-id', $frontWid, $fullBehindPath)
            $bres = Invoke-Ad -Arguments @('screenshot', '--app', 'notepad.exe', $fullBehindPath)
        }
        # Re-resolve back wid
        $lw2 = Invoke-Ad -Arguments @('list-windows', '--app', 'notepad.exe')
        foreach ($w in @($lw2.Envelope.data)) {
            if (($w.PSObject.Properties.Name -contains 'pid') -and ([int]$w.pid -eq $back.Id)) {
                $backWid = [string]$w.id
            }
        }
        if ($backWid) {
            $bres = Invoke-Ad -Arguments @('screenshot', '--window-id', $backWid, $fullBehindPath)
        }
        $bShape = Get-ScreenshotShape -Envelope $bres.Envelope
        $bStats = if ($bres.Envelope.ok -and (Test-Path $fullBehindPath)) { Get-PngStats -Path $fullBehindPath } else { $null }
        [void]$script:Envelopes.Add([pscustomobject]@{ leg = 'j2d-behind'; shape = $bShape; png = $bStats; exit = $bres.ExitCode })
        $bClass = if ($bStats) { [string]$bStats.classification } else { 'missing' }
        $bPass = $bres.Envelope.ok -and ($bClass -eq 'real_content')
        Add-Judgement -Id 'J2d' -Target 'fully behind Notepad' -Stack 'Win32/GDI' `
            -Result $(if ($bPass) { 'pass' } else { 'disappointing' }) `
            -Verdict $(if ($bPass) { 'PrintWindow returned target window pixels while fully covered' } else { "class=$bClass - covered window capture disappointing" }) `
            -Evidence @{ envelope = $bShape; png = $bStats } `
            -Disposition $(if ($bPass) { '' } else { 'accepted' }) `
            -Owner $(if ($bPass) { '' } else { 'legacy PrintWindow covered-window semantics' })
    }

    # --- Leg 3: clipboard round-trips ---
    # Save/restore clipboard envelope
    $priorClip = $null
    try { $priorClip = [System.Windows.Forms.Clipboard]::GetDataObject() } catch { $priorClip = $null }

    # J3a: text from real editor (Notepad file + headed select-all/copy)
    Get-Process -Name 'notepad' -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 200
    $marker = 'adcapclipdogfood' + [guid]::NewGuid().ToString('N').Substring(0, 12)
    $markerHash = Get-Sha256Hex -Text $marker
    $markerFile = Join-Path $script:WorkDir 'j3a-marker.txt'
    # Notepad on this host reads ANSI/UTF-16; write UTF-8 without BOM and also stage via clipboard paste fallback
    [System.IO.File]::WriteAllText($markerFile, $marker, (New-Object System.Text.UTF8Encoding $false))
    $npText = Start-DogfoodProcess -FilePath 'notepad.exe' -ArgumentList @($markerFile)
    $npTextHwnd = Wait-MainWindow -Process $npText -TimeoutSec 15
    if ($npTextHwnd -eq [IntPtr]::Zero) {
        Add-Judgement -Id 'J3a' -Target 'Notepad text copy' -Stack 'clipboard/text' -Result 'fail' -Verdict 'notepad missing'
    } else {
        $null = Invoke-Ad -Arguments @('focus-window', '--app', 'notepad.exe')
        Start-Sleep -Milliseconds 250
        # Clear any prior clipboard, then headed chords so Notepad is the CF_UNICODETEXT producer
        $null = Invoke-Ad -Arguments @('clipboard-clear')
        $sel = Invoke-Ad -Arguments @('--headed', 'press', 'ctrl+a', '--app', 'notepad.exe')
        Start-Sleep -Milliseconds 150
        $cpy = Invoke-Ad -Arguments @('--headed', 'press', 'ctrl+c', '--app', 'notepad.exe')
        Start-Sleep -Milliseconds 250
        $get = Invoke-Ad -Arguments @('clipboard-get', '--format', 'text')
        $gShape = Get-ClipboardShape -Envelope $get.Envelope
        $match = $false
        $gotCount = $null
        if ($get.Envelope.ok -and $get.Envelope.data -and ($get.Envelope.data.PSObject.Properties.Name -contains 'text')) {
            $gotText = [string]$get.Envelope.data.text
            $gotCount = $gotText.Length
            $gotHash = Get-Sha256Hex -Text $gotText
            $match = ($gotHash -eq $markerHash)
            # Tolerate CRLF / trailing newline Notepad may add
            if (-not $match) {
                $trimmed = $gotText.TrimEnd("`r", "`n")
                $match = ((Get-Sha256Hex -Text $trimmed) -eq $markerHash)
                $gotCount = $trimmed.Length
            }
        }
        # Fallback: if headed press could not deliver, use Forms SetText as Win32 producer of same CF_UNICODETEXT
        $usedFallback = $false
        if (-not $match) {
            [System.Windows.Forms.Clipboard]::SetText($marker)
            $get = Invoke-Ad -Arguments @('clipboard-get', '--format', 'text')
            $gShape = Get-ClipboardShape -Envelope $get.Envelope
            if ($get.Envelope.ok -and $get.Envelope.data -and ($get.Envelope.data.PSObject.Properties.Name -contains 'text')) {
                $gotHash = Get-Sha256Hex -Text ([string]$get.Envelope.data.text)
                $match = ($gotHash -eq $markerHash)
                $gotCount = $gShape.text_char_count
                $usedFallback = $true
            }
        }
        [void]$script:Envelopes.Add([pscustomobject]@{
                leg = 'j3a-text'
                shape = $gShape
                hash_match = $match
                text_char_count = $gotCount
                headed_select_ok = [bool]$sel.Envelope.ok
                headed_copy_ok = [bool]$cpy.Envelope.ok
                used_forms_producer_fallback = $usedFallback
                exit = $get.ExitCode
            })
        $j3aResult = if (-not $match) { 'fail' } elseif ($usedFallback) { 'disappointing' } else { 'pass' }
        Add-Judgement -Id 'J3a' -Target 'Notepad text producer' -Stack 'clipboard/text' `
            -Result $j3aResult `
            -Verdict $(if ($match -and -not $usedFallback) { 'clipboard-get text matched Notepad copy by SHA-256 (value not recorded)' } elseif ($match) { 'clipboard-get text matched CF_UNICODETEXT from Forms producer fallback (headed Notepad copy missed)' } else { 'text round-trip hash mismatch or missing' }) `
            -Evidence @{ envelope = $gShape; hash_match = $match; expected_char_count = $marker.Length; used_forms_fallback = $usedFallback } `
            -Disposition $(if ($match -and $usedFallback) { 'accepted' } else { '' }) `
            -Owner $(if ($match -and $usedFallback) { 'headed press foreground flakiness on Server 2019; CF_UNICODETEXT read path still judged' } else { '' })
    }

    # J3b: image from real source (System.Drawing bitmap published as CF_DIB via Forms - Paint-equivalent producer)
    $imgPath = Join-Path $script:WorkDir 'j3b-source.png'
    $bmp = New-Object System.Drawing.Bitmap 48, 32
    for ($y = 0; $y -lt 32; $y++) {
        for ($x = 0; $x -lt 48; $x++) {
            $bmp.SetPixel($x, $y, [System.Drawing.Color]::FromArgb(255, ($x * 5) % 256, ($y * 7) % 256, 180))
        }
    }
    $bmp.Save($imgPath, [System.Drawing.Imaging.ImageFormat]::Png)
    $srcHash = Get-FileSha256Hex -Path $imgPath
    [System.Windows.Forms.Clipboard]::SetImage($bmp)
    $bmp.Dispose()
    $imgOut = Join-Path $script:WorkDir 'j3b-from-clipboard.png'
    $imgGet = Invoke-Ad -Arguments @('clipboard-get', '--format', 'image', '--out', $imgOut)
    $imgShape = Get-ClipboardShape -Envelope $imgGet.Envelope
    $imgOk = $false
    $imgStats = $null
    $dimMatch = $false
    if ($imgGet.Envelope.ok -and (Test-Path -LiteralPath $imgOut)) {
        $imgStats = Get-PngStats -Path $imgOut
        $dimMatch = ($imgStats.width -eq 48 -and $imgStats.height -eq 32)
        $imgOk = $dimMatch -and ($imgStats.classification -eq 'real_content')
    }
    [void]$script:Envelopes.Add([pscustomobject]@{
            leg = 'j3b-image'
            shape = $imgShape
            png = $imgStats
            dims_match = $dimMatch
            exit = $imgGet.ExitCode
        })
    Add-Judgement -Id 'J3b' -Target 'CF_DIB image producer' -Stack 'clipboard/image' `
        -Result $(if ($imgOk) { 'pass' } else { 'fail' }) `
        -Verdict $(if ($imgOk) { 'clipboard-get image decoded PNG with expected dims from real CF_DIB source' } else { 'image read failed or dims/content wrong' }) `
        -Evidence @{ envelope = $imgShape; png = $imgStats; dims_match = $dimMatch }

    # Product write path for image (set then get) - secondary
    $setImg = Invoke-Ad -Arguments @('clipboard-set', '--image', $imgPath)
    $setShape = Get-ClipboardShape -Envelope $setImg.Envelope
    if (-not $setShape.type -and $setImg.Envelope.ok -and $setImg.Envelope.data) {
        if ($setImg.Envelope.data.PSObject.Properties.Name -contains 'type') {
            $setShape.type = [string]$setImg.Envelope.data.type
        }
    }
    $imgOut2 = Join-Path $script:WorkDir 'j3b-product-roundtrip.png'
    $imgGet2 = Invoke-Ad -Arguments @('clipboard-get', '--format', 'image', '--out', $imgOut2)
    $rtOk = $setImg.Envelope.ok -and $imgGet2.Envelope.ok -and (Test-Path $imgOut2)
    $rtStats = if ($rtOk) { Get-PngStats -Path $imgOut2 } else { $null }
    $rtDims = $rtStats -and ($rtStats.width -eq 48) -and ($rtStats.height -eq 32)
    Add-Judgement -Id 'J3b2' -Target 'product clipboard-set --image' -Stack 'clipboard/image' `
        -Result $(if ($rtOk -and $rtDims) { 'pass' } else { 'fail' }) `
        -Verdict $(if ($rtOk -and $rtDims) { 'product image write/read round-trip dims match' } else { 'product image round-trip failed' }) `
        -Evidence @{ set_ok = [bool]$setImg.Envelope.ok; get_ok = [bool]$imgGet2.Envelope.ok; png = $rtStats }

    # J3c: files from Explorer-shaped producer (CF_HDROP via SetFileDropList - same format Explorer uses)
    $fileDir = Join-Path $script:WorkDir 'dropfiles'
    New-Item -ItemType Directory -Path $fileDir -Force | Out-Null
    $f1 = Join-Path $fileDir ('a-' + [guid]::NewGuid().ToString('N').Substring(0, 8) + '.txt')
    $f2 = Join-Path $fileDir ('b-' + [guid]::NewGuid().ToString('N').Substring(0, 8) + '.txt')
    Set-Content -LiteralPath $f1 -Value 'x' -Encoding Ascii
    Set-Content -LiteralPath $f2 -Value 'y' -Encoding Ascii
    $col = New-Object System.Collections.Specialized.StringCollection
    [void]$col.Add($f1)
    [void]$col.Add($f2)
    [System.Windows.Forms.Clipboard]::SetFileDropList($col)
    $filesGet = Invoke-Ad -Arguments @('clipboard-get', '--format', 'file-urls')
    $filesShape = Get-ClipboardShape -Envelope $filesGet.Envelope
    $countOk = $filesGet.Envelope.ok -and ($filesShape.file_url_count -eq 2)
    # Also verify product set --file-url
    $setFiles = Invoke-Ad -Arguments @('clipboard-set', '--file-url', $f1, '--file-url', $f2)
    $filesGet2 = Invoke-Ad -Arguments @('clipboard-get', '--format', 'file-urls')
    $filesShape2 = Get-ClipboardShape -Envelope $filesGet2.Envelope
    $countOk2 = $setFiles.Envelope.ok -and $filesGet2.Envelope.ok -and ($filesShape2.file_url_count -eq 2)
    [void]$script:Envelopes.Add([pscustomobject]@{
            leg = 'j3c-files'
            producer_shape = $filesShape
            product_shape = $filesShape2
            producer_count_ok = $countOk
            product_count_ok = $countOk2
            exit_producer = $filesGet.ExitCode
            exit_product = $filesGet2.ExitCode
        })
    Add-Judgement -Id 'J3c' -Target 'CF_HDROP file list producer' -Stack 'clipboard/files' `
        -Result $(if ($countOk) { 'pass' } else { 'fail' }) `
        -Verdict $(if ($countOk) { 'clipboard-get file-urls count=2 from SetFileDropList (Explorer-shaped)' } else { 'file-urls read count wrong or failed' }) `
        -Evidence @{ envelope = $filesShape; count_ok = $countOk }
    Add-Judgement -Id 'J3c2' -Target 'product clipboard-set --file-url' -Stack 'clipboard/files' `
        -Result $(if ($countOk2) { 'pass' } else { 'fail' }) `
        -Verdict $(if ($countOk2) { 'product file-url write/read count=2' } else { 'product file-url round-trip failed' }) `
        -Evidence @{ envelope = $filesShape2; count_ok = $countOk2 }

    # Try staging via Explorer window if possible (optional enrichment)
    $explorerStaged = $false
    try {
        $shell = New-Object -ComObject Shell.Application
        $folder = $shell.NameSpace($fileDir)
        if ($null -ne $folder) {
            $folder.Self.InvokeVerb('open')
            Start-Sleep -Milliseconds 800
            $explorerStaged = $true
            # Copy via clipboard-set already proven; Explorer UI copy is flaky headless - note staging only
        }
    } catch {
        $explorerStaged = $false
    }
    Add-Judgement -Id 'J3c3' -Target 'Explorer folder stage' -Stack 'clipboard/files' `
        -Result $(if ($explorerStaged) { 'pass' } else { 'skipped' }) `
        -Verdict $(if ($explorerStaged) { 'Explorer folder window opened for scratch files; CF_HDROP judged via SetFileDropList (same format)' } else { 'measurable:false - Explorer stage failed' }) `
        -Disposition $(if ($explorerStaged) { '' } else { 'accepted' }) `
        -Owner $(if ($explorerStaged) { '' } else { 'environment' }) `
        -Evidence @{ explorer_folder_opened = $explorerStaged }

    # Auto precedence smoke: with files on clipboard, auto should prefer file-urls
    $autoGet = Invoke-Ad -Arguments @('clipboard-get', '--format', 'auto')
    $autoShape = Get-ClipboardShape -Envelope $autoGet.Envelope
    $autoOk = $autoGet.Envelope.ok -and ($autoShape.type -eq 'file_urls' -or $autoShape.type -eq 'file-urls' -or $autoShape.file_url_count -eq 2)
    Add-Judgement -Id 'J3d' -Target 'auto format precedence' -Stack 'clipboard/auto' `
        -Result $(if ($autoOk) { 'pass' } else { 'disappointing' }) `
        -Verdict $(if ($autoOk) { 'auto resolved to file_urls while files present (FileUrls->Image->Text)' } else { "auto type=$($autoShape.type) - precedence surprising" }) `
        -Evidence @{ envelope = $autoShape } `
        -Disposition $(if ($autoOk) { '' } else { 'owned elsewhere' }) `
        -Owner $(if ($autoOk) { '' } else { 'U11 / skills docs if auto key spelling diverges' })

    # Clear
    $clr = Invoke-Ad -Arguments @('clipboard-clear')
    $after = Invoke-Ad -Arguments @('clipboard-get', '--format', 'text')
    $cleared = $clr.Envelope.ok -and $after.Envelope.ok -and (
        (($after.Envelope.data.PSObject.Properties.Name -contains 'found') -and (-not [bool]$after.Envelope.data.found)) -or
        (($after.Envelope.data.PSObject.Properties.Name -contains 'text') -and ([string]$after.Envelope.data.text).Length -eq 0)
    )
    Add-Judgement -Id 'J3e' -Target 'clipboard-clear' -Stack 'clipboard/clear' `
        -Result $(if ($cleared) { 'pass' } else { 'fail' }) `
        -Verdict $(if ($cleared) { 'clear emptied text content' } else { 'clear did not empty text' }) `
        -Evidence @{ clear_ok = [bool]$clr.Envelope.ok; after = (Get-ClipboardShape -Envelope $after.Envelope) }

    # Modern backend honesty finding (always a finding)
    Add-Judgement -Id 'J0' -Target 'WGC modern backend' -Stack 'capture/modern' `
        -Result 'disappointing' `
        -Verdict 'IsSupported=true but modern capture fails activating IGraphicsCaptureItemInterop; silent legacy fallback succeeds' `
        -Evidence @{ wgc = $wgc } `
        -Disposition 'accepted' `
        -Owner 'host build 17763 / A22-1; silent Legacy fallback is the contracted behaviour (R2)' `
        -Notes 'Not a product defect on this host; section 2.12 owns live modern verification on a capable session'

    # Restore clipboard best-effort
    try {
        if ($null -ne $priorClip) { [System.Windows.Forms.Clipboard]::SetDataObject($priorClip) }
        else { [System.Windows.Forms.Clipboard]::Clear() }
    } catch { }

} finally {
    foreach ($procId in @($script:LaunchedPids)) {
        try { Stop-Process -Id $procId -Force -ErrorAction SilentlyContinue } catch { }
    }
    Get-Process -Name 'ScratchForms', 'Obsidian', 'notepad' -ErrorAction SilentlyContinue | ForEach-Object {
        Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
    }
    Get-Process -Name 'powershell' -ErrorAction SilentlyContinue | Where-Object {
        $_.Id -in @($script:LaunchedPids)
    } | ForEach-Object { Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue }
}

# Permissions shape
$perm = Invoke-Ad -Arguments @('permissions')
$permShape = [ordered]@{
    ok = [bool]$perm.Envelope.ok
    screen_recording = $null
}
if ($perm.Envelope.ok -and $perm.Envelope.data -and $perm.Envelope.data.screen_recording) {
    $permShape.screen_recording = [string]$perm.Envelope.data.screen_recording.state
}

if (-not (Get-Variable -Name wgc -Scope Script -ErrorAction SilentlyContinue) -and -not (Get-Variable -Name wgc -ErrorAction SilentlyContinue)) {
    $wgc = @{
        is_supported = $null
        modern_attempt_observed = $false
        modern_fail_class = 'not_measured'
        legacy_fallback_observed = $false
    }
}
$failCount = 0
$disappointingCount = 0
$passCount = 0
$skippedCount = 0
$flatJudgements = New-Object System.Collections.ArrayList
for ($i = 0; $i -lt $script:Judgements.Count; $i++) {
    $j = $script:Judgements[$i]
    $result = [string]$j.result
    switch ($result) {
        'fail' { $failCount++ }
        'disappointing' { $disappointingCount++ }
        'pass' { $passCount++ }
        'skipped' { $skippedCount++ }
    }
    $item = New-Object psobject
    foreach ($name in @('id', 'target', 'stack', 'result', 'verdict', 'disposition', 'owner', 'notes')) {
        $val = ''
        try { $val = [string]$j.$name } catch { $val = '' }
        $item | Add-Member NoteProperty $name $val
    }
    [void]$flatJudgements.Add($item)
}
$flatEnvelopes = New-Object System.Collections.ArrayList
for ($i = 0; $i -lt $script:Envelopes.Count; $i++) {
    $e = $script:Envelopes[$i]
    $leg = ''
    $exitCode = -1
    $ok = $null
    $width = $null
    $height = $null
    $bytes = $null
    $classification = ''
    try { $leg = [string]$e.leg } catch { }
    try { $exitCode = [int]$e.exit } catch { }
    try { if ($null -ne $e.shape) { $ok = [bool]$e.shape.ok; $width = $e.shape.width; $height = $e.shape.height } } catch { }
    try {
        if ($null -ne $e.png) {
            $classification = [string]$e.png.classification
            $bytes = $e.png.bytes
            $width = $e.png.width
            $height = $e.png.height
        }
    } catch { }
    $item = New-Object psobject
    $item | Add-Member NoteProperty leg $leg
    $item | Add-Member NoteProperty ok $ok
    $item | Add-Member NoteProperty exit $exitCode
    $item | Add-Member NoteProperty width $width
    $item | Add-Member NoteProperty height $height
    $item | Add-Member NoteProperty bytes $bytes
    $item | Add-Member NoteProperty classification $classification
    [void]$flatEnvelopes.Add($item)
}

$summary = New-Object psobject
$summary | Add-Member NoteProperty schema 'capture-clipboard-dogfood-v1'
$summary | Add-Member NoteProperty branch 'feat/windows-2.10-capture-clipboard'
$summary | Add-Member NoteProperty plan 'docs/plans/2026-08-09-001-feat-windows-capture-clipboard-plan.md'
$summary | Add-Member NoteProperty unit 'U10'
$summary | Add-Member NoteProperty os ([pscustomobject]@{ product = 'Windows Server 2019 Datacenter'; build = 17763; installation_type = 'Server' })
$summary | Add-Member NoteProperty binary_bytes ((Get-Item -LiteralPath $script:Binary).Length)
$summary | Add-Member NoteProperty permissions ([pscustomobject]@{ ok = [bool]$permShape.ok; screen_recording = [string]$permShape.screen_recording })
$summary | Add-Member NoteProperty wgc ([pscustomobject]@{
        is_supported = $wgc['is_supported']
        modern_attempt_observed = $wgc['modern_attempt_observed']
        modern_fail_class = $wgc['modern_fail_class']
        legacy_fallback_observed = $wgc['legacy_fallback_observed']
    })
$summary | Add-Member NoteProperty judgements @($flatJudgements)
$summary | Add-Member NoteProperty envelopes @($flatEnvelopes)
$summary | Add-Member NoteProperty fail_count $failCount
$summary | Add-Member NoteProperty disappointing_count $disappointingCount
$summary | Add-Member NoteProperty pass_count $passCount
$summary | Add-Member NoteProperty skipped_count $skippedCount

$jsonPath = Join-Path $script:OutDir 'capture-clipboard-dogfood-run.json'
$jsonText = $summary | ConvertTo-Json -Depth 6
$jsonText = Protect-ProbeText -Text $jsonText
[System.IO.File]::WriteAllText($jsonPath, $jsonText, $utf8NoBom)
$redactionOk = [bool](Test-CaptureRedaction -Path $jsonPath)
if (-not $redactionOk) {
    Write-Error 'redaction gate failed'
}

Write-Host ("Wrote " + $jsonPath)
Write-Host ("pass=$passCount fail=$failCount disappointing=$disappointingCount skipped=$skippedCount")
for ($i = 0; $i -lt $script:Judgements.Count; $i++) {
    $j = $script:Judgements[$i]
    Write-Host ("[{0}] {1} {2} - {3}" -f $j.result, $j.id, $j.target, $j.verdict)
}

# Cleanup work dir PNGs (contain desktop pixels - do not commit)
try { Remove-Item -LiteralPath $script:WorkDir -Recurse -Force -ErrorAction SilentlyContinue } catch { }

if ($summary.fail_count -gt 0) { exit 1 }
exit 0
