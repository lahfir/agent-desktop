#Requires -Version 5.1
<#
.SYNOPSIS
    R21's redaction gate: Test-CaptureRedaction (probes/windows/common.ps1)
    plus Test-CaptureCliRedaction (scripts/lib/capture-redaction-cli.psm1)
    over every committed capture and the dogfood report body.

.DESCRIPTION
    Two self-tests run before the real scan, and the real scan never
    executes if either fails - a redaction gate that has not been shown
    able to both catch and pass is not trustworthy over content nobody
    will re-read before it reaches a public repository:

      1. The CLI-envelope fixtures under
         scripts/fixtures/capture-redaction/{must-catch,must-pass}/ prove
         Test-CaptureCliRedaction's per-field rules (name, value,
         description, title, path[], occluder.name, pid) independently of
         this machine's identity.
      2. Test-CaptureRedaction's own machine-identity rules (user name,
         profile path) are proven against fixtures generated at run time
         from this process's live USERNAME/USERPROFILE and deleted
         afterward - committing a fixture that actually contains a real
         username would defeat the gate it exists to prove, and the value
         is not portable to another box in any case (mirrors
         check-e2e-windows-contract.ps1's own generated-not-committed
         rule13 fixtures).

    The real scan then walks every file under the capture roots named
    below (skipping a root that does not exist yet - the dogfood captures
    and report land in a later unit of this same PR) plus the dogfood
    report body when present, and fails if either check fails on any file
    or if zero files were scanned at all - a gate handed nothing to check
    is not a check.

