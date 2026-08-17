#Requires -Version 5.1
<#
.SYNOPSIS
    Row-versus-capture content helpers for 13-ledger-check.ps1.

.DESCRIPTION
    Shared program text: both the live ledger gate and its MUST-CATCH/MUST-PASS
    self-test call these functions. A row is audited two ways, and failing
    either fails the row:
      1. Quoted `field: value` pairs (Get-QuotedFieldValuePairs), resolved
         against a capture. The capture is the leaf the row names by filename
         when one is cited (Get-CitedCaptureLeaves); when the row cites no
         `.json` leaf, it falls back to every capture reachable from the
         row's own script path - the area directory is in that path
         (Get-RowScriptAreaDir / Get-CaptureObjectsForAreaDir) - because a
         script citation still names a discoverable capture even without a
         filename.
      2. A measured value stated in bare prose, anchored to one of a small,
         explicit set of counting verbs immediately before the number
         (`enumerates 133 processes`, `agrees at 133` referencing the noun
         the nearest prior `enumerates`/`totals` clause established) -
         Get-VerbAnchoredNumericCandidates. This is deliberately narrow: an
         earlier, broader design that matched any bare number against any
         nearby capture-leaf-key-shaped word audited materially more rows but
         produced dozens of false failures against unrelated numbers sharing
         a coincidental word (percentages, timings, row/version references),
         which is exactly the over-broad predicate this gate must not ship.
    A row with neither a capture citation (explicit or script-derived) nor a
    verb-anchored prose number is exempt - it asserts nothing this gate can
    check against a capture, and `C-*` external-documentation rows are always
    exempt.
#>

function Get-QuotedFieldValuePairs {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Text)
    $pairs = New-Object System.Collections.Generic.List[object]
    # Value excludes `(`/`)` and a leading second colon: both exclude source-code
    # citations (`code: e.code().0`, Rust `Type::method`) that share the backtick
    # `field: value` shape but are quoting implementation, not a captured value.
    foreach ($m in [regex]::Matches($Text, '`([A-Za-z_][A-Za-z0-9_.]*):(?!:)\s*([^`()]+)`')) {
        $pairs.Add([pscustomobject]@{
                Field = $m.Groups[1].Value
                Value = $m.Groups[2].Value.Trim()
            }) | Out-Null
    }
    return $pairs.ToArray()
}

function Get-CitedCaptureLeaves {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Text)
    $leaves = New-Object System.Collections.Generic.List[string]
    foreach ($m in [regex]::Matches($Text, '([A-Za-z0-9_\-./\\*]+\.json)')) {
        $leaf = Split-Path -Leaf $m.Groups[1].Value
        if (-not $leaves.Contains($leaf)) { $leaves.Add($leaf) | Out-Null }
    }
    return $leaves.ToArray()
}

function Get-RowScriptAreaDir {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Script)
    $clean = $Script.Trim().Trim('`')
    $m = [regex]::Match($clean, '^([0-9]{1,2}-[A-Za-z][A-Za-z0-9_\-]*)[\\/]')
    if ($m.Success) { return $m.Groups[1].Value }
    $m2 = [regex]::Match($clean, '^([0-9]{1,2}-[A-Za-z][A-Za-z0-9_\-]*)\.[A-Za-z0-9]+$')
    if ($m2.Success) { return $m2.Groups[1].Value }
    return $null
}

function Get-CaptureObjectsForAreaDir {
    param(
        [Parameter(Mandatory = $true)][string]$ProbeRoot,
        [Parameter(Mandatory = $true)][string]$AreaDir
    )
    $objects = New-Object System.Collections.Generic.List[object]
    $candidateDirs = @(
        (Join-Path (Join-Path $ProbeRoot $AreaDir) 'captures'),
        (Join-Path (Join-Path $ProbeRoot 'captures') $AreaDir)
    )
    foreach ($dir in $candidateDirs) {
        if (-not (Test-Path -LiteralPath $dir)) { continue }
        $files = Get-ChildItem -Path $dir -Filter *.json -File -ErrorAction SilentlyContinue |
            Where-Object { $_.Name -notlike '*.normalized' }
        foreach ($f in $files) {
            try {
                $raw = [IO.File]::ReadAllText($f.FullName)
                $objects.Add((ConvertFrom-Json -InputObject $raw)) | Out-Null
            } catch { }
        }
    }
    return $objects.ToArray()
}

