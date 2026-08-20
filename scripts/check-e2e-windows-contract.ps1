#Requires -Version 5.1
<#
.SYNOPSIS
    The Windows analog of test_harness_contract.py: reads
    tests/e2e-windows's own program text and fails when the harness violates
    its own structural discipline (thirteen rules), over a file set proven
    to be the whole tree - never a glob that quietly excludes scenarios/.

.DESCRIPTION
    Two halves, per U8 item 18: the SCAN (always runs) applies the thirteen
    rules to the real harness tree; -SelfTest additionally imports Lib.psm1
    (via Invoke-U7SelfTests.ps1) and proves the assertion/verdict/skip-ledger/
    lock-ordering primitives are live against the canned-envelope stub, with
    no desktop and no fixture required.

    Before either half touches the real tree, this script proves ITS OWN
    rules can both catch and pass: every MUST-CATCH/MUST-PASS fixture under
    scripts/fixtures/e2e-windows-contract/, a planted-file walk-reachability
    check, a file-set-equality check, and the 400-line-cap fixtures
    (generated at run time, never committed - two committed 401/399-line
    fixtures would themselves violate the cap this rule extends to scripts/).
    A gate that has not been shown able to fail on the class it guards is
    an unfalsifiable gate - the exact defect this file exists to not
    repeat, proven by the invert-verification below rather than assumed.
