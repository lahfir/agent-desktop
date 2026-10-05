[CmdletBinding()]
param(
    [switch]$Compare
)

$ErrorActionPreference = 'Stop'

$HarnessExit = 2
$FailureExit = 1

function Exit-Harness {
    param([string]$Message)
    Write-Host ('HARNESS-ABORT: ' + $Message)
    exit $HarnessExit
}

$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$commonPath = Join-Path $root 'common.ps1'
if (-not (Test-Path -LiteralPath $commonPath)) { Exit-Harness ('common.ps1 not found at ' + $commonPath) }
try { . $commonPath } catch { Exit-Harness ('failed to dot-source common.ps1: ' + $_.Exception.Message) }

$capturesRoot = Join-Path $root 'captures'
if (-not (Test-Path -LiteralPath $capturesRoot)) {
    try { New-Item -ItemType Directory -Path $capturesRoot -Force | Out-Null } catch { Exit-Harness ('cannot create captures dir: ' + $_.Exception.Message) }
}

function Get-CaptureFiles {
    return @(Get-ChildItem -LiteralPath $capturesRoot -Recurse -File -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -ne '.gitkeep' })
}

<#
    Every capture in the corpus, including the ones the probes that live in
    their own NN-* directory write beside themselves.

    The redaction gate is the only sweep that must see all of them. Scoping it
    to $capturesRoot alone made "redaction gate clean over N capture file(s)"
    true of a subset while reading as a statement about the corpus, and the
    directories it skipped are the ones holding the newest captures. This is
    deliberately not used by -Compare: the normalized-twin comparison applies
    to the committed twinned corpus under $capturesRoot, and the per-probe
    directories write no twin.
#>
function Get-AllCaptureFiles {
    $files = New-Object System.Collections.ArrayList
    foreach ($file in (Get-CaptureFiles)) { [void]$files.Add($file) }
    foreach ($dir in @(Get-ChildItem -LiteralPath $root -Directory -ErrorAction SilentlyContinue |
            Where-Object { $_.Name -match '^\d\d-' })) {
        $probeCaptures = Join-Path $dir.FullName 'captures'
        if (-not (Test-Path -LiteralPath $probeCaptures)) { continue }
        foreach ($file in @(Get-ChildItem -LiteralPath $probeCaptures -Recurse -File -ErrorAction SilentlyContinue |
                Where-Object { $_.Name -ne '.gitkeep' })) {
            [void]$files.Add($file)
        }
    }
    return @($files)
}

function Invoke-CompareMode {
    $drift = New-Object System.Collections.ArrayList
    $missing = New-Object System.Collections.ArrayList
    foreach ($file in (Get-CaptureFiles)) {
        if ($file.Name -like '*.normalized') { continue }
        $twin = $file.FullName + '.normalized'
        $expected = Get-NormalizedCapture -Text ([IO.File]::ReadAllText($file.FullName))
        if (-not (Test-Path -LiteralPath $twin)) {
            [void]$missing.Add($file.FullName)
            continue
        }
        $actual = [IO.File]::ReadAllText($twin)
        if ($actual -ne $expected) {
            $diff = Compare-Object -ReferenceObject ($actual -split "`r?`n") -DifferenceObject ($expected -split "`r?`n") |
                ForEach-Object { $_.SideIndicator + ' ' + $_.InputObject }
            [void]$drift.Add([pscustomobject]@{ File = $twin; Diff = ($diff -join [Environment]::NewLine) })
        }
    }
    Write-Host ''
    Write-Host '=== normalized comparison ==='
    foreach ($m in $missing) { Write-Host ('MISSING-TWIN ' + $m) }
    foreach ($d in $drift) {
        Write-Host ('DRIFT ' + $d.File)
        Write-Host $d.Diff
    }
    if ($missing.Count -eq 0 -and $drift.Count -eq 0) {
        Write-Host 'normalized diff empty'
        exit 0
    }
    Write-Host ('normalized drift: ' + $drift.Count + ' file(s), missing twins: ' + $missing.Count)
    exit $FailureExit
}

if ($Compare) { Invoke-CompareMode }

$pidLedger = Join-Path $env:TEMP ('agent-desktop-probe-pids-run-' + $PID + '.txt')
try {
    if (Test-Path -LiteralPath $pidLedger) { Remove-Item -LiteralPath $pidLedger -Force }
    New-Item -ItemType File -Path $pidLedger -Force | Out-Null
} catch { Exit-Harness ('cannot create pid ledger: ' + $_.Exception.Message) }
$env:AGENT_DESKTOP_PROBE_PIDS = $pidLedger

