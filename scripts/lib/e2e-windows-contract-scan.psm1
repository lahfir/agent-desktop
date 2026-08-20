#Requires -Version 5.1

<#
    e2e-windows-contract-scan.psm1 - the rule table (declared once, per
    plan item 14) and the tree-walk/scan primitives both the real gate run
    and the self-test call. Neither the tree scan nor the self-test paraphrases
    the other's rule set - both call Get-E2ERuleTable and Invoke-E2EContractScan.
#>

Set-StrictMode -Version 2.0

Import-Module (Join-Path $PSScriptRoot 'e2e-windows-contract-common.psm1') -Force -Global
Import-Module (Join-Path $PSScriptRoot 'e2e-windows-contract-rules-process.psm1') -Force -Global
Import-Module (Join-Path $PSScriptRoot 'e2e-windows-contract-rules-ref.psm1') -Force -Global
Import-Module (Join-Path $PSScriptRoot 'e2e-windows-contract-rules-misc.psm1') -Force -Global

function ConvertTo-E2ERelativePath {
    param([Parameter(Mandatory = $true)][string]$Root, [Parameter(Mandatory = $true)][string]$Path)
    $full = (Resolve-Path -LiteralPath $Path).ProviderPath
    $rootFull = (Resolve-Path -LiteralPath $Root).ProviderPath
    $rel = $full.Substring($rootFull.Length).TrimStart('\', '/')
    return ($rel -replace '\\', '/')
}

function Get-E2ETreeFileSet {
    <#
    .SYNOPSIS
        Recursive walk of Root for *.ps1/*.psm1/*.psd1, returned as
        Root-relative forward-slash paths - the same shape the real scan and
        the self-test both compare against a git-ls-files-derived set.
    #>
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][string]$Root)
    <#
        `return ,@(...)` throughout: `return @()` enumerates the empty
        array onto the output stream, which writes zero objects - a caller
        capturing `$x = Get-E2ETreeFileSet ...` then gets $null, not an
        empty array, and `$null.Count` throws under Set-StrictMode -Version
        2.0 (measured while building this check). The unary comma wraps the
        array as a single pipeline object so it survives the round trip at
        every count, including zero.
    #>
    if (-not (Test-Path -LiteralPath $Root)) { return , @() }
    <#
        -Include is filtered with a plain Where-Object, never trusted on its
        own: Windows PowerShell 5.1's Get-ChildItem -Recurse -Include only
        filters correctly when -Path itself carries the trailing wildcard
        (`$Root\*`) - passed a bare directory via -LiteralPath, -Include is
        silently ignored and every file recurses through unfiltered
        (measured while building this check: README.md came back from a
        walk that named only *.ps1/*.psm1/*.psd1).
    #>
    $extensions = @('.ps1', '.psm1', '.psd1')
    $files = Get-ChildItem -LiteralPath $Root -Recurse -File | Where-Object { $extensions -icontains $_.Extension }
    return , @($files | ForEach-Object { ConvertTo-E2ERelativePath -Root $Root -Path $_.FullName } | Sort-Object)
}

function Test-E2EFileSetEquality {
    <#
    .SYNOPSIS
        Pure comparison the real scan and the self-test both call: returns
        the symmetric difference between WalkSet and ReferenceSet, empty
        when they agree.
    #>
    [CmdletBinding()]
    param([string[]]$WalkSet = @(), [string[]]$ReferenceSet = @())
    <#
        New-Object -ArgumentList unrolls an array into one constructor
        argument per element rather than passing it as a single IEnumerable
        - "Cannot find an overload... argument count: N" for any N-element
        set, measured while building this check. Building an empty set and
        calling .UnionWith takes the array as one IEnumerable argument, the
        way the HashSet(IEnumerable<T>) constructor actually expects it.
    #>
    $walk = New-Object System.Collections.Generic.HashSet[string]
    $walk.UnionWith([string[]]$WalkSet)
    $reference = New-Object System.Collections.Generic.HashSet[string]
    $reference.UnionWith([string[]]$ReferenceSet)
    $onlyInWalk = @($walk | Where-Object { -not $reference.Contains($_) })
    $onlyInReference = @($reference | Where-Object { -not $walk.Contains($_) })
    return [pscustomobject]@{ OnlyInWalk = $onlyInWalk; OnlyInReference = $onlyInReference; Equal = ($onlyInWalk.Count -eq 0 -and $onlyInReference.Count -eq 0) }
}

function Get-E2ERuleTable {
    <#
    .SYNOPSIS
        The thirteen structural rules, declared once. AllowlistKeys backs
        rule 8's skip-token check; Root, when given, is walked for every
        *Native*.psm1 module's real Export-ModuleMember list and backs rule
        9's explicit entry-point table (finding #29) - omitted, rule 9 falls
        back to its function-name-convention backstop alone. ScopeFilter
        decides which relative paths a rule applies to, so scenario-only and
        selftest-excluded rules state their scope in one place rather than
        in caller conditionals.
    #>
    [CmdletBinding()]
    param([string[]]$AllowlistKeys = @(), [string]$Root = '')
    <#
        A module-scoped variable, never a dynamically-built scriptblock: a
        literal `{...}` defined inside this function is bound to this
        module's session state (so Test-Rule08ScenarioLegs, imported at the
        top of this file, resolves when the caller later runs `& $rule.Test`
        from outside this module) - measured while building this gate.
        [scriptblock]::Create(...) and .GetNewClosure() both produce a
        scriptblock bound to the *caller's* session state instead, so
        Test-Rule08ScenarioLegs was "not recognized" the moment the rule
        table crossed a module boundary; a literal scriptblock closing over
        a $script: variable has no such boundary to cross.
    #>
    $script:Rule08AllowlistKeys = $AllowlistKeys
    $script:Rule09NativeEntryPoints = if ($Root) { Get-E2ENativeEntryPoints -Root $Root } else { @() }
    $notSelftest = { param($rel) $rel -notmatch '^selftest/' }
    return @(
        [pscustomobject]@{ Id = 'rule01'; Name = 'one process-terminating call, in Run-E2E.ps1'; ScopeFilter = $notSelftest; Test = { param($p) Test-Rule01ExitCalls -Parsed $p } }
        [pscustomobject]@{ Id = 'rule02'; Name = 'no Remove-Item against a suite artifact'; ScopeFilter = $notSelftest; Test = { param($p) Test-Rule02RemoveItem -Parsed $p } }
        [pscustomobject]@{ Id = 'rule03'; Name = 'no python/bash/wsl/sh invocation'; ScopeFilter = { $true }; Test = { param($p) Test-Rule03ForbiddenInterpreter -Parsed $p } }
        [pscustomobject]@{ Id = 'rule04'; Name = 'no bare-ref helper'; ScopeFilter = $notSelftest; Test = { param($p) Test-Rule04BareRef -Parsed $p } }
        <#
            LibEnvelope.psm1 is U10's own envelope-reading door, split out of
            Lib.psm1 purely to keep that file under the 400-line cap (the
            same reason LibVerdict.psm1 exists) - it reads .ok/.error/
            .details/.data on a raw command/batch envelope exactly as
            Lib.psm1 does, so it carries the same exemption rather than a
            new one invented for it.
        #>
        [pscustomobject]@{ Id = 'rule05'; Name = 'no direct envelope field access outside Lib.psm1/LibEnvelope.psm1'; ScopeFilter = { param($rel) $rel -notin @('Lib.psm1', 'LibEnvelope.psm1') }; Test = { param($p) Test-Rule05EnvelopeFieldAccess -Parsed $p } }
        [pscustomobject]@{ Id = 'rule06'; Name = 'Write-Verdict reached from Run-E2E.ps1'; ScopeFilter = { param($rel) $rel -eq 'Run-E2E.ps1' }; Test = { param($p) Test-Rule06WriteVerdictReached -Parsed $p } }
        [pscustomobject]@{ Id = 'rule07'; Name = 'the five identity env vars are never touched'; ScopeFilter = { $true }; Test = { param($p) Test-Rule07EnvIdentity -Parsed $p } }
        [pscustomobject]@{ Id = 'rule08'; Name = 'scenario files declare legs and skip tokens'; ScopeFilter = { param($rel) $rel -match '^scenarios/' }; Test = { param($p) Test-Rule08ScenarioLegs -Parsed $p -AllowlistKeys $script:Rule08AllowlistKeys } }
        [pscustomobject]@{ Id = 'rule09'; Name = 'desktop-touching legs declare a lock'; ScopeFilter = { param($rel) $rel -match '^scenarios/' }; Test = { param($p) Test-Rule09DesktopTouchingLock -Parsed $p -NativeEntryPoints $script:Rule09NativeEntryPoints } }
        [pscustomobject]@{ Id = 'rule10'; Name = 'only Invoke-GuardedAgent invokes the staged binary'; ScopeFilter = { param($rel) ($rel -notmatch '^selftest/') -and ($rel -notin @('DesktopLease.psm1', 'BoundedProcess.psm1', 'Lib.psm1')) }; Test = { param($p) Test-Rule10StagedBinaryInvocation -Parsed $p } }
        [pscustomobject]@{ Id = 'rule11'; Name = 'no direct ConvertFrom-Json'; ScopeFilter = $notSelftest; Test = { param($p) Test-Rule11ConvertFromJson -Parsed $p } }
        <#
            CheckStubReachability is scoped to scenario files specifically,
            not merely "outside selftest/": every other production module
            (Lib.psm1 chief among them) legitimately *names*
            Stub-AgentDesktop.ps1 in a doc comment explaining why
            Set-TargetBinary exists at all, which is prose about the design,
            not a scenario wiring itself to the stub - the hazard rule 12b
            actually guards against.
        #>
        [pscustomobject]@{ Id = 'rule12'; Name = 'status reads use --property value; stub is selftest-only'; ScopeFilter = { $true }; Test = { param($p) $rel = $p.RelativePath; Test-Rule12PropertyAndStub -Parsed $p -CheckStubReachability:($rel -match '^scenarios/') } }
    )
}

function Invoke-E2EContractScan {
    <#
    .SYNOPSIS
        Applies every rule in RuleTable to every file in FileSet under Root,
        aggregating violations and per-rule executed-check counts.
    .OUTPUTS
        PSCustomObject: Violations (array), RuleChecksExecuted (hashtable
        ruleId -> count of files scoped-in and checked, zero means the rule
        never actually ran against this tree).
    #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string]$Root,
        [Parameter(Mandatory = $true)][string[]]$FileSet,
        [Parameter(Mandatory = $true)][array]$RuleTable
    )
    $violations = New-Object System.Collections.Generic.List[object]
    $checksExecuted = @{}
    foreach ($rule in $RuleTable) { $checksExecuted[$rule.Id] = 0 }

    foreach ($rel in $FileSet) {
        $full = Join-Path $Root ($rel -replace '/', '\')
        $parsed = Get-E2EParsedFile -Path $full
        $parsed | Add-Member -NotePropertyName 'RelativePath' -NotePropertyValue $rel -Force
        foreach ($rule in $RuleTable) {
            if (-not (& $rule.ScopeFilter $rel)) { continue }
            $checksExecuted[$rule.Id]++
            $hits = & $rule.Test $parsed
            foreach ($hit in $hits) {
                $hit | Add-Member -NotePropertyName 'File' -NotePropertyValue $rel -Force
                $violations.Add($hit)
            }
        }
    }
    return [pscustomobject]@{ Violations = $violations.ToArray(); RuleChecksExecuted = $checksExecuted }
}

Export-ModuleMember -Function @(
    'ConvertTo-E2ERelativePath', 'Get-E2ETreeFileSet', 'Test-E2EFileSetEquality', 'Get-E2ERuleTable', 'Invoke-E2EContractScan'
)
