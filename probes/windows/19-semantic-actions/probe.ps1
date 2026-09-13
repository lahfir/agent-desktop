#Requires -Version 5.1
<#
.SYNOPSIS
    Sub-phase 2.7 semantic-action write-surface probe (A19).

.DESCRIPTION
    Measures the UIA pattern write surface on the product's bounded CUIAutomation8
    client: semantic set, failure taxonomy, secure SetValue, UIPI Medium→High,
    SetFocus foreground effect, LegacyIAccessible.DoDefaultAction, combobox dance
    + nested scroll ladder geometry, and min-of-seven cost.

    Captures under captures\ as semantic-*-{devbox,ci}.json. Under -Label ci the
    mandatory captures must all exist and hold measurements. A dedicated marker
    (zza19secretzz) must never appear verbatim in any A19 capture.
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
$script:Spawned = New-Object System.Collections.ArrayList
$script:SecretMarker = 'zza19secretzz'

Register-MandatoryCapture -Name @(
    "semantic-set-$Label.json",
    "semantic-failure-$Label.json",
    "semantic-secure-$Label.json",
    "semantic-uipi-$Label.json",
    "semantic-focus-$Label.json",
    "semantic-legacy-$Label.json",
    "semantic-combo-$Label.json",
    "semantic-cost-$Label.json"
)

function Write-SemanticCapture {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Content
    )
    if ($Content -like ("*" + $script:SecretMarker + "*")) {
        throw ("secure marker present before redaction in " + $Name)
    }
    $redacted = Protect-ProbeText -Text $Content
    $path = Join-Path $script:CaptureDir $Name
    $utf8NoBom = New-Object System.Text.UTF8Encoding $false
    [IO.File]::WriteAllText($path, $redacted, $utf8NoBom)
    if (-not (Test-CaptureRedaction -Path $path)) {
        throw "redaction residue in $path"
    }
    $raw = [IO.File]::ReadAllText($path)
    if ($raw -like ("*" + $script:SecretMarker + "*")) {
        throw ("secure marker absence scan failed for " + $path)
    }
    return $path
}

function Assert-SecretMarkerAbsent {
    $hits = @()
    foreach ($file in Get-ChildItem -LiteralPath $script:CaptureDir -Filter ('*-' + $Label + '.json') -ErrorAction SilentlyContinue) {
        $text = [IO.File]::ReadAllText($file.FullName)
        if ($text -like ("*" + $script:SecretMarker + "*")) {
            $hits += $file.Name
        }
    }
    if ($hits.Count -gt 0) {
        throw ('A19 secret marker present in captures: ' + ($hits -join ', '))
    }
}

