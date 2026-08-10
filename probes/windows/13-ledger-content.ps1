#Requires -Version 5.1
<#
.SYNOPSIS
    Row-versus-capture content helpers for 13-ledger-check.ps1.

.DESCRIPTION
    Shared program text: both the live ledger gate and its MUST-CATCH/MUST-PASS
    self-test call these functions. A capture-citing row fails when a quoted
    `field: value` pair is absent from the cited JSON. Prose-only / C-series
    external-doc rows are exempt.
#>

function Get-QuotedFieldValuePairs {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Text)
    $pairs = New-Object System.Collections.Generic.List[object]
    foreach ($m in [regex]::Matches($Text, '`([A-Za-z_][A-Za-z0-9_.]*):\s*([^`]+)`')) {
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

function Test-RowIsProseOnlyExempt {
    param(
        [Parameter(Mandatory = $true)]$Row
    )
    if ($Row.Id -match '^C-') { return $true }
    $blob = [string]$Row.Observed + ' ' + [string]$Row.Script + ' ' + [string]$Row.Action
    $citations = @(Get-CitedCaptureLeaves -Text $blob)
    return ($citations.Count -eq 0)
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

function Test-RowCaptureContent {
    param(
        [Parameter(Mandatory = $true)]$Row,
        [Parameter(Mandatory = $true)][string]$ProbeRoot
    )
    $result = [pscustomobject]@{
        Audited  = $false
        Failures = @()
    }
    if (Test-RowIsProseOnlyExempt -Row $Row) {
        return $result
    }
    $blob = [string]$Row.Observed + ' ' + [string]$Row.Script + ' ' + [string]$Row.Action
    $pairs = @(Get-QuotedFieldValuePairs -Text $blob)
    if ($pairs.Count -eq 0) {
        return $result
    }
    $leaves = @(Get-CitedCaptureLeaves -Text $blob)
    if ($leaves.Count -eq 0) {
        return $result
    }
    $result.Audited = $true
    $failures = New-Object System.Collections.Generic.List[string]
    $captureObjects = New-Object System.Collections.Generic.List[object]
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
        $result.Failures = $failures.ToArray()
        return $result
    }
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
    } finally {
        try { Remove-Item -LiteralPath $tempRoot -Recurse -Force -ErrorAction SilentlyContinue } catch { }
    }
    return [pscustomobject]@{ Failures = $failures.ToArray() }
}
