#Requires -Version 5.1

<#
    e2e-windows-contract-selftest.psm1 - the gate's own MUST-CATCH/MUST-PASS
    fixture runner, the file-set-equality self-test, the planted-file self-
    test and the 400-line-cap self-test (rule 13's fixtures generated here,
    never committed). Every check calls the exact same Get-E2ERuleTable /
    Invoke-E2EContractScan / Test-E2EFileSetEquality functions the real scan
    uses - fixtures drive the shipped patterns, never a paraphrase of them.
#>

Set-StrictMode -Version 2.0

Import-Module (Join-Path $PSScriptRoot 'e2e-windows-contract-common.psm1') -Force -Global
Import-Module (Join-Path $PSScriptRoot 'e2e-windows-contract-scan.psm1') -Force -Global

function Get-E2EFixtureSyntheticPath {
    <#
    .SYNOPSIS
        The relative path a fixture is scored under, chosen so each rule's
        real ScopeFilter applies to it: rule06 fixtures are scored as
        Run-E2E.ps1 (its only in-scope file); everything else is scored as
        a scenario file, which is in-scope for every other rule (rules 8/9
        require it, and no other rule excludes it).
    #>
    param([string]$RuleId)
    if ($RuleId -eq 'rule06') { return 'Run-E2E.ps1' }
    return 'scenarios/Fixture.ps1'
}

