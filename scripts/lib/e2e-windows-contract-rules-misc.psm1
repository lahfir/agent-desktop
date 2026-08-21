#Requires -Version 5.1

<#
    e2e-windows-contract-rules-misc.psm1 - contract-gate rules 6, 7, 8, 11,
    12 and 14: Write-Verdict reachability, environment-identity protection,
    scenario leg/skip declarations, the ConvertFrom-Json ban, the
    --property text|name / stub-reachability pair, and the automatic-
    variable-assignment ban. Rule 13 (the 400-line cap) needs no AST and is
    a file-length check the orchestrator applies directly. Split from the
    other rule groups purely to keep every gate module under the 400-line
    cap.
#>

Set-StrictMode -Version 2.0

Import-Module (Join-Path $PSScriptRoot 'e2e-windows-contract-common.psm1') -Force -Global

function New-E2EViolation {
    param([string]$RuleId, [string]$Pattern, [int]$Line, [string]$Message)
    return [pscustomobject]@{ RuleId = $RuleId; Pattern = $Pattern; Line = $Line; Message = $Message }
}

function Test-Rule06WriteVerdictReached {
    <#
    .SYNOPSIS
        Rule 6's decidable slice: Run-E2E.ps1, the one exit path, must
        actually call Write-Verdict. The "finish-equivalent verifies every
        staged artifact" half has no referent yet - no scenario-sequencing
        or staging-verification logic exists in the tree until a later unit
        adds it - so this function checks only what is buildable today; the
        caller applies it only to Run-E2E.ps1.
    #>
    [CmdletBinding()]
    param($Parsed)
    $hits = New-Object System.Collections.Generic.List[object]
    $calls = Find-E2EAstNodes -Ast $Parsed.Ast -Predicate { $args[0] -is [System.Management.Automation.Language.CommandAst] }
    $found = $calls | Where-Object { Test-E2ECommandName -CommandAst $_ -Names @('Write-Verdict') }
    if (-not $found) {
        $hits.Add((New-E2EViolation -RuleId 'rule06' -Pattern 'no-write-verdict-call' -Line 1 -Message 'Run-E2E.ps1 never calls Write-Verdict'))
    }
    return $hits.ToArray()
}

function Test-Rule07EnvIdentity {
    <#
    .SYNOPSIS
        Rule 7: the isolated environment never clears or overrides
        USERNAME, COMPUTERNAME, USERPROFILE, LOCALAPPDATA or APPDATA - the
        capture-redaction gate derives every rule and residue check from
        those same five real values.
    #>
    [CmdletBinding()]
    param($Parsed)
    $hits = New-Object System.Collections.Generic.List[object]
    $protectedNames = @('username', 'computername', 'userprofile', 'localappdata', 'appdata')

    foreach ($node in (Find-E2EAstNodes -Ast $Parsed.Ast -Predicate { $args[0] -is [System.Management.Automation.Language.AssignmentStatementAst] })) {
        $left = $node.Left -as [System.Management.Automation.Language.VariableExpressionAst]
        if (-not $left) { continue }
        if ($left.VariablePath.DriveName -ine 'env') { continue }
        <#
            VariablePath.UserPath for $env:LOCALAPPDATA is "env:LOCALAPPDATA"
            (drive prefix included), never the bare variable name - measured
            while building this rule.
        #>
        $varName = $left.VariablePath.UserPath -replace '^env:', ''
        if ($protectedNames -icontains $varName) {
            $hits.Add((New-E2EViolation -RuleId 'rule07' -Pattern ('env-assigned-' + $varName.ToLowerInvariant()) -Line $node.Extent.StartLineNumber -Message "assigns `$env:$varName"))
        }
    }
    foreach ($node in (Find-E2EAstNodes -Ast $Parsed.Ast -Predicate { $args[0] -is [System.Management.Automation.Language.CommandAst] })) {
        if (-not (Test-E2ECommandName -CommandAst $node -Names @('Remove-Item', 'Clear-Item'))) { continue }
        foreach ($element in $node.CommandElements) {
            foreach ($candidate in $protectedNames) {
                if ($element.Extent.Text -imatch "env:\\?$candidate\b") {
                    $hits.Add((New-E2EViolation -RuleId 'rule07' -Pattern ('env-cleared-' + $candidate) -Line $node.Extent.StartLineNumber -Message "clears `$env:$candidate"))
                }
            }
        }
    }
    return $hits.ToArray()
}