function Get-KeyNameTokens {
    param([Parameter(Mandatory = $true)][string]$Key)
    $spaced = [regex]::Replace($Key, '(?<=[a-z0-9])(?=[A-Z])', ' ')
    $spaced = $spaced -replace '[^A-Za-z0-9]+', ' '
    $tokens = New-Object System.Collections.Generic.List[string]
    foreach ($w in ($spaced -split '\s+')) {
        $t = $w.Trim().ToLowerInvariant()
        if ($t.Length -ge 3 -and $t -notmatch '^\d+$') { $tokens.Add($t) | Out-Null }
    }
    return $tokens.ToArray()
}

function Get-SingularToken {
    param([Parameter(Mandatory = $true)][string]$Word)
    $w = ($Word.Trim().ToLowerInvariant() -replace '[^a-z0-9]', '')
    if ($w.Length -lt 4) { return $w }
    if ($w -match '(sses|xes|ches|shes|zes)$') { return $w.Substring(0, $w.Length - 2) }
    if ($w -match 'ies$') { return $w.Substring(0, $w.Length - 3) + 'y' }
    if ($w.EndsWith('s') -and -not $w.EndsWith('ss')) { return $w.Substring(0, $w.Length - 1) }
    return $w
}

function Add-JsonLeafTokens {
    param(
        [AllowNull()]$Node,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$KeyName,
        [Parameter(Mandatory = $true)][hashtable]$Index
    )
    if ($null -eq $Node) { return }
    if ($Node -is [string]) {
        return
    }
    if ($Node -is [System.Collections.IDictionary]) {
        foreach ($k in $Node.Keys) { Add-JsonLeafTokens -Node $Node[$k] -KeyName ([string]$k) -Index $Index }
        return
    }
    if ($Node -is [System.Collections.IEnumerable]) {
        foreach ($item in $Node) { Add-JsonLeafTokens -Node $item -KeyName $KeyName -Index $Index }
        return
    }
    if ($Node -is [System.Management.Automation.PSCustomObject]) {
        foreach ($p in $Node.PSObject.Properties) { Add-JsonLeafTokens -Node $p.Value -KeyName $p.Name -Index $Index }
        return
    }
    if (-not $KeyName) { return }
    foreach ($tok in (Get-KeyNameTokens -Key $KeyName)) {
        if (-not $Index.ContainsKey($tok)) { $Index[$tok] = New-Object System.Collections.Generic.List[object] }
        $Index[$tok].Add($Node) | Out-Null
    }
}

function Get-LeafTokenIndex {
    param([AllowNull()]$CaptureObjects)
    $index = @{}
    foreach ($obj in $CaptureObjects) { Add-JsonLeafTokens -Node $obj -KeyName '' -Index $index }
    return $index
}

function Get-VerbAnchoredNumericCandidates {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Text)
    $candidates = New-Object System.Collections.Generic.List[object]
    $pattern = '\b(?:enumerates|totals?|totalling)\s+(?<num1>\d{1,7})\s+(?<noun>[A-Za-z][A-Za-z0-9_]{2,30})|\bagrees at\s+(?<num2>\d{1,7})\b'
    $lastNoun = $null
    foreach ($m in [regex]::Matches($Text, $pattern)) {
        if ($m.Groups['noun'].Success) {
            $noun = $m.Groups['noun'].Value
            $candidates.Add([pscustomobject]@{ Number = $m.Groups['num1'].Value; Noun = $noun }) | Out-Null
            $lastNoun = $noun
        } elseif ($m.Groups['num2'].Success -and $lastNoun) {
            $candidates.Add([pscustomobject]@{ Number = $m.Groups['num2'].Value; Noun = $lastNoun }) | Out-Null
        }
    }
    return $candidates.ToArray()
}