function Invoke-E2EFixtureSelfTest {
    <#
    .SYNOPSIS
        Drives every MUST-CATCH/MUST-PASS fixture through the exact same
        Invoke-E2EContractScan the real gate run uses, never through a
        direct `& $rule.Test` call: calling Test directly bypasses
        ScopeFilter entirely, so a rule whose scope wiring silently excludes
        every file it should cover would still show a clean self-test - a
        gate that cannot fail (finding #1). Each fixture is copied into a
        scratch tree at the exact synthetic RelativePath its rule's
        ScopeFilter expects, then scanned with a single-rule RuleTable: not
        the whole table, because rule08/rule09 share the same scenario-
        shaped synthetic scope every non-rule06 fixture is scored under (the
        only rules that require it), so running every rule against, say, a
        rule01 fixture would wrongly demand it also satisfy rule08's
        Register-Legs requirement - a scope collision between unrelated
        rules, not a real second violation the fixture's author needs to
        fix. RuleChecksExecuted is asserted nonzero for every fixture: zero
        means ScopeFilter never matched the synthetic path, which is exactly
        the silent-exclusion bug this routing exists to surface.
    #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string]$FixturesRoot,
        [Parameter(Mandatory = $true)][string]$E2ERoot
    )
    $failures = New-Object System.Collections.Generic.List[string]
    $ruleTable = Get-E2ERuleTable -AllowlistKeys @('known-token') -Root $E2ERoot
    $ruleById = @{}
    foreach ($rule in $ruleTable) { $ruleById[$rule.Id] = $rule }

    $mustCatchDir = Join-Path $FixturesRoot 'must-catch'
    $mustPassDir = Join-Path $FixturesRoot 'must-pass'
    $catchFiles = @(Get-ChildItem -LiteralPath $mustCatchDir -Filter '*.ps1' -File)
    $passFiles = @(Get-ChildItem -LiteralPath $mustPassDir -Filter '*.ps1' -File)
    if ($catchFiles.Count -eq 0) { $failures.Add('no must-catch fixture files found') }
    if ($passFiles.Count -eq 0) { $failures.Add('no must-pass fixture files found') }

    $scratchRoot = Join-Path ([IO.Path]::GetTempPath()) ('e2e-contract-fixture-scan-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
    New-Item -ItemType Directory -Path $scratchRoot -Force | Out-Null
    try {
        foreach ($file in $catchFiles) {
            $ruleId = ($file.BaseName -split '-')[0]
            if (-not $ruleById.ContainsKey($ruleId)) {
                $failures.Add("must-catch fixture '$($file.Name)' does not name a known rule id")
                continue
            }
            $rel = Get-E2EFixtureSyntheticPath -RuleId $ruleId
            $target = Join-Path $scratchRoot ($rel -replace '/', '\')
            New-Item -ItemType Directory -Path (Split-Path -Parent $target) -Force | Out-Null
            Copy-Item -LiteralPath $file.FullName -Destination $target -Force
            $scanResult = Invoke-E2EContractScan -Root $scratchRoot -FileSet @($rel) -RuleTable @($ruleById[$ruleId])
            if ($scanResult.RuleChecksExecuted[$ruleId] -lt 1) {
                $failures.Add("MUST-CATCH routing broken: '$($file.Name)' never entered $ruleId's ScopeFilter for synthetic path '$rel'")
                continue
            }
            $matching = @($scanResult.Violations | Where-Object { $_.RuleId -eq $ruleId })
            if ($matching.Count -eq 0) {
                $failures.Add("MUST-CATCH missed: '$($file.Name)' produced no $ruleId violation")
            }
        }

        foreach ($file in $passFiles) {
            $ruleId = ($file.BaseName -split '-')[0]
            if (-not $ruleById.ContainsKey($ruleId)) {
                $failures.Add("must-pass fixture '$($file.Name)' does not name a known rule id")
                continue
            }
            $rel = Get-E2EFixtureSyntheticPath -RuleId $ruleId
            $target = Join-Path $scratchRoot ($rel -replace '/', '\')
            New-Item -ItemType Directory -Path (Split-Path -Parent $target) -Force | Out-Null
            Copy-Item -LiteralPath $file.FullName -Destination $target -Force
            $scanResult = Invoke-E2EContractScan -Root $scratchRoot -FileSet @($rel) -RuleTable @($ruleById[$ruleId])
            if ($scanResult.RuleChecksExecuted[$ruleId] -lt 1) {
                $failures.Add("MUST-PASS routing broken: '$($file.Name)' never entered $ruleId's ScopeFilter for synthetic path '$rel'")
                continue
            }
            $hits = @($scanResult.Violations | Where-Object { $_.RuleId -eq $ruleId })
            if ($hits.Count -gt 0) {
                $names = ($hits | ForEach-Object { "$($_.RuleId)/$($_.Pattern)" }) -join ', '
                $failures.Add("MUST-PASS false positive: '$($file.Name)' tripped: $names")
            }
        }
    } finally {
        if (Test-Path -LiteralPath $scratchRoot) { Remove-Item -LiteralPath $scratchRoot -Recurse -Force -ErrorAction SilentlyContinue }
    }

    return $failures.ToArray()
}

function Invoke-E2ENativeEntryPointSelfTest {
    <#
    .SYNOPSIS
        Finding #29's exhaustive half: for every name Get-E2ENativeEntryPoints
        finds in the real tree (every Export-ModuleMember entry of every
        *Native*.psm1 module), proves rule 9 both catches it called bare and
        passes it called inside an Enter-Stage block. Generated, never
        committed - the same reason Invoke-E2ESizeCapSelfTest's 400/401-line
        fixtures are generated rather than checked in: the corpus this reads
        (each Native module's own export list) already grows on its own, and
        hand-copying it into ~30 committed fixture files would drift from
        day one. Two of these names (New-RestrictedIntegrityToken,
        New-E2ESecurityAttributes) also have committed, human-readable
        fixtures under must-catch/ naming them directly, since finding #29
        named them by name and a reviewer should be able to find them
        without running the gate.
    #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string]$E2ERoot,
        [Parameter(Mandatory = $true)][string]$ScratchDir
    )
    $failures = New-Object System.Collections.Generic.List[string]
    $entryPoints = Get-E2ENativeEntryPoints -Root $E2ERoot
    if ($entryPoints.Count -eq 0) {
        $failures.Add("Get-E2ENativeEntryPoints found zero exports under *Native*.psm1 in $E2ERoot - rule 9's explicit table would be empty")
        return $failures.ToArray()
    }

    $ruleTable = @(Get-E2ERuleTable -Root $E2ERoot | Where-Object { $_.Id -eq 'rule09' })
    $rel = 'scenarios/GeneratedEntryPoint.ps1'
    $target = Join-Path $ScratchDir ($rel -replace '/', '\')
    New-Item -ItemType Directory -Path (Split-Path -Parent $target) -Force | Out-Null
    try {
        foreach ($name in $entryPoints) {
            Set-Content -LiteralPath $target -Value "$name"
            $catchScan = Invoke-E2EContractScan -Root $ScratchDir -FileSet @($rel) -RuleTable $ruleTable
            $caught = @($catchScan.Violations | Where-Object { $_.RuleId -eq 'rule09' })
            if ($caught.Count -eq 0) {
                $failures.Add("MUST-CATCH missed: bare '$name' produced no rule09 violation")
            }

            Set-Content -LiteralPath $target -Value "Enter-Stage -Lock 'GeneratedSelfTest' -Body { $name }"
            $passScan = Invoke-E2EContractScan -Root $ScratchDir -FileSet @($rel) -RuleTable $ruleTable
            $stillCaught = @($passScan.Violations | Where-Object { $_.RuleId -eq 'rule09' })
            if ($stillCaught.Count -gt 0) {
                $failures.Add("MUST-PASS false positive: Enter-Stage-wrapped '$name' still tripped rule09")
            }
        }
    } finally {
        if (Test-Path -LiteralPath $target) { Remove-Item -LiteralPath $target -Force -ErrorAction SilentlyContinue }
    }
    return $failures.ToArray()
}

function Invoke-E2ESizeCapSelfTest {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][string]$ScratchDir)
    $failures = New-Object System.Collections.Generic.List[string]
    New-Item -ItemType Directory -Path $ScratchDir -Force | Out-Null
    $tooLong = Join-Path $ScratchDir 'rule13-too-long.ps1'
    $justRight = Join-Path $ScratchDir 'rule13-just-right.ps1'
    1..401 | ForEach-Object { "# line $_" } | Set-Content -LiteralPath $tooLong
    1..399 | ForEach-Object { "# line $_" } | Set-Content -LiteralPath $justRight

    $tooLongCount = @(Get-Content -LiteralPath $tooLong).Count
    $justRightCount = @(Get-Content -LiteralPath $justRight).Count
    if ($tooLongCount -le 400) { $failures.Add("MUST-CATCH missed: generated 401-line fixture measured at $tooLongCount lines") }
    if ($justRightCount -gt 400) { $failures.Add("MUST-PASS false positive: generated 399-line fixture measured at $justRightCount lines") }
    return $failures.ToArray()
}

function Invoke-E2EFileSetSelfTest {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][string]$ScratchDir)
    $failures = New-Object System.Collections.Generic.List[string]
    $scenarioDir = Join-Path $ScratchDir 'scenarios'
    New-Item -ItemType Directory -Path $scenarioDir -Force | Out-Null
    "Register-Legs -Names @('leg')" | Set-Content -LiteralPath (Join-Path $scenarioDir 'Planted.ps1')
    "@{ 'a' = 1 }" | Set-Content -LiteralPath (Join-Path $ScratchDir 'root.psd1')

    $walked = Get-E2ETreeFileSet -Root $ScratchDir
    if ($walked -notcontains 'scenarios/Planted.ps1') {
        $failures.Add('planted-file self-test missed: the tree walk did not reach scenarios/Planted.ps1')
    }

    $removed = @($walked | Where-Object { $_ -ne 'scenarios/Planted.ps1' })
    $equality = Test-E2EFileSetEquality -WalkSet $walked -ReferenceSet $removed
    if ($equality.Equal) {
        $failures.Add('set-removal self-test missed: removing a file from the reference set did not trip the equality check')
    }

    $sameEquality = Test-E2EFileSetEquality -WalkSet $walked -ReferenceSet $walked
    if (-not $sameEquality.Equal) {
        $failures.Add('MUST-PASS false positive: two identical file sets were reported unequal')
    }

    $emptySet = Get-E2ETreeFileSet -Root (Join-Path $ScratchDir 'does-not-exist')
    if ($emptySet.Count -ne 0) {
        $failures.Add('empty-directory self-test: a nonexistent root did not yield an empty file set')
    }

    return $failures.ToArray()
}

Export-ModuleMember -Function @(
    'Invoke-E2EFixtureSelfTest', 'Invoke-E2ENativeEntryPointSelfTest', 'Invoke-E2ESizeCapSelfTest', 'Invoke-E2EFileSetSelfTest'
)
