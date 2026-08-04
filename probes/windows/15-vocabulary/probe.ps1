#Requires -Version 5.1
<#
.SYNOPSIS
    Sub-phase 2.3 vocabulary probe.

.DESCRIPTION
    Drives probe.rs, which answers the nine vocabulary questions the 2.3 plan
    refuses to infer - LabeledBy resolution and cost, HelpText and
    FullDescription content, the LegacyIAccessible DefaultAction gate, the
    ValueIsReadOnly and SelectionCanSelectMultiple state feeds, the
    IsControlElement and IsContentElement view flags, what a pattern-state
    property returns without its pattern, the RawView-versus-ControlView delta,
    the marginal cost of the expanded property set, and whether a cached
    element-returning property carries the request's properties.

    Captures are written beside this script under captures\, BOM-less UTF-8,
    through the corpus redaction gate in ..\common.ps1.

    A pass that runs and answers "no" is data and is recorded as such. A pass
    that never answered is not data, in either of the two ways it can fail: the
    probe binary exited non-zero, so the capture holds a placeholder rather than
    a measurement, or the pass never ran at all and wrote no capture. Under
    -Label ci the captures declared mandatory below must all exist and all hold
    measurements; a placeholder or an absent capture fails the run, because CI's
    only other signal is the artifact upload, which a placeholder satisfies
    exactly as well as a measurement does and which the passes that did run
    satisfy on behalf of the ones that did not. A dev-box run keeps the
    placeholder and stays green; the operator can see it.
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'ci')][string]$Label = 'devbox',
    [string]$UiAutomationVersion = '0.25.0'
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

Register-MandatoryCapture -Name @(
    "uia-vocabulary-$Label.json",
    "uia-vocabulary-wpf-$Label.json"
)

function Write-VocabularyCapture {
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

function Invoke-VocabularyProbe {
    param([int]$AttachHandle = 0)
    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
    if (-not $cargo) {
        return (New-NotMeasuredResult -Reason 'cargo is not installed on this machine')
    }
    $work = Join-Path ([IO.Path]::GetTempPath()) ('agent-desktop-vocabulary-' + [guid]::NewGuid())
    New-Item -ItemType Directory -Path (Join-Path $work 'src') -Force | Out-Null
    try {
        $manifest = @"
[package]
name = "agent-desktop-vocabulary-probe"
version = "0.0.0"
edition = "2021"

[dependencies]
serde_json = "1"
uiautomation = "=$UiAutomationVersion"

[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.62.2", features = [
  "Win32_System_Com",
  "Win32_UI_Accessibility",
] }
windows-sys = { version = "0.61", features = [
  "Win32_Foundation",
  "Win32_Graphics_Gdi",
  "Win32_System_Com",
  "Win32_System_LibraryLoader",
  "Win32_UI_WindowsAndMessaging",
] }

[workspace]
"@
        $utf8NoBom = New-Object System.Text.UTF8Encoding $false
        [IO.File]::WriteAllText((Join-Path $work 'Cargo.toml'), $manifest, $utf8NoBom)
        Copy-Item -LiteralPath (Join-Path $script:ProbeDir 'probe.rs') -Destination (Join-Path $work 'src\main.rs') -Force
        foreach ($module in @('probe_window.rs', 'probe_properties.rs', 'probe_measure.rs', 'probe_cost.rs')) {
            Copy-Item -LiteralPath (Join-Path $script:ProbeDir $module) -Destination (Join-Path $work "src\$module") -Force
        }
        $env:PROBE_UIAUTOMATION_VERSION = $UiAutomationVersion
        Push-Location $work
        try {
            & cargo build --quiet 2>&1 | Write-Verbose
            if ($LASTEXITCODE -ne 0) {
                return (New-NotMeasuredResult -Reason "cargo build failed with exit code $LASTEXITCODE")
            }
            if ($AttachHandle -ne 0) {
                $raw = & cargo run --quiet -- --attach $AttachHandle 2>$null | Out-String
            } else {
                $raw = & cargo run --quiet 2>$null | Out-String
            }
            if ($LASTEXITCODE -ne 0) {
                return (New-NotMeasuredResult -Reason "the probe exited with code $LASTEXITCODE")
            }
            return ($raw | ConvertFrom-Json)
        } finally {
            Pop-Location
        }
    } finally {
        Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
    }
}

$vocabulary = Invoke-VocabularyProbe
$path = Write-VocabularyCapture -Name "uia-vocabulary-$Label.json" -Content (ConvertTo-Json -InputObject $vocabulary -Depth 30)
Register-MandatoryPass -Capture $path -Result $vocabulary
Write-Host "wrote $path"

<#
    The Win32 client-side proxy has no MSAA relation to derive `LabeledBy`
    from, so the fixture above cannot answer the LabeledBy questions at all.
    WPF carries `AutomationProperties.LabeledBy` as a first-class relation, so
    the same measurements run a second time against the WPF scratch window -
    which is also a larger, real out-of-process XAML tree, and therefore the
    better datapoint for the marginal-cost question.
#>
function Invoke-WpfVocabularyProbe {
    $script = Join-Path $script:ProbeDir '..\scratch\ScratchWpf.ps1'
    if (-not (Test-Path -LiteralPath $script)) {
        return (New-NotMeasuredResult -Reason 'the WPF scratch fixture is not present')
    }
    $process = Start-Process -FilePath 'powershell.exe' `
        -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $script, '-Tag', 'vocab', '-TimeoutSeconds', '180') `
        -PassThru -WindowStyle Hidden
    try {
        $handle = 0
        for ($attempt = 0; $attempt -lt 60; $attempt++) {
            Start-Sleep -Milliseconds 500
            $process.Refresh()
            if ($process.HasExited) { break }
            if ($process.MainWindowHandle -ne 0) {
                $handle = [int]$process.MainWindowHandle
                break
            }
        }
        if ($handle -eq 0) {
            return (New-NotMeasuredResult -Reason 'the WPF scratch window never reported a handle')
        }
        Start-Sleep -Seconds 2
        return Invoke-VocabularyProbe -AttachHandle $handle
    } finally {
        if (-not $process.HasExited) {
            Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
        }
    }
}

$wpf = Invoke-WpfVocabularyProbe
$wpfPath = Write-VocabularyCapture -Name "uia-vocabulary-wpf-$Label.json" -Content (ConvertTo-Json -InputObject $wpf -Depth 30)
Register-MandatoryPass -Capture $wpfPath -Result $wpf
Write-Host "wrote $wpfPath"

$script:measurementGap = Get-MandatoryMeasurementGap
if ($Label -eq 'ci' -and $null -ne $script:measurementGap) {
    Write-ProbeResult -Probe '15-vocabulary' -Status 'fail' `
        -Message 'a mandatory pass produced no capture or recorded a placeholder instead of a measurement' `
        -Data $script:measurementGap
    exit 1
}

Write-ProbeResult -Probe '15-vocabulary' -Status 'ok' -Message 'vocabulary probes captured' -Data @{
    win32 = Split-Path -Leaf $path
    wpf = Split-Path -Leaf $wpfPath
}
exit 0
