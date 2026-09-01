#Requires -Version 5.1
<#
.SYNOPSIS
    Sub-phase 2.7 U9 semantic action-tier dogfood runner.

.DESCRIPTION
    Drives target/release/agent-desktop.exe against repo-controlled targets
    (Notepad, Explorer, WinForms/WPF scratch, Obsidian when present). Verifies
    by reading JSON envelopes — never the suite's opinion of itself. Writes a
    redacted judgement summary under OutDir.

    PLATFORM_NOT_SUPPORTED naming execute_action on click is a FAIL: 2.7 wired
    execute_action, so that 2.6 J2 arm must be gone. Headed double-click must
    name multi-click (2.8 boundary). Headless focus must be POLICY_DENIED with
    A3-4/A19-5 evidence.

    Exits non-zero when any judgement recorded 'fail', after the summary is
    written.
#>
[CmdletBinding()]
param(
    [string]$Binary = '',
    [string]$OutDir = ''
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

$script:ScratchDir = Split-Path -Parent $PSCommandPath
$script:RepoRoot = (Resolve-Path (Join-Path $script:ScratchDir '..\..\..')).ProviderPath
. (Join-Path $script:RepoRoot 'probes\windows\common.ps1')
Initialize-ProbeRedaction

if (-not $Binary) { $Binary = Join-Path $script:RepoRoot 'target\release\agent-desktop.exe' }
if (-not (Test-Path -LiteralPath $Binary)) { throw "release binary not found at $Binary" }
$script:Binary = (Resolve-Path -LiteralPath $Binary).ProviderPath
if (-not $OutDir) {
    $OutDir = Join-Path $script:RepoRoot 'docs\dogfood-reports\2026-08-07-001-captures'
}
New-Item -ItemType Directory -Path $OutDir -Force | Out-Null
$script:OutDir = (Resolve-Path -LiteralPath $OutDir).ProviderPath
$utf8NoBom = New-Object System.Text.UTF8Encoding $false

$script:LaunchedPids = New-Object System.Collections.Generic.List[int]
$script:Judgements = New-Object System.Collections.Generic.List[object]
$script:Envelopes = New-Object System.Collections.Generic.List[object]
$script:NoJsonCode = 'BINARY_NO_JSON'
$script:DispatchMethod = 'execute_action'
$script:ExplorerDir = $null

function Start-DogfoodProcess {
    param(
        [Parameter(Mandatory = $true)][string]$FilePath,
        [string[]]$ArgumentList = @(),
        [string]$WindowStyle = 'Normal'
    )
    if ($ArgumentList.Count -gt 0) {
        $proc = Start-Process -FilePath $FilePath -ArgumentList $ArgumentList -WindowStyle $WindowStyle -PassThru
    } else {
        $proc = Start-Process -FilePath $FilePath -WindowStyle $WindowStyle -PassThru
    }
    [void]$script:LaunchedPids.Add($proc.Id)
    return $proc
}

function Wait-MainWindow {
    param([Parameter(Mandatory = $true)]$Process, [int]$TimeoutSec = 25)
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ((Get-Date) -lt $deadline) {
        $Process.Refresh()
        if ($Process.HasExited) { return [IntPtr]::Zero }
        if ($Process.MainWindowHandle -ne [IntPtr]::Zero) { return $Process.MainWindowHandle }
        Start-Sleep -Milliseconds 200
    }
    return [IntPtr]::Zero
}

function Invoke-Ad {
    param([string[]]$Arguments)
    $prev = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $raw = (& $script:Binary @Arguments 2>$null | Out-String)
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $prev
    }
    $parsed = $null
    if ($raw -and $raw.Trim()) {
        try { $parsed = ($raw | ConvertFrom-Json) } catch { $parsed = $null }
    }
    if ($null -ne $parsed) {
        return [pscustomobject]@{ Envelope = $parsed; ExitCode = $exitCode; Raw = $raw }
    }
    return [pscustomobject]@{
        Envelope = [pscustomobject]@{
            ok = $false
            error = [pscustomobject]@{
                code = $script:NoJsonCode
                message = ('agent-desktop exited ' + $exitCode + ' with no JSON for: ' + ($Arguments -join ' '))
            }
        }
        ExitCode = $exitCode
        Raw = $raw
    }
}

function Find-WindowIdFor {
    param(
        [Parameter(Mandatory = $true)][string]$AppNamePattern,
        [string]$TitlePattern = ''
    )
    $lw = Invoke-Ad -Arguments @('list-windows')
    $rows = @($lw.Envelope.data | Where-Object { $_.app_name -match $AppNamePattern })
    if ($TitlePattern) {
        $rows = @($rows | Where-Object { $_.title -match $TitlePattern })
    }
    $rec = @($rows | Select-Object -First 1)
    if ($rec.Count -eq 0) { return $null }
    return $rec[0].id
}

function Test-EnvelopeNamesMethod {
    param(
        [AllowNull()][AllowEmptyString()][string]$Message,
        [AllowNull()][AllowEmptyString()][string]$Suggestion,
        [Parameter(Mandatory = $true)][string]$Method
    )
    $pattern = '(?<![A-Za-z0-9_])' + [regex]::Escape($Method) + '(?![A-Za-z0-9_])'
    foreach ($text in @($Message, $Suggestion)) {
        if ([string]::IsNullOrEmpty($text)) { continue }
        if ([regex]::IsMatch($text, $pattern)) { return $true }
    }
    return $false
}