.PARAMETER Path
    Override the default capture roots (used by windows-e2e.yml to point
    at the live run's own capture directory instead).
#>
[CmdletBinding()]
param(
    [string[]]$Path
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$scriptsDir = $PSScriptRoot
$repoRoot = Split-Path -Parent $scriptsDir

. (Join-Path $repoRoot 'probes\windows\common.ps1')
Import-Module (Join-Path $scriptsDir 'lib\capture-redaction-cli.psm1') -Force

$failed = 0
function Add-RedactionGateFailure {
    param([string]$Message)
    Write-Host "FAIL $Message"
    $script:failed = 1
}

# --- self-test 1: the committed CLI-envelope fixtures ---
$fixturesRoot = Join-Path $scriptsDir 'fixtures\capture-redaction'
$mustCatchDir = Join-Path $fixturesRoot 'must-catch'
$mustPassDir = Join-Path $fixturesRoot 'must-pass'
$mustCatchFiles = @(Get-ChildItem -LiteralPath $mustCatchDir -File -ErrorAction SilentlyContinue)
$mustPassFiles = @(Get-ChildItem -LiteralPath $mustPassDir -File -ErrorAction SilentlyContinue)
if ($mustCatchFiles.Count -eq 0) { Add-RedactionGateFailure "no MUST-CATCH fixtures found under $mustCatchDir" }
if ($mustPassFiles.Count -eq 0) { Add-RedactionGateFailure "no MUST-PASS fixtures found under $mustPassDir" }
foreach ($file in $mustCatchFiles) {
    $violations = Get-CaptureCliRedactionViolations -Path $file.FullName
    if ($violations.Count -eq 0) {
        Add-RedactionGateFailure "self-test: MUST-CATCH fixture $($file.Name) produced no Test-CaptureCliRedaction violation"
    }
}
foreach ($file in $mustPassFiles) {
    $violations = Get-CaptureCliRedactionViolations -Path $file.FullName
    if ($violations.Count -gt 0) {
        Add-RedactionGateFailure "self-test: MUST-PASS fixture $($file.Name) produced $($violations.Count) Test-CaptureCliRedaction violation(s): $($violations -join '; ')"
    }
    if (-not (Test-CaptureRedaction -Path $file.FullName)) {
        Add-RedactionGateFailure "self-test: MUST-PASS fixture $($file.Name) failed Test-CaptureRedaction"
    }
}

# --- self-test 2: Test-CaptureRedaction's machine-identity rules, against
#     fixtures generated here and never committed. ---
$identityScratch = Join-Path ([IO.Path]::GetTempPath()) ('agent-desktop-redaction-selftest-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
New-Item -ItemType Directory -Path $identityScratch -Force | Out-Null
try {
    $utf8NoBom = New-Object System.Text.UTF8Encoding $false
    $usernameCase = Join-Path $identityScratch 'must-catch-username.json'
    [IO.File]::WriteAllText($usernameCase, ('{"probe":"self-test","note":"logged in as ' + $env:USERNAME + '"}'), $utf8NoBom)
    if (Test-CaptureRedaction -Path $usernameCase) {
        Add-RedactionGateFailure 'self-test: a generated fixture carrying the live USERNAME was not caught by Test-CaptureRedaction'
    }
    if ($env:USERPROFILE) {
        $pathCase = Join-Path $identityScratch 'must-catch-userprofile-path.json'
        [IO.File]::WriteAllText($pathCase, ('{"probe":"self-test","note":"staged under ' + $env:USERPROFILE + '\\Documents"}'), $utf8NoBom)
        if (Test-CaptureRedaction -Path $pathCase) {
            Add-RedactionGateFailure 'self-test: a generated fixture carrying the live USERPROFILE path was not caught by Test-CaptureRedaction'
        }
    }
    $cleanCase = Join-Path $identityScratch 'must-pass-clean.json'
    [IO.File]::WriteAllText($cleanCase, '{"probe":"self-test","measurable":true,"count":3}', $utf8NoBom)
    if (-not (Test-CaptureRedaction -Path $cleanCase)) {
        Add-RedactionGateFailure 'self-test: a clean generated fixture with no identity residue was rejected by Test-CaptureRedaction'
    }
} finally {
    Remove-Item -LiteralPath $identityScratch -Recurse -Force -ErrorAction SilentlyContinue
}

if ($failed -ne 0) {
    Write-Host 'The redaction gate failed its own self-test; refusing to scan real captures on an untrusted gate.'
    exit 1
}
Write-Host 'OK: capture-redaction gate self-test passed (CLI-envelope fixtures, machine-identity fixtures).'

# --- the real scan ---
$roots = if ($Path) { $Path } else {
    @(
        (Join-Path $repoRoot 'probes\windows\24-fixture-e2e\captures'),
        (Join-Path $repoRoot 'docs\dogfood-reports\2026-08-16-001-captures')
    )
}
$reportPath = Join-Path $repoRoot 'docs\dogfood-reports\2026-08-16-001-feat-windows-2-12-fixture-e2e-harness-dogfood.md'

function Test-FileRedaction {
    param([Parameter(Mandatory = $true)][string]$FullPath)
    $ok = $true
    if (-not (Test-CaptureRedaction -Path $FullPath)) {
        Add-RedactionGateFailure "$FullPath`: Test-CaptureRedaction residue (see warning above)"
        $ok = $false
    }
    foreach ($violation in (Get-CaptureCliRedactionViolations -Path $FullPath)) {
        Add-RedactionGateFailure $violation
        $ok = $false
    }
    return $ok
}

$scanned = 0
foreach ($root in $roots) {
    if (-not (Test-Path -LiteralPath $root)) {
        Write-Host "SKIP: capture root does not exist yet: $root"
        continue
    }
    foreach ($file in (Get-ChildItem -LiteralPath $root -File -Recurse)) {
        $scanned++
        [void](Test-FileRedaction -FullPath $file.FullName)
    }
}
if (Test-Path -LiteralPath $reportPath) {
    $scanned++
    [void](Test-FileRedaction -FullPath $reportPath)
} elseif (-not $Path) {
    Write-Host "SKIP: dogfood report not written yet: $reportPath"
}

<#
    Zero scanned is only suspicious against the default, always-populated
    corpus roots - it is how a capture added later without ever being
    wired into this gate would go undetected. An explicit -Path override
    (windows-e2e.yml pointing at a live run's own capture directory) may
    legitimately not exist yet on a run that produced no captures, and
    upload-artifact's own if-no-files-found already handles that case; this
    gate must not invent a failure a caller who asked for a specific,
    possibly-empty directory did not.
#>
if ($scanned -eq 0 -and -not $Path) {
    Add-RedactionGateFailure 'zero files scanned over the default capture roots - a redaction gate handed nothing to check is not a check'
}

if ($failed -ne 0) {
    Write-Host 'FAIL: the capture redaction gate found residue.'
    exit 1
}
Write-Host "OK: $scanned file(s) carry no unredacted machine-identity or CLI-envelope content."
exit 0
