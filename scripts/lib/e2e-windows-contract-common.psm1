#Requires -Version 5.1

<#
    e2e-windows-contract-common.psm1 - shared AST-parsing helper for
    check-e2e-windows-contract.ps1's rule modules. Every rule that needs
    structural precision (rather than a text/regex scan) is built on
    PowerShell's own parser rather than on grep-style patterns, because the
    tree under test is PowerShell: an AST walk cannot be fooled by the word
    "exit" inside a doc comment the way a line-oriented regex can, and this
    corpus's doc comments use that exact word repeatedly in prose ("the
    process's own exit", "the exit code").
#>

Set-StrictMode -Version 2.0

function Get-E2EParsedFile {
    <#
    .SYNOPSIS
        Parses Path once and returns its Ast, Tokens and raw Text together,
        so every rule function that needs structure gets it from a single
        parse rather than re-parsing per rule.
    .OUTPUTS
        PSCustomObject: Path, Text, Ast, Tokens, ParseErrors.
    #>
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][string]$Path)
    $text = [IO.File]::ReadAllText($Path)
    $tokens = $null
    $parseErrors = $null
    $ast = [System.Management.Automation.Language.Parser]::ParseInput($text, [ref]$tokens, [ref]$parseErrors)
    return [pscustomobject]@{
        Path        = $Path
        Text        = $text
        Ast         = $ast
        Tokens      = $tokens
        ParseErrors = $parseErrors
    }
}

function Find-E2EAstNodes {
    <#
    .SYNOPSIS
        Ast.FindAll wrapper so every rule filters with the same call shape.
    #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]$Ast,
        [Parameter(Mandatory = $true)][scriptblock]$Predicate
    )
    return @($Ast.FindAll($Predicate, $true))
}

function Test-E2ECommandName {
    <#
    .SYNOPSIS
        Case-insensitive GetCommandName() match against one or more names,
        null-safe (a command invoked through a variable, e.g. `& $x`, has no
        static command name and must not throw here).
    #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]$CommandAst,
        [Parameter(Mandatory = $true)][string[]]$Names
    )
    $name = $CommandAst.GetCommandName()
    if (-not $name) { return $false }
    foreach ($candidate in $Names) {
        if ($name -ieq $candidate) { return $true }
    }
    return $false
}

Export-ModuleMember -Function @(
    'Get-E2EParsedFile',
    'Find-E2EAstNodes',
    'Test-E2ECommandName'
)