#>
[CmdletBinding()]
param(
    [switch]$SelfTest
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$scriptsDir = $PSScriptRoot
$repoRoot = Split-Path -Parent $scriptsDir
$e2eWindowsDir = Join-Path $repoRoot 'tests\e2e-windows'
$fixturesDir = Join-Path $scriptsDir 'fixtures\e2e-windows-contract'

Import-Module (Join-Path $scriptsDir 'lib\e2e-windows-contract-common.psm1') -Force
Import-Module (Join-Path $scriptsDir 'lib\e2e-windows-contract-scan.psm1') -Force
Import-Module (Join-Path $scriptsDir 'lib\e2e-windows-contract-selftest.psm1') -Force

$failed = 0
$failedRuleNames = New-Object System.Collections.Generic.List[string]

function Add-GateFailure {
    param([string]$RuleId, [string]$Message)
    Write-Host "FAIL [$RuleId] $Message"
    $script:failed = 1
    if ($RuleId -and -not $script:failedRuleNames.Contains($RuleId)) { $script:failedRuleNames.Add($RuleId) }
}

# --- Fixture self-test: MUST-CATCH / MUST-PASS over every rule. Always runs,
#     never gated behind -SelfTest - a rule that cannot fail on its own
#     fixtures must never reach the real tree. ---
$fixtureFailures = Invoke-E2EFixtureSelfTest -FixturesRoot $fixturesDir -E2ERoot $e2eWindowsDir
foreach ($f in $fixtureFailures) { Add-GateFailure -RuleId 'selftest' -Message $f }

# --- Rule 9's exhaustive entry-point self-test (finding #29): every name the
#     real tree's Native*.psm1 modules export, generated rather than
#     committed. ---
$nativeEntryScratchRoot = Join-Path ([System.IO.Path]::GetTempPath()) ('e2e-contract-native-entry-selftest-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
try {
    $nativeEntryFailures = Invoke-E2ENativeEntryPointSelfTest -E2ERoot $e2eWindowsDir -ScratchDir $nativeEntryScratchRoot
    foreach ($f in $nativeEntryFailures) { Add-GateFailure -RuleId 'rule09' -Message $f }
} finally {
    if (Test-Path -LiteralPath $nativeEntryScratchRoot) { Remove-Item -LiteralPath $nativeEntryScratchRoot -Recurse -Force -ErrorAction SilentlyContinue }
}

# --- File-set self-test: planted-file reachability, set-removal, identical-
#     set MUST-PASS, empty-directory MUST-CATCH. ---
$scratchRoot = Join-Path ([System.IO.Path]::GetTempPath()) ('e2e-contract-selftest-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
try {
    $fileSetFailures = Invoke-E2EFileSetSelfTest -ScratchDir $scratchRoot
    foreach ($f in $fileSetFailures) { Add-GateFailure -RuleId 'rule17' -Message $f }
} finally {
    if (Test-Path -LiteralPath $scratchRoot) { Remove-Item -LiteralPath $scratchRoot -Recurse -Force -ErrorAction SilentlyContinue }
}

# --- Rule 13's 400-line-cap fixtures: generated here, never committed. ---
$sizeScratchRoot = Join-Path ([System.IO.Path]::GetTempPath()) ('e2e-contract-size-selftest-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
try {
    $sizeFailures = Invoke-E2ESizeCapSelfTest -ScratchDir $sizeScratchRoot
    foreach ($f in $sizeFailures) { Add-GateFailure -RuleId 'rule13' -Message $f }
} finally {
    if (Test-Path -LiteralPath $sizeScratchRoot) { Remove-Item -LiteralPath $sizeScratchRoot -Recurse -Force -ErrorAction SilentlyContinue }
}

if ($failed -ne 0) {
    Write-Host 'The gate failed its own self-test; refusing to scan the real tree on an untrusted gate.'
    exit 1
}
Write-Host 'OK: gate self-test passed (fixtures, file-set equality, size cap).'

# --- The real scan: build the file set two ways and require them to agree
#     (rule 17), then apply every rule over the confirmed-complete set. ---
$walked = Get-E2ETreeFileSet -Root $e2eWindowsDir
Push-Location $repoRoot
try {
    $gitRaw = git ls-files --cached --others --exclude-standard -- 'tests/e2e-windows' 2>$null
} finally {
    Pop-Location
}
$gitListed = @($gitRaw | Where-Object { $_ -match '\.(ps1|psm1|psd1)$' } | ForEach-Object {
        $_.Substring('tests/e2e-windows/'.Length)
    } | Sort-Object)

if ($walked.Count -eq 0) {
    Add-GateFailure -RuleId 'rule17' -Message "no files scanned under $e2eWindowsDir"
} else {
    $equality = Test-E2EFileSetEquality -WalkSet $walked -ReferenceSet $gitListed
    if (-not $equality.Equal) {
        foreach ($f in $equality.OnlyInWalk) { Add-GateFailure -RuleId 'rule17' -Message "on disk but not tracked/untracked-clean: $f" }
        foreach ($f in $equality.OnlyInReference) { Add-GateFailure -RuleId 'rule17' -Message "tracked but missing from the walk: $f" }
    }
}

$allowlistPath = Join-Path $e2eWindowsDir 'skip-allowlist.psd1'
$allowlistKeys = @()
if (Test-Path -LiteralPath $allowlistPath) {
    $allowlistKeys = @((Import-PowerShellDataFile -Path $allowlistPath).Keys)
}
$ruleTable = Get-E2ERuleTable -AllowlistKeys $allowlistKeys -Root $e2eWindowsDir
$scanResult = Invoke-E2EContractScan -Root $e2eWindowsDir -FileSet $walked -RuleTable $ruleTable

# --- Finding #1: the aggregate check count is not enough - a rule whose
#     ScopeFilter breaks and matches nothing would still leave the SUM
#     nonzero as long as some other rule executed checks. Assert every rule
#     individually executed at least one check against the real tree. ---
foreach ($rule in $ruleTable) {
    if ($scanResult.RuleChecksExecuted[$rule.Id] -lt 1) {
        Add-GateFailure -RuleId $rule.Id -Message "$($rule.Id) executed zero checks against the real tree - its ScopeFilter matched no file (scope wiring may be broken)"
    }
}

$rule01Hits = @($scanResult.Violations | Where-Object { $_.RuleId -eq 'rule01' })
$otherHits = @($scanResult.Violations | Where-Object { $_.RuleId -ne 'rule01' })
if ($rule01Hits.Count -eq 0) {
    Add-GateFailure -RuleId 'rule01' -Message 'no process-terminating call found anywhere - Run-E2E.ps1 must exit'
} elseif ($rule01Hits.Count -gt 1 -or $rule01Hits[0].File -ne 'Run-E2E.ps1') {
    foreach ($hit in $rule01Hits) { Add-GateFailure -RuleId 'rule01' -Message "$($hit.File):$($hit.Line) $($hit.Message)" }
}
foreach ($hit in $otherHits) { Add-GateFailure -RuleId $hit.RuleId -Message "$($hit.File):$($hit.Line) $($hit.Message)" }

# --- Rule 13 over the real tree: no file under tests/e2e-windows/ exceeds
#     400 lines, and the gate's own scripts/ files are held to the same
#     cap - the AST-based rule table has no entry for this one because it
#     needs no parse, only Get-Content.Count. ---
$rule13Checked = 0
foreach ($rel in $walked) {
    $rule13Checked++
    $full = Join-Path $e2eWindowsDir ($rel -replace '/', '\')
    $lineCount = @(Get-Content -LiteralPath $full).Count
    if ($lineCount -gt 400) {
        Add-GateFailure -RuleId 'rule13' -Message "$rel is $lineCount lines, over the 400-line cap"
    }
}
$gateScriptFiles = @(Get-ChildItem -LiteralPath $scriptsDir -File -Filter '*.ps1') + @(Get-ChildItem -LiteralPath (Join-Path $scriptsDir 'lib') -File -Filter '*.psm1' -ErrorAction SilentlyContinue)
foreach ($file in $gateScriptFiles) {
    $rule13Checked++
    $lineCount = @(Get-Content -LiteralPath $file.FullName).Count
    if ($lineCount -gt 400) {
        Add-GateFailure -RuleId 'rule13' -Message "scripts/$($file.Name) is $lineCount lines, over the 400-line cap"
    }
}
if ($rule13Checked -eq 0) {
    Add-GateFailure -RuleId 'rule13' -Message 'rule 13 executed zero size checks'
} else {
    Write-Host "Rule 13 checked $rule13Checked file(s) for the 400-line cap."
}

$totalChecksExecuted = ($scanResult.RuleChecksExecuted.Values | Measure-Object -Sum).Sum + $rule13Checked
if ($totalChecksExecuted -eq 0) {
    Add-GateFailure -RuleId 'rule18' -Message 'the scan half executed zero checks'
} else {
    Write-Host "Scan half executed $totalChecksExecuted rule checks across $($walked.Count) file(s)."
}

if ($SelfTest) {
    $u7Path = Join-Path $e2eWindowsDir 'selftest\Invoke-U7SelfTests.ps1'
    $output = & powershell.exe -NoProfile -File $u7Path 2>&1
    $exitCode = $LASTEXITCODE
    $output | ForEach-Object { Write-Host $_ }
    $verdictLine = $output | Where-Object { $_ -match 'VERDICT passed=(\d+) failed=(\d+) total=(\d+)' } | Select-Object -Last 1
    if (-not $verdictLine) {
        Add-GateFailure -RuleId 'rule18' -Message '-SelfTest half: Invoke-U7SelfTests.ps1 produced no VERDICT line'
    } else {
        $null = $verdictLine -match 'VERDICT passed=(\d+) failed=(\d+) total=(\d+)'
        $total = [int]$Matches[3]
        if ($total -eq 0) {
            Add-GateFailure -RuleId 'rule18' -Message '-SelfTest half executed zero checks'
        } elseif ($exitCode -ne 0) {
            Add-GateFailure -RuleId 'rule18' -Message "-SelfTest half: Invoke-U7SelfTests.ps1 exited $exitCode"
        } else {
            Write-Host "-SelfTest half executed $total U7 primitive self-test case(s)."
        }
    }
}

if ($failed -ne 0) {
    Write-Host ("FAIL: contract violations in rule(s): " + ($failedRuleNames -join ', '))
    exit 1
}
Write-Host 'OK: the harness tree satisfies every structural contract rule.'
exit 0
