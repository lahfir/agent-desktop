#Requires -Version 5.1
<#
.SYNOPSIS
    Sub-phase 2.4 observation probe (A16).

.DESCRIPTION
    Orchestrates the observation measurements the 2.4 plan refuses to infer. On
    the dev box it runs three passes of probe.rs - against the probe's own
    Win32 fixture (hosted in a child process), against the WPF scratch window,
    and against the Chromium/Electron target (Obsidian) when present - plus the
    split-integrity read in which a lowered-integrity instance of the probe
    reads a window the probe's own High-integrity fixture owns. The raw Win32
    census (window enumeration, foreground semantics, process sources, DPI,
    IVirtualDesktopManager) is a sibling script this runner invokes.

    Every capture is written beside this script under captures\, BOM-less
    UTF-8, through the corpus redaction gate in ..\common.ps1. Chromium item
    captions follow the 2.3 scheduling rule: the Electron row runs first, with
    no other probe window on the desktop, after an idle settle.

    A pass that runs and answers "no" is data and is recorded as such. A pass
    that never answered is not data, in either of the two ways it can fail: the
    probe binary exited non-zero, so the capture holds a placeholder rather than
    a measurement, or the pass never ran at all and wrote no capture. Under
    -Label ci the captures declared mandatory below must all exist and all hold
    measurements; a placeholder or an absent capture fails the run, because CI's
    only other signal is the artifact upload, which a placeholder satisfies
    exactly as well as a measurement does and which the passes that did run
    satisfy on behalf of the ones that did not. A run that declared no
    mandatory captures at all fails the same way, because a gate handed nothing
    to check is not a check. A dev-box run keeps the placeholder and stays
    green; the operator can see it. The Chromium passes are dev-box only and are
    therefore not mandatory.
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'ci')][string]$Label = 'devbox',
    [switch]$SkipChromium,
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
$script:Spawned = New-Object System.Collections.ArrayList

Register-MandatoryCapture -Name @(
    "observation-census-$Label.json",
    "observation-win32-$Label.json",
    "observation-wpf-$Label.json",
    "observation-split-integrity-$Label.json"
)

