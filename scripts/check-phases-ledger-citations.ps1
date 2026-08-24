#Requires -Version 5.1
<#
.SYNOPSIS
    Fail when docs/phases.md and probes/windows/FINDINGS.md drift apart, or
    when a corrected sub-phase 2.12 statement's retired wording reappears.
    Self-tests MUST-CATCH / MUST-PASS against the same Test-PhasesLedgerCitations
    function the real run calls, plus a non-empty-input assertion so an empty
    or unparsed input cannot pass vacuously.

.DESCRIPTION
    Three rules:
      (a) every `A<n>-<m>` id cited in docs/phases.md must exist as a row id
          in probes/windows/FINDINGS.md.
      (b) every FINDINGS.md row whose action cell names `closure: 2.12` must
          have its row id cited somewhere in docs/phases.md - a row assigned
          to this sub-phase that the document never mentions is an
          undispositioned deferral.
      (c) none of the retired stems below - phrases sub-phase 2.12's own
          plan corrected out of docs/phases.md - may reappear anywhere in the
          file. Matching is case-insensitive substring over the WHOLE file,
          not scoped to any one section: a retired phrase can resurface in a
          different section with slightly different wording around it, which
          a section-scoped or whole-phrase match would walk past.
#>
[CmdletBinding()]
param(
    [string]$RepoRoot = '',
    [string]$PhasesPath = '',
    [string]$FindingsPath = ''
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

if ([string]::IsNullOrWhiteSpace($RepoRoot)) {
    $RepoRoot = Split-Path -Parent (Split-Path -Parent $PSCommandPath)
}
$RepoRoot = (Resolve-Path -LiteralPath $RepoRoot).ProviderPath

if ([string]::IsNullOrWhiteSpace($PhasesPath)) {
    $PhasesPath = Join-Path $RepoRoot 'docs\phases.md'
}
if ([string]::IsNullOrWhiteSpace($FindingsPath)) {
    $FindingsPath = Join-Path $RepoRoot 'probes\windows\FINDINGS.md'
}

# The minimum retired-stem list docs/phases.md's own correction pass
# names. A stem is kept short enough to catch a variant elsewhere in the
# document that restates the same corrected fact in different words (see
# the case-insensitive-whole-file note above) - each was chosen against a
# specific line this sub-phase's plan corrected in place.
$RetiredStems = @(
    'no probe has established',
    'mirroring `AgentDeskFixture.swift`',
    'A wrap-handling rule if the measurement calls for one',
    'workflow_dispatch`-triggered only',
    'workflow_dispatch`-only triggering',
    'the split-integrity rig this sub-phase registers',
    'is registered at 2.12',
    'where that runner is registered',
    'contended re-measure owned by §2.12',
    '§2.12 owns the `focused_window`'
)

function Get-CitedProbeIds {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Text)
    $ids = New-Object System.Collections.Generic.HashSet[string]
    foreach ($m in [regex]::Matches($Text, '(?<![A-Za-z0-9])A[0-9]+-[0-9]+(?![A-Za-z0-9])')) {
        [void]$ids.Add($m.Value)
    }
    return ($ids.GetEnumerator() | Sort-Object)
}

function Get-FindingsRows {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Text)
    $rows = New-Object System.Collections.Generic.List[object]
    foreach ($line in ($Text -split "`r?`n")) {
        $m = [regex]::Match($line, '^\|\s*(A[0-9]+-[0-9]+)\s*\|')
        if (-not $m.Success) { continue }
        $rows.Add([pscustomobject]@{
                Id                  = $m.Groups[1].Value
                NamesClosureAt212   = [regex]::IsMatch($line, 'closure:\s*2\.12\b', 'IgnoreCase')
            })
    }
    return $rows.ToArray()
}

