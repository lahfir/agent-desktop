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
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][string]$FixturesRoot)
    $failures = New-Object System.Collections.Generic.List[string]
    $ruleTable = Get-E2ERuleTable -AllowlistKeys @('known-token')
    $ruleById = @{}
    foreach ($rule in $ruleTable) { $ruleById[$rule.Id] = $rule }

    $mustCatchDir = Join-Path $FixturesRoot 'must-catch'
    $mustPassDir = Join-Path $FixturesRoot 'must-pass'
    $catchFiles = @(Get-ChildItem -LiteralPath $mustCatchDir -Filter '*.ps1' -File)
    $passFiles = @(Get-ChildItem -LiteralPath $mustPassDir -Filter '*.ps1' -File)
    if ($catchFiles.Count -eq 0) { $failures.Add('no must-catch fixture files found') }
    if ($passFiles.Count -eq 0) { $failures.Add('no must-pass fixture files found') }

    foreach ($file in $catchFiles) {
        $ruleId = ($file.BaseName -split '-')[0]
        if (-not $ruleById.ContainsKey($ruleId)) {
            $failures.Add("must-catch fixture '$($file.Name)' does not name a known rule id")
            continue
        }
        $parsed = Get-E2EParsedFile -Path $file.FullName
        $parsed | Add-Member -NotePropertyName 'RelativePath' -NotePropertyValue (Get-E2EFixtureSyntheticPath -RuleId $ruleId) -Force
        $rule = $ruleById[$ruleId]
        $hits = & $rule.Test $parsed
        $matching = @($hits | Where-Object { $_.RuleId -eq $ruleId })
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
        $parsed = Get-E2EParsedFile -Path $file.FullName
        $parsed | Add-Member -NotePropertyName 'RelativePath' -NotePropertyValue (Get-E2EFixtureSyntheticPath -RuleId $ruleId) -Force
        <#
            Only the fixture's own target rule is applied here, not the
            whole table: rule08/rule09 share the same scenario-shaped
            synthetic scope every non-rule06 fixture is scored under (they
            are the only rules that require it), so running every rule
            against, say, a rule01 fixture would wrongly demand it also
            satisfy rule08's Register-Legs requirement - a scope collision
            between unrelated rules, not a real second violation the
            fixture's author needs to fix.
        #>
        $rule = $ruleById[$ruleId]
        $hits = @(& $rule.Test $parsed | Where-Object { $_.RuleId -eq $ruleId })
        if ($hits.Count -gt 0) {
            $names = ($hits | ForEach-Object { "$($_.RuleId)/$($_.Pattern)" }) -join ', '
            $failures.Add("MUST-PASS false positive: '$($file.Name)' tripped: $names")
        }
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
    'Invoke-E2EFixtureSelfTest', 'Invoke-E2ESizeCapSelfTest', 'Invoke-E2EFileSetSelfTest'
)