function Resolve-CapturePathsForLeaf {
    param(
        [Parameter(Mandatory = $true)][string]$ProbeRoot,
        [Parameter(Mandatory = $true)][string]$Leaf
    )
    $found = New-Object System.Collections.Generic.List[string]
    if ($Leaf.Contains('*')) {
        $all = Get-ChildItem -Path $ProbeRoot -Recurse -Filter *.json -File -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -match '[\\/]captures[\\/]' -and $_.Name -notlike '*.normalized' -and $_.Name -like $Leaf }
        foreach ($f in $all) { $found.Add($f.FullName) | Out-Null }
        return $found.ToArray()
    }
    $exact = Get-ChildItem -Path $ProbeRoot -Recurse -Filter $Leaf -File -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match '[\\/]captures[\\/]' -and $_.Name -notlike '*.normalized' }
    foreach ($f in $exact) { $found.Add($f.FullName) | Out-Null }
    return $found.ToArray()
}

function Convert-LedgerValueToken {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Token)
    $t = $Token.Trim().TrimEnd('.', ',', ';')
    if ($t -eq 'true') { return $true }
    if ($t -eq 'false') { return $false }
    if ($t -eq 'null') { return $null }
    if ($t -match '^-?\d+$') { return [int64]$t }
    if ($t -match '^-?\d+\.\d+$') { return [double]$t }
    if (($t.StartsWith('"') -and $t.EndsWith('"')) -or ($t.StartsWith("'") -and $t.EndsWith("'"))) {
        return $t.Substring(1, $t.Length - 2)
    }
    return $t
}

function Get-JsonPropertyValuesByName {
    param(
        [AllowNull()]$Node,
        [Parameter(Mandatory = $true)][string]$Name
    )
    $hits = New-Object System.Collections.Generic.List[object]
    if ($null -eq $Node) { return $hits.ToArray() }
    if ($Node -is [System.Collections.IDictionary]) {
        foreach ($key in $Node.Keys) {
            if ([string]$key -eq $Name) { $hits.Add($Node[$key]) | Out-Null }
            foreach ($child in @(Get-JsonPropertyValuesByName -Node $Node[$key] -Name $Name)) {
                $hits.Add($child) | Out-Null
            }
        }
        return $hits.ToArray()
    }
    if ($Node -is [System.Collections.IEnumerable] -and -not ($Node -is [string])) {
        foreach ($item in $Node) {
            foreach ($child in @(Get-JsonPropertyValuesByName -Node $item -Name $Name)) {
                $hits.Add($child) | Out-Null
            }
        }
        return $hits.ToArray()
    }
    $props = $Node.PSObject.Properties
    if ($null -eq $props) { return $hits.ToArray() }
    foreach ($p in $props) {
        if ($p.Name -eq $Name) { $hits.Add($p.Value) | Out-Null }
        foreach ($child in @(Get-JsonPropertyValuesByName -Node $p.Value -Name $Name)) {
            $hits.Add($child) | Out-Null
        }
    }
    return $hits.ToArray()
}

function Get-JsonValueAtDottedPath {
    param(
        [AllowNull()]$Node,
        [Parameter(Mandatory = $true)][string]$Path
    )
    $cur = $Node
    foreach ($part in ($Path -split '\.')) {
        if ($null -eq $cur) { return $null }
        if ($cur -is [System.Collections.IDictionary]) {
            if (-not $cur.Contains($part)) { return $null }
            $cur = $cur[$part]
            continue
        }
        $prop = $cur.PSObject.Properties[$part]
        if ($null -eq $prop) { return $null }
        $cur = $prop.Value
    }
    return $cur
}