function Build-ProbeBinary {
    $result = [ordered]@{ skipped = $null; buildFailed = $false; work = $null; exe = $null }
    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
    if (-not $cargo) {
        $result.skipped = 'cargo is not installed on this machine'
        return $result
    }
    $work = Join-Path ([IO.Path]::GetTempPath()) ('agent-desktop-semantic-' + [guid]::NewGuid())
    New-Item -ItemType Directory -Path (Join-Path $work 'src') -Force | Out-Null
    try {
        $repoRoot = (Resolve-Path -LiteralPath (Join-Path (Get-ProbeRoot) '..\..')).ProviderPath.Replace('\', '/')
        $manifest = @(
            '[package]'
            'name = "agent-desktop-semantic-probe"'
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
            Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
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
            $result.exe = Join-Path $work 'target\debug\agent-desktop-semantic-probe.exe'
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
    if ($raw -like ("*" + $script:SecretMarker + "*")) {
        throw 'probe stdout contained the secure marker'
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

$script:paths = @{}
$built = $null

try {
    $built = Build-ProbeBinary
    if ($built.skipped) {
        Write-Host ("probe binary skipped: " + $built.skipped)
        if ($Label -eq 'ci') {
            $reason = if ($built.buildFailed) { 'probe build failed on CI' } else { 'the probe binary was unavailable on CI, so no mandatory pass ran' }
            Write-ProbeResult -Probe '19-semantic-actions' -Status 'fail' -Message $reason -Data $built
            exit 1
        }
        Write-ProbeResult -Probe '19-semantic-actions' -Status 'skip' -Message 'probe build unavailable' -Data $built
        exit 0
    }
    $exe = $built.exe

    $wpfHandle = [IntPtr]::Zero
    $wpfProcess = $null
    $wpfScript = Join-Path (Get-ProbeRoot) 'scratch\ScratchWpf.ps1'
    if (Test-Path -LiteralPath $wpfScript) {
        $wpfProcess = Start-Process -FilePath 'powershell.exe' `
            -ArgumentList @(
                '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $wpfScript,
                '-Tag', 'a19', '-Left', '40', '-Top', '40', '-TimeoutSeconds', '240',
                '-SecretMarker', $script:SecretMarker
            ) `
            -PassThru -WindowStyle Hidden
        [void]$script:Spawned.Add($wpfProcess.Id)
        $wpfHandle = Wait-ProbeWindow -Process $wpfProcess
        Start-Sleep -Seconds 2
    }

    $winformsHandle = [IntPtr]::Zero
    $winformsLegacyHandle = [IntPtr]::Zero
    $buildScratch = Join-Path (Get-ProbeRoot) 'scratch\build-scratch.ps1'
    $scratchExe = Join-Path (Get-ProbeRoot) 'scratch\bin\ScratchForms.exe'
    if (Test-Path -LiteralPath $buildScratch) {
        try {
            & $buildScratch -Force | Out-Null
            if (Test-Path -LiteralPath $scratchExe) {
                $wfHost = Start-Process -FilePath $scratchExe `
                    -ArgumentList @('--tag', 'a19hp', '--pos', '520,40', '--host-providers', '--secret-marker', $script:SecretMarker) `
                    -PassThru
                [void]$script:Spawned.Add($wfHost.Id)
                $winformsHandle = Wait-ProbeWindow -Process $wfHost
                Start-Sleep -Seconds 2

                $wfLegacy = Start-Process -FilePath $scratchExe `
                    -ArgumentList @('--tag', 'a19leg', '--pos', '520,420', '--secret-marker', $script:SecretMarker) `
                    -PassThru
                [void]$script:Spawned.Add($wfLegacy.Id)
                $winformsLegacyHandle = Wait-ProbeWindow -Process $wfLegacy
                Start-Sleep -Seconds 1
            }
        } catch {
            Write-Host ('ScratchForms skipped: ' + $_.Exception.Message)
        }
    }

    $decoyHandle = [IntPtr]::Zero
    $decoyProcess = $null
    if (Test-Path -LiteralPath $scratchExe) {
        $decoyProcess = Start-Process -FilePath $scratchExe `
            -ArgumentList @('--tag', 'a19decoy', '--pos', '900,40', '--host-providers') `
            -PassThru
        [void]$script:Spawned.Add($decoyProcess.Id)
        $decoyHandle = Wait-ProbeWindow -Process $decoyProcess
    }

    $semArgs = @('--semantic')
    if ($wpfHandle -ne [IntPtr]::Zero) { $semArgs += @('--wpf', $wpfHandle.ToString()) }
    if ($winformsHandle -ne [IntPtr]::Zero) { $semArgs += @('--winforms', $winformsHandle.ToString()) }
    $semantic = if ($wpfHandle -ne [IntPtr]::Zero -or $winformsHandle -ne [IntPtr]::Zero) {
        Invoke-ProbePass -Exe $exe -Arguments $semArgs
    } else {
        New-NotMeasuredResult -Reason 'no scratch windows available for semantic set'
    }
    $script:paths.semantic = Write-SemanticCapture -Name "semantic-set-$Label.json" -Content (ConvertTo-Json -InputObject $semantic -Depth 20)
    Register-MandatoryPass -Capture $script:paths.semantic -Result $semantic
    Write-Host "wrote $($script:paths.semantic)"

    $failArgs = @('--failure')
    if ($wpfHandle -ne [IntPtr]::Zero) { $failArgs += @('--wpf', $wpfHandle.ToString()) }
    $failure = if ($wpfHandle -ne [IntPtr]::Zero) {
        Invoke-ProbePass -Exe $exe -Arguments $failArgs
    } else {
        New-NotMeasuredResult -Reason 'WPF unavailable for failure taxonomy'
    }

    $killed = $null
    if (Test-Path -LiteralPath $wpfScript) {
        $wpfKill = Start-Process -FilePath 'powershell.exe' `
            -ArgumentList @(
                '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $wpfScript,
                '-Tag', 'a19kill', '-TimeoutSeconds', '60', '-SecretMarker', $script:SecretMarker
            ) `
            -PassThru -WindowStyle Hidden
        [void]$script:Spawned.Add($wpfKill.Id)
        $killHwnd = Wait-ProbeWindow -Process $wpfKill
        Start-Sleep -Seconds 2
        if ($killHwnd -ne [IntPtr]::Zero) {
            $killed = Invoke-ProbePass -Exe $exe -Arguments @(
                '--kill',
                '--kill-hwnd', $killHwnd.ToString(),
                '--kill-pid', $wpfKill.Id.ToString(),
                '--kill-aid', 'txtValue'
            )
        } else {
            $killed = New-NotMeasuredResult -Reason 'kill-leg WPF window never appeared'
        }
    } else {
        $killed = New-NotMeasuredResult -Reason 'WPF script missing for kill leg'
    }

    $failureMerged = [ordered]@{
        taxonomy = $failure
        killed_provider = $killed
    }
    $script:paths.failure = Write-SemanticCapture -Name "semantic-failure-$Label.json" -Content (ConvertTo-Json -InputObject $failureMerged -Depth 20)
    Register-MandatoryPass -Capture $script:paths.failure -Result $failureMerged
    Write-Host "wrote $($script:paths.failure)"

    $secArgs = @('--secure')
    if ($wpfHandle -ne [IntPtr]::Zero) { $secArgs += @('--wpf', $wpfHandle.ToString()) }
    if ($winformsHandle -ne [IntPtr]::Zero) { $secArgs += @('--winforms', $winformsHandle.ToString()) }
    $secure = Invoke-ProbePass -Exe $exe -Arguments $secArgs
    $script:paths.secure = Write-SemanticCapture -Name "semantic-secure-$Label.json" -Content (ConvertTo-Json -InputObject $secure -Depth 16)
    Register-MandatoryPass -Capture $script:paths.secure -Result $secure
    Write-Host "wrote $($script:paths.secure)"

    $uipi = $null
    try {
        if (-not (Test-Path -LiteralPath $scratchExe)) {
            $uipi = New-NotMeasuredResult -Reason 'unmeasurable_scratch_missing: ScratchForms.exe missing for UIPI leg'
        } else {
            $high = Start-Process -FilePath $scratchExe `
                -ArgumentList @('--tag', 'a19high', '--pos', '100,100', '--host-providers', '--secret-marker', $script:SecretMarker) `
                -PassThru
            [void]$script:Spawned.Add($high.Id)
            $highHwnd = Wait-ProbeWindow -Process $high
            Start-Sleep -Seconds 1
            if ($highHwnd -eq [IntPtr]::Zero) {
                $uipi = New-NotMeasuredResult -Reason 'unmeasurable_high_window_absent: High-owned scratch window never appeared'
            } else {
                $mediumOut = Join-Path ([IO.Path]::GetTempPath()) ('a19-uipi-' + [guid]::NewGuid() + '.json')
                $medium = Start-MediumIntegrityProcess -FilePath $exe -ArgumentList @(
                    '--uipi',
                    '--high', $highHwnd.ToString(),
                    '--value-id', 'txtValue',
                    '--invoke-id', 'btnAction',
                    '--out', $mediumOut
                )
                [void]$script:Spawned.Add($medium.ProcessId)
                $deadline = (Get-Date).AddSeconds(20)
                while ((Get-Date) -lt $deadline) {
                    $mediumProc = Get-Process -Id $medium.ProcessId -ErrorAction SilentlyContinue
                    if (-not $mediumProc -or $mediumProc.HasExited) { break }
                    if (Test-Path -LiteralPath $mediumOut) {
                        Start-Sleep -Milliseconds 200
                        break
                    }
                    Start-Sleep -Milliseconds 200
                }
                $mediumProc = Get-Process -Id $medium.ProcessId -ErrorAction SilentlyContinue
                if ($mediumProc -and -not $mediumProc.HasExited) {
                    try { Stop-ScratchProcess -ProcessId $medium.ProcessId } catch { }
                }
                if (Test-Path -LiteralPath $mediumOut) {
                    $uipi = Get-Content -LiteralPath $mediumOut -Raw | ConvertFrom-Json
                    try { Remove-Item -LiteralPath $mediumOut -Force } catch { }
                } else {
                    $uipi = New-NotMeasuredResult -Reason 'unmeasurable_medium_out_missing: Start-MediumIntegrityProcess launched the probe at Medium integrity but the --out file was never written'
                }
            }
        }
    } catch {
        $uipi = New-NotMeasuredResult -Reason ("unmeasurable_elevation_manufacture_unavailable: " + $_.Exception.Message)
    }
    $script:paths.uipi = Write-SemanticCapture -Name "semantic-uipi-$Label.json" -Content (ConvertTo-Json -InputObject $uipi -Depth 16)
    Register-MandatoryPass -Capture $script:paths.uipi -Result $uipi
    Write-Host "wrote $($script:paths.uipi)"

    $focusArgs = @('--focus')
    if ($wpfHandle -ne [IntPtr]::Zero) { $focusArgs += @('--wpf', $wpfHandle.ToString()) }
    if ($decoyHandle -ne [IntPtr]::Zero) { $focusArgs += @('--decoy', $decoyHandle.ToString()) }
    $focus = if ($wpfHandle -ne [IntPtr]::Zero) {
        Invoke-ProbePass -Exe $exe -Arguments $focusArgs
    } else {
        New-NotMeasuredResult -Reason 'WPF unavailable for SetFocus'
    }
    $script:paths.focus = Write-SemanticCapture -Name "semantic-focus-$Label.json" -Content (ConvertTo-Json -InputObject $focus -Depth 12)
    Register-MandatoryPass -Capture $script:paths.focus -Result $focus
    Write-Host "wrote $($script:paths.focus)"

    $legArgs = @('--legacy')
    if ($winformsLegacyHandle -ne [IntPtr]::Zero) {
        $legArgs += @('--winforms-legacy', $winformsLegacyHandle.ToString())
    } elseif ($winformsHandle -ne [IntPtr]::Zero) {
        $legArgs += @('--winforms', $winformsHandle.ToString())
    }
    $legacy = Invoke-ProbePass -Exe $exe -Arguments $legArgs
    $script:paths.legacy = Write-SemanticCapture -Name "semantic-legacy-$Label.json" -Content (ConvertTo-Json -InputObject $legacy -Depth 16)
    Register-MandatoryPass -Capture $script:paths.legacy -Result $legacy
    Write-Host "wrote $($script:paths.legacy)"

    $comboArgs = @('--combo')
    if ($wpfHandle -ne [IntPtr]::Zero) { $comboArgs += @('--wpf', $wpfHandle.ToString()) }
    $combo = if ($wpfHandle -ne [IntPtr]::Zero) {
        Invoke-ProbePass -Exe $exe -Arguments $comboArgs
    } else {
        New-NotMeasuredResult -Reason 'WPF unavailable for combobox/nested scroll'
    }
    $script:paths.combo = Write-SemanticCapture -Name "semantic-combo-$Label.json" -Content (ConvertTo-Json -InputObject $combo -Depth 20)
    Register-MandatoryPass -Capture $script:paths.combo -Result $combo
    Write-Host "wrote $($script:paths.combo)"

    $costArgs = @('--cost')
    if ($wpfHandle -ne [IntPtr]::Zero) { $costArgs += @('--wpf', $wpfHandle.ToString()) }
    $cost = if ($wpfHandle -ne [IntPtr]::Zero) {
        Invoke-ProbePass -Exe $exe -Arguments $costArgs
    } else {
        New-NotMeasuredResult -Reason 'WPF unavailable for cost'
    }
    $script:paths.cost = Write-SemanticCapture -Name "semantic-cost-$Label.json" -Content (ConvertTo-Json -InputObject $cost -Depth 12)
    Register-MandatoryPass -Capture $script:paths.cost -Result $cost
    Write-Host "wrote $($script:paths.cost)"

    Assert-SecretMarkerAbsent
} finally {
    foreach ($id in @($script:Spawned)) {
        $proc = Get-Process -Id $id -ErrorAction SilentlyContinue
        if ($proc) { try { Stop-ScratchProcess -ProcessId $id } catch { } }
    }
    if ($null -ne $built -and $built.work -and (Test-Path -LiteralPath $built.work)) {
        Remove-Item -LiteralPath $built.work -Recurse -Force -ErrorAction SilentlyContinue
    }
}

Assert-MandatoryMeasurement -Probe '19-semantic-actions' -Label $Label

Write-ProbeResult -Probe '19-semantic-actions' -Status 'ok' -Message 'semantic-action probes captured' -Data @{
    semantic = if ($script:paths.semantic) { Split-Path -Leaf $script:paths.semantic } else { '<none>' }
    failure = if ($script:paths.failure) { Split-Path -Leaf $script:paths.failure } else { '<none>' }
    secure = if ($script:paths.secure) { Split-Path -Leaf $script:paths.secure } else { '<none>' }
    uipi = if ($script:paths.uipi) { Split-Path -Leaf $script:paths.uipi } else { '<none>' }
    focus = if ($script:paths.focus) { Split-Path -Leaf $script:paths.focus } else { '<none>' }
    legacy = if ($script:paths.legacy) { Split-Path -Leaf $script:paths.legacy } else { '<none>' }
    combo = if ($script:paths.combo) { Split-Path -Leaf $script:paths.combo } else { '<none>' }
    cost = if ($script:paths.cost) { Split-Path -Leaf $script:paths.cost } else { '<none>' }
}
exit 0