$scripts = @(Get-ChildItem -LiteralPath $root -Filter '*.ps1' -File |
    Where-Object { $_.Name -match '^\d\d-' } | Sort-Object -Property Name)

Write-Host ('=== agent-desktop windows probe harness: ' + $scripts.Count + ' probe(s) ===')

$summaries = New-Object System.Collections.ArrayList
$failed = $false

foreach ($script in $scripts) {
    $probe = [IO.Path]::GetFileNameWithoutExtension($script.Name)
    $started = Get-Date
    $output = $null
    $code = 0
    try {
        $ErrorActionPreference = 'Continue'
        $output = & powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $script.FullName 2>&1
        $code = $LASTEXITCODE
    } catch [System.Management.Automation.CommandNotFoundException] {
        Exit-Harness ('could not launch probe ' + $probe + ': ' + $_.Exception.Message)
    } finally {
        $ErrorActionPreference = 'Stop'
    }
    foreach ($line in $output) { Write-Host ('  | ' + $line) }
    $resultLine = @($output | Where-Object { "$_" -like 'PROBE-RESULT|*' } | Select-Object -Last 1)
    $status = 'fail'
    $message = 'no PROBE-RESULT line emitted'
    if ($resultLine.Count -gt 0) {
        $parts = ("$($resultLine[0])") -split '\|'
        if ($parts.Count -ge 4) { $status = $parts[2]; $message = $parts[3] }
    }
    $captureDir = Join-Path $capturesRoot $probe
    $captureCount = 0
    if (Test-Path -LiteralPath $captureDir) {
        $captureCount = @(Get-ChildItem -LiteralPath $captureDir -File -Recurse -ErrorAction SilentlyContinue |
            Where-Object { $_.Name -notlike '*.normalized' -and $_.Name -ne '.gitkeep' }).Count
    }
    $verdict = $status
    if ($code -ne 0) { $verdict = 'fail'; $message = ('exit code ' + $code + '; ' + $message) }
    if ($verdict -ne 'skip' -and $captureCount -eq 0) { $verdict = 'fail'; $message = ('no capture produced; ' + $message) }
    if ($verdict -eq 'fail') { $failed = $true }
    $elapsed = [int]((Get-Date) - $started).TotalMilliseconds
    $line = ('[' + $verdict + '] ' + $probe + ' exit=' + $code + ' captures=' + $captureCount + ' ms=' + $elapsed + ' ' + $message)
    [void]$summaries.Add($line)
    Write-Host $line
}

Write-Host ''
Write-Host '=== redaction gate ==='
$residueFiles = New-Object System.Collections.ArrayList
$gatedFiles = @(Get-AllCaptureFiles)
foreach ($file in $gatedFiles) {
    $clean = $true
    try {
        $clean = Test-CaptureRedaction -Path $file.FullName
        if (-not (Test-CaptureNameEcho -Path $file.FullName)) { $clean = $false }
    } catch {
        Exit-Harness ('redaction gate could not read ' + $file.FullName + ': ' + $_.Exception.Message)
    }
    if (-not $clean) { [void]$residueFiles.Add($file.FullName) }
}
if ($residueFiles.Count -eq 0) {
    Write-Host ('redaction gate clean over ' + $gatedFiles.Count + ' capture file(s)')
} else {
    foreach ($f in $residueFiles) { Write-Host ('REDACTION-RESIDUE ' + $f) }
    $failed = $true
}

Write-Host ''
Write-Host '=== survivor check ==='
$survivors = @(Get-ScratchProcessIds)
if ($survivors.Count -eq 0) {
    Write-Host 'no probe-spawned process survived'
} else {
    foreach ($s in $survivors) { Write-Host ('SURVIVING-PROCESS ' + $s) }
    $failed = $true
}

Write-Host ''
Write-Host '=== summary ==='
foreach ($s in $summaries) { Write-Host $s }
Write-Host ('probes=' + $scripts.Count + ' residue_files=' + $residueFiles.Count + ' survivors=' + $survivors.Count)

Remove-Item -LiteralPath $pidLedger -Force -ErrorAction SilentlyContinue
$env:AGENT_DESKTOP_PROBE_PIDS = $null

if ($failed) {
    Write-Host 'RESULT: FAIL'
    exit $FailureExit
}
Write-Host 'RESULT: PASS'
exit 0
