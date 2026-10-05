#Requires -Version 5.1

<#
    e2e-windows-contract-rules-ref.psm1 - contract-gate rules 4, 5 and 9:
    the bare-ref helper ban, the envelope-field-access ban, and desktop-
    touching lock declarations. Split from the other rule groups purely to
    keep every gate module under the 400-line cap.
#>

Set-StrictMode -Version 2.0

Import-Module (Join-Path $PSScriptRoot 'e2e-windows-contract-common.psm1') -Force -Global

function New-E2EViolation {
    param([string]$RuleId, [string]$Pattern, [int]$Line, [string]$Message)
    return [pscustomobject]@{ RuleId = $RuleId; Pattern = $Pattern; Line = $Line; Message = $Message }
}

function Test-Rule04BareRef {
    <#
    .SYNOPSIS
        Rule 4: every Invoke-Target/Get-Target/Test-Target/Wait-Target call
        site passes a target object (never a literal string), and no
        command-line argument anywhere interpolates a bare `@snapshot:eN`
        ref shape.
    #>
    [CmdletBinding()]
    param($Parsed)
    $hits = New-Object System.Collections.Generic.List[object]
    $refShape = '@[A-Za-z0-9]*:e[0-9]+'
    $targetHelpers = @('Invoke-Target', 'Get-Target', 'Test-Target', 'Wait-Target')

    foreach ($node in (Find-E2EAstNodes -Ast $Parsed.Ast -Predicate { $args[0] -is [System.Management.Automation.Language.CommandAst] })) {
        if (Test-E2ECommandName -CommandAst $node -Names $targetHelpers) {
            $positional = $node.CommandElements | Where-Object {
                $_ -isnot [System.Management.Automation.Language.CommandParameterAst] -and $_.Extent.Text -ne $node.GetCommandName()
            } | Select-Object -First 1
            if ($positional -and ($positional -is [System.Management.Automation.Language.StringConstantExpressionAst] -or $positional -is [System.Management.Automation.Language.ExpandableStringExpressionAst])) {
                $hits.Add((New-E2EViolation -RuleId 'rule04' -Pattern 'bare-ref-literal-arg' -Line $node.Extent.StartLineNumber -Message "$($node.GetCommandName()) called with a literal string instead of a target object"))
            }
        }
    }

    <#
        A ref-shaped string literal anywhere in the file, not only as a
        direct CommandAst element: real call sites pass an -ArgumentList
        array (`@('click', '@snap:e1', ...)`), so the string sits inside an
        ArrayLiteralAst one level below the command, and a scan limited to
        direct CommandElements never reaches it.
    #>
    foreach ($element in (Find-E2EAstNodes -Ast $Parsed.Ast -Predicate {
                $args[0] -is [System.Management.Automation.Language.StringConstantExpressionAst] -or
                $args[0] -is [System.Management.Automation.Language.ExpandableStringExpressionAst]
            })) {
        if ($element.Value -match $refShape) {
            $hits.Add((New-E2EViolation -RuleId 'rule04' -Pattern 'bare-ref-interpolated' -Line $element.Extent.StartLineNumber -Message "a string literal interpolates a bare ref: $($element.Extent.Text)"))
        }
    }
    return $hits.ToArray()
}

function Test-Rule05EnvelopeFieldAccess {
    <#
    .SYNOPSIS
        Rule 5: outside Lib.psm1, no file may reference .ok/.error/
        .disposition/.data on a result object at all - Assert-Effect/
        Assert-NoEffect/Assert-Envelope are the only doors. Any dotted
        member-access AST node naming one of those four, in any nesting or
        operator context, is a violation - not only a top-level `if`.
    #>
    [CmdletBinding()]
    param($Parsed)
    $hits = New-Object System.Collections.Generic.List[object]
    $bannedFields = @('ok', 'error', 'disposition', 'data')
    <#
        $EventArgs is PowerShell's own automatic variable inside a
        Register-ObjectEvent -Action scriptblock (BoundedProcess.psm1's
        stdout/stderr drain handlers); its .Data is
        System.Diagnostics.DataReceivedEventArgs.Data, a .NET event payload
        that has nothing to do with a command envelope. Excluded by
        variable name, not by file, so a real envelope named $EventArgs
        elsewhere would still be caught - it is PowerShell's own reserved
        name that makes this collision structural, not a coincidence of
        this one file.
    #>
    foreach ($node in (Find-E2EAstNodes -Ast $Parsed.Ast -Predicate { $args[0] -is [System.Management.Automation.Language.MemberExpressionAst] })) {
        if ($node.Static) { continue }
        $memberName = $node.Member -as [System.Management.Automation.Language.StringConstantExpressionAst]
        if (-not $memberName) { continue }
        $baseVar = $node.Expression -as [System.Management.Automation.Language.VariableExpressionAst]
        if ($baseVar -and $baseVar.VariablePath.UserPath -ieq 'EventArgs') { continue }
        if ($bannedFields -icontains $memberName.Value) {
            $hits.Add((New-E2EViolation -RuleId 'rule05' -Pattern ('dot-access-' + $memberName.Value.ToLowerInvariant()) -Line $node.Extent.StartLineNumber -Message "direct '.$($memberName.Value)' access on a result object: $($node.Extent.Text)"))
        }
    }
    return $hits.ToArray()
}

