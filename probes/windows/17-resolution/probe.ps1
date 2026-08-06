#Requires -Version 5.1
<#
.SYNOPSIS
    Sub-phase 2.5 resolution probe (A17).

.DESCRIPTION
    Orchestrates the resolution measurements the 2.5 plan refuses to infer. On
    the dev box it runs a pass against the probe's own Win32 fixture (hosted in
    a child process), against the WPF scratch window, against the WinForms
    scratch fixture (the A7-3 ListBox swap arm), and against the Chromium/Electron
    target (Obsidian) when present. The Electron path/geometry survival leg
    (U1-8) stages a repo-controlled scratch Obsidian vault, measures the marker
    notes' paths and bounds across an in-page mutation and an app relaunch, and
    compares the captures.

    Every capture is written beside this script under captures\, BOM-less UTF-8,
    through the corpus redaction gate in ..\common.ps1.

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
    green; the operator can see it.
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
    "resolution-own-$Label.json",
    "resolution-wpf-$Label.json",
    "resolution-winforms-$Label.json",
    "resolution-swap-$Label.json"
)

function Write-ResolutionCapture {
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
    $result = [ordered]@{ skipped = $null; buildFailed = $false; work = $null; exe = $null }
    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
    if (-not $cargo) {
        $result.skipped = 'cargo is not installed on this machine'
        return $result
    }
    $work = Join-Path ([IO.Path]::GetTempPath()) ('agent-desktop-resolution-' + [guid]::NewGuid())
    New-Item -ItemType Directory -Path (Join-Path $work 'src') -Force | Out-Null
    try {
        $manifest = '  [package]' + "`n" + '  name = "agent-desktop-resolution-probe"' + "`n" + '  version = "0.0.0"' + "`n" + '  edition = "2021"' + "`n" + "`n" + '  [dependencies]' + "`n" + '  serde_json = "1"' + "`n" + '  uiautomation = "=' + $UiAutomationVersion + '"' + "`n" + '  windows-sys = { version = "0.61", features = [' + "`n" + '    "Win32_Foundation",' + "`n" + '    "Win32_Graphics_Gdi",' + "`n" + '    "Win32_System_Com",' + "`n" + '    "Win32_System_LibraryLoader",' + "`n" + '    "Win32_UI_WindowsAndMessaging",' + "`n" + '  ] }' + "`n" + "`n" + '  [workspace]'
        $utf8NoBom = New-Object System.Text.UTF8Encoding $false
        [IO.File]::WriteAllText((Join-Path $work 'Cargo.toml'), $manifest, $utf8NoBom)
        foreach ($file in (Get-ChildItem -LiteralPath $PSScriptRoot -Filter 'probe*.rs' | Select-Object -ExpandProperty Name)) {
            Copy-Item -LiteralPath (Join-Path $script:ProbeDir $file) -Destination (Join-Path $work "src\$file") -Force
        }
        Copy-Item -LiteralPath (Join-Path $work 'src\probe.rs') -Destination (Join-Path $work 'src\main.rs') -Force
        $env:PROBE_UIAUTOMATION_VERSION = $UiAutomationVersion
        Push-Location $work
        try {
            & cargo build --quiet 2>&1 | Write-Verbose
            if ($LASTEXITCODE -ne 0) {
                $result.skipped = "cargo build failed with exit code $LASTEXITCODE"
                $result.buildFailed = $true
                return $result
            }
            $result.work = $work
            $result.exe = Join-Path $work 'target\debug\agent-desktop-resolution-probe.exe'
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

function Wait-ProbeWindow {
    param(
        [Parameter(Mandatory = $true)]$Process,
        [int]$TimeoutSec = 30
    )
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ((Get-Date) -lt $deadline) {
        $Process.Refresh()
        if ($Process.HasExited) { return [IntPtr]::Zero }
        if ($Process.MainWindowHandle -ne [IntPtr]::Zero) { return $Process.MainWindowHandle }
        Start-Sleep -Milliseconds 300
    }
    return [IntPtr]::Zero
}

if (-not ('AgentDesktopResMin.Native' -as [type])) {
    Add-Type -TypeDefinition 'using System;
using System.Runtime.InteropServices;
namespace AgentDesktopResMin {
    public static class Native {
        [DllImport("user32.dll")]
        public static extern bool ShowWindowAsync(System.IntPtr hWnd, int nCmdShow);
    }
}'
}

function Invoke-MinimizeCoveringWindows {
    $obsidianPids = @(Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
    $covered = 0
    foreach ($process in Get-Process -ErrorAction SilentlyContinue) {
        $title = ''
        $handle = [IntPtr]::Zero
        $procPid = 0
        try {
            $title = $process.MainWindowTitle
            $handle = $process.MainWindowHandle
            $procPid = $process.Id
        } catch { continue }
        if ($title -eq '' -or $handle -eq [IntPtr]::Zero) { continue }
        if ($obsidianPids -contains [int]$procPid) { continue }
        [void][AgentDesktopResMin.Native]::ShowWindowAsync($handle, 6)
        $covered++
    }
    return "minimized $covered covering top-level window(s)"
}

function Wait-ChromiumTopLevelWindow {
    $processIds = @(Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
    if ($processIds.Count -eq 0) { return [IntPtr]::Zero }
    $deadline = (Get-Date).AddSeconds(45)
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

function Get-MarkerMap {
    param([Parameter(Mandatory = $true)]$Doc)
    $map = @{}
    if ($null -eq $Doc.measurements.marker_rows) { return $map }
    foreach ($row in @($Doc.measurements.marker_rows)) {
        if ($row.marker -and $row.name_digest) { $map[$row.name_digest] = $row }
    }
    return $map
}

function Invoke-ElectronSurvivalLeg {
    param(
        [Parameter(Mandatory = $true)][string]$Exe,
        [Parameter(Mandatory = $true)][IntPtr]$Handle,
        [bool]$CleanSlate
    )
    if (-not $CleanSlate) {
        $still = @(Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue).Count -gt 0
        if ($still) {
            return [ordered]@{
                skipped = 'a pre-existing Obsidian instance is running; its vault is not repo-controlled'
                leg = 'path/geometry survival across relaunch and in-page mutation'
            }
        }
    }
    Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue | ForEach-Object {
        Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
    }
    Start-Sleep -Seconds 2
    $vault = Join-Path ([IO.Path]::GetTempPath()) ('a17-obsidian-vault-' + [guid]::NewGuid())
    New-Item -ItemType Directory -Path $vault -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $vault '.obsidian') -Force | Out-Null
    $utf8NoBom = New-Object System.Text.UTF8Encoding $false
    [IO.File]::WriteAllText((Join-Path $vault '.obsidian\app.json'), '{"newFileLocation":"root","alwaysUpdateLinks":false}', $utf8NoBom)
    $markerNames = @('probe-note-alpha', 'probe-note-bravo', 'probe-note-charlie', 'probe-note-delta', 'probe-note-echo')
    foreach ($marker in $markerNames) {
        [IO.File]::WriteAllText((Join-Path $vault ($marker + '.md')), ('# ' + $marker) + "`r`n`r`n" + 'probe-controlled synthetic note body', $utf8NoBom)
    }
    $obsidianExe = Join-Path $env:LOCALAPPDATA 'Programs\Obsidian\Obsidian.exe'
    try {
        $obs = Start-Process -FilePath $obsidianExe -ArgumentList @($vault) -PassThru
        [void]$script:Spawned.Add($obs.Id)
        $waited = Wait-ChromiumTopLevelWindow
        if ($waited -eq [IntPtr]::Zero) {
            return [ordered]@{ skipped = 'Obsidian never presented a top-level window for the scratch vault' }
        }
        Start-Sleep -Seconds 20
        $phaseA = Invoke-ProbePass -Exe $Exe -Arguments @('--survival-arm', $waited.ToString())
        if ($LASTEXITCODE -ne 0) { $phaseA = [ordered]@{ probe_failed = $true } }

        [IO.File]::WriteAllText((Join-Path $vault 'probe-note-foxtrot.md'), "# probe-note-foxtrot`r`n", $utf8NoBom)
        Start-Sleep -Seconds 12
        $phaseB = Invoke-ProbePass -Exe $Exe -Arguments @('--survival-arm', $waited.ToString())
        if ($LASTEXITCODE -ne 0) { $phaseB = [ordered]@{ probe_failed = $true } }

        Stop-Process -Id $obs.Id -Force -ErrorAction SilentlyContinue
        Start-Sleep -Seconds 3
        $obs2 = Start-Process -FilePath $obsidianExe -ArgumentList @($vault) -PassThru
        [void]$script:Spawned.Add($obs2.Id)
        $waited2 = Wait-ChromiumTopLevelWindow
        Start-Sleep -Seconds 20
        $phaseC = $null
        if ($waited2 -ne [IntPtr]::Zero) {
            $phaseC = Invoke-ProbePass -Exe $Exe -Arguments @('--survival-arm', $waited2.ToString())
            if ($LASTEXITCODE -ne 0) { $phaseC = [ordered]@{ probe_failed = $true } }
        }

        $mapA = Get-MarkerMap -Doc $phaseA
        $mapB = Get-MarkerMap -Doc $phaseB
        $mapC = Get-MarkerMap -Doc $phaseC
        $digests = @($mapA.Keys | Sort-Object)
        $rows = New-Object System.Collections.ArrayList
        foreach ($digest in $digests) {
            $a = $mapA[$digest]
            $b = $mapB[$digest]
            $c = $mapC[$digest]
            $bPathSame = ($null -ne $b) -and ($b.path -join ',' -eq $a.path -join ',')
            $cPathSame = ($null -ne $c) -and ($c.path -join ',' -eq $a.path -join ',')
            $bBoundsSame = ($null -ne $b) -and ($b.bounds -join ',' -eq $a.bounds -join ',')
            $cBoundsSame = ($null -ne $c) -and ($c.bounds -join ',' -eq $a.bounds -join ',')
            [void]$rows.Add([ordered]@{
                name_digest = $digest
                name_length = $a.name_length
                in_after_mutation = ($null -ne $b)
                path_survived_mutation = $bPathSame
                bounds_same_after_mutation = $bBoundsSame
                in_after_relaunch = ($null -ne $c)
                path_survived_relaunch = $cPathSame
                bounds_same_after_relaunch = $cBoundsSame
            })
        }
        return [ordered]@{
            vault_marker_notes = $markerNames.Count
            phase_markers = @{
                after_mutation = ($mapB.Count)
                after_relaunch = ($mapC.Count)
            }
            marker_rows = $rows
            report_note = 'marker paths paired by content-free name digest, never by the note name itself; a path change under mutation or relaunch is the churn the geometry tier must survive'
        }
    } finally {
        Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue | ForEach-Object { Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue }
        if (Test-Path -LiteralPath $vault) {
            Remove-Item -LiteralPath $vault -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
}

$script:ownPath = $null
$script:wpfPath = $null
$script:swapPath = $null
$script:winformsPath = $null
$script:chromiumPath = $null
$script:survivalPath = $null

try {
    $built = Build-ProbeBinary
    if ($built.skipped) {
        Write-Host ("probe binary skipped: " + $built.skipped)
        if ($Label -eq 'ci') {
            # A missing prerequisite is a legitimate dev-box skip, but on CI the
            # workflow installs the toolchain, so either form of skip means no
            # mandatory pass ran at all - the same failure to measure the gate
            # below catches, reached before the gate exists.
            $reason = if ($built.buildFailed) { 'probe build failed on CI' } else { 'the probe binary was unavailable on CI, so no mandatory pass ran' }
            Write-ProbeResult -Probe '17-resolution' -Status 'fail' -Message $reason -Data $built
            exit 1
        }
        Write-ProbeResult -Probe '17-resolution' -Status 'skip' -Message 'probe build unavailable' -Data $built
        exit 0
    }
    $exe = $built.exe

    # --- pass 1: the probe's own Win32 fixture ---------------------------
    $own = Invoke-ProbePass -Exe $exe
    $script:ownPath = Write-ResolutionCapture -Name "resolution-own-$Label.json" -Content (ConvertTo-Json -InputObject $own -Depth 20)
    Register-MandatoryPass -Capture $script:ownPath -Result $own
    Write-Host "wrote $script:ownPath"

    # --- pass 2: the WPF scratch window ----------------------------------
    $wpfScript = Join-Path (Get-ProbeRoot) 'scratch\ScratchWpf.ps1'
    if (Test-Path -LiteralPath $wpfScript) {
        $wpfProcess = Start-Process -FilePath 'powershell.exe' `
            -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $wpfScript, '-Tag', 'obs', '-TimeoutSeconds', '120') `
            -PassThru -WindowStyle Hidden
        [void]$script:Spawned.Add($wpfProcess.Id)
        try {
            $handle = Wait-ProbeWindow -Process $wpfProcess
            if ($handle -ne [IntPtr]::Zero) {
                Start-Sleep -Seconds 2
                $wpf = Invoke-ProbePass -Exe $exe -Arguments @('--attach', $handle.ToString())
                $script:wpfPath = Write-ResolutionCapture -Name "resolution-wpf-$Label.json" -Content (ConvertTo-Json -InputObject $wpf -Depth 20)
                Register-MandatoryPass -Capture $script:wpfPath -Result $wpf
                Write-Host "wrote $script:wpfPath"
            } else {
                Write-Host 'WPF scratch window never reported a handle; WPF pass skipped'
            }
        } finally {
            if (-not $wpfProcess.HasExited) {
                Stop-Process -Id $wpfProcess.Id -Force -ErrorAction SilentlyContinue
            }
        }
    }

    # --- pass 3: the WinForms scratch fixture (A7-3 swap arm + census) -----
    $buildScratch = Join-Path (Get-ProbeRoot) 'scratch\build-scratch.ps1'
    if (Test-Path -LiteralPath $buildScratch) {
        try {
            & $buildScratch | Out-Null
            $scratchExe = Join-Path (Get-ProbeRoot) 'scratch\bin\ScratchForms.exe'
            if (Test-Path -LiteralPath $scratchExe) {
                $winforms = Start-Process -FilePath $scratchExe -ArgumentList @('--tag', 'resolution') -PassThru
                [void]$script:Spawned.Add($winforms.Id)
                $handle = Wait-ProbeWindow -Process $winforms
                if ($handle -ne [IntPtr]::Zero) {
                    Start-Sleep -Seconds 2
                    $winformsPass = Invoke-ProbePass -Exe $exe -Arguments @('--attach', $handle.ToString())
                    $script:winformsPath = Write-ResolutionCapture -Name "resolution-winforms-$Label.json" -Content (ConvertTo-Json -InputObject $winformsPass -Depth 20)
                    Register-MandatoryPass -Capture $script:winformsPath -Result $winformsPass
                    Write-Host "wrote $script:winformsPath"

                    $swap = Invoke-ProbePass -Exe $exe -Arguments @('--swap-arm', $handle.ToString())
                    $script:swapPath = Write-ResolutionCapture -Name "resolution-swap-$Label.json" -Content (ConvertTo-Json -InputObject $swap -Depth 20)
                    Register-MandatoryPass -Capture $script:swapPath -Result $swap
                    Write-Host "wrote $script:swapPath"
                } else {
                    Write-Host 'ScratchForms never reported a handle; WinForms passes skipped'
                }
            }
        } catch {
            Write-Host ('ScratchForms passes skipped: ' + $_.Exception.Message)
        }
    }

    # --- pass 4: the Chromium/Electron target (Obsidian), dev box only -----
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
                Start-Sleep -Seconds 20
                $chromium = Invoke-ProbePass -Exe $exe -Arguments @('--attach', $chromeHandle.ToString())
                $script:chromiumPath = Write-ResolutionCapture -Name "resolution-chromium-$Label.json" -Content (ConvertTo-Json -InputObject $chromium -Depth 20)
                Write-Host "wrote $script:chromiumPath"

                $survival = Invoke-ElectronSurvivalLeg -Exe $exe -Handle $chromeHandle -CleanSlate (-not $preexisting)
                $script:survivalPath = Write-ResolutionCapture -Name "resolution-electron-survival-$Label.json" -Content (ConvertTo-Json -InputObject $survival -Depth 20)
                Write-Host "wrote $script:survivalPath"
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

Assert-MandatoryMeasurement -Probe '17-resolution' -Label $Label

Write-ProbeResult -Probe '17-resolution' -Status 'ok' -Message 'resolution probes captured' -Data @{
    own = if ($script:ownPath) { Split-Path -Leaf $script:ownPath } else { '<none>' }
    wpf = if ($script:wpfPath) { Split-Path -Leaf $script:wpfPath } else { '<none>' }
    swap = if ($script:swapPath) { Split-Path -Leaf $script:swapPath } else { '<none>' }
    winforms = if ($script:winformsPath) { Split-Path -Leaf $script:winformsPath } else { '<none>' }
    chromium = if ($script:chromiumPath) { Split-Path -Leaf $script:chromiumPath } else { '<none>' }
    electron_survival = if ($script:survivalPath) { Split-Path -Leaf $script:survivalPath } else { '<none>' }
}
exit 0
