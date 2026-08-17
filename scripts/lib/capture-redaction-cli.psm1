#Requires -Version 5.1

<#
    capture-redaction-cli.psm1 - R21's CLI-envelope half: Test-CaptureCliRedaction.

    probes/windows/common.ps1's own Test-CaptureRedaction catches the
    machine-wide residue classes (user name, machine name, profile paths,
    domain SIDs). It has no idea that agent-desktop's own JSON envelope
    carries real target content in a small, fixed set of fields regardless
    of which machine produced it: `name`, `value`, `description`, `title`,
    every element of a `path` array (crates/core/src/commands/find.rs:194
    and live_locator/select.rs's node_label both populate it as
    "role:label"), `error.details.checks[].occluder.name`, and any literal
    pid field (pid/processid/parentprocessid/ownerprocessid/sessionprocessid
    - the corpus safety envelope forbids a pid outright, never merely an
    unreduced one). A capture
    that scrubs every profile path and still embeds a real Obsidian note
    title in `path[]` passes Test-CaptureRedaction cleanly - that gap is
    what this module closes.

    This module has no dependency on probes/windows/common.ps1 by design:
    scripts/ is CI tooling, probes/ is the measurement corpus, and the one
    primitive this module actually needs from there (undoing serde_json's
    \uXXXX escaping so a correctly-reduced `<redacted:N chars>` marker can be
    recognised after JSON-encoding) is small enough to carry its own copy
    rather than reach across that boundary.
#>

Set-StrictMode -Version 2.0

$script:ReducedMarkerPattern = '^<redacted:\d+ chars>$'
$script:UnnamedPattern = '^\(unnamed .*\)$'

function ConvertFrom-CliJsonEscape {
    <#
    .SYNOPSIS
        Reverses serde_json's string escaping (\uXXXX, \n, \", \\, ...) in a
        single left-to-right pass, so a reduction marker written as
        `<redacted:5 chars>` and re-serialized as `<redacted:5
        chars>` is recognised as reduced rather than flagged as raw
        content. Mirrors probes/windows/common.ps1's
        ConvertFrom-ProbeJsonEscape.
    #>
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Text)
    if ([string]::IsNullOrEmpty($Text)) { return $Text }
    return [regex]::Replace($Text, '\\(u[0-9a-fA-F]{4}|.)', {
            param($m)
            $token = $m.Groups[1].Value
            if ($token.Length -eq 5) { return ([string][char][Convert]::ToInt32($token.Substring(1), 16)) }
            if ($token -eq 'n') { return "`n" }
            if ($token -eq 'r') { return "`r" }
            if ($token -eq 't') { return "`t" }
            if ($token -eq 'b') { return "`b" }
            if ($token -eq 'f') { return "`f" }
            return $token
        })
}

function Test-CliRedactionValueReduced {
    <#
    .SYNOPSIS
        True when a captured field's decoded value is empty, an
        agent-desktop "no meaningful name" sentinel, or a
        Protect-ProbeName-shaped reduction. False for any other literal
        text - the fail-safe direction, since a value this function cannot
        prove reduced must be treated as unreduced content.
    .PARAMETER AllowRolePrefix
        path[] elements are node_label's "role:label" - only the label half
        is target content, so the role prefix before the first ':' is
        stripped before the reduction check.
    #>
    param(
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$RawJsonString,
        [switch]$AllowRolePrefix
    )
    if ([string]::IsNullOrEmpty($RawJsonString)) { return $true }
    $decoded = ConvertFrom-CliJsonEscape -Text $RawJsonString
    if ($AllowRolePrefix) {
        $colonIndex = $decoded.IndexOf(':')
        if ($colonIndex -ge 0) {
            $decoded = $decoded.Substring($colonIndex + 1)
        }
    }
    if ($decoded -eq '') { return $true }
    if ($decoded -match $script:UnnamedPattern) { return $true }
    return ($decoded -match $script:ReducedMarkerPattern)
}

<#
    .SYNOPSIS
        Walks Text (a capture file's raw content, or a markdown report
        body) as text rather than requiring it to parse as one JSON
        document - the same reason Test-CaptureRedaction works over text:
        a dogfood report body is markdown carrying JSON fragments, not one
        parseable object, and a regex-over-text approach that already works
        for one field-name class extends to five without adding a JSON
        parser dependency this corpus does not otherwise need.

        Deliberately over-catches rather than under-catches: the `path[]`
        array body is matched non-greedily up to the first `]`, so a path
        label containing a literal ']' character would split early and
        could produce a spurious violation on an otherwise-reduced capture.
        A gate that guards a public-repository leak must fail toward "flag
        it" on an ambiguous case, never toward "let it through".
    .OUTPUTS
        String[] - one message per violation found, empty when none.
    #>
