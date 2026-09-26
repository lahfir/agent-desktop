#Requires -Version 5.1
<#
.SYNOPSIS
    Hosted-runner Windows.Graphics.Capture (WGC) modern-capture reading (area 24, sub-phase 2.12, U14).

.DESCRIPTION
    A22-1 measured `GraphicsCaptureSession.IsSupported` on this corpus's dev
    box (Server 2019, build 17763) and recorded it unsupported; every later
    A22 capture that touches the modern backend records the cross-integrity
    WGC arm as `supported_cross_direction_not_instrumented` rather than an
    actual frame capture, so no committed row anywhere proves modern-capture
    pixels. This probe is the reading that closes that gap: it attempts a
    real WGC frame capture of the host's primary monitor - not merely an
    `IsSupported` check - built as a standalone scratch crate that links the
    `windows` crate directly (the exact API surface
    crates/windows/src/system/capture_modern.rs and capture_d3d.rs use in
    production, transcribed rather than imported: those modules are
    `pub(crate)` inside a private `system` module and are not reachable from
    outside agent-desktop-windows).

    Three branches:
      - `unsupported_on_host`  - IsSupported is false.
      - `supported_capture_succeeded` - IsSupported is true and a captured
        frame produced at least one non-black pixel.
      - `supported_capture_failed` - IsSupported is true but the capture
        pipeline (item activation, device creation, frame pool, session, or
        texture readback) failed; the reason is recorded. This is this
        corpus's own dev box's reading, run while building this probe:
        IsSupported is true on build 17763 but IGraphicsCaptureItemInterop
        activation returns E_NOINTERFACE (HRESULT 0x80004002), reproducing
        capture_modern.rs's own `interop_is_available` doc note verbatim.

    Corpus safety: the capture records shapes and counts only - width,
    height, a sampled/non-zero pixel count and a boolean - never the pixel
    bytes themselves, mirroring 22-capture-clipboard's own
    NonZeroPixels/AppearsBlack fields for the legacy backend.

    Run: powershell -NoProfile -ExecutionPolicy Bypass -File .\probes\windows\24-fixture-e2e\10-hosted-modern-capture.ps1 -Label <devbox|ci>
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'ci')][string]$Label = 'devbox'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

. (Join-Path (Split-Path -Parent $PSCommandPath) '..\common.ps1')
Initialize-ProbeRedaction

$script:Probe = '24-fixture-e2e-10-hosted-modern-capture'
$script:ProbeDir = Split-Path -Parent $PSCommandPath
$script:CaptureDir = Join-Path $script:ProbeDir 'captures'
if (-not (Test-Path -LiteralPath $script:CaptureDir)) {
    New-Item -ItemType Directory -Path $script:CaptureDir -Force | Out-Null
}

Register-MandatoryCapture -Name @("hosted-modern-capture-$Label.json")

function Write-A24Capture {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Content
    )
    $redacted = Protect-ProbeText -Text $Content
    $path = Join-Path $script:CaptureDir $Name
    $utf8NoBom = New-Object System.Text.UTF8Encoding $false
    [IO.File]::WriteAllText($path, $redacted, $utf8NoBom)
    $normalized = Get-NormalizedCapture -Text $redacted
    [IO.File]::WriteAllText(($path + '.normalized'), $normalized, $utf8NoBom)
    if (-not (Test-CaptureRedaction -Path $path)) {
        throw "redaction residue in $path"
    }
    return $path
}

<#
    Builds the standalone scratch crate this probe links against: a private
    Cargo.toml depending on the `windows` crate alone (the same version and
    feature set crates/windows/Cargo.toml pins), never agent-desktop-windows
    - that crate's capture modules are pub(crate) and unreachable from
    outside it. Mirrors 17-resolution/probe.ps1's Build-ProbeBinary shape.