function Test-PhasesLedgerCitations {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$PhasesText,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$FindingsText,
        [Parameter(Mandatory = $true)][string[]]$RetiredStems
    )
    $failures = New-Object System.Collections.Generic.List[string]

    $citedIds = @(Get-CitedProbeIds -Text $PhasesText)
    $findingsRows = @(Get-FindingsRows -Text $FindingsText)
    $findingsIds = @{}
    foreach ($row in $findingsRows) { $findingsIds[$row.Id] = $true }

    # The tautology trap this gate must not fall into: an empty or
    # unparsed phases.md (a bad path, a truncated read, a regex that stopped
    # matching after an unrelated markdown edit) would cite zero ids and
    # therefore find zero rule-(a) violations by construction, reading as a
    # clean pass. Zero cited ids is refused outright rather than trusted.
    if ($citedIds.Count -eq 0) {
        $failures.Add('non-empty-input assertion: zero A<n>-<m> ids parsed from the phases text - the input is empty, unreadable, or the citation pattern stopped matching') | Out-Null
    }
    if ($findingsRows.Count -eq 0) {
        $failures.Add('non-empty-input assertion: zero ledger rows parsed from the findings text - the input is empty, unreadable, or the row pattern stopped matching') | Out-Null
    }

    # Rule (a): every cited id must resolve to a real ledger row.
    foreach ($id in $citedIds) {
        if (-not $findingsIds.ContainsKey($id)) {
            $failures.Add('rule (a): docs/phases.md cites ' + $id + ', which does not exist as a row id in probes/windows/FINDINGS.md') | Out-Null
        }
    }

    # Rule (b): every row this sub-phase claims (closure: 2.12) must be
    # disposed of in the document, not merely recorded in the ledger.
    foreach ($row in $findingsRows) {
        if (-not $row.NamesClosureAt212) { continue }
        $citedSet = @{}
        foreach ($id in $citedIds) { $citedSet[$id] = $true }
        if (-not $citedSet.ContainsKey($row.Id)) {
            $failures.Add('rule (b): FINDINGS.md row ' + $row.Id + ' names closure: 2.12 but is never cited in docs/phases.md - an undispositioned deferral') | Out-Null
        }
    }

    # Rule (c): no retired stem may reappear, anywhere in the file.
    foreach ($stem in $RetiredStems) {
        if ($PhasesText.IndexOf($stem, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
            $failures.Add("rule (c): retired stem still present in docs/phases.md: '" + $stem + "'") | Out-Null
        }
    }

    return [pscustomobject]@{
        Failures      = $failures.ToArray()
        CitedIdCount  = $citedIds.Count
        FindingsCount = $findingsRows.Count
    }
}

function Invoke-PhasesLedgerCitationsSelfTest {
    param([Parameter(Mandatory = $true)][string[]]$RetiredStems)
    $failures = New-Object System.Collections.Generic.List[string]

    # --- Non-empty-input trap: empty phases text must fail even though it
    # trivially contains zero violations of rules (a) and (c). ---
    $emptyInput = Test-PhasesLedgerCitations -PhasesText '' -FindingsText '| A1-1 | s | managed | api-contract | e | o | CONFIRMS | closure: 2.12 |' -RetiredStems $RetiredStems
    if ($emptyInput.Failures.Count -lt 1) {
        $failures.Add('MUST CATCH, missed: empty phases text passed instead of tripping the non-empty-input assertion') | Out-Null
    }

    # --- Rule (a) MUST CATCH: a cited id absent from the ledger. ---
    $ruleAText = 'This sentence cites `A99-1`, a probe row that does not exist.'
    $ruleAFindings = '| A1-1 | script | managed | api-contract | expectation | observed | CONFIRMS | action closure: 2.15 |'
    $ruleA = Test-PhasesLedgerCitations -PhasesText $ruleAText -FindingsText $ruleAFindings -RetiredStems $RetiredStems
    if (-not ($ruleA.Failures | Where-Object { $_ -match '^rule \(a\)' })) {
        $failures.Add('MUST CATCH, missed: a cited id absent from FINDINGS.md did not fail rule (a)') | Out-Null
    }

    # --- Rule (a) MUST PASS: every cited id resolves. ---
    $ruleAPassText = 'Grounded by `A1-1` and `A1-1` again, plus prose with no other ids.'
    $ruleAPass = Test-PhasesLedgerCitations -PhasesText $ruleAPassText -FindingsText $ruleAFindings -RetiredStems $RetiredStems
    if ($ruleAPass.Failures | Where-Object { $_ -match '^rule \(a\)' }) {
        $failures.Add('MUST PASS, false positive: a fully-resolved citation set failed rule (a)') | Out-Null
    }

    # --- Rule (b) MUST CATCH: a closure:2.12 ledger row never cited. ---
    $ruleBFindings = '| A5-3 | script | managed | api-contract | expectation | observed | DEFERRED | still open, closure: 2.12 |'
    $ruleBText = 'This document cites only `A1-1`, for something unrelated, and names the other row nowhere.'
    $ruleB = Test-PhasesLedgerCitations -PhasesText $ruleBText -FindingsText $ruleBFindings -RetiredStems $RetiredStems
    if (-not ($ruleB.Failures | Where-Object { $_ -match '^rule \(b\)' })) {
        $failures.Add('MUST CATCH, missed: an uncited closure:2.12 row did not fail rule (b)') | Out-Null
    }

    # --- Rule (b) MUST PASS: the closure:2.12 row is cited. ---
    $ruleBPassText = 'This sub-phase measured it, citing `A5-3` directly.'
    $ruleBPass = Test-PhasesLedgerCitations -PhasesText $ruleBPassText -FindingsText $ruleBFindings -RetiredStems $RetiredStems
    if ($ruleBPass.Failures | Where-Object { $_ -match '^rule \(b\)' }) {
        $failures.Add('MUST PASS, false positive: a cited closure:2.12 row failed rule (b)') | Out-Null
    }

    # --- Rule (c): one MUST-CATCH and one MUST-PASS fixture per stem, so the
    # gate is shown not to fire on the corrected wording, not merely to fire
    # on the retired one. ---
    $baseFindings = '| A1-1 | script | managed | api-contract | expectation | observed | CONFIRMS | action closure: 2.15 |'
    $correctedByStem = @{
        'no probe has established' = 'A24-8 has since measured the HWND uniqueness-counter wrap rate under real churn.'
        'mirroring `AgentDeskFixture.swift`' = 'Fixture targets mirror `AgentDeskFixture.swift` for four of five classes.'
        'A wrap-handling rule if the measurement calls for one' = 'A wrap-handling rule ships beside the field.'
        'workflow_dispatch`-triggered only' = 'triggered only by `workflow_dispatch` and by `push` to feat/windows-adapter'
        'workflow_dispatch`-only triggering' = 'the runner`s workflow_dispatch-and-push triggering is written down'
        'the split-integrity rig this sub-phase registers' = 'the A24-4-staged Medium observer used above'
        'is registered at 2.12' = 'is registered at 2.15'
        'where that runner is registered' = 'sub-phase 2.15 now owns registering it'
        'contended re-measure owned by §2.12' = '§2.12 re-measured it contended and recorded the result as a probe row beside it'
        '§2.12 owns the `focused_window`' = 'the focused_window identity question rides §2.14 rather than §2.12'
    }
    foreach ($stem in $RetiredStems) {
        $catchText = 'Some surrounding prose. ' + $stem + ' More surrounding prose.'
        $catch = Test-PhasesLedgerCitations -PhasesText $catchText -FindingsText $baseFindings -RetiredStems $RetiredStems
        if (-not ($catch.Failures | Where-Object { $_ -match '^rule \(c\)' -and $_.Contains($stem) })) {
            $failures.Add("MUST CATCH, missed: retired stem '" + $stem + "' did not fail rule (c)") | Out-Null
        }

        if (-not $correctedByStem.ContainsKey($stem)) {
            $failures.Add("fixture gap: no MUST-PASS corrected-wording fixture declared for stem '" + $stem + "'") | Out-Null
            continue
        }
        $passText = 'Some surrounding prose. ' + $correctedByStem[$stem] + ' More surrounding prose.'
        $pass = Test-PhasesLedgerCitations -PhasesText $passText -FindingsText $baseFindings -RetiredStems $RetiredStems
        if ($pass.Failures | Where-Object { $_ -match '^rule \(c\)' }) {
            $failures.Add("MUST PASS, false positive: the corrected wording for stem '" + $stem + "' still failed rule (c)") | Out-Null
        }
    }

    # --- Full clean-document MUST PASS: a realistic small document with no
    # violations of any rule passes with zero failures. ---
    $cleanFindings = @(
        '| A1-1 | s | managed | api-contract | e | o | CONFIRMS | action closure: 2.15 |',
        '| A2-4 | s | managed | api-contract | e | o | DEFERRED | still open, closure: 2.12 |'
    ) -join "`n"
    $cleanPhases = 'This document cites `A1-1` for one fact and `A2-4` for another, and retires no stems.'
    $clean = Test-PhasesLedgerCitations -PhasesText $cleanPhases -FindingsText $cleanFindings -RetiredStems $RetiredStems
    if ($clean.Failures.Count -gt 0) {
        $failures.Add('MUST PASS, false positive: a clean document with no violations failed (' + ($clean.Failures -join '; ') + ')') | Out-Null
    }

    return [pscustomobject]@{ Failures = $failures.ToArray() }
}

