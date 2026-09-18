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

function Test-Rule15MeasuredAndDiscarded {
    <#
    .SYNOPSIS
        Rule 15: a leg must not measure something and then discard it.
        Within a scenario function, a variable that is assigned and then
        either never read at all, or read only inside a Write-Host, is a
        measurement the leg took and did not act on.

        This is the shape four shipped legs had. `contended-focus-steal-rate`
        counted how many trials won the foreground and printed it; the leg
        gated on the trial count instead, so zero wins passed.
        `split-integrity-capture-recorded` computed whether any pixels were
        produced and printed it. `chromium-menu-attempt-bounded` read four
        menu probes and referenced none of them again.

        What it does NOT catch, stated so nobody reads a pass here as more
        than it is: a leg verified by the command's own success flag, which
        is a real assertion on a value that proves the wrong thing.
        `reliability-wait-enabled-delayed-button` was that shape, and only a
        reviewer can see it.

        Two exclusions, both deliberate. A `$script:`- or `$global:`-scoped
        variable is read across functions, which a per-function rule cannot
        see, so it is skipped rather than reported as a false positive. And
        a measurement a leg genuinely records rather than gates - a cost
        baseline is the standing example - opts out with a
        `rule15-reported:` comment naming why, so the exemption is written
        down at the assignment instead of being argued for in review.
    #>
    [CmdletBinding()]
    param($Parsed)
    $hits = New-Object System.Collections.Generic.List[object]
    $ignored = @('null', '_', 'args', 'psitem', 'true', 'false')
    $printers = @('Write-Host', 'Write-Verbose', 'Write-Information', 'Write-Debug', 'Write-Warning')
    $exempt = @()
    foreach ($token in $Parsed.Tokens) {
        if ($token.Kind -eq [System.Management.Automation.Language.TokenKind]::Comment -and $token.Text -match 'rule15-reported:') {
            $exempt += $token.Extent.StartLineNumber
            $exempt += ($token.Extent.StartLineNumber + 1)
        }
    }

    $functions = Find-E2EAstNodes -Ast $Parsed.Ast -Predicate {
        $args[0] -is [System.Management.Automation.Language.FunctionDefinitionAst]
    }
    foreach ($function in $functions) {
        $functionExempt = $false
        foreach ($line in $exempt) {
            if ($line -ge $function.Extent.StartLineNumber -and $line -le $function.Extent.EndLineNumber) {
                $functionExempt = $true
                break
            }
        }
        $printRanges = @()
        foreach ($call in (Find-E2EAstNodes -Ast $function.Body -Predicate { $args[0] -is [System.Management.Automation.Language.CommandAst] })) {
            if (Test-E2ECommandName -CommandAst $call -Names $printers) {
                $printRanges += , @($call.Extent.StartOffset, $call.Extent.EndOffset)
            }
        }

        $assignments = @{}
        foreach ($node in (Find-E2EAstNodes -Ast $function.Body -Predicate { $args[0] -is [System.Management.Automation.Language.AssignmentStatementAst] })) {
            $left = $node.Left -as [System.Management.Automation.Language.VariableExpressionAst]
            if (-not $left) { continue }
            if ($left.VariablePath.DriveName) { continue }
            $name = $left.VariablePath.UserPath.ToLowerInvariant()
            if ($ignored -contains $name) { continue }
            if ($name -match ':') { continue }
            if ($functionExempt) { continue }
            if (-not $assignments.ContainsKey($name)) {
                $assignments[$name] = [pscustomobject]@{ Line = $node.Extent.StartLineNumber; Offsets = @() }
            }
            $assignments[$name].Offsets += $left.Extent.StartOffset
        }

        $reads = @{}
        foreach ($node in (Find-E2EAstNodes -Ast $function.Body -Predicate { $args[0] -is [System.Management.Automation.Language.VariableExpressionAst] })) {
            if ($node.VariablePath.DriveName) { continue }
            $name = $node.VariablePath.UserPath.ToLowerInvariant()
            if (-not $assignments.ContainsKey($name)) { continue }
            if ($assignments[$name].Offsets -contains $node.Extent.StartOffset) { continue }
            if (-not $reads.ContainsKey($name)) { $reads[$name] = @() }
            $reads[$name] += $node.Extent.StartOffset
        }

        foreach ($name in $assignments.Keys) {
            $line = $assignments[$name].Line
            if (-not $reads.ContainsKey($name)) {
                $hits.Add((New-E2EViolation -RuleId 'rule15' -Pattern 'measured-and-never-read' -Line $line -Message "function '$($function.Name)' assigns `$$name and never reads it - a measurement the leg does not act on"))
                continue
            }
            $actedOn = $false
            foreach ($offset in $reads[$name]) {
                $printed = $false
                foreach ($range in $printRanges) {
                    if ($offset -ge $range[0] -and $offset -lt $range[1]) { $printed = $true; break }
                }
                if (-not $printed) { $actedOn = $true; break }
            }
            if (-not $actedOn) {
                $hits.Add((New-E2EViolation -RuleId 'rule15' -Pattern 'measured-and-only-printed' -Line $line -Message "function '$($function.Name)' assigns `$$name and only prints it - the leg reports the measurement without gating on it"))
            }
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
    'Test-Rule11ConvertFromJson', 'Test-Rule12PropertyAndStub', 'Test-Rule14AutomaticVariableAssignment',
    'Test-Rule15MeasuredAndDiscarded'
)
