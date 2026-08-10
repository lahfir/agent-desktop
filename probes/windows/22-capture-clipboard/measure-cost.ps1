#Requires -Version 5.1
<#
.SYNOPSIS
    Hot-path capture/clipboard cost baseline (min-of-seven, warm-up discarded).
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'ci')][string]$Label = 'devbox'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

. (Join-Path (Split-Path -Parent $PSCommandPath) '..\common.ps1')
. (Join-Path (Split-Path -Parent $PSCommandPath) 'native.ps1')
Initialize-ProbeRedaction
Initialize-CaptureClipboardNative

$script:ProbeDir = Split-Path -Parent $PSCommandPath
$script:CaptureDir = Join-Path $script:ProbeDir 'captures'
if (-not (Test-Path -LiteralPath $script:CaptureDir)) {
    New-Item -ItemType Directory -Path $script:CaptureDir -Force | Out-Null
}

function Write-CaptureFile {
    param([string]$Name, [string]$Content)
    $redacted = Protect-ProbeText -Text $Content
    $path = Join-Path $script:CaptureDir $Name
    $utf8NoBom = New-Object System.Text.UTF8Encoding $false
    [IO.File]::WriteAllText($path, $redacted, $utf8NoBom)
    $normalized = Get-NormalizedCapture -Text $redacted
    [IO.File]::WriteAllText(($path + '.normalized'), $normalized, $utf8NoBom)
    if (-not (Test-CaptureRedaction -Path $path)) { throw "redaction residue in $path" }
    return $path
}

function Summarize-Samples {
    param([double[]]$Samples)
    $sorted = @($Samples | Sort-Object)
    $used = @($sorted | Select-Object -Skip 1)
    $medianIdx = [int][Math]::Floor($used.Count / 2)
    return [ordered]@{
        samples_ms       = @($Samples)
        min_ms           = [double]($used | Measure-Object -Minimum).Minimum
        median_ms        = [double]($used | Sort-Object)[$medianIdx]
        max_ms           = [double]($used | Measure-Object -Maximum).Maximum
        n                = $used.Count
        warmup_discarded = $true
    }
}

$hwnd = [AgentDesktopProbe.A22.Capture22]::CreatePaintedWindow()
try {
    $legacyWindow = New-Object System.Collections.ArrayList
    $legacyDisplay = New-Object System.Collections.ArrayList
    $wicProxy = New-Object System.Collections.ArrayList
    $clipText = New-Object System.Collections.ArrayList
    $clipImage = New-Object System.Collections.ArrayList

    for ($i = 0; $i -lt 8; $i++) {
        $sw = [Diagnostics.Stopwatch]::StartNew()
        [void][AgentDesktopProbe.A22.Capture22]::CapturePrintWindow($hwnd, $true)
        $sw.Stop()
        [void]$legacyWindow.Add([double]$sw.Elapsed.TotalMilliseconds)

        $sw.Restart()
        [void][AgentDesktopProbe.A22.Capture22]::CaptureBitBltPrimary()
        $sw.Stop()
        [void]$legacyDisplay.Add([double]$sw.Elapsed.TotalMilliseconds)

        $sw.Restart()
        [void][AgentDesktopProbe.A22.Capture22]::MeasureWicRoundTrip()
        $sw.Stop()
        [void]$wicProxy.Add([double]$sw.Elapsed.TotalMilliseconds)

        $sw.Restart()
        if ([AgentDesktopProbe.A22.Capture22]::OpenClipboard([IntPtr]::Zero)) {
            [void][AgentDesktopProbe.A22.Capture22]::EmptyClipboard()
            [void][AgentDesktopProbe.A22.Capture22]::CloseClipboard()
        }
        $sw.Stop()
        [void]$clipText.Add([double]$sw.Elapsed.TotalMilliseconds)

        $sw.Restart()
        [void][AgentDesktopProbe.A22.Capture22]::MeasureDibShapes()
        $sw.Stop()
        [void]$clipImage.Add([double]$sw.Elapsed.TotalMilliseconds)
    }

    $capture = [ordered]@{
        probe              = '22-capture-clipboard'
        question           = 'hot-path legacy capture, WIC-proxy encode, clipboard text/image (min-of-seven, warm-up discarded)'
        methodology_cites  = @('A15-13', 'A18-7', 'A21-cost')
        legacy_window      = Summarize-Samples -Samples @($legacyWindow)
        legacy_display     = Summarize-Samples -Samples @($legacyDisplay)
        wic_encode_proxy   = Summarize-Samples -Samples @($wicProxy)
        clipboard_text     = Summarize-Samples -Samples @($clipText)
        clipboard_image    = Summarize-Samples -Samples @($clipImage)
    }
    $path = Write-CaptureFile -Name "capture-cost-$Label.json" -Content (ConvertTo-Json -InputObject $capture -Depth 8)
    Write-Host "wrote $path"
} finally {
    if ($hwnd -ne [IntPtr]::Zero) {
        [void][AgentDesktopProbe.A22.Capture22]::DestroyWindow($hwnd)
    }
}
