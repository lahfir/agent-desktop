#Requires -Version 5.1

<#
    e2e-windows-contract-rules-process.psm1 - contract-gate rules 1, 2, 3
    and 10: process-termination, destructive delete, forbidden interpreters,
    and the staged-binary invocation gate. Split from the other rule groups
    purely to keep every gate module under the 400-line cap.
#>

Set-StrictMode -Version 2.0

Import-Module (Join-Path $PSScriptRoot 'e2e-windows-contract-common.psm1') -Force -Global

function New-E2EViolation {
    param([string]$RuleId, [string]$Pattern, [int]$Line, [string]$Message)
    return [pscustomobject]@{ RuleId = $RuleId; Pattern = $Pattern; Line = $Line; Message = $Message }
}

function Test-Rule01ExitCalls {
    <#
    .SYNOPSIS
        Rule 1's five terminating-call patterns: exit, [Environment]::Exit,
        [System.Environment]::Exit, $host.SetShouldExit, Stop-Process naming
        $PID. Scope (exactly one hit, in Run-E2E.ps1, selftest/** excluded
        as its own process boundary) is enforced by the caller, which
        aggregates this function's per-file hits across the tree.
    #>
    [CmdletBinding()]
    param($Parsed)
    $hits = New-Object System.Collections.Generic.List[object]

    foreach ($node in (Find-E2EAstNodes -Ast $Parsed.Ast -Predicate { $args[0] -is [System.Management.Automation.Language.ExitStatementAst] })) {
        $hits.Add((New-E2EViolation -RuleId 'rule01' -Pattern 'exit-statement' -Line $node.Extent.StartLineNumber -Message 'exit statement'))
    }

    foreach ($node in (Find-E2EAstNodes -Ast $Parsed.Ast -Predicate { $args[0] -is [System.Management.Automation.Language.InvokeMemberExpressionAst] })) {
        if (-not $node.Static) { continue }
        if ($node.Member.Extent.Text -ine 'Exit') { continue }
        $typeExpr = $node.Expression -as [System.Management.Automation.Language.TypeExpressionAst]
        if (-not $typeExpr) { continue }
        $typeName = $typeExpr.TypeName.FullName
        if ($typeName -ieq 'Environment' -or $typeName -ieq 'System.Environment') {
            $hits.Add((New-E2EViolation -RuleId 'rule01' -Pattern 'environment-exit' -Line $node.Extent.StartLineNumber -Message "[$typeName]::Exit"))
        }
    }

    foreach ($node in (Find-E2EAstNodes -Ast $Parsed.Ast -Predicate { $args[0] -is [System.Management.Automation.Language.InvokeMemberExpressionAst] })) {
        if ($node.Static) { continue }
        if ($node.Member.Extent.Text -ine 'SetShouldExit') { continue }
        $varExpr = $node.Expression -as [System.Management.Automation.Language.VariableExpressionAst]
        if ($varExpr -and $varExpr.VariablePath.UserPath -ieq 'host') {
            $hits.Add((New-E2EViolation -RuleId 'rule01' -Pattern 'host-setshouldexit' -Line $node.Extent.StartLineNumber -Message '$host.SetShouldExit'))
        }
    }

    foreach ($node in (Find-E2EAstNodes -Ast $Parsed.Ast -Predicate { $args[0] -is [System.Management.Automation.Language.CommandAst] })) {
        if (-not (Test-E2ECommandName -CommandAst $node -Names @('Stop-Process'))) { continue }
        $namesPid = $node.CommandElements | Where-Object {
            $var = $_ -as [System.Management.Automation.Language.VariableExpressionAst]
            $var -and $var.VariablePath.UserPath -ieq 'PID'
        }
        if ($namesPid) {
            $hits.Add((New-E2EViolation -RuleId 'rule01' -Pattern 'stop-process-pid' -Line $node.Extent.StartLineNumber -Message 'Stop-Process naming $PID'))
        }
    }

    return $hits.ToArray()
}

function Test-Rule02RemoveItem {
    <#
    .SYNOPSIS
        Rule 2: no Remove-Item against a filesystem artifact; only an
        Env:\ removal (an environment variable, never a suite artifact) or
        the recoverable primitive is legitimate. selftest/** is excluded:
        its cases manage their own scratch files, never a suite artifact
        created through Enter-IsolatedEnvironment/Copy-ImmutableArtifact.
    #>
    [CmdletBinding()]
    param($Parsed)
    $hits = New-Object System.Collections.Generic.List[object]
    foreach ($node in (Find-E2EAstNodes -Ast $Parsed.Ast -Predicate { $args[0] -is [System.Management.Automation.Language.CommandAst] })) {
        if (-not (Test-E2ECommandName -CommandAst $node -Names @('Remove-Item'))) { continue }
        $targetsEnv = $false
        foreach ($element in $node.CommandElements) {
            $text = $element.Extent.Text
            if ($text -imatch '^[''"]?Env:') { $targetsEnv = $true; break }
        }
        if (-not $targetsEnv) {
            $hits.Add((New-E2EViolation -RuleId 'rule02' -Pattern 'remove-item-filesystem-path' -Line $node.Extent.StartLineNumber -Message 'Remove-Item against a non-Env: path; use Remove-ItemRecoverable'))
        }
    }
    return $hits.ToArray()
}

function Test-Rule03ForbiddenInterpreter {
    <#
    .SYNOPSIS
        Rule 3: no python, python3, bash, sh or wsl invocation anywhere.
    #>
    [CmdletBinding()]
    param($Parsed)
    $hits = New-Object System.Collections.Generic.List[object]
    $forbidden = @('python', 'python3', 'bash', 'sh', 'wsl')
    foreach ($node in (Find-E2EAstNodes -Ast $Parsed.Ast -Predicate { $args[0] -is [System.Management.Automation.Language.CommandAst] })) {
        if (Test-E2ECommandName -CommandAst $node -Names $forbidden) {
            $hits.Add((New-E2EViolation -RuleId 'rule03' -Pattern ('interpreter-' + $node.GetCommandName().ToLowerInvariant()) -Line $node.Extent.StartLineNumber -Message "forbidden interpreter invocation: $($node.GetCommandName())"))
        }
    }
    return $hits.ToArray()
}

function Test-Rule10StagedBinaryInvocation {
    <#
    .SYNOPSIS
        Rule 10: only Invoke-GuardedAgent may invoke the staged
        agent-desktop.exe. Catches the `&` call operator (including on a
        variable this file itself assigned from a matching string literal -
        the tracked-assignment half below), Start-Process, Start-Job,
        Invoke-Expression, and a static [...Process]::Start(...) call, each
        naming something that looks like the staged binary. Scope
        (DesktopLease.psm1/BoundedProcess.psm1/Lib.psm1 implement the
        primitive; selftest/** deliberately drives both guarded paths to
        prove the lease-adoption protocol) is enforced by the caller.

        Residual left deliberately unclosed: a call-operator target bound
        through a variable whose value cannot be seen as a string literal
        anywhere in the same file - e.g. read from a parameter, a config
        file, or built by string concatenation - has no text for this
        AST-only rule to match against. Only same-file, direct
        `$var = '...'` assignment is tracked.
    #>
    [CmdletBinding()]
    param($Parsed)
    $hits = New-Object System.Collections.Generic.List[object]
    $bindingRegex = '(?i)(agent-desktop|StagedBinary|AgentDesktopBinary)'

    $assignedBindingVars = New-Object System.Collections.Generic.HashSet[string]
    foreach ($assign in (Find-E2EAstNodes -Ast $Parsed.Ast -Predicate { $args[0] -is [System.Management.Automation.Language.AssignmentStatementAst] })) {
        $left = $assign.Left -as [System.Management.Automation.Language.VariableExpressionAst]
        if (-not $left) { continue }
        if ($assign.Right.Extent.Text -match $bindingRegex) {
            [void]$assignedBindingVars.Add($left.VariablePath.UserPath)
        }
    }

    foreach ($node in (Find-E2EAstNodes -Ast $Parsed.Ast -Predicate { $args[0] -is [System.Management.Automation.Language.CommandAst] })) {
        $ampersand = [System.Management.Automation.Language.TokenKind]::Ampersand
        if ($node.InvocationOperator -eq $ampersand -and $node.CommandElements.Count -gt 0) {
            $targetElement = $node.CommandElements[0]
            $target = $targetElement.Extent.Text
            $targetVariable = $targetElement -as [System.Management.Automation.Language.VariableExpressionAst]
            $matchesTrackedAssignment = $targetVariable -and $assignedBindingVars.Contains($targetVariable.VariablePath.UserPath)
            if ($target -match $bindingRegex -or $matchesTrackedAssignment) {
                $hits.Add((New-E2EViolation -RuleId 'rule10' -Pattern 'call-operator-staged-binary' -Line $node.Extent.StartLineNumber -Message "call-operator invocation of the staged binary: $target"))
            }
            continue
        }
        if (Test-E2ECommandName -CommandAst $node -Names @('Start-Process')) {
            $matched = $node.CommandElements | Where-Object { $_.Extent.Text -match $bindingRegex }
            if ($matched) {
                $hits.Add((New-E2EViolation -RuleId 'rule10' -Pattern 'start-process-staged-binary' -Line $node.Extent.StartLineNumber -Message 'Start-Process naming the staged binary'))
            }
            continue
        }
        if (Test-E2ECommandName -CommandAst $node -Names @('Start-Job')) {
            $matched = $node.CommandElements | Where-Object { $_.Extent.Text -match $bindingRegex }
            if ($matched) {
                $hits.Add((New-E2EViolation -RuleId 'rule10' -Pattern 'start-job-staged-binary' -Line $node.Extent.StartLineNumber -Message 'Start-Job naming the staged binary'))
            }
            continue
        }
        if (Test-E2ECommandName -CommandAst $node -Names @('Invoke-Expression', 'iex')) {
            $matched = $node.CommandElements | Where-Object { $_.Extent.Text -match $bindingRegex }
            if ($matched) {
                $hits.Add((New-E2EViolation -RuleId 'rule10' -Pattern 'invoke-expression-staged-binary' -Line $node.Extent.StartLineNumber -Message 'Invoke-Expression naming the staged binary'))
            }
        }
    }

    foreach ($node in (Find-E2EAstNodes -Ast $Parsed.Ast -Predicate { $args[0] -is [System.Management.Automation.Language.InvokeMemberExpressionAst] })) {
        if (-not $node.Static) { continue }
        if ($node.Member.Extent.Text -ine 'Start') { continue }
        $typeExpr = $node.Expression -as [System.Management.Automation.Language.TypeExpressionAst]
        if (-not $typeExpr) { continue }
        $typeName = $typeExpr.TypeName.FullName
        if ($typeName -inotmatch '(^|\.)Process$') { continue }
        $matched = $node.Arguments | Where-Object { $_.Extent.Text -match $bindingRegex }
        if ($matched) {
            $hits.Add((New-E2EViolation -RuleId 'rule10' -Pattern 'process-start-staged-binary' -Line $node.Extent.StartLineNumber -Message "[$typeName]::Start naming the staged binary"))
        }
    }

    return $hits.ToArray()
}

Export-ModuleMember -Function @(
    'Test-Rule01ExitCalls', 'Test-Rule02RemoveItem', 'Test-Rule03ForbiddenInterpreter', 'Test-Rule10StagedBinaryInvocation'
)