function Write-ObservationCapture {
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

function Build-ProbeBinary {
    # buildFailed distinguishes a genuinely broken build from a missing
    # prerequisite: cargo being absent is an environment fact a dev box can
    # legitimately skip on, while a compile failure means the source list
    # below or the probe sources themselves are broken and must not be
    # reported as a benign skip under -Label ci (see the caller).
    $result = [ordered]@{ skipped = $null; buildFailed = $false; work = $null; exe = $null }
    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
    if (-not $cargo) {
        $result.skipped = 'cargo is not installed on this machine'
        return $result
    }
    $work = Join-Path ([IO.Path]::GetTempPath()) ('agent-desktop-observation-' + [guid]::NewGuid())
    New-Item -ItemType Directory -Path (Join-Path $work 'src') -Force | Out-Null
    try {
        $manifest = @"
[package]
name = "agent-desktop-observation-probe"
version = "0.0.0"
edition = "2021"

[dependencies]
serde_json = "1"
uiautomation = "=$UiAutomationVersion"
windows = { version = "0.62.2", features = [
  "Win32_System_Com",
  "Win32_UI_Accessibility",
] }
windows-sys = { version = "0.61", features = [
  "Win32_Foundation",
  "Win32_Graphics_Gdi",
  "Win32_System_Com",
  "Win32_System_LibraryLoader",
  "Win32_System_Threading",
  "Win32_System_ProcessStatus",
  "Win32_UI_WindowsAndMessaging",
] }

[workspace]
"@
        $utf8NoBom = New-Object System.Text.UTF8Encoding $false
        [IO.File]::WriteAllText((Join-Path $work 'Cargo.toml'), $manifest, $utf8NoBom)
        foreach ($file in @('probe.rs', 'probe_window.rs', 'probe_properties.rs', 'probe_measure.rs', 'probe_measure_arms.rs', 'probe_cost.rs')) {
            Copy-Item -LiteralPath (Join-Path $script:ProbeDir $file) -Destination (Join-Path $work "src\$file") -Force
        }
        Copy-Item -LiteralPath (Join-Path $work 'src\probe.rs') -Destination (Join-Path $work 'src\main.rs') -Force
        $env:PROBE_UIAUTOMATION_VERSION = $UiAutomationVersion
        Push-Location $work
        try {
            & cargo build --quiet 2>&1 | Write-Verbose
            if ($LASTEXITCODE -ne 0) {
                $result.skipped = "cargo build failed with exit code $LASTEXITCODE; detailed logs were captured on $VerbosePreference"
                $result.buildFailed = $true
                return $result
            }
            $result.work = $work
            $result.exe = Join-Path $work 'target\debug\agent-desktop-observation-probe.exe'
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

function Invoke-ProbePass {
    param(
        [Parameter(Mandatory = $true)][string]$Exe,
        [string[]]$Arguments = @()
    )
    $raw = (& $Exe @Arguments 2>$null | Out-String)
    if ($LASTEXITCODE -ne 0) {
        return (New-NotMeasuredResult -Reason "the probe exited with code $LASTEXITCODE")
    }
    return ($raw | ConvertFrom-Json)
}

function Invoke-SplitIntegrityRead {
    param(
        [Parameter(Mandatory = $true)][string]$Exe,
        [Parameter(Mandatory = $true)][IntPtr]$WindowHandle
    )
    $highOut = Join-Path $script:CaptureDir "split-high-$Label.json"
    $mediumOut = Join-Path $script:CaptureDir "split-medium-$Label.json"
    $high = & $Exe --read-hwnd $WindowHandle.ToString() --out $highOut 2>$null | Out-String
    $highCode = $LASTEXITCODE
    $highJson = $null
    if (Test-Path -LiteralPath $highOut) {
        $highJson = Get-Content -LiteralPath $highOut -Raw | ConvertFrom-Json
        Remove-Item -LiteralPath $highOut -Force -ErrorAction SilentlyContinue
    }
    try {
        $medium = Start-MediumIntegrityProcess -FilePath $Exe -ArgumentList @('--read-hwnd', $WindowHandle.ToString(), '--out', $mediumOut)
        $mediumJson = $null
        $waitUntil = (Get-Date).AddSeconds(30)
        while ((Get-Date) -lt $waitUntil) {
            if (Test-Path -LiteralPath $mediumOut) { break }
            Start-Sleep -Milliseconds 250
        }
        if (Test-Path -LiteralPath $mediumOut) {
            $mediumJson = Get-Content -LiteralPath $mediumOut -Raw | ConvertFrom-Json
            Remove-Item -LiteralPath $mediumOut -Force -ErrorAction SilentlyContinue
        }
        Start-Sleep -Seconds 1
        return [ordered]@{
            window_handle_int = $WindowHandle.ToInt64()
            high_integrity_read = @{
                exit_code = $highCode
                root_resolved = ($null -ne $highJson -and $highJson.measurements.root.resolved)
                token_read = ($null -ne $highJson -and $highJson.measurements.process_token.'creation_time_seconds' -ne $null)
            }
            medium_integrity_read = @{
                process_id = $medium.ProcessId
                integrity_sid = $medium.IntegritySid
                produced_capture = ($null -ne $mediumJson)
                root_resolved = ($null -ne $mediumJson -and $mediumJson.measurements.root.resolved)
                token_read = ($null -ne $mediumJson -and $mediumJson.measurements.process_token.'creation_time_seconds' -ne $null)
            }
        }
    } catch {
        return [ordered]@{
            window_handle_int = $WindowHandle.ToInt64()
            high_integrity_read = @{
                exit_code = $highCode
                root_resolved = ($null -ne $highJson -and $highJson.measurements.root.resolved)
            }
            medium_integrity_read = @{ skipped = $_.Exception.Message }
        }
    }
}

$script:foundTop = [IntPtr]::Zero

function Invoke-MinimizeCoveringWindows {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
namespace AgentDesktopObsMin {
    public static class Native {
        [DllImport("user32.dll")]
        public static extern bool EnumWindows(EnumWindowsProc cb, IntPtr lParam);
        public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
        [DllImport("user32.dll")]
        public static extern bool ShowWindow(IntPtr hWnd, int cmd);
        [DllImport("user32.dll", SetLastError = true)]
        public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
        [DllImport("user32.dll")]
        public static extern bool IsWindowVisible(IntPtr hWnd);
        [DllImport("user32.dll")]
        public static extern bool IsIconic(IntPtr hWnd);
    }
}
'@ | Out-Null
    $obsidianPids = @(Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
    # Script-scoped, not local: the EnumWindows callback below runs as a
    # marshaled delegate invocation, not a plain closure call, so a local
    # variable increment here does not persist back to this function's own
    # scope. The previous local $scriptcoveringCount name looked scoped but
    # was not, which is why the returned count always read 0.
    $script:coveringCount = 0
    [AgentDesktopObsMin.Native]::EnumWindows({
        param($hWnd, $lParam)
        $winPid = 0
        [void][AgentDesktopObsMin.Native]::GetWindowThreadProcessId($hWnd, [ref]$winPid)
        $visible = [AgentDesktopObsMin.Native]::IsWindowVisible($hWnd)
        $iconic = [AgentDesktopObsMin.Native]::IsIconic($hWnd)
        if ($visible -and (-not $iconic) -and (-not ($obsidianPids -contains [int]$winPid))) {
            [void][AgentDesktopObsMin.Native]::ShowWindow($hWnd, 6)
            $script:coveringCount++
        }
        return $true
    }, [IntPtr]::Zero) | Out-Null
    return "minimized $script:coveringCount covering top-level window(s)"
}

function Wait-ChromiumTopLevelWindow {
    $processIds = @(Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
    if ($processIds.Count -eq 0) { return [IntPtr]::Zero }
    $deadline = (Get-Date).AddSeconds(40)
    while ((Get-Date) -lt $deadline) {
        $main = @(Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue |
            Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero } |
            Select-Object -First 1)
        if ($main.Count -gt 0) {
            $ps = Get-Process -Id $main[0].Id
            $ps.Refresh()
            return $main[0].MainWindowHandle
        }
        Start-Sleep -Milliseconds 500
    }
    return [IntPtr]::Zero
}

$script:ownPath = $null
$script:wpfPath = $null
$script:chromiumPath = $null
$script:chromiumSettledPath = $null
$script:splitPath = $null

try {
    $censusScript = Join-Path $script:ProbeDir 'census.ps1'
    if (Test-Path -LiteralPath $censusScript) {
        & powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $censusScript -Label $Label 2>&1 | Write-Verbose
        $censusExit = $LASTEXITCODE
        $censusCapture = Join-Path $script:CaptureDir "observation-census-$Label.json"
        # Presence alone would let a stale capture from an earlier run stand in
        # for a census that failed this time, so the exit code has to agree.
        if ($censusExit -eq 0 -and (Test-Path -LiteralPath $censusCapture)) {
            Register-MandatoryPass -Capture $censusCapture -Result (Get-Content -LiteralPath $censusCapture -Raw | ConvertFrom-Json)
        } else {
            Write-Host ("the census script produced no measurement (exit code $censusExit)")
        }
    }

    $built = Build-ProbeBinary
    if ($built.skipped) {
        Write-Host ("probe binary skipped: " + $built.skipped)
        if ($Label -eq 'ci') {
            # A missing prerequisite (no cargo) is a legitimate dev-box skip,
            # but under -Label ci the workflow installs the toolchain, so any
            # skip here means no mandatory pass ran at all. Either form must
            # fail the workflow rather than exit 0 through the skip path below:
            # a green run that silently produced no capture is worse than a red
            # one, because CI's artifact upload has no other signal that the
            # observation corpus never got built.
            $reason = if ($built.buildFailed) { 'probe build failed on CI' } else { 'the probe binary was unavailable on CI, so no mandatory pass ran' }
            Write-ProbeResult -Probe '16-observation' -Status 'fail' -Message $reason -Data $built
            exit 1
        }
        Write-ProbeResult -Probe '16-observation' -Status 'skip' -Message 'probe build unavailable' -Data $built
        exit 0
    }
    $exe = $built.exe

    # --- pass 1: the probe's own Win32 fixture ---------------------------
    $own = Invoke-ProbePass -Exe $exe
    $script:ownPath = Write-ObservationCapture -Name "observation-win32-$Label.json" -Content (ConvertTo-Json -InputObject $own -Depth 20)
    Register-MandatoryPass -Capture $script:ownPath -Result $own
    Write-Host "wrote $script:ownPath"

    # --- pass 2: the WPF scratch window (multicolour provider stack) -------
    $wpfScript = Join-Path (Get-ProbeRoot) 'scratch\ScratchWpf.ps1'
    $wpfResult = $null
    if (Test-Path -LiteralPath $wpfScript) {
        $wpfProcess = Start-Process -FilePath 'powershell.exe' `
            -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $wpfScript, '-Tag', 'obs', '-TimeoutSeconds', '120') `
            -PassThru -WindowStyle Hidden
        [void]$script:Spawned.Add($wpfProcess.Id)
        try {
            $handle = [IntPtr]::Zero
            for ($attempt = 0; $attempt -lt 40; $attempt++) {
                Start-Sleep -Milliseconds 500
                $wpfProcess.Refresh()
                if ($wpfProcess.HasExited) { break }
                if ($wpfProcess.MainWindowHandle -ne [IntPtr]::Zero) { $handle = $wpfProcess.MainWindowHandle; break }
            }
            if ($handle -ne [IntPtr]::Zero) {
                Start-Sleep -Seconds 2
                $wpfResult = Invoke-ProbePass -Exe $exe -Arguments @('--attach', $handle.ToString())
                $script:wpfPath = Write-ObservationCapture -Name "observation-wpf-$Label.json" -Content (ConvertTo-Json -InputObject $wpfResult -Depth 20)
                Register-MandatoryPass -Capture $script:wpfPath -Result $wpfResult
                Write-Host "wrote $script:wpfPath"

                $splitReport = Invoke-SplitIntegrityRead -Exe $exe -WindowHandle $handle
                $script:splitPath = Write-ObservationCapture -Name "observation-split-integrity-$Label.json" -Content (ConvertTo-Json -InputObject $splitReport -Depth 10)
                Register-MandatoryPass -Capture $script:splitPath -Result $splitReport
                Write-Host "wrote $script:splitPath"
            } else {
                Write-Host 'WPF scratch window never reported a handle; WPF and split-integrity passes skipped'
            }
        } finally {
            if (-not $wpfProcess.HasExited) {
                Stop-Process -Id $wpfProcess.Id -Force -ErrorAction SilentlyContinue
            }
        }
    }

    # --- pass 3: the Chromium/Electron target (Obsidian), dev box only -----
    $chromium = $null
    if (-not $SkipChromium -and $Label -eq 'devbox') {
        $obsidianExe = Join-Path $env:LOCALAPPDATA 'Programs\Obsidian\Obsidian.exe'
        if (Test-Path -LiteralPath $obsidianExe) {
            $preexisting = @(Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue).Count -gt 0
            if (-not $preexisting) {
                Write-Host (Invoke-MinimizeCoveringWindows)
                $obsProc = Start-Process -FilePath $obsidianExe -PassThru
                [void]$script:Spawned.Add($obsProc.Id)
            } else {
                Write-Host 'an Obsidian instance the probe did not launch was already running; Chromium pass reads it read-only'
            }
            $chromeHandle = Wait-ChromiumTopLevelWindow
            if ($chromeHandle -ne [IntPtr]::Zero) {
                $firstContact = Invoke-ProbePass -Exe $exe -Arguments @('--attach', $chromeHandle.ToString())
                $script:chromiumPath = Write-ObservationCapture -Name "observation-chromium-first-contact-$Label.json" -Content (ConvertTo-Json -InputObject $firstContact -Depth 20)
                Write-Host "wrote $script:chromiumPath"

                Start-Sleep -Seconds 20
                $settled = Invoke-ProbePass -Exe $exe -Arguments @('--attach', $chromeHandle.ToString())
                $script:chromiumSettledPath = Write-ObservationCapture -Name "observation-chromium-$Label.json" -Content (ConvertTo-Json -InputObject $settled -Depth 20)
                Write-Host "wrote $script:chromiumSettledPath"
            } else {
                Write-Host 'no Obsidian-owned Chromium top-level window appeared; Chromium pass skipped'
            }
        } else {
            Write-Host 'Obsidian is not installed on this box; Chromium pass skipped'
        }
    }
} finally {
    foreach ($id in @($script:Spawned)) {
        $proc = Get-Process -Id $id -ErrorAction SilentlyContinue
        if ($proc) { try { Stop-ScratchProcess -ProcessId $id } catch { } }
    }
    if ($null -ne $built -and $built.work -and (Test-Path -LiteralPath $built.work)) {
        Remove-Item -LiteralPath $built.work -Recurse -Force -ErrorAction SilentlyContinue
    }
}

Assert-MandatoryMeasurement -Probe '16-observation' -Label $Label

Write-ProbeResult -Probe '16-observation' -Status 'ok' -Message 'observation probes captured' -Data @{
    win32 = if ($script:ownPath) { Split-Path -Leaf $script:ownPath } else { '<none>' }
    wpf = if ($script:wpfPath) { Split-Path -Leaf $script:wpfPath } else { '<none>' }
    chromium_first_contact = if ($script:chromiumPath) { Split-Path -Leaf $script:chromiumPath } else { '<none>' }
    chromium_settled = if ($script:chromiumSettledPath) { Split-Path -Leaf $script:chromiumSettledPath } else { '<none>' }
    split_integrity = if ($script:splitPath) { Split-Path -Leaf $script:splitPath } else { '<none>' }
}
exit 0
