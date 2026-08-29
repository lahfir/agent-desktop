#Requires -Version 5.1

<#
    capture-redaction-cli.psm1 - R21's CLI-envelope half: Test-CaptureCliRedaction.

    probes/windows/common.ps1's own Test-CaptureRedaction catches the
    machine-wide residue classes (user name, machine name, profile paths,
    domain SIDs). It has no idea that agent-desktop's own JSON envelope
    carries real target content in a small, fixed set of fields regardless
    of which machine produced it: `name`, `value`, `description`, `title`,
    `app_name`, `body`, `attribution` (the serialized NotificationInfo
    field names the shell/notification surfaces publish), every element of
    a `path` array (crates/core/src/commands/find.rs:194
    and live_locator/select.rs's node_label both populate it as
    "role:label"), every element of an `actions` array (notification
    verb-button labels), `error.details.checks[].occluder.name`, and any literal
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

        The `occluder{...}` body tolerates exactly one level of brace
        nesting (a `bounds` sub-object), because the shipped envelope is
        `{role, name, bounds}`, not the flat `{name, topmost}` shape a
        purely brace-free pattern would require - a body regex that cannot
        see past the first `{` inside `bounds` never matches the real
        shape at all, so the occluder.name check would silently check
        nothing on genuine captures.

        The `path[]` array is walked JSON-aware (Find-CliJsonArrayEnd),
        tracking quote state character by character, rather than matched
        with a `.*?` up to the first `]`: a path label's own text can
        legitimately contain a literal ']' (real window/document titles do),
        and a text-only regex would end the array match inside that quoted
        string, leaving every element after it - including a genuinely
        unreduced one - outside the scanned body entirely. The `actions[]`
        array gets the identical JSON-aware walk: notification verb-button
        labels are plain strings, so unlike `path[]` there is no role
        prefix to allow, and a label element that does not decode to the
        reduced marker or the unnamed sentinel fails closed.
    .OUTPUTS
        String[] - one message per violation found, empty when none.
    #>
function Find-CliJsonArrayEnd {
    <#
    .SYNOPSIS
        Given Text and the index of the character immediately after a
        JSON array's opening `[`, returns the index of that array's true
        matching `]`, respecting quoted-string content (so a literal ']'
        inside an element's own text never ends the array early) and
        escaped characters within those strings. Returns -1 if the array
        never closes.
    #>
    param([Parameter(Mandatory = $true)][string]$Text, [Parameter(Mandatory = $true)][int]$StartIndex)
    $depth = 1
    $inString = $false
    $i = $StartIndex
    while ($i -lt $Text.Length) {
        $ch = $Text[$i]
        if ($inString) {
            if ($ch -eq '\') { $i++ }
            elseif ($ch -eq '"') { $inString = $false }
        } else {
            if ($ch -eq '"') { $inString = $true }
            elseif ($ch -eq '[') { $depth++ }
            elseif ($ch -eq ']') {
                $depth--
                if ($depth -eq 0) { return $i }
            }
        }
        $i++
    }
    return -1
}

function Get-CaptureCliRedactionViolationsFromText {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Text,
        [string]$Source = '<text>'
    )
    $violations = New-Object System.Collections.Generic.List[string]
    $regexOptions = [System.Text.RegularExpressions.RegexOptions]'IgnoreCase, Singleline'

    $fieldPattern = '"(?<field>name|value|description|title|app_name|body|attribution)"\s*:\s*"(?<val>(?:\\.|[^"\\])*)"'
    foreach ($m in [regex]::Matches($Text, $fieldPattern, $regexOptions)) {
        $raw = $m.Groups['val'].Value
        if (-not (Test-CliRedactionValueReduced -RawJsonString $raw)) {
            [void]$violations.Add("$Source`: field '$($m.Groups['field'].Value)' holds unreduced content ($($raw.Length) raw chars)")
        }
    }

    $occluderPattern = '"occluder"\s*:\s*\{(?<body>(?:[^{}]|\{[^{}]*\})*)\}'
    foreach ($m in [regex]::Matches($Text, $occluderPattern, $regexOptions)) {
        $nameMatch = [regex]::Match($m.Groups['body'].Value, '"name"\s*:\s*"(?<val>(?:\\.|[^"\\])*)"', $regexOptions)
        if ($nameMatch.Success) {
            $raw = $nameMatch.Groups['val'].Value
            if (-not (Test-CliRedactionValueReduced -RawJsonString $raw)) {
                [void]$violations.Add("$Source`: field 'error.details.checks[].occluder.name' holds unreduced content ($($raw.Length) raw chars)")
            }
        }
    }

    $pathArrayOpenPattern = '"path"\s*:\s*\['
    foreach ($m in [regex]::Matches($Text, $pathArrayOpenPattern, $regexOptions)) {
        $bodyStart = $m.Index + $m.Length
        $endIndex = Find-CliJsonArrayEnd -Text $Text -StartIndex $bodyStart
        if ($endIndex -lt 0) { continue }
        $body = $Text.Substring($bodyStart, $endIndex - $bodyStart)
        $elementPattern = '"(?<val>(?:\\.|[^"\\])*)"'
        foreach ($element in [regex]::Matches($body, $elementPattern, $regexOptions)) {
            $raw = $element.Groups['val'].Value
            if (-not (Test-CliRedactionValueReduced -RawJsonString $raw -AllowRolePrefix)) {
                [void]$violations.Add("$Source`: field 'path[]' holds an unreduced element ($($raw.Length) raw chars)")
            }
        }
    }

    <#
        Notification verb-button labels are plain strings ("Reply", "Archive"),
        not node_label's "role:label" composition, so no AllowRolePrefix here:
        stripping before the first ':' would let a label whose own text leads
        with a colon-prefixed fragment through unchecked. Fail closed on the
        same rule the scalar fields use.
    #>
    $actionsArrayOpenPattern = '"actions"\s*:\s*\['
    foreach ($m in [regex]::Matches($Text, $actionsArrayOpenPattern, $regexOptions)) {
        $bodyStart = $m.Index + $m.Length
        $endIndex = Find-CliJsonArrayEnd -Text $Text -StartIndex $bodyStart
        if ($endIndex -lt 0) { continue }
        $body = $Text.Substring($bodyStart, $endIndex - $bodyStart)
        $elementPattern = '"(?<val>(?:\\.|[^"\\])*)"'
        foreach ($element in [regex]::Matches($body, $elementPattern, $regexOptions)) {
            $raw = $element.Groups['val'].Value
            if (-not (Test-CliRedactionValueReduced -RawJsonString $raw)) {
                [void]$violations.Add("$Source`: field 'actions[]' holds an unreduced element ($($raw.Length) raw chars)")
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
        unreduced name/value/description/title/app_name/body/attribution/
        path[]/actions[]/occluder.name
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
    'Find-CliJsonArrayEnd',
    'Get-CaptureCliRedactionViolationsFromText',
    'Get-CaptureCliRedactionViolations',
    'Test-CaptureCliRedaction'
)