function Get-E2ENativeEntryPoints {
    <#
    .SYNOPSIS
        Every function name Export-ModuleMember actually lists in a
        *Native*.psm1 module found anywhere under Root - the explicit half
        of rule 9's entry-point check. The `-imatch 'Native'` FUNCTION-NAME
        convention alone misses a real export whose own name does not
        contain the word, e.g. NativeToken.psm1's New-RestrictedIntegrityToken
        or NativeTypes.psm1's New-E2ESecurityAttributes - both desktop-
        touching, neither visible to that predicate. This reads each
        matching MODULE's own Export-ModuleMember call instead of guessing
        from the called name, so a module gaining a new export is covered on
        its next scan without hand-editing a list here. The residual this
        still cannot see: a future desktop-touching module whose FILENAME
        does not contain "Native" - Test-Rule09DesktopTouchingLock keeps the
        function-name convention as a backstop for exactly that case.
    #>
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][string]$Root)
    $names = New-Object System.Collections.Generic.List[string]
    if (-not (Test-Path -LiteralPath $Root)) { return , @() }
    $nativeModules = @(Get-ChildItem -LiteralPath $Root -File -Filter '*.psm1' -Recurse | Where-Object { $_.BaseName -imatch 'Native' })
    foreach ($module in $nativeModules) {
        $parsed = Get-E2EParsedFile -Path $module.FullName
        foreach ($call in (Find-E2EAstNodes -Ast $parsed.Ast -Predicate { $args[0] -is [System.Management.Automation.Language.CommandAst] })) {
            if (-not (Test-E2ECommandName -CommandAst $call -Names @('Export-ModuleMember'))) { continue }
            <#
                Skip CommandElements[0]: it is the "Export-ModuleMember"
                command-name token itself, parsed as its own
                StringConstantExpressionAst - walking the whole CommandAst
                (rather than only its arguments) would add the literal text
                "Export-ModuleMember" to the entry-point table, measured
                while building this function.
            #>
            foreach ($element in ($call.CommandElements | Select-Object -Skip 1)) {
                foreach ($literal in (Find-E2EAstNodes -Ast $element -Predicate { $args[0] -is [System.Management.Automation.Language.StringConstantExpressionAst] })) {
                    $names.Add($literal.Value)
                }
            }
        }
    }
    return , @($names | Sort-Object -Unique)
}

function Test-Rule09DesktopTouchingLock {
    <#
    .SYNOPSIS
        Rule 9: a call to Invoke-Target, Invoke-Guarded, Invoke-GuardedAgent,
        Start-StagedIntegrityProcess, any name NativeEntryPoints lists (the
        real, current Export-ModuleMember contents of every *Native*.psm1
        module - see Get-E2ENativeEntryPoints), any other function whose own
        name contains "Native" (the naming-convention backstop for a module
        this scan cannot see), or an Add-Type call carrying an inline
        -TypeDefinition/-MemberDefinition P/Invoke definition, must be
        lexically enclosed in an Enter-Stage script-block argument. Applies
        to scenario-authored call sites only - the caller scopes this to
        files under scenarios/**, never to the modules that implement or
        wrap these primitives.
    #>
    [CmdletBinding()]
    param($Parsed, [string[]]$NativeEntryPoints = @())
    $hits = New-Object System.Collections.Generic.List[object]
    $namedEntryPoints = @('Invoke-Target', 'Invoke-Guarded', 'Invoke-GuardedAgent', 'Start-StagedIntegrityProcess')

    function Test-E2EEnclosedInEnterStage {
        param($Node)
        $cursor = $Node.Parent
        while ($cursor) {
            if ($cursor -is [System.Management.Automation.Language.ScriptBlockExpressionAst]) {
                $owner = $cursor.Parent -as [System.Management.Automation.Language.CommandAst]
                if ($owner -and (Test-E2ECommandName -CommandAst $owner -Names @('Enter-Stage'))) {
                    return $true
                }
            }
            $cursor = $cursor.Parent
        }
        return $false
    }

    foreach ($node in (Find-E2EAstNodes -Ast $Parsed.Ast -Predicate { $args[0] -is [System.Management.Automation.Language.CommandAst] })) {
        $name = $node.GetCommandName()
        if (-not $name) { continue }
        $isEntryPoint = ($namedEntryPoints -icontains $name) -or ($name -imatch 'Native') -or ($NativeEntryPoints -icontains $name)
        if ($isEntryPoint) {
            if (-not (Test-E2EEnclosedInEnterStage -Node $node)) {
                $hits.Add((New-E2EViolation -RuleId 'rule09' -Pattern ('entry-point-' + $name) -Line $node.Extent.StartLineNumber -Message "$name is not lexically enclosed in an Enter-Stage block"))
            }
            continue
        }
        if (Test-E2ECommandName -CommandAst $node -Names @('Add-Type')) {
            $inlineDefinition = $node.CommandElements | Where-Object {
                $param = $_ -as [System.Management.Automation.Language.CommandParameterAst]
                $param -and ($param.ParameterName -ieq 'TypeDefinition' -or $param.ParameterName -ieq 'MemberDefinition')
            }
            if ($inlineDefinition -and -not (Test-E2EEnclosedInEnterStage -Node $node)) {
                $hits.Add((New-E2EViolation -RuleId 'rule09' -Pattern 'add-type-inline-pinvoke' -Line $node.Extent.StartLineNumber -Message 'Add-Type with an inline P/Invoke definition is not lexically enclosed in an Enter-Stage block'))
            }
        }
    }
    return $hits.ToArray()
}

Export-ModuleMember -Function @(
    'Test-Rule04BareRef', 'Test-Rule05EnvelopeFieldAccess', 'Get-E2ENativeEntryPoints', 'Test-Rule09DesktopTouchingLock'
)