function Get-CaptureCliRedactionViolationsFromText {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Text,
        [string]$Source = '<text>'
    )
    $violations = New-Object System.Collections.Generic.List[string]
    $regexOptions = [System.Text.RegularExpressions.RegexOptions]'IgnoreCase, Singleline'

    $fieldPattern = '"(?<field>name|value|description|title)"\s*:\s*"(?<val>(?:\\.|[^"\\])*)"'
    foreach ($m in [regex]::Matches($Text, $fieldPattern, $regexOptions)) {
        $raw = $m.Groups['val'].Value
        if (-not (Test-CliRedactionValueReduced -RawJsonString $raw)) {
            [void]$violations.Add("$Source`: field '$($m.Groups['field'].Value)' holds unreduced content ($($raw.Length) raw chars)")
        }
    }

    $occluderPattern = '"occluder"\s*:\s*\{(?<body>[^{}]*?)\}'
    foreach ($m in [regex]::Matches($Text, $occluderPattern, $regexOptions)) {
        $nameMatch = [regex]::Match($m.Groups['body'].Value, '"name"\s*:\s*"(?<val>(?:\\.|[^"\\])*)"', $regexOptions)
        if ($nameMatch.Success) {
            $raw = $nameMatch.Groups['val'].Value
            if (-not (Test-CliRedactionValueReduced -RawJsonString $raw)) {
                [void]$violations.Add("$Source`: field 'error.details.checks[].occluder.name' holds unreduced content ($($raw.Length) raw chars)")
            }
        }
    }

    $pathArrayPattern = '"path"\s*:\s*\[(?<body>.*?)\]'
    foreach ($m in [regex]::Matches($Text, $pathArrayPattern, $regexOptions)) {
        $elementPattern = '"(?<val>(?:\\.|[^"\\])*)"'
        foreach ($element in [regex]::Matches($m.Groups['body'].Value, $elementPattern, $regexOptions)) {
            $raw = $element.Groups['val'].Value
            if (-not (Test-CliRedactionValueReduced -RawJsonString $raw -AllowRolePrefix)) {
                [void]$violations.Add("$Source`: field 'path[]' holds an unreduced element ($($raw.Length) raw chars)")
            }
        }
    }

    <#
        The corpus safety envelope forbids a pid in any capture, not merely
        an unreduced name - a literal pid is never safe regardless of its
        value, unlike name/value/description/title which are safe once
        reduced. Same field-name set probes/windows/common.ps1's
        Get-NormalizedCapture already bucket-normalizes for diffing, reused
        here as the redaction rule itself rather than only a display
        convenience.
    #>
    $pidPattern = '"(?<field>pid|processid|parentprocessid|ownerprocessid|sessionprocessid)"\s*:\s*"?-?\d+"?'
    foreach ($m in [regex]::Matches($Text, $pidPattern, $regexOptions)) {
        [void]$violations.Add("$Source`: field '$($m.Groups['field'].Value)' holds a literal pid")
    }

    <#
        Write-Output -NoEnumerate, not `return , @(...)`: the comma-wrap
        idiom only survives one function-call boundary. A caller two frames
        up (Get-CaptureCliRedactionViolations calling this, itself called
        by check-capture-redaction.ps1) re-enumerates a comma-wrapped
        return and silently unwraps a 0- or 1-element array back to a bare
        scalar or a doubly-nested array - measured while building this
        module. -NoEnumerate holds across any call depth.
    #>
    Write-Output -NoEnumerate $violations
}

function Get-CaptureCliRedactionViolations {
    param([Parameter(Mandatory = $true)][string]$Path)
    $text = [IO.File]::ReadAllText($Path)
    $violations = Get-CaptureCliRedactionViolationsFromText -Text $text -Source $Path
    Write-Output -NoEnumerate $violations
}

function Test-CaptureCliRedaction {
    <#
    .SYNOPSIS
        R21's CLI-envelope redaction check. True when Path carries no
        unreduced name/value/description/title/path[]/occluder.name
        content; false (with each violation on its own Write-Warning line)
        otherwise. Mirrors Test-CaptureRedaction's bool-return, log-on-
        failure shape.
    #>
    param([Parameter(Mandatory = $true)][string]$Path)
    $violations = Get-CaptureCliRedactionViolations -Path $Path
    if ($violations.Count -gt 0) {
        foreach ($violation in $violations) { Write-Warning $violation }
        return $false
    }
    return $true
}

Export-ModuleMember -Function @(
    'ConvertFrom-CliJsonEscape',
    'Test-CliRedactionValueReduced',
    'Get-CaptureCliRedactionViolationsFromText',
    'Get-CaptureCliRedactionViolations',
    'Test-CaptureCliRedaction'
)