$selfTest = Invoke-PhasesLedgerCitationsSelfTest -RetiredStems $RetiredStems
foreach ($f in $selfTest.Failures) {
    Write-Host ('self-test FAIL: ' + $f)
}
if ($selfTest.Failures.Count -gt 0) {
    Write-Host 'FAIL: phases-ledger-citations gate failed its own self-test'
    exit 1
}
Write-Host 'OK: phases-ledger-citations gate self-test passed'

if (-not (Test-Path -LiteralPath $PhasesPath)) {
    Write-Host ('FAIL: missing ' + $PhasesPath)
    exit 1
}
if (-not (Test-Path -LiteralPath $FindingsPath)) {
    Write-Host ('FAIL: missing ' + $FindingsPath)
    exit 1
}

$phasesText = [IO.File]::ReadAllText($PhasesPath)
$findingsText = [IO.File]::ReadAllText($FindingsPath)

$real = Test-PhasesLedgerCitations -PhasesText $phasesText -FindingsText $findingsText -RetiredStems $RetiredStems
foreach ($f in $real.Failures) {
    Write-Host ('FAIL: ' + $f)
}
if ($real.Failures.Count -gt 0) {
    Write-Host ('FAIL: ' + $real.Failures.Count + ' citation/ledger failure(s) (cited ' + $real.CitedIdCount + ' id(s), ' + $real.FindingsCount + ' ledger row(s))')
    exit 1
}
Write-Host ('OK: every cited id resolves, every closure:2.12 row is cited, and no retired stem reappears (cited ' + $real.CitedIdCount + ' id(s), ' + $real.FindingsCount + ' ledger row(s))')
exit 0