#>
function Build-CaptureProbeBinary {
    $result = [ordered]@{ skipped = $null; buildFailed = $false; exe = $null }
    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
    if (-not $cargo) {
        $result.skipped = 'cargo is not installed on this machine'
        return $result
    }
    $work = Join-Path ([IO.Path]::GetTempPath()) ('agent-desktop-a24-wgc-' + [guid]::NewGuid())
    New-Item -ItemType Directory -Path (Join-Path $work 'src') -Force | Out-Null
    try {
        $manifest = '  [package]' + "`n" + '  name = "agent-desktop-hosted-modern-capture-probe"' + "`n" + '  version = "0.0.0"' + "`n" + '  edition = "2021"' + "`n" + "`n" + '  [dependencies]' + "`n" + '  serde_json = "1"' + "`n" + '  windows = { version = "=0.62.2", features = [' + "`n" + '    "Graphics_Capture",' + "`n" + '    "Graphics_DirectX_Direct3D11",' + "`n" + '    "Win32_Foundation",' + "`n" + '    "Win32_Graphics_Direct3D",' + "`n" + '    "Win32_Graphics_Direct3D11",' + "`n" + '    "Win32_Graphics_Dxgi_Common",' + "`n" + '    "Win32_Graphics_Gdi",' + "`n" + '    "Win32_System_Com",' + "`n" + '    "Win32_System_WinRT_Direct3D11",' + "`n" + '    "Win32_System_WinRT_Graphics_Capture",' + "`n" + '  ] }' + "`n" + "`n" + '  [workspace]'
        $utf8NoBom = New-Object System.Text.UTF8Encoding $false
        [IO.File]::WriteAllText((Join-Path $work 'Cargo.toml'), $manifest, $utf8NoBom)
        Copy-Item -LiteralPath (Join-Path $script:ProbeDir '10-hosted-modern-capture.rs') -Destination (Join-Path $work 'src\main.rs') -Force
        Push-Location $work
        try {
            & cargo build --quiet 2>&1 | Write-Verbose
            if ($LASTEXITCODE -ne 0) {
                $result.skipped = "cargo build failed with exit code $LASTEXITCODE"
                $result.buildFailed = $true
                return $result
            }
            $result.exe = Join-Path $work 'target\debug\agent-desktop-hosted-modern-capture-probe.exe'
            return $result
        } finally {
            Pop-Location
        }
    } catch {
        $result.skipped = ('build failed: ' + $_.Exception.Message)
        $result.buildFailed = $true
        return $result
    }
}

function Measure-HostedModernCapture {
    $built = Build-CaptureProbeBinary
    if ($built.buildFailed -or -not $built.exe -or -not (Test-Path -LiteralPath $built.exe)) {
        return [ordered]@{
            measurable = $false
            branch     = 'probe_binary_build_failed'
            detail     = [string]$built.skipped
        }
    }
    $raw = (& $built.exe 2>$null | Out-String)
    $exitCode = $LASTEXITCODE
    if ($exitCode -ne 0) {
        return [ordered]@{
            measurable = $false
            branch     = 'probe_binary_exited_nonzero'
            exit_code  = $exitCode
        }
    }
    try {
        return ($raw | ConvertFrom-Json)
    } catch {
        return [ordered]@{
            measurable = $false
            branch     = 'probe_binary_output_not_json'
            error_class = $_.Exception.GetType().Name
        }
    }
}

# ---------------------------------------------------------------- main

$question = 'does a real Windows.Graphics.Capture frame capture against this host''s primary monitor produce non-degenerate pixels, and which branch fires (unsupported, supported-and-succeeded, supported-and-failed)'

$reading = $null
try { $reading = Measure-HostedModernCapture } catch {
    $reading = [ordered]@{ measurable = $false; branch = 'probe_threw'; error_class = $_.Exception.GetType().Name }
}

$result = [ordered]@{
    probe    = $script:Probe
    question = $question
    cites    = @('A22-1')
    reading  = $reading
}

<#
    `unsupported_on_host`, `supported_capture_succeeded` and
    `supported_capture_failed` are the three legitimate answers this probe
    exists to distinguish - all real measurements. The remaining four
    branches (the scratch crate did not build, its binary exited non-zero,
    its stdout was not JSON, or the whole probe threw) are not an answer to
    the question at all; they are this measurement failing to run.
    Registering $result for those as-is would pass Register-MandatoryPass's
    gate (a populated dictionary carries no not_measured marker at its own
    top level - Test-PassNotMeasured never looks inside `.reading`), so a
    scratch-crate compile failure on the hosted lane would go green with no
    reading, which is exactly the failure mode 17-resolution/probe.ps1's
    Invoke-ProbePass exists to prevent for its own probe binary. The
    infra-failure branches are therefore registered through
    New-NotMeasuredResult instead, which -Label ci's mandatory-measurement
    gate does fail on; the written capture still keeps the full diagnostic
    $result for a human to read.
#>
$infraFailureBranches = @('probe_binary_build_failed', 'probe_binary_exited_nonzero', 'probe_binary_output_not_json', 'probe_threw')
$registeredResult = $result
if ($infraFailureBranches -contains [string]$reading.branch) {
    $registeredResult = New-NotMeasuredResult -Reason ("$($reading.branch): " + (ConvertTo-Json -InputObject $reading -Depth 6 -Compress))
}

$overallError = $null
$capturePath = $null
try {
    $capturePath = Write-A24Capture -Name "hosted-modern-capture-$Label.json" -Content (ConvertTo-Json -InputObject $result -Depth 10)
    Register-MandatoryPass -Capture $capturePath -Result $registeredResult
} catch {
    $overallError = $_.Exception.GetType().Name
}

if ($null -ne $overallError) {
    Write-ProbeResult -Probe $script:Probe -Status 'fail' -Message ('probe threw while writing capture: ' + $overallError) -Data @{ error_class = $overallError }
    exit 1
}

Assert-MandatoryMeasurement -Probe $script:Probe -Label $Label

Write-ProbeResult -Probe $script:Probe -Status 'ok' -Message 'hosted modern-capture probe captured' -Data @{
    capture = Split-Path -Leaf $capturePath
    branch  = [string]$reading.branch
}
exit 0