function Test-JsonValuesAgree {
    param(
        [AllowNull()]$Actual,
        [AllowNull()]$Expected
    )
    if ($null -eq $Actual -and $null -eq $Expected) { return $true }
    if ($null -eq $Actual -or $null -eq $Expected) { return $false }
    if ($Actual -is [bool] -or $Expected -is [bool]) {
        return ([bool]$Actual) -eq ([bool]$Expected)
    }
    if (($Actual -is [ValueType]) -and ($Expected -is [ValueType]) -and -not ($Actual -is [char]) -and -not ($Expected -is [char])) {
        return [double]$Actual -eq [double]$Expected
    }
    $a = ([string]$Actual).Replace('/', '\').TrimEnd('\')
    $e = ([string]$Expected).Replace('/', '\').TrimEnd('\')
    return ($a -ieq $e)
}

function Test-CaptureContainsFieldValue {
    param(
        [AllowNull()]$CaptureObject,
        [Parameter(Mandatory = $true)][string]$Field,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$ValueText
    )
    if ($null -eq $CaptureObject) { return $false }
    $expected = Convert-LedgerValueToken -Token $ValueText
    if ($Field -match '\.') {
        $actual = Get-JsonValueAtDottedPath -Node $CaptureObject -Path $Field
        return (Test-JsonValuesAgree -Actual $actual -Expected $expected)
    }
    $leaf = ($Field -split '\.')[-1]
    foreach ($actual in @(Get-JsonPropertyValuesByName -Node $CaptureObject -Name $leaf)) {
        if (Test-JsonValuesAgree -Actual $actual -Expected $expected) { return $true }
    }
    return $false
}

function Test-CaptureFieldExistsAnywhere {
    param(
        [AllowNull()]$CaptureObjects,
        [Parameter(Mandatory = $true)][string]$Field
    )
    foreach ($obj in $CaptureObjects) {
        if ($Field -match '\.') {
            if ($null -ne (Get-JsonValueAtDottedPath -Node $obj -Path $Field)) { return $true }
            continue
        }
        $leaf = ($Field -split '\.')[-1]
        if (@(Get-JsonPropertyValuesByName -Node $obj -Name $leaf).Count -gt 0) { return $true }
    }
    return $false
}

function Test-RowCaptureContent-CitedLeafPairs {
    param(
        [Parameter(Mandatory = $true)]$Row,
        [Parameter(Mandatory = $true)][string]$ProbeRoot,
        [Parameter(Mandatory = $true)][string]$Blob
    )
    $result = [pscustomobject]@{ Audited = $false; Failures = @() }
    if ($Row.Id -match '^C-') {
        return $result
    }
    $pairs = @(Get-QuotedFieldValuePairs -Text $Blob)
    if ($pairs.Count -eq 0) {
        return $result
    }
    $failures = New-Object System.Collections.Generic.List[string]
    $captureObjects = New-Object System.Collections.Generic.List[object]
    $leaves = @(Get-CitedCaptureLeaves -Text $Blob)
    if ($leaves.Count -gt 0) {
        foreach ($leaf in $leaves) {
            $paths = @(Resolve-CapturePathsForLeaf -ProbeRoot $ProbeRoot -Leaf $leaf)
            if ($paths.Count -eq 0) {
                $failures.Add($Row.Id + ': cited capture leaf ' + $leaf + ' not found under captures/') | Out-Null
                continue
            }
            foreach ($path in $paths) {
                $raw = [IO.File]::ReadAllText($path)
                $captureObjects.Add((ConvertFrom-Json -InputObject $raw)) | Out-Null
            }
        }
        if ($captureObjects.Count -eq 0) {
            $result.Audited = $true
            $result.Failures = $failures.ToArray()
            return $result
        }
        # Explicit citation: the row named this exact file, so every quoted pair is
        # checked strictly against it - an absent field is as much a defect as a wrong
        # value, unchanged from before the widening.
        $result.Audited = $true
        foreach ($pair in $pairs) {
            $matched = $false
            foreach ($obj in $captureObjects) {
                if (Test-CaptureContainsFieldValue -CaptureObject $obj -Field $pair.Field -ValueText $pair.Value) {
                    $matched = $true
                    break
                }
            }
            if (-not $matched) {
                $failures.Add($Row.Id + ': quoted pair `' + $pair.Field + ': ' + $pair.Value + '` not found in cited capture(s)') | Out-Null
            }
        }
        $result.Failures = $failures.ToArray()
        return $result
    }
    # No `.json` leaf named - fall back to every capture reachable from the row's own
    # script path (KTD12: "the area directory is in that path"). This citation is
    # implicit, so a pair whose field name matches nothing anywhere in the area is
    # treated as a narrative colon this gate cannot attribute to a capture (skipped,
    # not audited), not as a broken citation - only a pair whose field genuinely
    # exists somewhere in the area, with the wrong value, is a real, auditable defect.
    $areaDir = Get-RowScriptAreaDir -Script ([string]$Row.Script)
    if (-not $areaDir) {
        return $result
    }
    foreach ($obj in @(Get-CaptureObjectsForAreaDir -ProbeRoot $ProbeRoot -AreaDir $areaDir)) {
        $captureObjects.Add($obj) | Out-Null
    }
    if ($captureObjects.Count -eq 0) {
        return $result
    }
    foreach ($pair in $pairs) {
        if (-not (Test-CaptureFieldExistsAnywhere -CaptureObjects $captureObjects -Field $pair.Field)) {
            continue
        }
        $result.Audited = $true
        $matched = $false
        foreach ($obj in $captureObjects) {
            if (Test-CaptureContainsFieldValue -CaptureObject $obj -Field $pair.Field -ValueText $pair.Value) {
                $matched = $true
                break
            }
        }
        if (-not $matched) {
            $failures.Add($Row.Id + ': quoted pair `' + $pair.Field + ': ' + $pair.Value + '` exists in a capture reachable from `' + $Row.Script + '` (area ' + $areaDir + ') but not with this value') | Out-Null
        }
    }
    $result.Failures = $failures.ToArray()
    return $result
}

function Test-RowCaptureContent-ProseStatedValues {
    param(
        [Parameter(Mandatory = $true)]$Row,
        [Parameter(Mandatory = $true)][string]$ProbeRoot,
        [Parameter(Mandatory = $true)][string]$Blob
    )
    $result = [pscustomobject]@{ Audited = $false; Failures = @() }
    if ($Row.Id -match '^C-') {
        return $result
    }
    $candidates = @(Get-VerbAnchoredNumericCandidates -Text $Blob)
    if ($candidates.Count -eq 0) {
        return $result
    }
    $areaDir = Get-RowScriptAreaDir -Script ([string]$Row.Script)
    if (-not $areaDir) {
        return $result
    }
    $captureObjects = @(Get-CaptureObjectsForAreaDir -ProbeRoot $ProbeRoot -AreaDir $areaDir)
    if ($captureObjects.Count -eq 0) {
        return $result
    }
    $tokenIndex = Get-LeafTokenIndex -CaptureObjects $captureObjects
    $failures = New-Object System.Collections.Generic.List[string]
    foreach ($cand in $candidates) {
        $matchedValues = New-Object System.Collections.Generic.List[object]
        foreach ($sub in (Get-KeyNameTokens -Key $cand.Noun)) {
            foreach ($tv in (@($sub, (Get-SingularToken -Word $sub)) | Select-Object -Unique)) {
                if ($tokenIndex.ContainsKey($tv)) { foreach ($v in $tokenIndex[$tv]) { $matchedValues.Add($v) | Out-Null } }
            }
        }
        if ($matchedValues.Count -eq 0) {
            continue
        }
        $result.Audited = $true
        $expected = Convert-LedgerValueToken -Token $cand.Number
        $agrees = $false
        foreach ($v in $matchedValues) {
            if (Test-JsonValuesAgree -Actual $v -Expected $expected) { $agrees = $true; break }
        }
        if (-not $agrees) {
            $seen = ($matchedValues | Select-Object -Unique) -join ', '
            $failures.Add($Row.Id + ': prose states `' + $cand.Number + ' ' + $cand.Noun + '` but no capture leaf reachable from `' + $Row.Script + '` (area ' + $areaDir + ') agrees - values seen: ' + $seen) | Out-Null
        }
    }
    $result.Failures = $failures.ToArray()
    return $result
}

function Test-RowCaptureContent {
    param(
        [Parameter(Mandatory = $true)]$Row,
        [Parameter(Mandatory = $true)][string]$ProbeRoot
    )
    $blob = [string]$Row.Observed + ' ' + [string]$Row.Script + ' ' + [string]$Row.Action
    $proseBlob = [string]$Row.Observed + ' ' + [string]$Row.Action
    $cited = Test-RowCaptureContent-CitedLeafPairs -Row $Row -ProbeRoot $ProbeRoot -Blob $blob
    $prose = Test-RowCaptureContent-ProseStatedValues -Row $Row -ProbeRoot $ProbeRoot -Blob $proseBlob
    $failures = New-Object System.Collections.Generic.List[string]
    foreach ($f in $cited.Failures) { $failures.Add($f) | Out-Null }
    foreach ($f in $prose.Failures) { $failures.Add($f) | Out-Null }
    return [pscustomobject]@{
        Audited  = ($cited.Audited -or $prose.Audited)
        Failures = $failures.ToArray()
    }
}

function Invoke-LedgerContentSelfTest {
    param([Parameter(Mandatory = $true)][string]$ProbeRoot)
    $failures = New-Object System.Collections.Generic.List[string]
    $tempRoot = Join-Path $env:TEMP ('ledger-content-selftest-' + [guid]::NewGuid().ToString('N'))
    $capDir = Join-Path $tempRoot '16-observation\captures'
    New-Item -ItemType Directory -Path $capDir -Force | Out-Null
    try {
        $census = [ordered]@{
            window_census = [ordered]@{
                total_enumerated = 147
                by_factor        = [ordered]@{
                    invisible         = 137
                    zero_size         = 81
                    iconic            = 8
                    visible_nonempty  = 66
                    cloaked           = 6
                    tool              = 43
                }
            }
            virtual_desktop_manager = [ordered]@{
                clsid_registered     = $true
                clsid_inproc_server  = 'C:\Windows\system32\twinapi.dll'
            }
            process_enumeration = [ordered]@{
                toolhelp_process_count = 133
                cim_process_count      = 133
            }
        }
        $censusPath = Join-Path $capDir 'observation-census-devbox.json'
        [IO.File]::WriteAllText($censusPath, (ConvertTo-Json -InputObject $census -Depth 8))

        $wrongCounts = [pscustomobject]@{
            Id       = 'A16-1-wrong-counts'
            Script   = '16-observation/census.ps1'
            Observed = 'observation-census-devbox.json `zero_size: 93` `visible_nonempty: 68` `tool: 51`'
            Action   = ''
        }
        $absentField = [pscustomobject]@{
            Id       = 'A16-9-absent-field'
            Script   = '16-observation/census.ps1'
            Observed = 'observation-census-devbox.json `threading_model: Both`'
            Action   = ''
        }
        $corrected = [pscustomobject]@{
            Id       = 'A16-1-corrected'
            Script   = '16-observation/census.ps1'
            Observed = 'observation-census-devbox.json `zero_size: 81` `visible_nonempty: 66` `tool: 43`'
            Action   = ''
        }
        $proseOnly = [pscustomobject]@{
            Id       = 'C-8'
            Script   = 'Windows.Graphics.Capture API documentation'
            Observed = '1903 is the base floor; cursor toggle requires 19041+'
            Action   = ''
        }
        # A16-3's exact prose shape: a measured value stated in bare prose, disagreeing
        # with its capture, citing a script (`16-observation/census.ps1`) rather than a
        # capture leaf filename. This is the shape that evaded the old
        # pairs-plus-cited-leaf-only check before the widening and must fail under it.
        $proseWrong = [pscustomobject]@{
            Id       = 'A16-3-wrong-prose'
            Script   = '`16-observation/census.ps1`'
            Observed = 'a Toolhelp32 snapshot (`CreateToolhelp32Snapshot` + `Process32FirstW/NextW`) enumerates 132 processes with a non-empty image name; `Get-CimInstance Win32_Process` agrees at 132; the probe process''s own `StartTime` is present'
            Action   = 'CONFIRMS'
        }
        $proseCorrected = [pscustomobject]@{
            Id       = 'A16-3-corrected-prose'
            Script   = '`16-observation/census.ps1`'
            Observed = 'a Toolhelp32 snapshot (`CreateToolhelp32Snapshot` + `Process32FirstW/NextW`) enumerates 133 processes with a non-empty image name; `Get-CimInstance Win32_Process` agrees at 133; the probe process''s own `StartTime` is present'
            Action   = 'CONFIRMS'
        }
        # Cites a real script/area but every prose number's noun is unrelated to any
        # capture leaf key in that area - genuinely nothing measurable for this gate to
        # check, and must stay exempt rather than being coerced into a false failure.
        $proseNothingMeasurable = [pscustomobject]@{
            Id       = 'A16-2-nothing-measurable'
            Script   = '`16-observation/census.ps1`'
            Observed = 'the probe waited 20 seconds for the shell to settle before reading the foreground window a second time'
            Action   = ''
        }
        # Fallback-path asymmetry (A8-1's real shape): the field name exists nowhere in
        # any capture reachable from the script-area fallback, so this is a narrative
        # colon this gate cannot attribute to a capture, not a broken citation - unlike
        # the explicit-citation case (`$absentField` below), which fails on exactly this
        # shape because the row named a specific file. Must be silently exempt, never a
        # false failure, and this asymmetry is the whole point of the existence gate.
        $pairFallbackFieldAbsentEverywhere = [pscustomobject]@{
            Id       = 'A8-1-fallback-field-absent'
            Script   = '`16-observation/census.ps1`'
            Observed = '`handler: PropertyChanged`'
            Action   = ''
        }
        # A14-3's real shape: the field (`toolhelp_process_count`) genuinely exists in
        # the reachable capture, but the quoted value is a source-code expression
        # (Rust method-call-with-parens), not a captured scalar. Must never become a
        # candidate pair at all - proves the value-side regex exclusion, not just the
        # existence gate, and is the fixture that fails if `[^`()]+` regresses to
        # `[^`]+`.
        $pairFallbackSourceCodeValue = [pscustomobject]@{
            Id       = 'A14-3-fallback-source-code-value'
            Script   = '`16-observation/census.ps1`'
            Observed = '`toolhelp_process_count: get_count().0`'
            Action   = ''
        }
        # Quoted `field: value` pairs (the pre-widening, already-trusted shape) but citing
        # only the script - no `.json` leaf filename anywhere in the row. Exercises part
        # (a) of the widening: leaf resolution falls back to every capture reachable from
        # the row's own script-area path.
        $pairFallbackWrong = [pscustomobject]@{
            Id       = 'A16-1-pair-fallback-wrong'
            Script   = '`16-observation/census.ps1`'
            Observed = '`zero_size: 93` `visible_nonempty: 68` `tool: 51`'
            Action   = ''
        }
        $pairFallbackCorrected = [pscustomobject]@{
            Id       = 'A16-1-pair-fallback-corrected'
            Script   = '`16-observation/census.ps1`'
            Observed = '`zero_size: 81` `visible_nonempty: 66` `tool: 43`'
            Action   = ''
        }

        $wrong = Test-RowCaptureContent -Row $wrongCounts -ProbeRoot $tempRoot
        if ($wrong.Failures.Count -lt 1) {
            $failures.Add('MUST CATCH, missed: A16-1-wrong-counts did not fail') | Out-Null
        }
        $absent = Test-RowCaptureContent -Row $absentField -ProbeRoot $tempRoot
        if ($absent.Failures.Count -lt 1) {
            $failures.Add('MUST CATCH, missed: A16-9-absent-field did not fail') | Out-Null
        }
        $ok = Test-RowCaptureContent -Row $corrected -ProbeRoot $tempRoot
        if ($ok.Failures.Count -gt 0) {
            $failures.Add('MUST PASS, false positive: corrected row failed (' + ($ok.Failures -join '; ') + ')') | Out-Null
        }
        $cRow = Test-RowCaptureContent -Row $proseOnly -ProbeRoot $tempRoot
        if ($cRow.Audited -or $cRow.Failures.Count -gt 0) {
            $failures.Add('MUST PASS, false positive: prose-only C-row was not exempt') | Out-Null
        }
        $proseWrongResult = Test-RowCaptureContent -Row $proseWrong -ProbeRoot $tempRoot
        if (-not $proseWrongResult.Audited) {
            $failures.Add('MUST CATCH, missed: A16-3-wrong-prose (bare prose value, script-only citation) was not audited at all') | Out-Null
        } elseif ($proseWrongResult.Failures.Count -lt 1) {
            $failures.Add('MUST CATCH, missed: A16-3-wrong-prose (bare prose 132 vs capture 133) did not fail') | Out-Null
        }
        $proseCorrectedResult = Test-RowCaptureContent -Row $proseCorrected -ProbeRoot $tempRoot
        if ($proseCorrectedResult.Failures.Count -gt 0) {
            $failures.Add('MUST PASS, false positive: A16-3-corrected-prose failed (' + ($proseCorrectedResult.Failures -join '; ') + ')') | Out-Null
        }
        if (-not $proseCorrectedResult.Audited) {
            $failures.Add('MUST PASS, coverage regression: A16-3-corrected-prose (script-cited bare prose value) was not audited') | Out-Null
        }
        $proseNothingResult = Test-RowCaptureContent -Row $proseNothingMeasurable -ProbeRoot $tempRoot
        if ($proseNothingResult.Audited -or $proseNothingResult.Failures.Count -gt 0) {
            $failures.Add('MUST PASS, false positive: A16-2-nothing-measurable (unrelated prose numbers) was not exempt') | Out-Null
        }
        $pairFallbackWrongResult = Test-RowCaptureContent -Row $pairFallbackWrong -ProbeRoot $tempRoot
        if (-not $pairFallbackWrongResult.Audited) {
            $failures.Add('MUST CATCH, missed: A16-1-pair-fallback-wrong (quoted pairs, script-only citation) was not audited at all') | Out-Null
        } elseif ($pairFallbackWrongResult.Failures.Count -lt 1) {
            $failures.Add('MUST CATCH, missed: A16-1-pair-fallback-wrong did not fail') | Out-Null
        }
        $pairFallbackCorrectedResult = Test-RowCaptureContent -Row $pairFallbackCorrected -ProbeRoot $tempRoot
        if (-not $pairFallbackCorrectedResult.Audited) {
            $failures.Add('MUST PASS, coverage regression: A16-1-pair-fallback-corrected (script-only citation) was not audited') | Out-Null
        }
        if ($pairFallbackCorrectedResult.Failures.Count -gt 0) {
            $failures.Add('MUST PASS, false positive: A16-1-pair-fallback-corrected failed (' + ($pairFallbackCorrectedResult.Failures -join '; ') + ')') | Out-Null
        }
        $fieldAbsentResult = Test-RowCaptureContent -Row $pairFallbackFieldAbsentEverywhere -ProbeRoot $tempRoot
        if ($fieldAbsentResult.Audited -or $fieldAbsentResult.Failures.Count -gt 0) {
            $failures.Add('MUST PASS, false positive: A8-1-fallback-field-absent (field absent from every reachable capture) was not exempt') | Out-Null
        }
        $sourceCodeValueResult = Test-RowCaptureContent -Row $pairFallbackSourceCodeValue -ProbeRoot $tempRoot
        if ($sourceCodeValueResult.Audited -or $sourceCodeValueResult.Failures.Count -gt 0) {
            $failures.Add('MUST PASS, false positive: A14-3-fallback-source-code-value (parenthesised source-code value on an existing field) was not exempt') | Out-Null
        }
    } finally {
        try { Remove-Item -LiteralPath $tempRoot -Recurse -Force -ErrorAction SilentlyContinue } catch { }
    }
    return [pscustomobject]@{ Failures = $failures.ToArray() }
}
