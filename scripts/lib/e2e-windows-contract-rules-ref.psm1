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

function Test-Rule09DesktopTouchingLock {
    <#
    .SYNOPSIS
        Rule 9: a call to Invoke-Target, Invoke-Guarded, Invoke-GuardedAgent,
        Start-StagedIntegrityProcess, or any function whose name contains
        "Native" (every export of Native.psm1/NativeTypes.psm1/
        NativeDesktop.psm1, present and future, by naming convention) must
        be lexically enclosed in an Enter-Stage script-block argument.
        Applies to scenario-authored call sites only - the caller scopes
        this to files under scenarios/**, never to the modules that
        implement or wrap these primitives.
    #>
    [CmdletBinding()]
    param($Parsed)
    $hits = New-Object System.Collections.Generic.List[object]
    $namedEntryPoints = @('Invoke-Target', 'Invoke-Guarded', 'Invoke-GuardedAgent', 'Start-StagedIntegrityProcess')

    foreach ($node in (Find-E2EAstNodes -Ast $Parsed.Ast -Predicate { $args[0] -is [System.Management.Automation.Language.CommandAst] })) {
        $name = $node.GetCommandName()
        if (-not $name) { continue }
        $isEntryPoint = ($namedEntryPoints -icontains $name) -or ($name -imatch 'Native')
        if (-not $isEntryPoint) { continue }

        $enclosed = $false
        $cursor = $node.Parent
        while ($cursor) {
            if ($cursor -is [System.Management.Automation.Language.ScriptBlockExpressionAst]) {
                $owner = $cursor.Parent -as [System.Management.Automation.Language.CommandAst]
                if ($owner -and (Test-E2ECommandName -CommandAst $owner -Names @('Enter-Stage'))) {
                    $enclosed = $true
                    break
                }
            }
            $cursor = $cursor.Parent
        }
        if (-not $enclosed) {
            $hits.Add((New-E2EViolation -RuleId 'rule09' -Pattern ('entry-point-' + $name) -Line $node.Extent.StartLineNumber -Message "$name is not lexically enclosed in an Enter-Stage block"))
        }
    }
    return $hits.ToArray()
}

Export-ModuleMember -Function @(
    'Test-Rule04BareRef', 'Test-Rule05EnvelopeFieldAccess', 'Test-Rule09DesktopTouchingLock'
)
