#Requires -Version 5.1

<#
    LibEnvelope.psm1 - U10's own envelope-inspection doors, split out of
    Lib.psm1 purely to keep that file under the 400-line cap (the same
    reason LibVerdict.psm1 exists) rather than because the concern differs:
    like Lib.psm1, this file reads .ok/.error/.details/.data on a raw
    command envelope, so it carries the same rule05 exemption
    (scripts/lib/e2e-windows-contract-scan.psm1's rule05 ScopeFilter names
    this file beside Lib.psm1) and the same rule10 exemption for the one
    staged-binary call `Invoke-AgentDesktopBatch` makes through
    Invoke-AgentDesktop. Two doors: Assert-EnvelopeCheckOccluder reads a
    single action envelope's nested `error.details.checks[]` array (R21's
    exact field path - Assert-Envelope's own -Details only compares flat
    key/value pairs, so the occlusion leg needs a door of its own) and
    Invoke-AgentDesktopBatch composes a `batch` call and returns per-entry
    results as plain, non-banned-named fields - never a raw dictionary a
    caller could be tempted to index into.
#>

Set-StrictMode -Version 2.0

function Assert-EnvelopeCheckOccluder {
    <#
    .SYNOPSIS
        Asserts a failed action envelope's error.details.checks[] carries a
        named check (default receives_events) in Fail status whose
        occluder.name equals ExpectedOccluderName - the one assertion this
        suite needs that reads inside the checks array rather than a flat
        details key, so it is its own door rather than an Assert-Envelope
        parameter.
    #>
    param(
        [Parameter(Mandatory = $true)]$Envelope,
        [Parameter(Mandatory = $true)][string]$ErrorCode,
        [string]$Check = 'receives_events',
        [Parameter(Mandatory = $true)][string]$ExpectedOccluderName
    )
    if ($Envelope['ok'] -ne $false) {
        throw "Assert-EnvelopeCheckOccluder: expected ok=false, got ok=$($Envelope['ok'])"
    }
    $err = $Envelope['error']
    if (-not $err -or $err['code'] -ne $ErrorCode) {
        throw "Assert-EnvelopeCheckOccluder: expected error.code '$ErrorCode', got '$($err['code'])'"
    }
    $checks = $null
    if ($err['details']) { $checks = $err['details']['checks'] }
    if (-not $checks) {
        throw "Assert-EnvelopeCheckOccluder: error.details.checks is missing or empty"
    }
    $hit = $checks | Where-Object { $_['check'] -eq $Check -and $_['status'] -eq 'fail' }
    if (-not $hit) {
        throw "Assert-EnvelopeCheckOccluder: no failed '$Check' check in error.details.checks"
    }
    $occluderName = $null
    if ($hit -is [array]) { $occluderName = $hit[0]['occluder']['name'] } else { $occluderName = $hit['occluder']['name'] }
    if ($occluderName -ne $ExpectedOccluderName) {
        throw "Assert-EnvelopeCheckOccluder: expected occluder.name '$ExpectedOccluderName', got '$occluderName'"
    }
}

function Invoke-AgentDesktopBatch {
    <#
    .SYNOPSIS
        Builds a `batch` payload from Entries (each @{ Command; Args }),
        invokes it through Invoke-AgentDesktop (Lib.psm1's one staged-binary
        door) and flattens each result entry into a plain, safely-named
        object - never a raw envelope a caller could index into banned
        fields on. This is the mechanism macOS's own AE6 leg
        (tests/e2e/scenarios/acceptance.sh) uses: `batch` captures the
        signal baseline immediately before the acting entry runs, so an
        event a later entry waits for is detected against a truly
        pre-action baseline with no concurrency of any kind - one process,
        sequential entries, no detached spawn needed.
    .OUTPUTS
        PSCustomObject[]: one entry per batch step - EntryOk, ErrorCode,
        ActionOk (Invoke-Target-shaped success), Found (wait's own
        data.found), EventKind, EventApp, EventSurface, EventWindowId.
        Fields that do not apply to a given entry's command are $null.
    #>
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][hashtable[]]$Entries, [int]$TimeoutSeconds = 20)
    $payload = @($Entries | ForEach-Object { @{ command = $_.Command; args = $_.Args } })
    $json = ConvertTo-Json -InputObject $payload -Depth 10 -Compress
    $envelope = Invoke-AgentDesktop -Arguments @('batch', $json, '--stop-on-error') -TimeoutSeconds $TimeoutSeconds
    if ($envelope['ok'] -ne $true) {
        throw "Invoke-AgentDesktopBatch: the batch call itself failed: $($envelope['error']['code'])"
    }
    $results = $envelope['data']['results']
    if (-not $results) { throw 'Invoke-AgentDesktopBatch: batch response carried no results' }
    return @($results | ForEach-Object {
            $entryOk = ($_['ok'] -eq $true)
            $errorCode = $null
            $found = $null
            $eventKind = $null
            $eventApp = $null
            $eventSurface = $null
            $eventWindowId = $null
            if ($entryOk) {
                $data = $_['data']
                if ($data -and $data.ContainsKey('found')) { $found = [bool]$data['found'] }
                if ($data -and $data['event']) {
                    $event = $data['event']
                    $eventKind = $event['kind']
                    $eventApp = $event['app']
                    $eventSurface = $event['surface']
                    $eventWindowId = $event['window_id']
                }
            } else {
                $errorCode = $_['error']['code']
            }
            [pscustomobject]@{
                EntryOk       = $entryOk
                ErrorCode     = $errorCode
                Found         = $found
                EventKind     = $eventKind
                EventApp      = $eventApp
                EventSurface  = $eventSurface
                EventWindowId = $eventWindowId
            }
        })
}

function Test-EnvelopeValueEquals {
    <#
    .SYNOPSIS
        Whole-value equality for an envelope detail field, correct for both
        scalars and arrays. `$array -ne $other` is PowerShell's array-filter
        form (elements of $array not equal to $other, RHS never iterated),
        not a whole-array comparison - it is truthy even when both arrays
        print identically, which is exactly the shape
        `supported_surfaces = @('window','focused','sheet')` takes.
        Assert-Envelope's own -Details comparison calls this rather than a
        bare -ne/-eq for that reason.
    #>
    [CmdletBinding()]
    param($Actual, $Expected)
    $actualIsArray = ($Actual -is [array])
    $expectedIsArray = ($Expected -is [array])
    if ($actualIsArray -or $expectedIsArray) {
        if (-not $actualIsArray -or -not $expectedIsArray) { return $false }
        if ($Actual.Count -ne $Expected.Count) { return $false }
        for ($i = 0; $i -lt $Actual.Count; $i++) {
            if ($Actual[$i] -ne $Expected[$i]) { return $false }
        }
        return $true
    }
    return ($Actual -eq $Expected)
}

Export-ModuleMember -Function @('Assert-EnvelopeCheckOccluder', 'Invoke-AgentDesktopBatch', 'Test-EnvelopeValueEquals')