function Test-Rule08ScenarioLegs {
    <#
    .SYNOPSIS
        Rule 8: every scenario file calls Register-Legs, and every
        Add-Skip -Token literal names a key present in AllowlistKeys.
        Applies only to files the caller identifies as scenario files
        (under tests/e2e-windows/scenarios/).
    #>
    [CmdletBinding()]
    param($Parsed, [string[]]$AllowlistKeys = @())
    $hits = New-Object System.Collections.Generic.List[object]
    $calls = Find-E2EAstNodes -Ast $Parsed.Ast -Predicate { $args[0] -is [System.Management.Automation.Language.CommandAst] }

    $registered = $calls | Where-Object { Test-E2ECommandName -CommandAst $_ -Names @('Register-Legs') }
    if (-not $registered) {
        $hits.Add((New-E2EViolation -RuleId 'rule08' -Pattern 'no-register-legs' -Line 1 -Message 'scenario file never calls Register-Legs'))
    }

    foreach ($node in ($calls | Where-Object { Test-E2ECommandName -CommandAst $_ -Names @('Add-Skip') })) {
        $tokenArg = $null
        for ($i = 0; $i -lt $node.CommandElements.Count; $i++) {
            $element = $node.CommandElements[$i]
            $param = $element -as [System.Management.Automation.Language.CommandParameterAst]
            if ($param -and $param.ParameterName -ieq 'Token' -and ($i + 1) -lt $node.CommandElements.Count) {
                $tokenArg = $node.CommandElements[$i + 1]
                break
            }
        }
        $tokenLiteral = $tokenArg -as [System.Management.Automation.Language.StringConstantExpressionAst]
        if ($tokenLiteral -and ($AllowlistKeys -notcontains $tokenLiteral.Value)) {
            $hits.Add((New-E2EViolation -RuleId 'rule08' -Pattern 'undeclared-skip-token' -Line $node.Extent.StartLineNumber -Message "Add-Skip token '$($tokenLiteral.Value)' is not a key in skip-allowlist.psd1"))
        }
    }
    return $hits.ToArray()
}

function Test-Rule11ConvertFromJson {
    <#
    .SYNOPSIS
        Rule 11: no harness file calls ConvertFrom-Json directly; JSON
        parsing goes through ConvertFrom-AgentJson (R12). The caller scopes
        this away from selftest/, where Stub-AgentDesktop.ps1 parses its own
        tiny state file and one U6 self-test demonstrates the recursion
        ceiling ConvertFrom-AgentJson exists to lift.
    #>
    [CmdletBinding()]
    param($Parsed)
    $hits = New-Object System.Collections.Generic.List[object]
    foreach ($node in (Find-E2EAstNodes -Ast $Parsed.Ast -Predicate { $args[0] -is [System.Management.Automation.Language.CommandAst] })) {
        if (Test-E2ECommandName -CommandAst $node -Names @('ConvertFrom-Json')) {
            $hits.Add((New-E2EViolation -RuleId 'rule11' -Pattern 'convertfrom-json' -Line $node.Extent.StartLineNumber -Message 'ConvertFrom-Json called directly; use ConvertFrom-AgentJson'))
        }
    }
    return $hits.ToArray()
}

function Test-Rule12PropertyAndStub {
    <#
    .SYNOPSIS
        Rule 12: no status read uses `get --property text` or
        `--property name` (R3 requires `value`), and Stub-AgentDesktop.ps1
        is never referenced outside selftest/ - the caller applies the
        stub-reachability half only to non-selftest files.
    #>
    [CmdletBinding()]
    param($Parsed, [switch]$CheckStubReachability)
    $hits = New-Object System.Collections.Generic.List[object]
    foreach ($match in [regex]::Matches($Parsed.Text, "(['""])--property\1\s*,\s*(['""])(text|name)\2")) {
        $line = ($Parsed.Text.Substring(0, $match.Index) -split "`n").Count
        $hits.Add((New-E2EViolation -RuleId 'rule12' -Pattern ('property-' + $match.Groups[3].Value) -Line $line -Message "status read uses --property $($match.Groups[3].Value); use --property value"))
    }
    if ($CheckStubReachability -and $Parsed.Text -match 'Stub-AgentDesktop') {
        $hits.Add((New-E2EViolation -RuleId 'rule12' -Pattern 'stub-reachable-outside-selftest' -Line 1 -Message 'Stub-AgentDesktop.ps1 referenced outside selftest/'))
    }
    return $hits.ToArray()
}

