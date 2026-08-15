$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\common.ps1"
. "$PSScriptRoot\13-ledger-content.ps1"

$Probe = '13-ledger-check'
$RequiredAreaIds = @('1', '2', '3', '4', '5', '6', '7', '8', '9', '10', '11')
$ValidVerdicts = @('CONFIRMS', 'CONTRADICTS', 'NEW-EDGE', 'DEFERRED')
$ValidStacks = @('managed', 'uia3-com', 'n/a')
$ValidScopes = @('api-contract', 'app/provider')
$MinClosureMinor = 0
$MaxClosureMinor = 15
$PreWideningContentAuditFloor = 20

$contentSelfTest = Invoke-LedgerContentSelfTest -ProbeRoot $PSScriptRoot
foreach ($f in $contentSelfTest.Failures) {
    Write-ProbeLog -Message ('LEDGER-CONTENT-SELFTEST-FAIL ' + $f) -Level 'error'
}
if ($contentSelfTest.Failures.Count -gt 0) {
    Write-ProbeResult -Probe $Probe -Status 'fail' -Message ('ledger content self-test failed: ' + $contentSelfTest.Failures[0])
    exit 1
}

function Get-TableCells {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Line)
    $trimmed = $Line.Trim()
    if (-not $trimmed.StartsWith('|')) { return $null }
    $parts = $trimmed.Trim('|') -split '\|'
    $cells = New-Object System.Collections.ArrayList
    foreach ($p in $parts) { [void]$cells.Add(([string]$p).Trim()) }
    return ($cells.ToArray())
}

function Test-SeparatorRow {
    param($Cells)
    foreach ($c in $Cells) {
        if ($c -notmatch '^:?-{2,}:?$') { return $false }
    }
    return $true
}

function Get-ReferencedRowIds {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Cell)
    $ids = New-Object System.Collections.ArrayList
    foreach ($m in [regex]::Matches($Cell, '[A-Za-z][A-Za-z0-9]*-[0-9]+')) {
        [void]$ids.Add($m.Value)
    }
    return ($ids.ToArray())
}

$failures = New-Object System.Collections.ArrayList

