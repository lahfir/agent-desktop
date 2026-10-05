#Requires -Version 5.1
<#
.SYNOPSIS
    Invert-verified self-test for run-windows-e2e-ci.ps1's refusal guard
    (U14 invert item 20): with AGENT_DESKTOP_NATIVE_E2E_RUNNER unset,
    run-windows-e2e-ci.ps1 must exit 2 having touched neither the staged
    release binary nor started any product or fixture process.

.DESCRIPTION
    "Asserted by the absence of the build output" is not usable: on any
    machine that has already run `cargo build --release`, the binary
    already exists, so that predicate is vacuously true whether or not the
    guard actually fired. This self-test instead requires the binary to
    already exist (a prior CI step builds it) and asserts its
    LastWriteTime is byte-for-byte unchanged across the invocation, plus a
    pid snapshot before/after proving no new agent-desktop.exe or
    AgentDeskFixture.exe process was started. Run on the hosted
    windows-latest lane, never against a real desktop lease.
#>
[CmdletBinding()]
param()

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $PSCommandPath))
$binaryPath = Join-Path $repoRoot 'target\release\agent-desktop.exe'
$runScript = Join-Path $repoRoot 'scripts\run-windows-e2e-ci.ps1'

if (-not (Test-Path -LiteralPath $binaryPath)) {
    throw "run-windows-e2e-ci-refusal-selftest: $binaryPath must already exist (build the release binary first) - an absent binary would make the unchanged-mtime predicate vacuously true"
}

$beforeWrite = (Get-Item -LiteralPath $binaryPath).LastWriteTimeUtc
$beforePids = @(Get-Process -Name 'agent-desktop', 'AgentDeskFixture' -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)

Remove-Item Env:AGENT_DESKTOP_NATIVE_E2E_RUNNER -ErrorAction SilentlyContinue
& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $runScript
$exitCode = $LASTEXITCODE

$afterWrite = (Get-Item -LiteralPath $binaryPath).LastWriteTimeUtc
$afterPids = @(Get-Process -Name 'agent-desktop', 'AgentDeskFixture' -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
$newPids = @($afterPids | Where-Object { $beforePids -notcontains $_ })

$failed = 0
if ($exitCode -ne 2) {
    Write-Host "FAIL: run-windows-e2e-ci.ps1 exited $exitCode without AGENT_DESKTOP_NATIVE_E2E_RUNNER set; expected 2"
    $failed = 1
}
if ($afterWrite -ne $beforeWrite) {
    Write-Host "FAIL: $binaryPath's last-write time changed ($beforeWrite -> $afterWrite) - the refusal guard let a build proceed"
    $failed = 1
}
if ($newPids.Count -gt 0) {
    Write-Host "FAIL: new agent-desktop/AgentDeskFixture process(es) appeared during the refusal invocation: $($newPids -join ', ')"
    $failed = 1
}

if ($failed -ne 0) { exit 1 }
Write-Host 'OK: run-windows-e2e-ci.ps1 refused with exit 2, the staged binary was untouched, and no product/fixture process was started.'
exit 0
