#Requires -Version 5.1
<#
.SYNOPSIS
    Fail if Win32_UI_Shell appears in crates/windows/Cargo.toml or resolves
    into the windows crate's real feature graph. Self-tests MUST-CATCH /
    MUST-PASS against the same Test-Win32UiShellExclusion function the CI
    step calls.
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
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$FeatureTreeText
    )
    $failures = New-Object System.Collections.Generic.List[string]
    if ($CargoTomlText -match 'Win32_UI_Shell') {
        $failures.Add('Win32_UI_Shell appears in crates/windows/Cargo.toml') | Out-Null
    }
    if ($FeatureTreeText -match 'Win32_UI_Shell') {
        $failures.Add('Win32_UI_Shell resolves into the agent-desktop-windows feature graph (cargo tree -e features)') | Out-Null
    }
    return [pscustomobject]@{ Failures = $failures.ToArray() }
}

# Runs the real, resolved-dependency-graph leg. `cargo tree -e features` prints
# every feature actually activated for the crate, transitively, regardless of
# which dependency turned it on or how many indirection hops away it lives —
# unlike scanning Cargo.lock text, which never contained per-feature strings
# once the lockfile moved to format v4. `--locked` refuses to silently
# re-resolve if Cargo.lock is stale, and the loop checks both the host target
# and the pinned MSVC target the same way the sibling "Check dependency
# isolation" step does, since the windows crate's dependencies are gated on
# `cfg(target_os = "windows")`.
function Get-WindowsCrateFeatureTree {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot
    )
    $manifestPath = Join-Path $RepoRoot 'Cargo.toml'
    $chunks = New-Object System.Collections.Generic.List[string]
    foreach ($target in @('', 'x86_64-pc-windows-msvc')) {
        $arguments = @('tree', '--locked', '-e', 'features', '-p', 'agent-desktop-windows', '--manifest-path', $manifestPath)
        if ($target) { $arguments += @('--target', $target) }
        $output = & cargo @arguments 2>&1
        if ($LASTEXITCODE -ne 0) {
            throw ('cargo ' + ($arguments -join ' ') + ' failed: ' + ($output -join "`n"))
        }
        $chunks.Add(($output -join "`n")) | Out-Null
    }
    return ($chunks -join "`n")
}

function Invoke-Win32UiShellSelfTest {
    $failures = New-Object System.Collections.Generic.List[string]

    $mustCatchToml = Test-Win32UiShellExclusion -CargoTomlText 'features = ["Win32_UI_Shell"]' -FeatureTreeText ''
    if ($mustCatchToml.Failures.Count -lt 1) {
        $failures.Add('MUST CATCH, missed: Cargo.toml featuring Win32_UI_Shell did not fail') | Out-Null
    }

    # Shaped like a real `cargo tree -e features` line for the windows crate
    # (see the "OK" fixture below for the sibling, currently-real shape this
    # is derived from) so the fixture would actually appear if a dependency
    # ever turned Win32_UI_Shell on.
    $badTree = @"
agent-desktop-windows v0.7.0 (C:\repo\crates\windows)
├── windows feature "Win32_UI_Shell"
│   ├── windows v0.62.2 (*)
│   └── windows feature "Win32_UI"
│       └── windows feature "Win32" (*)
"@
    $mustCatchTree = Test-Win32UiShellExclusion -CargoTomlText 'features = ["Win32_Foundation"]' -FeatureTreeText $badTree
    if ($mustCatchTree.Failures.Count -lt 1) {
        $failures.Add('MUST CATCH, missed: resolved feature graph containing Win32_UI_Shell did not fail') | Out-Null
    }

    # Real neighboring features (Win32_UI_Accessibility, Win32_UI_WindowsAndMessaging)
    # sit one edit away from "Win32_UI_Shell" in that same "Win32_UI_*" family,
    # so this also proves the check does not false-positive on lookalikes.
    $goodTree = @"
agent-desktop-windows v0.7.0 (C:\repo\crates\windows)
├── windows feature "Win32_UI_Accessibility"
│   ├── windows v0.62.2 (*)
│   └── windows feature "Win32_UI"
│       └── windows feature "Win32" (*)
├── windows-sys feature "Win32_UI_WindowsAndMessaging"
│   └── windows-sys feature "Win32_UI"
│       └── windows-sys feature "Win32" (*)
"@
    $mustPass = Test-Win32UiShellExclusion -CargoTomlText 'features = ["Win32_Foundation", "Win32_Graphics_Gdi"]' -FeatureTreeText $goodTree
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
if (-not (Test-Path -LiteralPath $tomlPath)) {
    Write-Host ('FAIL: missing ' + $tomlPath)
    exit 1
}

$tomlText = [IO.File]::ReadAllText($tomlPath)
try {
    $featureTreeText = Get-WindowsCrateFeatureTree -RepoRoot $RepoRoot
} catch {
    Write-Host ('FAIL: could not resolve the agent-desktop-windows feature graph: ' + $_.Exception.Message)
    exit 1
}

$real = Test-Win32UiShellExclusion -CargoTomlText $tomlText -FeatureTreeText $featureTreeText
foreach ($f in $real.Failures) {
    Write-Host ('FAIL: ' + $f)
}
if ($real.Failures.Count -gt 0) {
    exit 1
}
Write-Host 'OK: Win32_UI_Shell is absent from crates/windows/Cargo.toml and its resolved feature graph'
exit 0