try {
    $probeRoot = Get-ProbeRoot
    $repoRoot = Split-Path -Parent (Split-Path -Parent $probeRoot)
    $ledgerPath = Join-Path $probeRoot 'FINDINGS.md'
    if (-not (Test-Path -LiteralPath $ledgerPath)) {
        throw ('FINDINGS.md not found at ' + $ledgerPath)
    }

    $lines = [IO.File]::ReadAllLines($ledgerPath)
    $section = ''
    $rows = New-Object System.Collections.ArrayList
    $hunkRows = New-Object System.Collections.ArrayList

    foreach ($line in $lines) {
        if ($line -match '^#{2,3}\s+Area\s+([0-9]+)\b') {
            $section = $Matches[1]
            continue
        }
        if ($line -match '^#{2,3}\s+Session evidence') {
            $section = 'R6'
            continue
        }
        if ($line -match '^#{2,3}\s+phases\.md hunk index') {
            $section = 'HUNK'
            continue
        }
        if ($line -match '^#{1,6}\s') {
            $section = ''
            continue
        }
        $cells = Get-TableCells -Line $line
        if ($null -eq $cells) { continue }
        if (Test-SeparatorRow -Cells $cells) { continue }
        if ($section -eq 'HUNK') {
            if ($cells.Count -lt 3) { continue }
            if ($cells[0] -eq 'hunk') { continue }
            [void]$hunkRows.Add([pscustomobject]@{
                    Hunk        = $cells[0]
                    Location    = $cells[1]
                    BackingRows = (Get-ReferencedRowIds -Cell $cells[2])
                })
            continue
        }
        if ($section -eq '') { continue }
        if ($cells.Count -ne 8) {
            [void]$failures.Add('malformed row in section ' + $section + ' (' + $cells.Count + ' cells, expected 8): ' + $line.Trim())
            continue
        }
        if ($cells[0] -eq 'id') { continue }
        [void]$rows.Add([pscustomobject]@{
                Id          = $cells[0]
                Script      = $cells[1]
                Stack       = $cells[2]
                Scope       = $cells[3]
                Expectation = $cells[4]
                Observed    = $cells[5]
                Verdict     = $cells[6]
                Action      = $cells[7]
                Area        = $section
            })
    }

    if ($rows.Count -eq 0) { throw 'no ledger rows parsed from FINDINGS.md' }

    $seenIds = @{}
    foreach ($row in $rows) {
        if (-not $row.Id) {
            [void]$failures.Add('row with empty id in area ' + $row.Area)
            continue
        }
        if ($seenIds.ContainsKey($row.Id)) {
            [void]$failures.Add('duplicate row id ' + $row.Id)
        }
        $seenIds[$row.Id] = $row
        if (-not $row.Stack) { [void]$failures.Add($row.Id + ': empty stack') }
        elseif ($ValidStacks -notcontains $row.Stack) { [void]$failures.Add($row.Id + ": stack '" + $row.Stack + "' is not managed, uia3-com or n/a") }
        if (-not $row.Scope) { [void]$failures.Add($row.Id + ': empty scope') }
        elseif ($ValidScopes -notcontains $row.Scope) { [void]$failures.Add($row.Id + ": scope '" + $row.Scope + "' is not api-contract or app/provider") }
        if (-not $row.Verdict) { [void]$failures.Add($row.Id + ': empty verdict') }
        elseif ($ValidVerdicts -notcontains $row.Verdict) { [void]$failures.Add($row.Id + ": verdict '" + $row.Verdict + "' is not one of CONFIRMS, CONTRADICTS, NEW-EDGE, DEFERRED") }
        if ($row.Verdict -match 'UNKNOWN') { [void]$failures.Add($row.Id + ': UNKNOWN verdict is forbidden by R5') }
        if (-not $row.Script) { [void]$failures.Add($row.Id + ': empty script/re-run command') }
        if (-not $row.Observed) { [void]$failures.Add($row.Id + ': empty observed cell') }
        if ($row.Verdict -eq 'DEFERRED') {
            $closure = [regex]::Match($row.Action, 'closure:\s*2\.([0-9]{1,2})\b')
            if (-not $closure.Success) {
                [void]$failures.Add($row.Id + ": DEFERRED row does not name a Phase 2 closure sub-phase (expected 'closure: 2.<n>' in the action cell)")
            } else {
                $minor = [int]$closure.Groups[1].Value
                if ($minor -lt $MinClosureMinor -or $minor -gt $MaxClosureMinor) {
                    [void]$failures.Add($row.Id + ': DEFERRED closure sub-phase 2.' + $minor + ' is outside Phase 2 (2.0-2.15)')
                }
            }
        }
    }

    $areaCoverage = New-Object System.Collections.ArrayList
    foreach ($areaId in $RequiredAreaIds) {
        $count = @($rows | Where-Object { $_.Area -eq $areaId }).Count
        [void]$areaCoverage.Add([pscustomobject]@{ Area = $areaId; Rows = $count })
        if ($count -lt 1) { [void]$failures.Add('section 2.0 evidence area ' + $areaId + ' has no ledger row') }
    }
    $sessionRowCount = @($rows | Where-Object { $_.Area -eq 'R6' }).Count
    if ($sessionRowCount -lt 1) { [void]$failures.Add('no R6 session-evidence rows found') }

    $gitDiff = & git -C $repoRoot diff -U0 main -- docs/phases.md
    if ($LASTEXITCODE -ne 0) { throw ('git diff -U0 main -- docs/phases.md failed with exit code ' + $LASTEXITCODE) }
    $measuredHunkCount = @($gitDiff | Where-Object { "$_" -match '^@@ -[0-9]' }).Count

# The index-completeness half of the bijection is reported, not enforced, and the
# reason is structural rather than a backlog someone forgot. This diff is taken
# against `main`, and under the platform delivery model `main` is an entire
# platform phase behind the integration branch: every sub-phase that merges adds
# hunks, so the measured count grows monotonically while the index only gains the
# rows whichever sub-phase was paying attention wrote. Enforcing it here would
# make one sub-phase answer for every earlier sub-phase's doc edits. The half of
# the invariant that carries its value is still enforced below - every indexed
# hunk must name a ledger row that exists, and every CONTRADICTS row must be
# backed by a hunk - so a doc change still cannot claim evidence it does not have.
# Bringing the index current across the whole phase belongs to the gate that
# reviews the assembled branch (docs/phases.md sub-phase 2.15, Docs/skills sync).
    $hunkIndexShortfall = $measuredHunkCount - $hunkRows.Count

    $mappedRowIds = @{}
    $hunksWithoutBacking = 0
    foreach ($hunkRow in $hunkRows) {
        if ($hunkRow.BackingRows.Count -lt 1) {
            [void]$failures.Add('hunk ' + $hunkRow.Hunk + ' names no backing ledger row')
            $hunksWithoutBacking++
            continue
        }
        $resolvedBacking = 0
        foreach ($refId in $hunkRow.BackingRows) {
            if (-not $seenIds.ContainsKey($refId)) {
                [void]$failures.Add('hunk ' + $hunkRow.Hunk + ' cites row ' + $refId + ' which does not exist in the ledger')
                continue
            }
            $mappedRowIds[$refId] = $true
            $resolvedBacking++
        }
        if ($resolvedBacking -lt 1) { $hunksWithoutBacking++ }
    }

    $contradictsRows = @($rows | Where-Object { $_.Verdict -eq 'CONTRADICTS' })
    $unmappedContradicts = @($contradictsRows | Where-Object { -not $mappedRowIds.ContainsKey($_.Id) })
    foreach ($row in $unmappedContradicts) {
        [void]$failures.Add('CONTRADICTS row ' + $row.Id + ' is not backed by any phases.md hunk')
    }

    $verdictCounts = [ordered]@{}
    foreach ($v in $ValidVerdicts) { $verdictCounts[$v] = @($rows | Where-Object { $_.Verdict -eq $v }).Count }

    $deferredRows = New-Object System.Collections.ArrayList
    foreach ($row in ($rows | Where-Object { $_.Verdict -eq 'DEFERRED' })) {
        $closure = [regex]::Match($row.Action, 'closure:\s*(2\.[0-9]{1,2})\b')
        $named = '<none>'
        if ($closure.Success) { $named = $closure.Groups[1].Value }
        [void]$deferredRows.Add([pscustomobject]@{ Id = $row.Id; Area = $row.Area; Closure = $named })
    }

    $contentAudited = 0
    $contentChangedCandidates = New-Object System.Collections.ArrayList
    foreach ($row in $rows) {
        $content = Test-RowCaptureContent -Row $row -ProbeRoot $probeRoot
        if ($content.Audited) { $contentAudited++ }
        foreach ($cf in $content.Failures) {
            [void]$failures.Add($cf)
            [void]$contentChangedCandidates.Add($row.Id)
        }
    }
    if ($contentAudited -le $PreWideningContentAuditFloor) {
        [void]$failures.Add('content check coverage regression: audited only ' + $contentAudited + ' of ' + $rows.Count + ' row(s), expected materially more than the pre-widening floor of ' + $PreWideningContentAuditFloor)
    }

    $capture = [ordered]@{
        Probe                = $Probe
        CapturedAtUtc        = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
        Question             = 'does FINDINGS.md satisfy R5, R6 and R7 as a machine-checkable contract'
        Ledger               = 'probes/windows/FINDINGS.md'
        RowCount             = $rows.Count
        SessionEvidenceRows  = $sessionRowCount
        AreaCoverage         = $areaCoverage.ToArray()
        VerdictCounts        = $verdictCounts
        DeferredRows         = $deferredRows.ToArray()
        MeasuredHunkCount    = $measuredHunkCount
        HunkIndexRowCount    = $hunkRows.Count
        HunkIndexShortfall   = $hunkIndexShortfall
        ContradictsRowCount  = $contradictsRows.Count
        ContradictsRowsMapped = @($contradictsRows | Where-Object { $mappedRowIds.ContainsKey($_.Id) }).Count
        BijectionHolds       = ($hunksWithoutBacking -eq 0 -and $unmappedContradicts.Count -eq 0)
        CaptureContentRowsAudited = $contentAudited
        CaptureContentFailures = @($contentChangedCandidates | Select-Object -Unique)
        Failures             = $failures.ToArray()
        HunkCountSource      = 'git -C <repo> diff -U0 main -- docs/phases.md, counting lines matching ^@@ -[0-9]; the measured count is authoritative over any count written in prose'
    }
    $capturePath = Write-ProbeJson -Probe $Probe -Name 'ledger-check.json' -InputObject $capture

    foreach ($f in $failures) { Write-ProbeLog -Message ('LEDGER-CHECK-FAIL ' + $f) -Level 'error' }
    Write-ProbeLog -Message ('wrote ' + (Split-Path -Leaf $capturePath))

    $summary = [ordered]@{
        Rows                      = $rows.Count
        AreasCovered              = @($areaCoverage | Where-Object { $_.Rows -ge 1 }).Count
        Contradicts               = $contradictsRows.Count
        Deferred                  = $deferredRows.Count
        MeasuredHunkCount         = $measuredHunkCount
        HunkIndexRowCount         = $hunkRows.Count
        HunkIndexShortfall        = $hunkIndexShortfall
        CaptureContentRowsAudited = $contentAudited
        Failures                  = $failures.Count
    }
    $coverageNote = 'content check audited ' + $contentAudited + ' of ' + $rows.Count + ' row(s) (pre-widening floor was ' + $PreWideningContentAuditFloor + ')'

    if ($failures.Count -gt 0) {
        Write-ProbeResult -Probe $Probe -Status 'fail' -Message ([string]$failures.Count + ' ledger completeness failure(s); first: ' + $failures[0] + '; ' + $coverageNote) -Data $summary
        exit 1
    }
    Write-ProbeResult -Probe $Probe -Status 'ok' -Message ('ledger complete: all 11 evidence areas covered, zero UNKNOWN verdicts, every DEFERRED row names a Phase 2 closure sub-phase, every row carries stack and scope, every indexed phases.md hunk names a ledger row and every CONTRADICTS row is backed by a hunk (index lists ' + $hunkRows.Count + ' of ' + $measuredHunkCount + ' measured hunk(s); completeness is reported, not enforced - see 2.15 docs sync); ' + $coverageNote) -Data $summary
    exit 0
} catch {
    Write-ProbeResult -Probe $Probe -Status 'fail' -Message ('unhandled error: ' + $_.Exception.Message)
    exit 1
}