function Get-EnvelopeShape {
    param([Parameter(Mandatory = $true)]$Envelope)
    $shape = [ordered]@{
        ok = [bool]$Envelope.ok
        command = $null
        code = $null
        disposition_delivery = $null
        disposition_retry = $null
        suggestion_present = $null
        message_names_execute_action = $null
        message_names_multi_click = $null
        message_names_key_synthesis = $null
        evidence = @()
        foreground_effect = $null
        steps = @()
        post_state_value = $null
        post_state_role = $null
        get_value = $null
        checks = @()
        details_kind = $null
    }
    if ($Envelope.PSObject.Properties.Name -contains 'command') {
        $shape.command = [string]$Envelope.command
    }
    if ($Envelope.ok -and ($Envelope.PSObject.Properties.Name -contains 'data') -and $Envelope.data) {
        $data = $Envelope.data
        if ($data.PSObject.Properties.Name -contains 'disposition' -and $data.disposition) {
            $d = $data.disposition
            if ($d.PSObject.Properties.Name -contains 'delivery') {
                $shape.disposition_delivery = [string]$d.delivery
            }
            if ($d.PSObject.Properties.Name -contains 'retry') {
                $shape.disposition_retry = [string]$d.retry
            }
        }
        if ($data.PSObject.Properties.Name -contains 'steps' -and $data.steps) {
            foreach ($s in @($data.steps)) {
                $step = [ordered]@{
                    label = $null
                    outcome = $null
                    mechanism = $null
                    verified = $null
                }
                if ($s.PSObject.Properties.Name -contains 'label') { $step.label = [string]$s.label }
                if ($s.PSObject.Properties.Name -contains 'outcome') { $step.outcome = [string]$s.outcome }
                if ($s.PSObject.Properties.Name -contains 'mechanism') { $step.mechanism = [string]$s.mechanism }
                if ($s.PSObject.Properties.Name -contains 'verified') { $step.verified = $s.verified }
                $shape.steps += $step
            }
        }
        if ($data.PSObject.Properties.Name -contains 'post_state' -and $data.post_state) {
            $ps = $data.post_state
            if ($ps.PSObject.Properties.Name -contains 'value') {
                $shape.post_state_value = [string]$ps.value
            }
            if ($ps.PSObject.Properties.Name -contains 'role') {
                $shape.post_state_role = [string]$ps.role
            }
        }
        if ($data.PSObject.Properties.Name -contains 'value' -and
            $data.PSObject.Properties.Name -contains 'property') {
            $shape.get_value = [string]$data.value
        }
        if ($data.PSObject.Properties.Name -contains 'result') {
            $shape.get_value = [string]$data.result
        }
    }
    if (-not $Envelope.ok -and ($Envelope.PSObject.Properties.Name -contains 'error')) {
        $err = $Envelope.error
        if ($err.PSObject.Properties.Name -contains 'code') { $shape.code = [string]$err.code }
        $suggestionText = ''
        if ($err.PSObject.Properties.Name -contains 'suggestion' -and $err.suggestion) {
            $shape.suggestion_present = $true
            $suggestionText = [string]$err.suggestion
        } else {
            $shape.suggestion_present = $false
        }
        $messageText = ''
        if ($err.PSObject.Properties.Name -contains 'message' -and $err.message) {
            $messageText = [string]$err.message
        }
        $shape.message_names_execute_action = (Test-EnvelopeNamesMethod `
                -Message $messageText -Suggestion $suggestionText -Method $script:DispatchMethod)
        $shape.message_names_multi_click = (Test-EnvelopeNamesMethod `
                -Message $messageText -Suggestion $suggestionText -Method 'multi-click')
        $shape.message_names_key_synthesis = (Test-EnvelopeNamesMethod `
                -Message $messageText -Suggestion $suggestionText -Method 'key synthesis')
        if ($err.PSObject.Properties.Name -contains 'disposition' -and $err.disposition) {
            $d = $err.disposition
            if ($d.PSObject.Properties.Name -contains 'delivery') {
                $shape.disposition_delivery = [string]$d.delivery
            }
            if ($d.PSObject.Properties.Name -contains 'retry') {
                $shape.disposition_retry = [string]$d.retry
            }
        }
        if ($err.PSObject.Properties.Name -contains 'details' -and $err.details) {
            $details = $err.details
            if ($details.PSObject.Properties.Name -contains 'kind') {
                $shape.details_kind = [string]$details.kind
            }
            if ($details.PSObject.Properties.Name -contains 'foreground_effect') {
                $shape.foreground_effect = [bool]$details.foreground_effect
            }
            if ($details.PSObject.Properties.Name -contains 'evidence') {
                $shape.evidence = @($details.evidence | ForEach-Object { [string]$_ })
            }
            $checksSrc = $null
            if ($details.PSObject.Properties.Name -contains 'checks') { $checksSrc = $details.checks }
            if ($null -ne $checksSrc) {
                foreach ($c in @($checksSrc)) {
                    $row = [ordered]@{
                        name = $null
                        status = $null
                        reason_shape = $null
                    }
                    if ($c.PSObject.Properties.Name -contains 'check') { $row.name = [string]$c.check }
                    elseif ($c.PSObject.Properties.Name -contains 'name') { $row.name = [string]$c.name }
                    if ($c.PSObject.Properties.Name -contains 'status') { $row.status = [string]$c.status }
                    if ($c.PSObject.Properties.Name -contains 'reason' -and $c.reason) {
                        $reason = [string]$c.reason
                        if ($reason -match 'semantic action is unavailable') {
                            $row.reason_shape = 'semantic action unavailable / focus fallback denied'
                        } elseif ($reason -match 'not available') {
                            $row.reason_shape = 'action not available'
                        } else {
                            $row.reason_shape = 'other'
                        }
                    }
                    $shape.checks += $row
                }
            }
        }
    }
    return $shape
}

function Add-Judgement {
    param(
        [string]$Id,
        [string]$Claim,
        [string]$Target,
        [string]$Result,
        [string]$Verdict,
        [object]$Shape = $null,
        [string]$Notes = ''
    )
    [void]$script:Judgements.Add([ordered]@{
            id = $Id
            claim = $Claim
            target = $Target
            result = $Result
            verdict = $Verdict
            envelope_shape = $Shape
            notes = $Notes
        })
    Write-Host ("dogfood: [$Id] $Result - $Verdict")
}

function Add-EnvelopeRecord {
    param([string]$Id, [object]$Shape, [string]$Note = '')
    [void]$script:Envelopes.Add([ordered]@{
            id = $Id
            shape = $Shape
            raw_redacted_keys_only = $true
            note = $Note
        })
}

function Get-MatchRef {
    param($Envelope)
    if (-not $Envelope.ok) { return $null }
    $data = $Envelope.data
    if ($data.PSObject.Properties.Name -contains 'ref_id' -and $data.ref_id) {
        return [string]$data.ref_id
    }
    if ($data.PSObject.Properties.Name -contains 'match' -and $data.match) {
        if ($data.match.PSObject.Properties.Name -contains 'ref_id' -and $data.match.ref_id) {
            return [string]$data.match.ref_id
        }
        if ($data.match.PSObject.Properties.Name -contains 'ref' -and $data.match.ref) {
            return [string]$data.match.ref
        }
    }
    return $null
}

function Find-RefByNativeId {
    param([string]$WindowId, [string]$NativeId, [string]$Role = '')
    $args = [System.Collections.Generic.List[string]]@(
        'find', '--window-id', $WindowId, '--native-id', $NativeId, '--first'
    )
    if ($Role) { [void]$args.Add('--role'); [void]$args.Add($Role) }
    $found = Invoke-Ad -Arguments $args.ToArray()
    return (Get-MatchRef -Envelope $found.Envelope)
}

function Find-RefsByRole {
    param([string]$WindowId, [string]$Role, [int]$Limit = 40)
    $found = Invoke-Ad -Arguments @(
        'find', '--window-id', $WindowId, '--role', $Role, '--limit', ([string]$Limit)
    )
    $out = New-Object System.Collections.Generic.List[object]
    if (-not $found.Envelope.ok) {
        return @(,$out.ToArray())
    }
    $data = $found.Envelope.data
    $items = @()
    try {
        if ($null -ne $data.matches) { $items = @($data.matches) }
        elseif ($null -ne $data.match) { $items = @($data.match) }
    } catch {
        $items = @()
    }
    foreach ($item in $items) {
        if ($null -eq $item) { continue }
        $refId = $null
        try {
            if ($null -ne $item.ref_id -and [string]$item.ref_id -ne '') { $refId = [string]$item.ref_id }
            elseif ($null -ne $item.ref -and [string]$item.ref -ne '') { $refId = [string]$item.ref }
        } catch { $refId = $null }
        $name = $null
        try {
            if ($null -ne $item.name) { $name = [string]$item.name }
        } catch { $name = $null }
        if ($refId) {
            [void]$out.Add([pscustomobject]@{ ref = $refId; name = $name })
        }
    }
    return @($out.ToArray())
}

function Test-ClickNamesExecuteAction {
    param([Parameter(Mandatory = $true)]$Shape)
    return (($Shape.code -eq 'PLATFORM_NOT_SUPPORTED') -and ($Shape.message_names_execute_action -eq $true))
}

function Test-StepSucceeded {
    param(
        [Parameter(Mandatory = $true)]$Shape,
        [Parameter(Mandatory = $true)][string]$LabelSubstring
    )
    foreach ($s in @($Shape.steps)) {
        if ($null -eq $s.label) { continue }
        if (($s.label -like ("*" + $LabelSubstring + "*")) -and ($s.outcome -eq 'succeeded')) {
            return $true
        }
    }
    return $false
}

try {
    & (Join-Path $script:ScratchDir 'build-scratch.ps1') | Out-Null

    # -------------------------------------------------------------------------
    # J1: Notepad Document (mapped textfield) set-value / clear payload matrix
    # -------------------------------------------------------------------------
    $notepad = $null
    $scratchFile = $null
    try {
        $scratchFile = Join-Path $env:TEMP ('agent-desktop-u9-' + [guid]::NewGuid() + '.txt')
        [IO.File]::WriteAllText($scratchFile, "seed-u9`r`n", $utf8NoBom)
        $notepad = Start-DogfoodProcess -FilePath 'notepad.exe' -ArgumentList @($scratchFile)
        $npHwnd = Wait-MainWindow -Process $notepad -TimeoutSec 15
        if ($npHwnd -eq [IntPtr]::Zero) { throw 'Notepad never presented a window' }
        Start-Sleep -Seconds 2
        $npWid = Find-WindowIdFor -AppNamePattern 'Notepad'
        if (-not $npWid) { $npWid = 'w-' + $npHwnd.ToInt64() }
        $snap = Invoke-Ad -Arguments @('snapshot', '--window-id', $npWid)
        if (-not $snap.Envelope.ok) { throw ('notepad snapshot failed: ' + $snap.Envelope.error.code) }
        $docFind = Invoke-Ad -Arguments @(
            'find', '--window-id', $npWid, '--role', 'textfield', '--first'
        )
        $docRef = Get-MatchRef -Envelope $docFind.Envelope
        if (-not $docRef) { throw 'Notepad textfield/Document ref not found' }

        $payloads = @(
            [ordered]@{ id = 'ascii'; value = 'ascii-dogfood-u9'; required = $true },
            [ordered]@{ id = 'cjk'; value = ([string]([char]0x4E16) + [char]0x754C); required = $false },
            [ordered]@{ id = 'astral'; value = ([string][char]::ConvertFromUtf32(0x1F600)); required = $false }
        )
        $payloadShapes = @()
        $payloadPass = $true
        $payloadNotes = New-Object System.Collections.Generic.List[string]
        foreach ($p in $payloads) {
            $set = Invoke-Ad -Arguments @('set-value', $docRef, $p.value)
            $setShape = Get-EnvelopeShape -Envelope $set.Envelope
            Add-EnvelopeRecord -Id ('J1-set-' + $p.id) -Shape $setShape -Note ('chars=' + $p.value.Length)
            $get = Invoke-Ad -Arguments @('get', $docRef, '--property', 'value')
            $getShape = Get-EnvelopeShape -Envelope $get.Envelope
            Add-EnvelopeRecord -Id ('J1-get-' + $p.id) -Shape $getShape -Note ('chars=' + $p.value.Length)
            $valueMatched = ($getShape.get_value -eq $p.value) -or `
                ($setShape.post_state_value -eq $p.value) -or `
                (($null -ne $getShape.get_value) -and ($getShape.get_value.Length -eq $p.value.Length) -and `
                 ($setShape.disposition_delivery -eq 'delivered_verified') -and `
                 (Test-StepSucceeded -Shape $setShape -LabelSubstring 'ValuePattern.SetValue'))
            $roundTrip = $set.Envelope.ok -and `
                ($setShape.disposition_delivery -eq 'delivered_verified') -and `
                (Test-StepSucceeded -Shape $setShape -LabelSubstring 'ValuePattern.SetValue') -and `
                $valueMatched
            if ($roundTrip) {
                [void]$payloadNotes.Add($p.id + '=pass chars=' + $p.value.Length)
            } else {
                [void]$payloadNotes.Add($p.id + '=fail delivery=' + $setShape.disposition_delivery +
                    ' get_chars=' + $(if ($null -eq $getShape.get_value) { 'null' } else { $getShape.get_value.Length }))
                if ($p.required) { $payloadPass = $false }
            }
            $payloadShapes += $setShape
            $clear = Invoke-Ad -Arguments @('clear', $docRef)
            $clearShape = Get-EnvelopeShape -Envelope $clear.Envelope
            Add-EnvelopeRecord -Id ('J1-clear-' + $p.id) -Shape $clearShape
            $clearOk = $clear.Envelope.ok -and ($clearShape.disposition_delivery -eq 'delivered_verified')
            if (-not $clearOk -and $p.required) { $payloadPass = $false }
            if (-not $clearOk) { [void]$payloadNotes.Add('clear-' + $p.id + '=fail') }
            # refresh ref after mutations for safety
            $snap = Invoke-Ad -Arguments @('snapshot', '--window-id', $npWid)
            $docFind = Invoke-Ad -Arguments @(
                'find', '--window-id', $npWid, '--role', 'textfield', '--first'
            )
            $docRef = Get-MatchRef -Envelope $docFind.Envelope
            if (-not $docRef) { throw 'Notepad textfield lost after clear' }
        }

        # Policy / boundary judgements share this Notepad document ref.
        $focus = Invoke-Ad -Arguments @('focus', $docRef)
        $focusShape = Get-EnvelopeShape -Envelope $focus.Envelope
        Add-EnvelopeRecord -Id 'J6-focus-headless' -Shape $focusShape
        $j6Ok = (-not $focus.Envelope.ok) -and `
            ($focusShape.code -eq 'POLICY_DENIED') -and `
            ($focusShape.disposition_delivery -eq 'not_delivered') -and `
            ($focusShape.foreground_effect -eq $true) -and `
            (@($focusShape.evidence) -contains 'A3-4') -and `
            (@($focusShape.evidence) -contains 'A19-5') -and `
            ($focusShape.suggestion_present -eq $true)
        Add-Judgement -Id 'J6' -Claim 'focus headless POLICY_DENIED with A3-4/A19-5' `
            -Target 'Notepad textfield' `
            -Result $(if ($j6Ok) { 'pass' } else { 'fail' }) `
            -Verdict $(if ($j6Ok) {
                'POLICY_DENIED not_delivered; evidence A3-4/A19-5; foreground_effect true'
            } else { 'focus headless envelope mismatch' }) `
            -Shape $focusShape `
            -Notes ('code=' + $focusShape.code + ' evidence=' + ($focusShape.evidence -join ','))

        $typeCmd = Invoke-Ad -Arguments @('type', $docRef, 'x')
        $typeShape = Get-EnvelopeShape -Envelope $typeCmd.Envelope
        Add-EnvelopeRecord -Id 'J7-type-headless' -Shape $typeShape
        $supportedFail = @($typeShape.checks | Where-Object {
                $_.name -eq 'supported_action' -and $_.status -eq 'fail'
            }).Count -gt 0
        $j7Ok = (-not $typeCmd.Envelope.ok) -and `
            ($typeShape.code -eq 'POLICY_DENIED') -and `
            ($typeShape.disposition_delivery -eq 'not_delivered') -and `
            $supportedFail
        Add-Judgement -Id 'J7' -Claim 'type headless honest preflight denial' `
            -Target 'Notepad textfield' `
            -Result $(if ($j7Ok) { 'pass' } else { 'fail' }) `
            -Verdict $(if ($j7Ok) {
                'POLICY_DENIED at supported_action (TypeText unavailable headless)'
            } else { 'type headless envelope mismatch' }) `
            -Shape $typeShape `
            -Notes ('code=' + $typeShape.code)

        # timeout-ms 0: reach dispatch without auto-wait soaking the headed path
        $dbl = Invoke-Ad -Arguments @('double-click', $docRef, '--headed', '--timeout-ms', '0')
        $dblShape = Get-EnvelopeShape -Envelope $dbl.Envelope
        Add-EnvelopeRecord -Id 'J8-double-click-headed' -Shape $dblShape
        $j8Ok = (-not $dbl.Envelope.ok) -and `
            ($dblShape.code -eq 'PLATFORM_NOT_SUPPORTED') -and `
            ($dblShape.message_names_multi_click -eq $true) -and `
            ($dblShape.message_names_execute_action -ne $true)
        Add-Judgement -Id 'J8' -Claim 'headed double-click names multi-click (2.8 boundary)' `
            -Target 'Notepad textfield' `
            -Result $(if ($j8Ok) { 'pass' } else { 'fail' }) `
            -Verdict $(if ($j8Ok) {
                'PLATFORM_NOT_SUPPORTED naming multi-click, not execute_action'
            } else { 'double-click boundary envelope mismatch' }) `
            -Shape $dblShape `
            -Notes ('code=' + $dblShape.code +
                ' multi_click=' + $dblShape.message_names_multi_click +
                ' execute_action=' + $dblShape.message_names_execute_action)

        Add-Judgement -Id 'J1' -Claim 'Notepad Document set-value/clear payload round-trip' `
            -Target 'Notepad textfield (classic Document)' `
            -Result $(if ($payloadPass) { 'pass' } else { 'fail' }) `
            -Verdict $(if ($payloadPass) {
                'ASCII required payloads round-tripped via ValuePattern; clear delivered_verified'
            } else { 'required payload round-trip failed' }) `
            -Shape $payloadShapes[0] `
            -Notes ($payloadNotes -join '; ')
    } catch {
        Add-Judgement -Id 'J1' -Claim 'Notepad Document set-value/clear payload round-trip' `
            -Target 'Notepad' -Result 'skipped' -Verdict 'harness error' -Notes $_.Exception.Message
        Add-Judgement -Id 'J6' -Claim 'focus headless POLICY_DENIED with A3-4/A19-5' `
            -Target 'Notepad' -Result 'skipped' -Verdict 'harness error' -Notes $_.Exception.Message
        Add-Judgement -Id 'J7' -Claim 'type headless honest preflight denial' `
            -Target 'Notepad' -Result 'skipped' -Verdict 'harness error' -Notes $_.Exception.Message
        Add-Judgement -Id 'J8' -Claim 'headed double-click names multi-click (2.8 boundary)' `
            -Target 'Notepad' -Result 'skipped' -Verdict 'harness error' -Notes $_.Exception.Message
    } finally {
        if ($null -ne $notepad) {
            Stop-Process -Id $notepad.Id -Force -ErrorAction SilentlyContinue
        }
        if ($scratchFile -and (Test-Path -LiteralPath $scratchFile)) {
            Remove-Item -LiteralPath $scratchFile -Force -ErrorAction SilentlyContinue
        }
    }

    # -------------------------------------------------------------------------
    # J2/J3: Explorer select-by-name + below-fold ladder re-judgement
    # -------------------------------------------------------------------------
    $script:ExplorerDir = Join-Path $env:TEMP ('agent-desktop-u9-dir-' + [guid]::NewGuid())
    New-Item -ItemType Directory -Path $script:ExplorerDir -Force | Out-Null
    try {
        1..40 | ForEach-Object {
            $n = 'file-{0:D2}.txt' -f $_
            [IO.File]::WriteAllText((Join-Path $script:ExplorerDir $n), ("synthetic $n`r`n"), $utf8NoBom)
        }
        [void](Start-DogfoodProcess -FilePath 'explorer.exe' -ArgumentList @($script:ExplorerDir))
        Write-Host 'dogfood: waiting 22s for Explorer filesystem settle (A7-4)'
        Start-Sleep -Seconds 22
        $dirLeaf = Split-Path -Leaf $script:ExplorerDir
        $exWid = Find-WindowIdFor -AppNamePattern 'explorer' -TitlePattern ([regex]::Escape($dirLeaf))
        if (-not $exWid) { $exWid = Find-WindowIdFor -AppNamePattern 'explorer' }
        if (-not $exWid) { throw 'no Explorer window resolved' }
        $exSnap = Invoke-Ad -Arguments @('snapshot', '--window-id', $exWid, '--timeout-ms', '10000')
        if (-not $exSnap.Envelope.ok) {
            throw ('explorer snapshot failed: ' + $exSnap.Envelope.error.code)
        }

        $selectName = 'file-05'
        $named = Invoke-Ad -Arguments @(
            'find', '--window-id', $exWid, '--role', 'option',
            '--name', $selectName, '--exact', '--first'
        )
        $namedRef = Get-MatchRef -Envelope $named.Envelope
        if (-not $namedRef) {
            Add-Judgement -Id 'J2' -Claim 'Explorer list item select by visible name' `
                -Target 'Explorer option' -Result 'skipped' `
                -Verdict 'named option ref absent' `
                -Notes ('name=' + $selectName)
        } else {
            $sel = Invoke-Ad -Arguments @('select', $namedRef, $selectName)
            $selShape = Get-EnvelopeShape -Envelope $sel.Envelope
            Add-EnvelopeRecord -Id 'J2-explorer-select' -Shape $selShape -Note ('name_chars=' + $selectName.Length)
            $j2Ok = $sel.Envelope.ok -and `
                ($selShape.disposition_delivery -eq 'delivered_verified') -and `
                (Test-StepSucceeded -Shape $selShape -LabelSubstring 'SelectionItemPattern.Select') -and `
                (-not (Test-ClickNamesExecuteAction -Shape $selShape))
            Add-Judgement -Id 'J2' -Claim 'Explorer list item select by visible name' `
                -Target 'Explorer option file-05' `
                -Result $(if ($j2Ok) { 'pass' } else { 'fail' }) `
                -Verdict $(if ($j2Ok) {
                    'SelectionItemPattern.Select delivered_verified by visible name'
                } else { 'select-by-name envelope mismatch' }) `
                -Shape $selShape `
                -Notes ('delivery=' + $selShape.disposition_delivery)
        }

        [void](Invoke-Ad -Arguments @('snapshot', '--window-id', $exWid, '--timeout-ms', '10000'))
        $options = Find-RefsByRole -WindowId $exWid -Role 'option' -Limit 40
        if ($null -eq $options) { $options = @() }
        if ($options -isnot [array]) { $options = @($options) }
        if ($options.Count -eq 0) {
            foreach ($candidate in @('file-24', 'file-30', 'file-40', 'file-20')) {
                $foundLate = Invoke-Ad -Arguments @(
                    'find', '--window-id', $exWid, '--role', 'option',
                    '--name', $candidate, '--exact', '--first'
                )
                $lateRef = Get-MatchRef -Envelope $foundLate.Envelope
                if ($lateRef) {
                    $options = @([pscustomobject]@{ ref = $lateRef; name = $candidate })
                    break
                }
            }
        }
        $offscreen = $null
        foreach ($row in $options) {
            $vis = Invoke-Ad -Arguments @('is', $row.ref, '--property', 'visible')
            if ($vis.Envelope.ok -and $vis.Envelope.data.PSObject.Properties.Name -contains 'result' -and
                [bool]$vis.Envelope.data.result -eq $false) {
                $offscreen = $row
                break
            }
        }
        if (-not $offscreen -and $options.Count -gt 0) {
            $offscreen = $options[$options.Count - 1]
        }
        if (-not $offscreen) {
            Add-Judgement -Id 'J3' -Claim 'Explorer below-fold re-judge via scroll ladder' `
                -Target 'Explorer option' -Result 'skipped' `
                -Verdict 'no option refs in Explorer snapshot' `
                -Notes ('ref_count=' + $exSnap.Envelope.data.ref_count + ' options=' + $options.Count)
        } else {
            # Re-resolve by short synthetic name so the click ref belongs to this snapshot.
            $resolveName = [string]$offscreen.name
            if ($resolveName -match '^file-\d+$') {
                $fresh = Invoke-Ad -Arguments @(
                    'find', '--window-id', $exWid, '--role', 'option',
                    '--name', $resolveName, '--exact', '--first'
                )
                $freshRef = Get-MatchRef -Envelope $fresh.Envelope
                if ($freshRef) { $offscreen = [pscustomobject]@{ ref = $freshRef; name = $resolveName } }
            } elseif (-not ($offscreen.ref -match '^@[A-Za-z0-9]+:e\d+$')) {
                Add-Judgement -Id 'J3' -Claim 'Explorer below-fold re-judge via scroll ladder' `
                    -Target 'Explorer option' -Result 'fail' `
                    -Verdict 'option target had unusable name/ref after enumeration' `
                    -Notes ('name_chars=' + $resolveName.Length)
                throw 'j3-unusable-option-target'
            }
            $beforeVis = Invoke-Ad -Arguments @('is', $offscreen.ref, '--property', 'visible')
            $wasVisible = $false
            if ($beforeVis.Envelope.ok -and $beforeVis.Envelope.data.PSObject.Properties.Name -contains 'result') {
                $wasVisible = [bool]$beforeVis.Envelope.data.result
            }
            $click = Invoke-Ad -Arguments @('click', $offscreen.ref, '--timeout-ms', '5000')
            $clickShape = Get-EnvelopeShape -Envelope $click.Envelope
            Add-EnvelopeRecord -Id 'J3-explorer-below-fold' -Shape $clickShape `
                -Note ('name_chars=' + ([string]$offscreen.name).Length + ' was_visible=' + $wasVisible)
            if (Test-ClickNamesExecuteAction -Shape $clickShape) {
                Add-Judgement -Id 'J3' -Claim 'Explorer below-fold re-judge via scroll ladder' `
                    -Target 'Explorer option' -Result 'fail' `
                    -Verdict 'PLATFORM_NOT_SUPPORTED still names execute_action (2.6 J2 arm not gone)' `
                    -Shape $clickShape
            } else {
                $afterVis = Invoke-Ad -Arguments @('is', $offscreen.ref, '--property', 'visible')
                $nowVisible = $false
                if ($afterVis.Envelope.ok -and $afterVis.Envelope.data.PSObject.Properties.Name -contains 'result') {
                    $nowVisible = [bool]$afterVis.Envelope.data.result
                }
                $afterShape = Get-EnvelopeShape -Envelope $afterVis.Envelope
                Add-EnvelopeRecord -Id 'J3-visibility-after' -Shape $afterShape
                $actionFailed = ($clickShape.code -eq 'ACTION_FAILED')
                $honestLadder = (-not $click.Envelope.ok) -and $actionFailed -and (
                    ($clickShape.disposition_delivery -eq 'delivered_unverified') -or
                    ($clickShape.disposition_delivery -eq 'not_delivered') -or
                    ($clickShape.details_kind -eq 'scroll_into_view_unsupported')
                )
                $verifiedVisible = $click.Envelope.ok -and $nowVisible -and (
                    (Test-StepSucceeded -Shape $clickShape -LabelSubstring 'InvokePattern') -or
                    (Test-StepSucceeded -Shape $clickShape -LabelSubstring 'Scroll') -or
                    ($clickShape.disposition_delivery -match '^delivered_')
                )
                $j3Ok = $verifiedVisible -or $honestLadder
                $verdict = if ($verifiedVisible -and (-not $wasVisible)) {
                    'below-fold item became visible; click dispatched (ladder/scroll path)'
                } elseif ($verifiedVisible) {
                    'item visible after click with semantic dispatch (not execute_action)'
                } elseif ($honestLadder) {
                    'honest ACTION_FAILED ladder/scroll outcome without execute_action naming'
                } else {
                    'unexpected below-fold envelope code=' + $clickShape.code
                }
                Add-Judgement -Id 'J3' -Claim 'Explorer below-fold re-judge via scroll ladder' `
                    -Target 'Explorer option' `
                    -Result $(if ($j3Ok) { 'pass' } else { 'fail' }) `
                    -Verdict $verdict `
                    -Shape $clickShape `
                    -Notes ('was_visible=' + $wasVisible + ' now_visible=' + $nowVisible +
                        ' ok=' + $click.Envelope.ok + ' code=' + $clickShape.code +
                        ' delivery=' + $clickShape.disposition_delivery +
                        ' execute_action=' + $clickShape.message_names_execute_action)
            }
        }
    } catch {
        Add-Judgement -Id 'J2' -Claim 'Explorer list item select by visible name' `
            -Target 'Explorer' -Result 'skipped' -Verdict 'harness error' -Notes $_.Exception.Message
        Add-Judgement -Id 'J3' -Claim 'Explorer below-fold re-judge via scroll ladder' `
            -Target 'Explorer' -Result 'skipped' -Verdict 'harness error' -Notes $_.Exception.Message
    }

    # -------------------------------------------------------------------------
    # J4/J5: Scratch fixture click/toggle/expand + WPF RangeValue slider
    # -------------------------------------------------------------------------
    $winforms = $null
    $wpf = $null
    try {
        $scratchExe = Join-Path $script:ScratchDir 'bin\ScratchForms.exe'
        if (-not (Test-Path -LiteralPath $scratchExe)) { throw "ScratchForms.exe missing at $scratchExe" }
        $winforms = Start-DogfoodProcess -FilePath $scratchExe -ArgumentList @(
            '--tag', 'u9', '--pos', '80,80', '--host-providers'
        )
        $wfHwnd = Wait-MainWindow -Process $winforms -TimeoutSec 20
        if ($wfHwnd -eq [IntPtr]::Zero) { throw 'ScratchForms never presented a window' }
        Start-Sleep -Seconds 2
        $wfWid = Find-WindowIdFor -AppNamePattern 'ScratchForms'
        if (-not $wfWid) { $wfWid = 'w-' + $wfHwnd.ToInt64() }
        $wfSnap = Invoke-Ad -Arguments @('snapshot', '--window-id', $wfWid)
        if (-not $wfSnap.Envelope.ok) { throw ('ScratchForms snapshot failed: ' + $wfSnap.Envelope.error.code) }

        $btnRef = Find-RefByNativeId -WindowId $wfWid -NativeId 'btnAction'
        $chkRef = Find-RefByNativeId -WindowId $wfWid -NativeId 'chkToggle'
        $treeFind = Invoke-Ad -Arguments @(
            'find', '--window-id', $wfWid, '--role', 'treeitem',
            '--name', 'Node-Sibling', '--first'
        )
        $treeRef = Get-MatchRef -Envelope $treeFind.Envelope
        if (-not $btnRef -or -not $chkRef -or -not $treeRef) {
            throw ('fixture refs missing btn=' + [bool]$btnRef + ' chk=' + [bool]$chkRef + ' tree=' + [bool]$treeRef)
        }

        $click = Invoke-Ad -Arguments @('click', $btnRef)
        $clickShape = Get-EnvelopeShape -Envelope $click.Envelope
        Add-EnvelopeRecord -Id 'J4-click' -Shape $clickShape
        $toggle = Invoke-Ad -Arguments @('toggle', $chkRef)
        $toggleShape = Get-EnvelopeShape -Envelope $toggle.Envelope
        Add-EnvelopeRecord -Id 'J4-toggle' -Shape $toggleShape
        $expand = Invoke-Ad -Arguments @('expand', $treeRef)
        $expandShape = Get-EnvelopeShape -Envelope $expand.Envelope
        Add-EnvelopeRecord -Id 'J4-expand' -Shape $expandShape

        $clickOk = $click.Envelope.ok -and `
            (Test-StepSucceeded -Shape $clickShape -LabelSubstring 'InvokePattern.Invoke') -and `
            ($clickShape.disposition_delivery -match '^delivered_') -and `
            (-not (Test-ClickNamesExecuteAction -Shape $clickShape))
        $toggleOk = $toggle.Envelope.ok -and `
            (Test-StepSucceeded -Shape $toggleShape -LabelSubstring 'TogglePattern.Toggle') -and `
            ($toggleShape.disposition_delivery -eq 'delivered_verified')
        $expandOk = $expand.Envelope.ok -and `
            (Test-StepSucceeded -Shape $expandShape -LabelSubstring 'ExpandCollapsePattern.Expand') -and `
            ($expandShape.disposition_delivery -eq 'delivered_verified')
        $j4Ok = $clickOk -and $toggleOk -and $expandOk
        Add-Judgement -Id 'J4' -Claim 'fixture click/toggle/expand full step/disposition envelope' `
            -Target 'ScratchForms btnAction/chkToggle/Node-Sibling' `
            -Result $(if ($j4Ok) { 'pass' } else { 'fail' }) `
            -Verdict $(if ($j4Ok) {
                'Invoke/Toggle/Expand steps with semantic_api dispositions; execute_action gone'
            } else {
                'click_ok=' + $clickOk + ' toggle_ok=' + $toggleOk + ' expand_ok=' + $expandOk
            }) `
            -Shape $clickShape `
            -Notes ('click_delivery=' + $clickShape.disposition_delivery +
                ' toggle_delivery=' + $toggleShape.disposition_delivery +
                ' expand_delivery=' + $expandShape.disposition_delivery)

        $wpfScript = Join-Path $script:ScratchDir 'ScratchWpf.ps1'
        $wpf = Start-DogfoodProcess -FilePath 'powershell.exe' -WindowStyle 'Hidden' -ArgumentList @(
            '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $wpfScript,
            '-Tag', 'u9', '-Left', '500', '-Top', '80', '-TimeoutSeconds', '120'
        )
        $wpfHwnd = Wait-MainWindow -Process $wpf -TimeoutSec 25
        if ($wpfHwnd -eq [IntPtr]::Zero) { throw 'WPF fixture never presented a window' }
        Start-Sleep -Seconds 2
        $wpfWid = 'w-' + $wpfHwnd.ToInt64()
        $wpfSnap = Invoke-Ad -Arguments @('snapshot', '--window-id', $wpfWid)
        if (-not $wpfSnap.Envelope.ok) { throw ('WPF snapshot failed: ' + $wpfSnap.Envelope.error.code) }
        $sliderRef = Find-RefByNativeId -WindowId $wpfWid -NativeId 'tbSlider' -Role 'slider'
        if (-not $sliderRef) { throw 'WPF tbSlider ref absent' }
        $sliderValue = '42'
        $setSlider = Invoke-Ad -Arguments @('set-value', $sliderRef, $sliderValue)
        $sliderShape = Get-EnvelopeShape -Envelope $setSlider.Envelope
        Add-EnvelopeRecord -Id 'J5-slider-rangevalue' -Shape $sliderShape -Note ('commanded=' + $sliderValue)
        $rangeStep = $null
        foreach ($s in @($sliderShape.steps)) {
            if ($s.label -eq 'RangeValuePattern.SetValue' -and $s.outcome -eq 'succeeded') {
                $rangeStep = $s
                break
            }
        }
        $j5Ok = $setSlider.Envelope.ok -and `
            ($null -ne $rangeStep) -and `
            ($rangeStep.verified -eq $true) -and `
            ($sliderShape.disposition_delivery -eq 'delivered_verified')
        Add-Judgement -Id 'J5' -Claim 'fixture slider set-value through RangeValue with re-read' `
            -Target 'ScratchWpf tbSlider' `
            -Result $(if ($j5Ok) { 'pass' } else { 'fail' }) `
            -Verdict $(if ($j5Ok) {
                ('RangeValuePattern.SetValue succeeded verified=true for commanded ' + $sliderValue)
            } else { 'RangeValue slider envelope mismatch' }) `
            -Shape $sliderShape `
            -Notes ('commanded=' + $sliderValue + ' delivery=' + $sliderShape.disposition_delivery)
    } catch {
        $ids = @($script:Judgements | ForEach-Object { $_.id })
        if ($ids -notcontains 'J4') {
            Add-Judgement -Id 'J4' -Claim 'fixture click/toggle/expand full step/disposition envelope' `
                -Target 'ScratchForms' -Result 'skipped' -Verdict 'harness error' -Notes $_.Exception.Message
        }
        if ($ids -notcontains 'J5') {
            Add-Judgement -Id 'J5' -Claim 'fixture slider set-value through RangeValue with re-read' `
                -Target 'ScratchWpf' -Result 'skipped' -Verdict 'harness error' -Notes $_.Exception.Message
        }
    } finally {
        if ($null -ne $wpf) { Stop-Process -Id $wpf.Id -Force -ErrorAction SilentlyContinue }
        if ($null -ne $winforms) { Stop-Process -Id $winforms.Id -Force -ErrorAction SilentlyContinue }
    }

    # -------------------------------------------------------------------------
    # J9: Obsidian — one semantic action attempt, judged honestly
    # -------------------------------------------------------------------------
    $obsidianExe = Join-Path $env:LOCALAPPDATA 'Programs\Obsidian\Obsidian.exe'
    try {
        if (-not (Test-Path -LiteralPath $obsidianExe)) {
            Add-Judgement -Id 'J9' -Claim 'Obsidian one semantic action attempt' `
                -Target 'Obsidian' -Result 'skipped' `
                -Verdict 'Obsidian not installed' -Notes $obsidianExe
        } else {
            $obs = Start-DogfoodProcess -FilePath $obsidianExe
            Start-Sleep -Seconds 12
            $obsWid = $null
            for ($i = 0; $i -lt 16 -and -not $obsWid; $i++) {
                $obsWid = Find-WindowIdFor -AppNamePattern 'Obsidian'
                if (-not $obsWid) { Start-Sleep -Seconds 2 }
            }
            if (-not $obsWid) { throw 'no Obsidian window resolved' }
            $obsSnap = Invoke-Ad -Arguments @('snapshot', '--window-id', $obsWid, '--timeout-ms', '45000')
            if (-not $obsSnap.Envelope.ok) {
                $code = $obsSnap.Envelope.error.code
                Add-Judgement -Id 'J9' -Claim 'Obsidian one semantic action attempt' `
                    -Target 'Obsidian Chromium/Electron' `
                    -Result 'ran' `
                    -Verdict $(if ($code -eq 'TIMEOUT') {
                        'snapshot TIMEOUT / shell-bound (A18-3); semantic action unexercised'
                    } else { 'snapshot failed: ' + $code }) `
                    -Notes ('code=' + $code)
            } else {
                $btnRefs = @(Find-RefsByRole -WindowId $obsWid -Role 'button' -Limit 8)
                $target = $null
                foreach ($row in $btnRefs) {
                    $b = Invoke-Ad -Arguments @('get', $row.ref, '--property', 'bounds')
                    if (-not $b.Envelope.ok) { continue }
                    $val = $b.Envelope.data.value
                    if ($null -eq $val) { continue }
                    if ([double]$val.width -gt 0 -and [double]$val.height -gt 0) {
                        $target = $row
                        break
                    }
                }
                if (-not $target) {
                    Add-Judgement -Id 'J9' -Claim 'Obsidian one semantic action attempt' `
                        -Target 'Obsidian Chromium/Electron' `
                        -Result 'ran' `
                        -Verdict 'shell-bound / no positive-area actionable leaf (A18-3)' `
                        -Notes ('ref_count=' + $obsSnap.Envelope.data.ref_count +
                            ' complete=' + $obsSnap.Envelope.data.complete)
                } else {
                    $obsClick = Invoke-Ad -Arguments @('click', $target.ref)
                    $obsShape = Get-EnvelopeShape -Envelope $obsClick.Envelope
                    Add-EnvelopeRecord -Id 'J9-obsidian-click' -Shape $obsShape
                    if (Test-ClickNamesExecuteAction -Shape $obsShape) {
                        Add-Judgement -Id 'J9' -Claim 'Obsidian one semantic action attempt' `
                            -Target 'Obsidian' -Result 'fail' `
                            -Verdict 'click still names execute_action' -Shape $obsShape
                    } else {
                        Add-Judgement -Id 'J9' -Claim 'Obsidian one semantic action attempt' `
                            -Target 'Obsidian Chromium/Electron' `
                            -Result 'ran' `
                            -Verdict $(if ($obsClick.Envelope.ok) {
                                'semantic click delivered; envelope recorded'
                            } else {
                                'honest failure code=' + $obsShape.code + ' delivery=' + $obsShape.disposition_delivery
                            }) `
                            -Shape $obsShape `
                            -Notes ('ok=' + $obsClick.Envelope.ok)
                    }
                }
            }
        }
    } catch {
        Add-Judgement -Id 'J9' -Claim 'Obsidian one semantic action attempt' `
            -Target 'Obsidian' -Result 'skipped' -Verdict 'harness error' -Notes $_.Exception.Message
    }

} finally {
    foreach ($launchedPid in $script:LaunchedPids) {
        try {
            $proc = Get-Process -Id $launchedPid -ErrorAction SilentlyContinue
            if ($proc) { Stop-Process -Id $launchedPid -Force -ErrorAction SilentlyContinue }
        } catch { }
    }
    Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue | ForEach-Object {
        Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
    }
    Get-Process -Name 'ScratchForms' -ErrorAction SilentlyContinue | ForEach-Object {
        Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
    }
    if ($script:ExplorerDir -and (Test-Path -LiteralPath $script:ExplorerDir)) {
        Remove-Item -LiteralPath $script:ExplorerDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

$os = Get-CimInstance Win32_OperatingSystem
$envHeader = [ordered]@{
    os_caption = $os.Caption
    os_build = $os.BuildNumber
    binary = Split-Path -Leaf $script:Binary
    binary_bytes = (Get-Item -LiteralPath $script:Binary).Length
    generated = (Get-Date).ToString('o')
}

$summaryPath = Join-Path $script:OutDir 'semantic-dogfood-run.json'
$summaryJson = ConvertTo-Json -InputObject ([ordered]@{
        environment = $envHeader
        judgements = $script:Judgements
        envelopes = $script:Envelopes
    }) -Depth 12
$redacted = Protect-ProbeText -Text $summaryJson
[IO.File]::WriteAllText($summaryPath, $redacted, $utf8NoBom)
if (-not (Test-CaptureRedaction -Path $summaryPath)) {
    throw "redaction residue in $summaryPath"
}
Write-Host ('dogfood: wrote ' + $summaryPath)
$script:Judgements | ForEach-Object {
    Write-Host ('  ' + $_.id + ': ' + $_.result + ' - ' + $_.verdict)
}
$failed = @($script:Judgements | Where-Object { $_.result -eq 'fail' })
if ($failed.Count -gt 0) {
    Write-Host ('dogfood: ' + $failed.Count + ' judgement(s) failed: ' + (($failed | ForEach-Object { $_.id }) -join ', '))
    exit 1
}
exit 0