function Test-Rule14AutomaticVariableAssignment {
    <#
    .SYNOPSIS
        Rule 14: no assignment statement writes to a name PowerShell
        reserves as an automatic variable, taken from the published
        about_Automatic_Variables reference (PowerShell 7.6 docs; its list
        is a strict superset of 5.1's, so a 7.6-only name banned here costs
        nothing on this suite's pinned 5.1) rather than guessed. This is the
        class SplitIntegrity.ps1's own bug belonged to: it assigned its
        target to `$input`, PowerShell's own pipeline enumerator - the
        assignment read back correctly at first, but any later construct
        that re-binds pipeline input resets `$input`, so a subsequent
        `$input.RefId` read the enumerator instead of the target and the
        property did not exist. Renamed there; this rule makes the whole
        class unrepresentable rather than re-testing that one call site.
        Every name below was found genuinely assigned bare (not
        scope/drive-qualified - `$script:foo` and `$env:foo` name different
        variables entirely, already rule 7's concern for the five identity
        vars) somewhere in this tree before this rule shipped:
        InteractionHeaded.ps1 ($input), Lib.psm1 ($matches), Native.psm1
        ($error), NativeDesktop.psm1 ($pid, inside an EnumWindows callback
        scriptblock) and LibEnvelope.psm1 ($event) - all renamed in the
        same PR that added this rule.

        $null is deliberately excluded. `$null = expr` is PowerShell's own
        idiomatic output-discard pattern - this harness already relies on
        it (selftest/U6SelfTestCasesLease.ps1) - and it cannot exhibit this
        bug class at all: unlike every other automatic variable, $null has
        no state a later read can be silently corrupted from, because
        assigning to it is a no-op and it always reads back as $null. That
        is "genuinely assignable in ordinary correct code" in a way none of
        the 46 banned names below are, so it is the one automatic variable
        left out rather than forcing every discard in this tree into a
        throwaway-variable contortion.
    #>
    [CmdletBinding()]
    param($Parsed)
    $hits = New-Object System.Collections.Generic.List[object]
    $bannedNames = @(
        'args', 'consolefilename', 'enabledexperimentalfeatures', 'error', 'event',
        'eventargs', 'eventsubscriber', 'executioncontext', 'false', 'foreach', 'home',
        'host', 'input', 'iscoreclr', 'islinux', 'ismacos', 'iswindows', 'lastexitcode',
        'matches', 'myinvocation', 'nestedpromptlevel', 'pid', 'profile',
        'psboundparameters', 'pscmdlet', 'pscommandpath', 'psculture', 'psdebugcontext',
        'psedition', 'pshome', 'psitem', 'psscriptroot', 'pssenderinfo', 'psuiculture',
        'psversiontable', 'pwd', 'sender', 'shellid', 'stacktrace', 'switch', 'this',
        'true', '_', '$', '?', '^'
    )

    function Get-Rule14AssignedVariables {
        <#
        .SYNOPSIS
            Every VariableExpressionAst a single assignment statement's Left
            side actually names, direct (`$x = ...`, `${x} = ...`) or tuple
            (`$a, $b = ...`) - never a member/index write (`$x.Foo = ...`,
            `$x['k'] = ...`), which names a property or a dictionary key,
            not the variable $x itself, and must not be flagged.
        #>
        param($Left)
        $found = New-Object System.Collections.Generic.List[object]
        $direct = $Left -as [System.Management.Automation.Language.VariableExpressionAst]
        if ($direct) {
            $found.Add($direct)
            return $found.ToArray()
        }
        $tuple = $Left -as [System.Management.Automation.Language.ArrayLiteralAst]
        if ($tuple) {
            foreach ($element in $tuple.Elements) {
                $elementVar = $element -as [System.Management.Automation.Language.VariableExpressionAst]
                if ($elementVar) { $found.Add($elementVar) }
            }
        }
        return $found.ToArray()
    }

    foreach ($node in (Find-E2EAstNodes -Ast $Parsed.Ast -Predicate { $args[0] -is [System.Management.Automation.Language.AssignmentStatementAst] })) {
        foreach ($target in (Get-Rule14AssignedVariables -Left $node.Left)) {
            $varName = $target.VariablePath.UserPath
            if ($bannedNames -icontains $varName) {
                $hits.Add((New-E2EViolation -RuleId 'rule14' -Pattern ('automatic-var-' + $varName.ToLowerInvariant()) -Line $node.Extent.StartLineNumber -Message "assigns to `$$varName, a PowerShell automatic variable"))
            }
        }
    }
    return $hits.ToArray()
}

Export-ModuleMember -Function @(
    'Test-Rule06WriteVerdictReached', 'Test-Rule07EnvIdentity', 'Test-Rule08ScenarioLegs',
    'Test-Rule11ConvertFromJson', 'Test-Rule12PropertyAndStub', 'Test-Rule14AutomaticVariableAssignment'
)
