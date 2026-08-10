#Requires -Version 5.1
<#
.SYNOPSIS
    Fail if Win32_UI_Shell appears in crates/windows/Cargo.toml or resolves
    into Cargo.lock for that crate. Self-tests MUST-CATCH / MUST-PASS against
    the same Test-Win32UiShellExclusion function the CI step calls.
#>
[CmdletBinding()]
param(
    [string]$RepoRoot = ''
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

if ([string]::IsNullOrWhiteSpace($RepoRoot)) {
    $RepoRoot = Split-Path -Parent (Split-Path -Parent $PSCommandPath)
}
$RepoRoot = (Resolve-Path -LiteralPath $RepoRoot).ProviderPath

function Test-Win32UiShellExclusion {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$CargoTomlText,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$CargoLockText
    )
    $failures = New-Object System.Collections.Generic.List[string]
    if ($CargoTomlText -match 'Win32_UI_Shell') {
        $failures.Add('Win32_UI_Shell appears in crates/windows/Cargo.toml') | Out-Null
    }
    if ($CargoLockText -match 'Win32_UI_Shell') {
        $failures.Add('Win32_UI_Shell resolves into Cargo.lock for the windows crate graph') | Out-Null
    }
    return [pscustomobject]@{ Failures = $failures.ToArray() }
}

function Get-WindowsCrateLockSlice {
    param(
        [Parameter(Mandatory = $true)][string]$LockText,
        [Parameter(Mandatory = $true)][string]$PackageName
    )
    $lines = $LockText -split "`r?`n"
    $buffer = New-Object System.Collections.Generic.List[string]
    $collecting = $false
    $inPackage = $false
    foreach ($line in $lines) {
        if ($line -match '^\[\[package\]\]\s*$') {
            if ($collecting) { break }
            $inPackage = $true
            $buffer.Clear()
            $buffer.Add($line) | Out-Null
            continue
        }
        if (-not $inPackage) { continue }
        $buffer.Add($line) | Out-Null
        if ($line -match ('^\s*name\s*=\s*"' + [regex]::Escape($PackageName) + '"\s*$')) {
            $collecting = $true
        }
    }
    if (-not $collecting) { return '' }
    return ($buffer -join "`n")
}

function Invoke-Win32UiShellSelfTest {
    $failures = New-Object System.Collections.Generic.List[string]

    $mustCatchToml = Test-Win32UiShellExclusion -CargoTomlText 'features = ["Win32_UI_Shell"]' -CargoLockText ''
    if ($mustCatchToml.Failures.Count -lt 1) {
        $failures.Add('MUST CATCH, missed: Cargo.toml featuring Win32_UI_Shell did not fail') | Out-Null
    }

    $badLock = @"
name = "agent-desktop-windows"
features = ["Win32_UI_Shell"]
"@
    $mustCatchLock = Test-Win32UiShellExclusion -CargoTomlText 'features = ["Win32_Foundation"]' -CargoLockText $badLock
    if ($mustCatchLock.Failures.Count -lt 1) {
        $failures.Add('MUST CATCH, missed: Cargo.lock slice featuring Win32_UI_Shell did not fail') | Out-Null
    }

    $goodLock = @"
name = "agent-desktop-windows"
features = ["Win32_Foundation"]
"@
    $mustPass = Test-Win32UiShellExclusion -CargoTomlText 'features = ["Win32_Foundation", "Win32_Graphics_Gdi"]' -CargoLockText $goodLock
    if ($mustPass.Failures.Count -gt 0) {
        $failures.Add('MUST PASS, false positive: clean feature set failed (' + ($mustPass.Failures -join '; ') + ')') | Out-Null
    }

    return [pscustomobject]@{ Failures = $failures.ToArray() }
}

$selfTest = Invoke-Win32UiShellSelfTest
foreach ($f in $selfTest.Failures) {
    Write-Host ('self-test FAIL: ' + $f)
}
if ($selfTest.Failures.Count -gt 0) {
    Write-Host 'FAIL: Win32_UI_Shell exclusion gate failed its own self-test'
    exit 1
}
Write-Host 'OK: Win32_UI_Shell exclusion gate self-test passed'

$tomlPath = Join-Path $RepoRoot 'crates\windows\Cargo.toml'
$lockPath = Join-Path $RepoRoot 'Cargo.lock'
if (-not (Test-Path -LiteralPath $tomlPath)) {
    Write-Host ('FAIL: missing ' + $tomlPath)
    exit 1
}
if (-not (Test-Path -LiteralPath $lockPath)) {
    Write-Host ('FAIL: missing ' + $lockPath)
    exit 1
}

$tomlText = [IO.File]::ReadAllText($tomlPath)
$lockText = [IO.File]::ReadAllText($lockPath)
$lockSlice = Get-WindowsCrateLockSlice -LockText $lockText -PackageName 'agent-desktop-windows'
$scanLock = if ([string]::IsNullOrEmpty($lockSlice)) { $lockText } else { $lockSlice }

$real = Test-Win32UiShellExclusion -CargoTomlText $tomlText -CargoLockText $scanLock
foreach ($f in $real.Failures) {
    Write-Host ('FAIL: ' + $f)
}
if ($real.Failures.Count -gt 0) {
    exit 1
}
Write-Host 'OK: Win32_UI_Shell is absent from crates/windows/Cargo.toml and its Cargo.lock resolution'
exit 0
