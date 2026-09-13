#Requires -Version 5.1
<#
.SYNOPSIS
    Sub-phase 2.6 actionability probe (A18).

.DESCRIPTION
    Orchestrates the actionability measurements U1 refuses to infer: ScrollIntoView,
    ElementFromPoint corroboration (including elevated and same-root overlays), hang
    defense, Unknown triggers, cost, envelope staging, Chromium five-point hits, and
    the DPI by-construction branch. Every UIA call rides the product's bounded
    CUIAutomation8 client via agent-desktop-windows::tree::automation::automation_client.

    Captures under captures\ as actionability-*-{devbox,ci}.json, redacted through
    common.ps1. Under -Label ci the mandatory captures must all exist and hold
    measurements.
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
    "actionability-scroll-$Label.json",
    "actionability-corroborate-$Label.json",
    "actionability-hang-$Label.json",
    "actionability-unknown-$Label.json",
    "actionability-cost-$Label.json",
    "actionability-envelope-$Label.json",
    "actionability-dpi-$Label.json"
)

function Write-ActionabilityCapture {
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
    $work = Join-Path ([IO.Path]::GetTempPath()) ('agent-desktop-actionability-' + [guid]::NewGuid())
    New-Item -ItemType Directory -Path (Join-Path $work 'src') -Force | Out-Null
    try {
        $repoRoot = (Resolve-Path -LiteralPath (Join-Path (Get-ProbeRoot) '..\..')).ProviderPath.Replace('\', '/')
        $manifest = @(
            '[package]'
            'name = "agent-desktop-actionability-probe"'
            'version = "0.0.0"'
            'edition = "2021"'
            ''
            '[dependencies]'
            'serde_json = "1"'
            ('uiautomation = "=' + $UiAutomationVersion + '"')
            ('agent-desktop-core = { path = "' + $repoRoot + '/crates/core" }')
            ('agent-desktop-windows = { path = "' + $repoRoot + '/crates/windows" }')
            'windows-sys = { version = "0.61", features = ['
            '  "Win32_Foundation",'
            '  "Win32_Graphics_Gdi",'
            '  "Win32_System_Com",'
            '  "Win32_System_LibraryLoader",'
            '  "Win32_UI_WindowsAndMessaging",'
            '] }'
            ''
            '[workspace]'
        ) -join "`n"
        $utf8NoBom = New-Object System.Text.UTF8Encoding $false
        [IO.File]::WriteAllText((Join-Path $work 'Cargo.toml'), $manifest, $utf8NoBom)
        foreach ($file in (Get-ChildItem -LiteralPath $PSScriptRoot -Filter 'probe*.rs' | Select-Object -ExpandProperty Name)) {
            Copy-Item -LiteralPath (Join-Path $script:ProbeDir $file) -Destination (Join-Path $work "src\$file") -Force
        }
        Copy-Item -LiteralPath (Join-Path $work 'src\probe.rs') -Destination (Join-Path $work 'src\main.rs') -Force
        $env:PROBE_UIAUTOMATION_VERSION = $UiAutomationVersion
        Push-Location $work
        $previousTargetDir = $env:CARGO_TARGET_DIR
        $previousEap = $ErrorActionPreference
        try {
            # Cursor's shell injects CARGO_TARGET_DIR into a sandbox cache; force
            # the probe binary next to this temp crate so the orchestrator can
            # find it at the path it records.
            Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
            # cargo writes progress to stderr; with Stop that becomes a
            # terminating error before the binary exists.
            $ErrorActionPreference = 'Continue'
            $buildOutput = & cargo build 2>&1 | ForEach-Object { "$_" } | Out-String
            $buildExit = $LASTEXITCODE
            $ErrorActionPreference = $previousEap
            if ($buildExit -ne 0) {
                $result.skipped = "cargo build failed with exit code $buildExit"
                $result.buildFailed = $true
                $result.buildOutput = $buildOutput
                return $result
            }
            $result.work = $work
            $result.exe = Join-Path $work 'target\debug\agent-desktop-actionability-probe.exe'
            if (-not (Test-Path -LiteralPath $result.exe)) {
                $result.skipped = 'cargo build reported success but the probe binary is missing'
                $result.buildFailed = $true
                $result.buildOutput = $buildOutput
                return $result
            }
            return $result
        } finally {
            $ErrorActionPreference = $previousEap
            if ($null -ne $previousTargetDir -and $previousTargetDir -ne '') {
                $env:CARGO_TARGET_DIR = $previousTargetDir
            }
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
    try {
        return ($raw | ConvertFrom-Json)
    } catch {
        return (New-NotMeasuredResult -Reason ('JSON parse failed: ' + $_.Exception.Message))
    }
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

function Start-HostOccluder {
    param(
        [Parameter(Mandatory = $true)][string]$Exe,
        [int]$X = 140,
        [int]$Y = 140
    )
    $outFile = Join-Path ([IO.Path]::GetTempPath()) ('a18-host-' + [guid]::NewGuid() + '.txt')
    $proc = Start-Process -FilePath $Exe `
        -ArgumentList @('--host', '--x', "$X", '--y', "$Y") `
        -PassThru -WindowStyle Hidden `
        -RedirectStandardOutput $outFile `
        -RedirectStandardError (Join-Path ([IO.Path]::GetTempPath()) ('a18-host-err-' + [guid]::NewGuid() + '.txt'))
    [void]$script:Spawned.Add($proc.Id)
    $deadline = (Get-Date).AddSeconds(20)
    $hwnd = [IntPtr]::Zero
    while ((Get-Date) -lt $deadline) {
        if (Test-Path -LiteralPath $outFile) {
            $text = Get-Content -LiteralPath $outFile -ErrorAction SilentlyContinue
            foreach ($line in @($text)) {
                if ($line -match '^HWND=(-?\d+)$') {
                    $hwnd = [IntPtr]::new([int64]$Matches[1])
                    break
                }
            }
        }
        if ($hwnd -ne [IntPtr]::Zero) { break }
        if ($proc.HasExited) { break }
        Start-Sleep -Milliseconds 150
    }
    return [pscustomobject]@{ Process = $proc; Handle = $hwnd; OutFile = $outFile }
}

function Wait-ChromiumTopLevelWindow {
    $deadline = (Get-Date).AddSeconds(45)
    while ((Get-Date) -lt $deadline) {
        $main = @(Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue |
            Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero } |
            Select-Object -First 1)
        if ($main.Count -gt 0) {
            return $main[0].MainWindowHandle
        }
        Start-Sleep -Milliseconds 500
    }
    return [IntPtr]::Zero
}

$script:paths = @{}
$built = $null

try {
    $built = Build-ProbeBinary
    if ($built.skipped) {
        Write-Host ("probe binary skipped: " + $built.skipped)
        if ($Label -eq 'ci') {
            $reason = if ($built.buildFailed) { 'probe build failed on CI' } else { 'the probe binary was unavailable on CI, so no mandatory pass ran' }
            Write-ProbeResult -Probe '18-actionability' -Status 'fail' -Message $reason -Data $built
            exit 1
        }
        Write-ProbeResult -Probe '18-actionability' -Status 'skip' -Message 'probe build unavailable' -Data $built
        exit 0
    }
    $exe = $built.exe

    # --- DPI by-construction (no live measurement) -------------------------
    $dpi = Invoke-ProbePass -Exe $exe -Arguments @('--dpi')
    $script:paths.dpi = Write-ActionabilityCapture -Name "actionability-dpi-$Label.json" -Content (ConvertTo-Json -InputObject $dpi -Depth 12)
    Register-MandatoryPass -Capture $script:paths.dpi -Result $dpi
    Write-Host "wrote $($script:paths.dpi)"

    # --- hang + unknown (no external fixtures) -----------------------------
    $hang = Invoke-ProbePass -Exe $exe -Arguments @('--hang')
    $script:paths.hang = Write-ActionabilityCapture -Name "actionability-hang-$Label.json" -Content (ConvertTo-Json -InputObject $hang -Depth 12)
    Register-MandatoryPass -Capture $script:paths.hang -Result $hang
    Write-Host "wrote $($script:paths.hang)"

    $unknown = Invoke-ProbePass -Exe $exe -Arguments @('--unknown')
    $script:paths.unknown = Write-ActionabilityCapture -Name "actionability-unknown-$Label.json" -Content (ConvertTo-Json -InputObject $unknown -Depth 12)
    Register-MandatoryPass -Capture $script:paths.unknown -Result $unknown
    Write-Host "wrote $($script:paths.unknown)"

    # --- WPF + WinForms fixtures -------------------------------------------
    $wpfHandle = [IntPtr]::Zero
    $wpfProcess = $null
    $wpfScript = Join-Path (Get-ProbeRoot) 'scratch\ScratchWpf.ps1'
    if (Test-Path -LiteralPath $wpfScript) {
        $wpfProcess = Start-Process -FilePath 'powershell.exe' `
            -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $wpfScript, '-Tag', 'a18', '-Left', '40', '-Top', '40', '-TimeoutSeconds', '180') `
            -PassThru -WindowStyle Hidden
        [void]$script:Spawned.Add($wpfProcess.Id)
        $wpfHandle = Wait-ProbeWindow -Process $wpfProcess
        Start-Sleep -Seconds 2
        if ($wpfHandle -ne [IntPtr]::Zero) {
            # Z-order for probe-owned window is applied in the corroborate arm.
        }
    }

    $winformsHandle = [IntPtr]::Zero
    $winformsProcess = $null
    $buildScratch = Join-Path (Get-ProbeRoot) 'scratch\build-scratch.ps1'
    if (Test-Path -LiteralPath $buildScratch) {
        try {
            & $buildScratch | Out-Null
            $scratchExe = Join-Path (Get-ProbeRoot) 'scratch\bin\ScratchForms.exe'
            if (Test-Path -LiteralPath $scratchExe) {
                $winformsProcess = Start-Process -FilePath $scratchExe -ArgumentList @('--tag', 'a18', '--pos', '40,40') -PassThru
                [void]$script:Spawned.Add($winformsProcess.Id)
                $winformsHandle = Wait-ProbeWindow -Process $winformsProcess
                Start-Sleep -Seconds 2
            }
        } catch {
            Write-Host ('ScratchForms skipped: ' + $_.Exception.Message)
        }
    }

    # Foreign-process occluder host
    $foreign = Start-HostOccluder -Exe $exe -X 140 -Y 140
    $foreignHandle = $foreign.Handle

    if ($wpfHandle -ne [IntPtr]::Zero) {
        $scroll = Invoke-ProbePass -Exe $exe -Arguments @('--scroll', '--wpf', $wpfHandle.ToString())
        # killed-provider leg: re-launch WPF, capture element, kill
        $wpfKill = Start-Process -FilePath 'powershell.exe' `
            -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $wpfScript, '-Tag', 'a18kill', '-TimeoutSeconds', '60') `
            -PassThru -WindowStyle Hidden
        [void]$script:Spawned.Add($wpfKill.Id)
        $killHwnd = Wait-ProbeWindow -Process $wpfKill
        Start-Sleep -Seconds 2
        $killed = $null
        if ($killHwnd -ne [IntPtr]::Zero) {
            $killed = Invoke-ProbePass -Exe $exe -Arguments @(
                '--kill-scroll', $killHwnd.ToString(),
                '--kill-pid', $wpfKill.Id.ToString()
            )
        } else {
            $killed = New-NotMeasuredResult -Reason 'kill-leg WPF window never appeared'
        }
        $scrollMerged = [ordered]@{
            scroll = $scroll
            killed_provider = $killed
        }
        $script:paths.scroll = Write-ActionabilityCapture -Name "actionability-scroll-$Label.json" -Content (ConvertTo-Json -InputObject $scrollMerged -Depth 16)
        Register-MandatoryPass -Capture $script:paths.scroll -Result $scrollMerged
        Write-Host "wrote $($script:paths.scroll)"

        $envelopeArgs = @('--envelope', '--wpf', $wpfHandle.ToString())
        if ($winformsHandle -ne [IntPtr]::Zero) {
            $envelopeArgs += @('--winforms', $winformsHandle.ToString())
        }
        $envelope = Invoke-ProbePass -Exe $exe -Arguments $envelopeArgs
        $script:paths.envelope = Write-ActionabilityCapture -Name "actionability-envelope-$Label.json" -Content (ConvertTo-Json -InputObject $envelope -Depth 12)
        Register-MandatoryPass -Capture $script:paths.envelope -Result $envelope
        Write-Host "wrote $($script:paths.envelope)"
    } else {
        $placeholder = New-NotMeasuredResult -Reason 'WPF scratch window unavailable'
        $script:paths.scroll = Write-ActionabilityCapture -Name "actionability-scroll-$Label.json" -Content (ConvertTo-Json -InputObject $placeholder -Depth 6)
        Register-MandatoryPass -Capture $script:paths.scroll -Result $placeholder
        $script:paths.envelope = Write-ActionabilityCapture -Name "actionability-envelope-$Label.json" -Content (ConvertTo-Json -InputObject $placeholder -Depth 6)
        Register-MandatoryPass -Capture $script:paths.envelope -Result $placeholder
    }

    $corrArgs = @('--corroborate')
    if ($wpfHandle -ne [IntPtr]::Zero) { $corrArgs += @('--wpf', $wpfHandle.ToString()) }
    if ($winformsHandle -ne [IntPtr]::Zero) { $corrArgs += @('--winforms', $winformsHandle.ToString()) }
    if ($foreignHandle -ne [IntPtr]::Zero) { $corrArgs += @('--foreign', $foreignHandle.ToString()) }
    $corroborate = Invoke-ProbePass -Exe $exe -Arguments $corrArgs

    # Elevated High occluder over Medium target
    $elevated = $null
    try {
        $mediumScratch = Join-Path (Get-ProbeRoot) 'scratch\bin\ScratchForms.exe'
        if (-not (Test-Path -LiteralPath $mediumScratch)) {
            $elevated = New-NotMeasuredResult -Reason 'ScratchForms.exe missing for elevated leg'
        } else {
            $medium = Start-MediumIntegrityProcess -FilePath $mediumScratch -ArgumentList @('--tag', 'a18med', '--pos', '100,100')
            [void]$script:Spawned.Add($medium.ProcessId)
            $high = Start-HostOccluder -Exe $exe -X 110 -Y 110
            $highHwnd = $high.Handle
            if ($medium.MainWindowHandle -ne [IntPtr]::Zero -and $highHwnd -ne [IntPtr]::Zero) {
                $elevated = Invoke-ProbePass -Exe $exe -Arguments @(
                    '--elevated',
                    '--medium', $medium.MainWindowHandle.ToString(),
                    '--high', $highHwnd.ToString()
                )
            } else {
                $elevated = New-NotMeasuredResult -Reason 'elevated leg windows never appeared'
            }
        }
    } catch {
        $elevated = New-NotMeasuredResult -Reason ('elevated leg: ' + $_.Exception.Message)
    }

    $corrMerged = [ordered]@{
        corroborate = $corroborate
        elevated_occluder = $elevated
    }
    $script:paths.corroborate = Write-ActionabilityCapture -Name "actionability-corroborate-$Label.json" -Content (ConvertTo-Json -InputObject $corrMerged -Depth 16)
    Register-MandatoryPass -Capture $script:paths.corroborate -Result $corrMerged
    Write-Host "wrote $($script:paths.corroborate)"

    # Cost (own fixture from corroborate's in-process hosts is gone; use WPF + desktop)
    $costArgs = @('--cost')
    if ($wpfHandle -ne [IntPtr]::Zero) { $costArgs += @('--wpf', $wpfHandle.ToString()) }
    if ($foreignHandle -ne [IntPtr]::Zero) { $costArgs += @('--own', $foreignHandle.ToString()) }

    # Chromium (devbox only unless SkipChromium)
    $chromiumHandle = [IntPtr]::Zero
    $chromiumDoc = $null
    if (-not $SkipChromium -and $Label -eq 'devbox') {
        $obsidianExe = Join-Path $env:LOCALAPPDATA 'Programs\Obsidian\Obsidian.exe'
        if (Test-Path -LiteralPath $obsidianExe) {
            $preexisting = @(Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue).Count -gt 0
            if (-not $preexisting) {
                $obsProc = Start-Process -FilePath $obsidianExe -PassThru
                [void]$script:Spawned.Add($obsProc.Id)
            } else {
                Write-Host 'pre-existing Obsidian; Chromium pass reads it read-only'
            }
            $chromiumHandle = Wait-ChromiumTopLevelWindow
            if ($chromiumHandle -ne [IntPtr]::Zero) {
                Start-Sleep -Seconds 20
                # A16-11 fresh client: new probe process after settle
                $chromiumDoc = Invoke-ProbePass -Exe $exe -Arguments @('--chromium-arm', '--chromium', $chromiumHandle.ToString())
                $costArgs += @('--chromium', $chromiumHandle.ToString())
                $chromePath = Write-ActionabilityCapture -Name "actionability-chromium-$Label.json" -Content (ConvertTo-Json -InputObject $chromiumDoc -Depth 14)
                Write-Host "wrote $chromePath"
            } else {
                $skipChrome = New-NotMeasuredResult -Reason 'Obsidian never presented a top-level window'
                Write-ActionabilityCapture -Name "actionability-chromium-$Label.json" -Content (ConvertTo-Json -InputObject $skipChrome -Depth 6) | Out-Null
            }
        } else {
            $skipChrome = New-NotMeasuredResult -Reason 'Obsidian is not installed on this box'
            Write-ActionabilityCapture -Name "actionability-chromium-$Label.json" -Content (ConvertTo-Json -InputObject $skipChrome -Depth 6) | Out-Null
        }
    } elseif ($SkipChromium) {
        $skipChrome = New-NotMeasuredResult -Reason 'SkipChromium requested'
        Write-ActionabilityCapture -Name "actionability-chromium-$Label.json" -Content (ConvertTo-Json -InputObject $skipChrome -Depth 6) | Out-Null
    }

    $cost = Invoke-ProbePass -Exe $exe -Arguments $costArgs
    $script:paths.cost = Write-ActionabilityCapture -Name "actionability-cost-$Label.json" -Content (ConvertTo-Json -InputObject $cost -Depth 12)
    Register-MandatoryPass -Capture $script:paths.cost -Result $cost
    Write-Host "wrote $($script:paths.cost)"

} finally {
    foreach ($id in @($script:Spawned)) {
        $proc = Get-Process -Id $id -ErrorAction SilentlyContinue
        if ($proc) { try { Stop-ScratchProcess -ProcessId $id } catch { } }
    }
    if ($null -ne $built -and $built.work -and (Test-Path -LiteralPath $built.work)) {
        Remove-Item -LiteralPath $built.work -Recurse -Force -ErrorAction SilentlyContinue
    }
}

Assert-MandatoryMeasurement -Probe '18-actionability' -Label $Label

Write-ProbeResult -Probe '18-actionability' -Status 'ok' -Message 'actionability probes captured' -Data @{
    scroll = if ($script:paths.scroll) { Split-Path -Leaf $script:paths.scroll } else { '<none>' }
    corroborate = if ($script:paths.corroborate) { Split-Path -Leaf $script:paths.corroborate } else { '<none>' }
    hang = if ($script:paths.hang) { Split-Path -Leaf $script:paths.hang } else { '<none>' }
    unknown = if ($script:paths.unknown) { Split-Path -Leaf $script:paths.unknown } else { '<none>' }
    cost = if ($script:paths.cost) { Split-Path -Leaf $script:paths.cost } else { '<none>' }
    envelope = if ($script:paths.envelope) { Split-Path -Leaf $script:paths.envelope } else { '<none>' }
    dpi = if ($script:paths.dpi) { Split-Path -Leaf $script:paths.dpi } else { '<none>' }
}
exit 0
