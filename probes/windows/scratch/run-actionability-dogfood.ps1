#Requires -Version 5.1
<#
.SYNOPSIS
    Sub-phase 2.6 U6 actionability/occlusion dogfood runner.

.DESCRIPTION
    Drives target/release/agent-desktop.exe against repo-controlled targets
    (WinForms/WPF scratch, Notepad as foreign occluder, Explorer, Obsidian when
    present). Verifies by reading JSON envelopes — never the suite's opinion of
    itself. Writes a redacted judgement summary under OutDir.

    Zero-foreground interference: minimize/restore and SetWindowPos only on
    processes this script started. Foreign windows are never raised.

    PLATFORM_NOT_SUPPORTED is ambiguous on its own while execute_action is
    unimplemented: a click that scrolled, passed the gate and reached dispatch
    carries it, and so does a click whose scroll seam fell through to the
    trait default without ever reaching dispatch. The envelope names the
    method that was unsupported, so every judgement that accepts
    PLATFORM_NOT_SUPPORTED demands execute_action by name and fails on any
    other, which is the signature of an unimplemented seam answering for the
    whole command.

    Exits non-zero when any judgement recorded 'fail', after the summary is
    written. A judgement that fails and a run that reports success are the
    same defect this suite exists to catch, one level up.
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
    $OutDir = Join-Path $script:RepoRoot 'docs\dogfood-reports\2026-08-06-001-captures'
}
New-Item -ItemType Directory -Path $OutDir -Force | Out-Null
$script:OutDir = (Resolve-Path -LiteralPath $OutDir).ProviderPath
$utf8NoBom = New-Object System.Text.UTF8Encoding $false

if (-not ('AgentDesktopActionabilityDogfood.Native' -as [type])) {
    $nativeCs = Join-Path $script:ScratchDir 'ActionabilityNative.cs'
    $nativeDll = Join-Path $script:ScratchDir 'bin\ActionabilityNative.dll'
    $csc = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
    if (-not (Test-Path -LiteralPath $csc)) { throw "csc.exe not found at $csc" }
    New-Item -ItemType Directory -Path (Split-Path -Parent $nativeDll) -Force | Out-Null
    $needBuild = $true
    if ((Test-Path -LiteralPath $nativeDll) -and (Test-Path -LiteralPath $nativeCs)) {
        if ((Get-Item -LiteralPath $nativeDll).LastWriteTimeUtc -gt (Get-Item -LiteralPath $nativeCs).LastWriteTimeUtc) {
            $needBuild = $false
        }
    }
    if ($needBuild) {
        $cscOut = & $csc /nologo /target:library /langversion:5 /out:$nativeDll $nativeCs 2>&1
        if ($LASTEXITCODE -ne 0) { throw ("ActionabilityNative.dll compile failed: " + ($cscOut | Out-String)) }
    }
    [void][Reflection.Assembly]::LoadFrom($nativeDll)
}

$script:LaunchedPids = New-Object System.Collections.Generic.List[int]
$script:Judgements = New-Object System.Collections.Generic.List[object]
$script:Envelopes = New-Object System.Collections.Generic.List[object]
$script:NoJsonCode = 'BINARY_NO_JSON'
$script:DispatchMethod = 'execute_action'
$script:ScrollMethod = 'scroll_into_view'

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
    param([Parameter(Mandatory = $true)][string]$AppNamePattern)
    $lw = Invoke-Ad -Arguments @('list-windows')
    $rec = @($lw.Envelope.data | Where-Object { $_.app_name -match $AppNamePattern } | Select-Object -First 1)
    if ($rec.Count -eq 0) { return $null }
    return $rec[0].id
}

<#
    Which adapter method an error names, as a boolean, never the text that
    said so. A capture from this runner is committed, so the raw message - it
    can carry a window title, a file name or a path - stays inside this
    function and only the answer leaves it. The word-boundary guard is what
    keeps 'scroll_into_view' from also matching a sentence about
    'scroll_into_view_unsupported'.
#>
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

function Test-EnvelopeFromBinary {
    param([Parameter(Mandatory = $true)]$Shape)
    return ($Shape.code -ne $script:NoJsonCode)
}

<#
    Dispatch was reached, so everything before it - resolve, preflight,
    scroll_into_view - answered. This is the only honest PLATFORM_NOT_SUPPORTED
    while execute_action is unimplemented.
#>
function Test-DispatchReached {
    param([Parameter(Mandatory = $true)]$Shape)
    return (($Shape.code -eq 'PLATFORM_NOT_SUPPORTED') -and ($Shape.message_names_execute_action -eq $true))
}

<#
    A seam short of dispatch answered for the whole command: the scroll
    override missing and the trait default propagating, or any other adapter
    method still on its default. Indistinguishable from a healthy dispatch by
    code alone, which is why the method name is read.
#>
function Test-UnsupportedSeamBeforeDispatch {
    param([Parameter(Mandatory = $true)]$Shape)
    return (($Shape.code -eq 'PLATFORM_NOT_SUPPORTED') -and ($Shape.message_names_execute_action -ne $true))
}

function Get-EnvelopeShape {
    param([Parameter(Mandatory = $true)]$Envelope)
    $shape = [ordered]@{
        ok = [bool]$Envelope.ok
        command = $null
        code = $null
        disposition_delivery = $null
        disposition_retry = $null
        recovery_strategy = $null
        checks = @()
        details_kind = $null
        occluder_role_present = $null
        occluder_name_present = $null
        suggestion_present = $null
        message_names_execute_action = $null
        message_names_scroll_into_view = $null
    }
    if ($Envelope.PSObject.Properties.Name -contains 'command') { $shape.command = [string]$Envelope.command }
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
        $shape.message_names_scroll_into_view = (Test-EnvelopeNamesMethod `
                -Message $messageText -Suggestion $suggestionText -Method $script:ScrollMethod)
        if ($err.PSObject.Properties.Name -contains 'disposition') {
            $d = $err.disposition
            if ($d.PSObject.Properties.Name -contains 'delivery') { $shape.disposition_delivery = [string]$d.delivery }
            if ($d.PSObject.Properties.Name -contains 'retry') { $shape.disposition_retry = [string]$d.retry }
        }
        if ($err.PSObject.Properties.Name -contains 'recovery' -and $err.recovery) {
            if ($err.recovery.PSObject.Properties.Name -contains 'strategy') {
                $shape.recovery_strategy = [string]$err.recovery.strategy
            }
        }
        $details = $null
        if ($err.PSObject.Properties.Name -contains 'details') { $details = $err.details }
        if ($null -ne $details) {
            if ($details.PSObject.Properties.Name -contains 'kind') {
                $shape.details_kind = [string]$details.kind
            }
            $checksSrc = $null
            if ($details.PSObject.Properties.Name -contains 'checks') { $checksSrc = $details.checks }
            elseif ($details.PSObject.Properties.Name -contains 'report' -and $details.report -and
                    $details.report.PSObject.Properties.Name -contains 'checks') {
                $checksSrc = $details.report.checks
            }
            if ($null -ne $checksSrc) {
                $checkShapes = @()
                foreach ($c in @($checksSrc)) {
                    $row = [ordered]@{
                        name = $null
                        status = if ($c.PSObject.Properties.Name -contains 'status') { [string]$c.status } else { $null }
                        reason_shape = $null
                        occluder_role = $null
                        occluder_name_present = $null
                    }
                    if ($c.PSObject.Properties.Name -contains 'check') {
                        $row.name = [string]$c.check
                    } elseif ($c.PSObject.Properties.Name -contains 'name') {
                        $row.name = [string]$c.name
                    }
                    if ($c.PSObject.Properties.Name -contains 'reason' -and $c.reason) {
                        $reason = [string]$c.reason
                        if ($reason -match '^occluded by\s+(\S+)') {
                            $row.reason_shape = 'occluded by <role>'
                        } elseif ($reason -match 'bounds are zero') {
                            $row.reason_shape = 'bounds are zero-sized'
                        } elseif ($reason -match 'enabled state is false') {
                            $row.reason_shape = 'live enabled state is false'
                        } else {
                            $row.reason_shape = 'other'
                        }
                    }
                    if ($c.PSObject.Properties.Name -contains 'occluder' -and $c.occluder) {
                        $occ = $c.occluder
                        if ($occ.PSObject.Properties.Name -contains 'role') {
                            $row.occluder_role = [string]$occ.role
                            $shape.occluder_role_present = $true
                        }
                        if ($occ.PSObject.Properties.Name -contains 'name' -and $null -ne $occ.name -and $occ.name -ne '') {
                            $row.occluder_name_present = $true
                            $shape.occluder_name_present = $true
                        } else {
                            $row.occluder_name_present = $false
                            if ($null -eq $shape.occluder_name_present) { $shape.occluder_name_present = $false }
                        }
                    }
                    $checkShapes += $row
                }
                $shape.checks = $checkShapes
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
    param([string]$Id, [object]$Shape, [string]$RawSnippet = '')
    [void]$script:Envelopes.Add([ordered]@{
            id = $Id
            shape = $Shape
            raw_redacted_keys_only = $true
            note = $RawSnippet
        })
}

function Find-RefByNativeId {
    param([string]$WindowId, [string]$NativeId, [string]$Role = '')
    $args = [System.Collections.Generic.List[string]]@('find', '--window-id', $WindowId, '--native-id', $NativeId, '--first')
    if ($Role) { [void]$args.Add('--role'); [void]$args.Add($Role) }
    $found = Invoke-Ad -Arguments $args.ToArray()
    if (-not $found.Envelope.ok) { return $null }
    $data = $found.Envelope.data
    if ($data.PSObject.Properties.Name -contains 'ref_id') { return [string]$data.ref_id }
    if ($data.PSObject.Properties.Name -contains 'match' -and $data.match) {
        if ($data.match.PSObject.Properties.Name -contains 'ref_id') { return [string]$data.match.ref_id }
        if ($data.match.PSObject.Properties.Name -contains 'ref') { return [string]$data.match.ref }
    }
    if ($data.PSObject.Properties.Name -contains 'matches') {
        $m = @($data.matches) | Select-Object -First 1
        if ($null -ne $m) {
            if ($m.PSObject.Properties.Name -contains 'ref_id') { return [string]$m.ref_id }
            if ($m.PSObject.Properties.Name -contains 'ref') { return [string]$m.ref }
        }
    }
    $json = $found.Raw
    $m2 = [regex]::Match($json, '"ref(?:_id)?"\s*:\s*"(?<r>@[A-Za-z0-9_:-]+)"')
    if ($m2.Success) { return $m2.Groups['r'].Value }
    return $null
}

function Find-RefsByRole {
    param([string]$WindowId, [string]$Role, [int]$Limit = 20)
    $found = Invoke-Ad -Arguments @('find', '--window-id', $WindowId, '--role', $Role, '--limit', ([string]$Limit))
    $refs = New-Object System.Collections.Generic.List[string]
    if (-not $found.Envelope.ok) { return $refs }
    $data = $found.Envelope.data
    $items = @()
    if ($data.PSObject.Properties.Name -contains 'matches') { $items = @($data.matches) }
    elseif ($data.PSObject.Properties.Name -contains 'match') { $items = @($data.match) }
    foreach ($item in $items) {
        if ($item.PSObject.Properties.Name -contains 'ref_id') { [void]$refs.Add([string]$item.ref_id) }
        elseif ($item.PSObject.Properties.Name -contains 'ref') { [void]$refs.Add([string]$item.ref) }
    }
    if ($refs.Count -eq 0) {
        $matches = [regex]::Matches($found.Raw, '"ref(?:_id)?"\s*:\s*"(?<r>@[A-Za-z0-9_:-]+)"')
        foreach ($m in $matches) {
            $v = $m.Groups['r'].Value
            if (-not $refs.Contains($v)) { [void]$refs.Add($v) }
        }
    }
    return $refs
}

function Invoke-HeadedClick {
    param([string]$Ref, [int]$TimeoutMs = 0)
    return Invoke-Ad -Arguments @('click', $Ref, '--headed', '--timeout-ms', ([string]$TimeoutMs))
}

function Get-BoundsCenter {
    param([string]$Ref)
    $g = Invoke-Ad -Arguments @('get', $Ref, '--property', 'bounds')
    if (-not $g.Envelope.ok) { return $null }
    $b = $g.Envelope.data.value
    if ($null -eq $b) { return $null }
    $x = [double]$b.x; $y = [double]$b.y; $w = [double]$b.width; $h = [double]$b.height
    return [ordered]@{ x = $x; y = $y; width = $w; height = $h; cx = ($x + $w / 2.0); cy = ($y + $h / 2.0) }
}

function Set-OwnedWindowRect {
    param([IntPtr]$Handle, [int]$X, [int]$Y, [int]$W, [int]$H, [switch]$TopMost)
    $after = if ($TopMost) {
        [AgentDesktopActionabilityDogfood.Native]::HWND_TOPMOST
    } else {
        [AgentDesktopActionabilityDogfood.Native]::HWND_NOTOPMOST
    }
    $flags = [AgentDesktopActionabilityDogfood.Native]::SWP_SHOWWINDOW
    [void][AgentDesktopActionabilityDogfood.Native]::SetWindowPos($Handle, $after, $X, $Y, $W, $H, $flags)
}

function Clear-OwnedTopMost {
    param([IntPtr]$Handle)
    [void][AgentDesktopActionabilityDogfood.Native]::SetWindowPos(
        $Handle,
        [AgentDesktopActionabilityDogfood.Native]::HWND_NOTOPMOST,
        0, 0, 0, 0,
        ([AgentDesktopActionabilityDogfood.Native]::SWP_NOSIZE -bor
         [AgentDesktopActionabilityDogfood.Native]::SWP_NOMOVE -bor
         [AgentDesktopActionabilityDogfood.Native]::SWP_SHOWWINDOW)
    )
}

try {
    & (Join-Path $script:ScratchDir 'build-scratch.ps1') | Out-Null
    $scratchExe = Join-Path $script:ScratchDir 'bin\ScratchForms.exe'
    if (-not (Test-Path -LiteralPath $scratchExe)) { throw "ScratchForms.exe missing at $scratchExe" }

    # -------------------------------------------------------------------------
    # J1/J2: foreign-process occluder (Notepad over WinForms btnAction) + clear
    # -------------------------------------------------------------------------
    $winforms = $null
    $notepad = $null
    try {
        $winforms = Start-DogfoodProcess -FilePath $scratchExe -ArgumentList @('--tag', 'u6', '--pos', '80,80', '--host-providers')
        $wfHwnd = Wait-MainWindow -Process $winforms -TimeoutSec 20
        if ($wfHwnd -eq [IntPtr]::Zero) { throw 'ScratchForms never presented a window' }
        Start-Sleep -Seconds 2
        $wid = Find-WindowIdFor 'ScratchForms'
        if (-not $wid) { $wid = 'w-' + $wfHwnd.ToInt64() }
        $snap = Invoke-Ad -Arguments @('snapshot', '--window-id', $wid)
        if (-not $snap.Envelope.ok) { throw ('snapshot failed: ' + $snap.Envelope.error.code) }
        $actionRef = Find-RefByNativeId -WindowId $wid -NativeId 'btnAction'
        if (-not $actionRef) { throw 'btnAction ref not found' }
        $bounds = Get-BoundsCenter -Ref $actionRef
        if (-not $bounds) { throw 'btnAction bounds unavailable' }

        $scratchFile = Join-Path $env:TEMP ('agent-desktop-u6-' + [guid]::NewGuid() + '.txt')
        [IO.File]::WriteAllText($scratchFile, "synthetic u6 occluder`r`n", $utf8NoBom)
        $notepad = Start-DogfoodProcess -FilePath 'notepad.exe' -ArgumentList @($scratchFile)
        $npHwnd = Wait-MainWindow -Process $notepad -TimeoutSec 15
        if ($npHwnd -eq [IntPtr]::Zero) { throw 'Notepad never presented a window' }
        # Cover the target control with the owned Notepad window (foreign process).
        $coverX = [int]([math]::Floor($bounds.x - 40))
        $coverY = [int]([math]::Floor($bounds.y - 40))
        Set-OwnedWindowRect -Handle $npHwnd -X $coverX -Y $coverY -W 420 -H 320 -TopMost
        Start-Sleep -Milliseconds 400

        $covered = Invoke-HeadedClick -Ref $actionRef -TimeoutMs 0
        $coveredShape = Get-EnvelopeShape -Envelope $covered.Envelope
        Add-EnvelopeRecord -Id 'J1-foreign-occluder' -Shape $coveredShape
        $j1Ok = (-not $covered.Envelope.ok) -and `
            ($coveredShape.code -eq 'ACTION_FAILED') -and `
            (@($coveredShape.checks | Where-Object {
                    $_.name -eq 'receives_events' -and $_.status -eq 'fail' -and $_.reason_shape -eq 'occluded by <role>'
                }).Count -gt 0) -and `
            ($coveredShape.occluder_role_present -eq $true) -and `
            ($coveredShape.suggestion_present -eq $true)
        Add-Judgement -Id 'J1' -Claim 'foreign-process occluder names occluder on headed click' `
            -Target 'ScratchForms btnAction under Notepad' `
            -Result $(if ($j1Ok) { 'pass' } else { 'fail' }) `
            -Verdict $(if ($j1Ok) { 'occluder named with honest recovery' } else { 'envelope did not match expected occlusion shape' }) `
            -Shape $coveredShape `
            -Notes ('code=' + $coveredShape.code + ' recovery=' + $coveredShape.recovery_strategy)

        # Dismiss occluder (kill owned Notepad only).
        Clear-OwnedTopMost -Handle $npHwnd
        Stop-Process -Id $notepad.Id -Force -ErrorAction SilentlyContinue
        $notepad = $null
        Start-Sleep -Milliseconds 500

        # Re-snapshot so the ref is fresh after z-order change (same control).
        $snap2 = Invoke-Ad -Arguments @('snapshot', '--window-id', $wid)
        if (-not $snap2.Envelope.ok) { throw ('post-dismiss snapshot failed: ' + $snap2.Envelope.error.code) }
        $actionRef2 = Find-RefByNativeId -WindowId $wid -NativeId 'btnAction'
        if (-not $actionRef2) { throw 'btnAction ref missing after dismiss' }
        $clear = Invoke-HeadedClick -Ref $actionRef2 -TimeoutMs 0
        $clearShape = Get-EnvelopeShape -Envelope $clear.Envelope
        Add-EnvelopeRecord -Id 'J2-gate-pass-dispatch' -Shape $clearShape
        $j2Seam = Test-UnsupportedSeamBeforeDispatch -Shape $clearShape
        $j2Ok = (-not $clear.Envelope.ok) -and (Test-DispatchReached -Shape $clearShape)
        $j2Verdict = if ($j2Ok) {
            'PLATFORM_NOT_SUPPORTED naming execute_action, so the gate passed and dispatch was reached'
        } elseif ($j2Seam) {
            'unimplemented seam answered before dispatch, the gate never proved it passed'
        } elseif (-not (Test-EnvelopeFromBinary -Shape $clearShape)) {
            'binary produced no JSON envelope'
        } else {
            'unexpected code after gate pass'
        }
        Add-Judgement -Id 'J2' -Claim 'unoccluded headed click reaches honest pre-2.7 dispatch' `
            -Target 'ScratchForms btnAction after Notepad dismissed' `
            -Result $(if ($j2Ok) { 'pass' } else { 'fail' }) `
            -Verdict $j2Verdict `
            -Shape $clearShape `
            -Notes ('code=' + $clearShape.code +
                ' names_execute_action=' + $clearShape.message_names_execute_action +
                ' names_scroll_into_view=' + $clearShape.message_names_scroll_into_view)

        # ---------------------------------------------------------------------
        # J3: same-root overlay (btnCovered under btnOverlay)
        # ---------------------------------------------------------------------
        try {
            $coveredRef = Find-RefByNativeId -WindowId $wid -NativeId 'btnCovered'
            if (-not $coveredRef) { throw 'btnCovered ref not found' }
            # Enlarge overlay in parent-client coords so every five-point
            # candidate is intercepted (partial fixture overlap lets one inset reach).
            Add-Type -AssemblyName UIAutomationClient -ErrorAction SilentlyContinue
            Add-Type -AssemblyName UIAutomationTypes -ErrorAction SilentlyContinue
            $fixtureRoot = [System.Windows.Automation.AutomationElement]::FromHandle($wfHwnd)
            if ($null -eq $fixtureRoot) { throw 'fixture AutomationElement root is null' }
            $byId = {
                param($aid)
                $c = New-Object System.Windows.Automation.PropertyCondition (
                    [System.Windows.Automation.AutomationElement]::AutomationIdProperty), $aid
                return $fixtureRoot.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $c)
            }
            $byName = {
                param($name)
                $c = New-Object System.Windows.Automation.PropertyCondition (
                    [System.Windows.Automation.AutomationElement]::NameProperty), $name
                return $fixtureRoot.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $c)
            }
            $coveredEl = (& $byId 'btnCovered')
            if ($null -eq $coveredEl) { $coveredEl = (& $byName 'Covered') }
            $overlayEl = (& $byId 'btnOverlay')
            if ($null -eq $overlayEl) { $overlayEl = (& $byName 'Overlay') }
            if ($null -eq $coveredEl -or $null -eq $overlayEl) {
                throw 'btnCovered/btnOverlay elements missing from fixture tree'
            }
            $cb = $coveredEl.Current.BoundingRectangle
            $oh = [IntPtr]$overlayEl.Current.NativeWindowHandle
            if ($oh -eq [IntPtr]::Zero) { throw 'btnOverlay has no HWND (cannot resize)' }
            $parent = [AgentDesktopActionabilityDogfood.Native]::GetParent($oh)
            if ($parent -eq [IntPtr]::Zero) { $parent = $wfHwnd }
            $pt = New-Object AgentDesktopActionabilityDogfood.Native+POINT
            $pt.X = [int]$cb.X
            $pt.Y = [int]$cb.Y
            [void][AgentDesktopActionabilityDogfood.Native]::ScreenToClient($parent, [ref]$pt)
            [void][AgentDesktopActionabilityDogfood.Native]::SetWindowPos(
                $oh, [IntPtr]::Zero,
                $pt.X, $pt.Y,
                [int][math]::Max(1, [math]::Ceiling($cb.Width)),
                [int][math]::Max(1, [math]::Ceiling($cb.Height)),
                [AgentDesktopActionabilityDogfood.Native]::SWP_SHOWWINDOW)
            [void][AgentDesktopActionabilityDogfood.Native]::BringWindowToTop($oh)
            Start-Sleep -Milliseconds 300
            $coveredRef = Find-RefByNativeId -WindowId $wid -NativeId 'btnCovered'
            if (-not $coveredRef) { throw 'btnCovered ref missing after overlay resize' }
            $sameRoot = Invoke-HeadedClick -Ref $coveredRef -TimeoutMs 0
            $sameShape = Get-EnvelopeShape -Envelope $sameRoot.Envelope
            Add-EnvelopeRecord -Id 'J3-same-root-overlay' -Shape $sameShape
            $j3Ok = (-not $sameRoot.Envelope.ok) -and `
                ($sameShape.code -eq 'ACTION_FAILED') -and `
                (@($sameShape.checks | Where-Object {
                        $_.name -eq 'receives_events' -and $_.status -eq 'fail' -and $_.reason_shape -eq 'occluded by <role>'
                    }).Count -gt 0) -and `
                ($sameShape.occluder_role_present -eq $true)
            $occRole = ''
            $occRow = @($sameShape.checks | Where-Object { $_.occluder_role }) | Select-Object -First 1
            if ($null -ne $occRow) { $occRole = [string]$occRow.occluder_role }
            Add-Judgement -Id 'J3' -Claim 'same-root in-window overlay names occluder' `
                -Target 'ScratchForms btnCovered under btnOverlay' `
                -Result $(if ($j3Ok) { 'pass' } else { 'fail' }) `
                -Verdict $(if ($j3Ok) { 'in-window occluder named' } else { 'same-root arm did not name occluder' }) `
                -Shape $sameShape `
                -Notes ('code=' + $sameShape.code + ' occluder_role=' + $occRole)
        } catch {
            Add-Judgement -Id 'J3' -Claim 'same-root in-window overlay names occluder' `
                -Target 'ScratchForms btnCovered' -Result 'skipped' -Verdict 'harness error' `
                -Notes $_.Exception.Message
        }

        # ---------------------------------------------------------------------
        # J6: minimized-window guard (owned fixture only)
        # ---------------------------------------------------------------------
        try {
            $minRef = Find-RefByNativeId -WindowId $wid -NativeId 'btnAction'
            if (-not $minRef) { throw 'btnAction missing for minimize leg' }
            [void][AgentDesktopActionabilityDogfood.Native]::ShowWindow($wfHwnd, [AgentDesktopActionabilityDogfood.Native]::SW_MINIMIZE)
            Start-Sleep -Milliseconds 300
            $iconic = [AgentDesktopActionabilityDogfood.Native]::IsIconic($wfHwnd)
            $minClick = Invoke-HeadedClick -Ref $minRef -TimeoutMs 0
            $minShape = Get-EnvelopeShape -Envelope $minClick.Envelope
            Add-EnvelopeRecord -Id 'J6-minimized-guard' -Shape $minShape
            $namedOccluder = @($minShape.checks | Where-Object {
                    $_.name -eq 'receives_events' -and $_.reason_shape -eq 'occluded by <role>'
                }).Count -gt 0
            # Headed focus restores the window before hit_test; the guard still
            # holds when the envelope never invents a phantom InterceptedBy.
            # "No occluder was named" is only evidence when something judged
            # the click: a crashed binary and a seam that answered before any
            # guard ran both name no occluder while proving nothing.
            $j6FromBinary = Test-EnvelopeFromBinary -Shape $minShape
            $j6Seam = Test-UnsupportedSeamBeforeDispatch -Shape $minShape
            $j6Ok = $j6FromBinary -and (-not $j6Seam) -and (-not $namedOccluder) -and (-not $minClick.Envelope.ok)
            $j6Verdict = if ($j6Ok) {
                'no occluder-named envelope (pre_click_iconic=' + $iconic + ' code=' + $minShape.code + ')'
            } elseif (-not $j6FromBinary) {
                'binary produced no JSON envelope, so nothing judged the minimized target'
            } elseif ($j6Seam) {
                'unimplemented seam answered before any guard ran, the guard was never exercised'
            } elseif ($namedOccluder) {
                'invented occlusion against minimized target'
            } else {
                'headed click reported success against a minimized target'
            }
            Add-Judgement -Id 'J6' -Claim 'minimized-window guard does not invent InterceptedBy' `
                -Target 'minimized ScratchForms btnAction' `
                -Result $(if ($j6Ok) { 'pass' } else { 'fail' }) `
                -Verdict $j6Verdict `
                -Shape $minShape `
                -Notes ('is_iconic_before_click=' + $iconic + ' named_occluder=' + $namedOccluder +
                    ' code=' + $minShape.code +
                    ' names_execute_action=' + $minShape.message_names_execute_action)
            [void][AgentDesktopActionabilityDogfood.Native]::ShowWindow($wfHwnd, [AgentDesktopActionabilityDogfood.Native]::SW_RESTORE)
        } catch {
            Add-Judgement -Id 'J6' -Claim 'minimized-window guard does not invent InterceptedBy' `
                -Target 'minimized ScratchForms' -Result 'skipped' -Verdict 'harness error' `
                -Notes $_.Exception.Message
        }
    } catch {
        Add-Judgement -Id 'J1-J2' -Claim 'scratch foreign occluder cluster' `
            -Target 'ScratchForms' -Result 'skipped' -Verdict 'harness error' `
            -Notes $_.Exception.Message
    } finally {
        if ($null -ne $notepad) {
            Stop-Process -Id $notepad.Id -Force -ErrorAction SilentlyContinue
        }
        if ($null -ne $winforms) {
            Stop-Process -Id $winforms.Id -Force -ErrorAction SilentlyContinue
        }
    }

    # -------------------------------------------------------------------------
    # J4: below-fold Explorer list item — scroll seam or honest unsupported
    # -------------------------------------------------------------------------
    $explorerDir = Join-Path $env:TEMP ('agent-desktop-u6-dir-' + [guid]::NewGuid())
    New-Item -ItemType Directory -Path $explorerDir -Force | Out-Null
    try {
        1..40 | ForEach-Object {
            $n = 'file-{0:D2}.txt' -f $_
            [IO.File]::WriteAllText((Join-Path $explorerDir $n), ("synthetic $n`r`n"), $utf8NoBom)
        }
        [void](Start-DogfoodProcess -FilePath 'explorer.exe' -ArgumentList @($explorerDir))
        Write-Host 'dogfood: waiting 22s for Explorer filesystem settle (A7-4)'
        Start-Sleep -Seconds 22
        $exWid = Find-WindowIdFor 'explorer'
        if (-not $exWid) { throw 'no Explorer window resolved' }
        $exSnap = Invoke-Ad -Arguments @('snapshot', '--window-id', $exWid, '--timeout-ms', '10000')
        if (-not $exSnap.Envelope.ok) { throw ('explorer snapshot failed: ' + $exSnap.Envelope.error.code) }
        $listRefs = @(Find-RefsByRole -WindowId $exWid -Role 'listitem' -Limit 40)
        if (@($listRefs).Count -eq 0) {
            $listRefs = @(Find-RefsByRole -WindowId $exWid -Role 'treeitem' -Limit 40)
        }
        $offscreenRef = $null
        foreach ($r in $listRefs) {
            $vis = Invoke-Ad -Arguments @('is', $r, '--property', 'visible')
            if ($vis.Envelope.ok -and $vis.Envelope.data.PSObject.Properties.Name -contains 'result' -and
                [bool]$vis.Envelope.data.result -eq $false) {
                $offscreenRef = $r
                break
            }
        }
        if (-not $offscreenRef -and @($listRefs).Count -gt 0) {
            $offscreenRef = $listRefs[@($listRefs).Count - 1]
        }
        if (-not $offscreenRef) {
            Add-Judgement -Id 'J4' -Claim 'below-fold Explorer auto-scroll or honest unsupported' `
                -Target 'Explorer listitem' -Result 'skipped' `
                -Verdict 'no listitem/treeitem refs in Explorer snapshot' `
                -Notes ('ref_count=' + $exSnap.Envelope.data.ref_count)
        } else {
            $scrollClick = Invoke-Ad -Arguments @('click', $offscreenRef, '--timeout-ms', '3000')
            $scrollShape = Get-EnvelopeShape -Envelope $scrollClick.Envelope
            Add-EnvelopeRecord -Id 'J4-explorer-scroll' -Shape $scrollShape
            # The leg the scroll seam exists for, so it is the leg that must
            # not accept the seam's absence. A click that scrolled and reached
            # dispatch and a click whose scroll_into_view fell through to the
            # trait default both carry PLATFORM_NOT_SUPPORTED; only the method
            # named in the envelope separates them.
            $fromBinary = Test-EnvelopeFromBinary -Shape $scrollShape
            $seamBeforeDispatch = Test-UnsupportedSeamBeforeDispatch -Shape $scrollShape
            $scrolled = Test-DispatchReached -Shape $scrollShape
            $unsupported = ($scrollShape.details_kind -eq 'scroll_into_view_unsupported')
            $visibleFail = @($scrollShape.checks | Where-Object { $_.name -eq 'visible' -and $_.status -eq 'fail' }).Count -gt 0
            $observedUnverified = ($scrollShape.code -eq 'ACTION_FAILED' -and `
                $scrollShape.disposition_delivery -eq 'delivered_unverified')
            $notDelivered = ($scrollShape.code -eq 'ACTION_FAILED' -and `
                $scrollShape.disposition_delivery -eq 'not_delivered')
            $j4Ok = $fromBinary -and (-not $seamBeforeDispatch) -and `
                ($scrolled -or $unsupported -or $visibleFail -or $observedUnverified -or $notDelivered)
            $verdict = if ($seamBeforeDispatch) {
                'unimplemented seam answered before dispatch: scroll_into_view never carried the below-fold item to the gate'
            } elseif (-not $fromBinary) {
                'binary produced no JSON envelope'
            } elseif ($scrolled) {
                'scroll verified then honest PLATFORM_NOT_SUPPORTED naming execute_action'
            } elseif ($unsupported) {
                'honest scroll_into_view_unsupported / not_delivered'
            } elseif ($observedUnverified) {
                'honest observation-judged delivered_unverified (KTD5)'
            } elseif ($notDelivered) {
                'honest ACTION_FAILED not_delivered on below-fold item'
            } elseif ($visibleFail) {
                'visible fail retained (scroll unavailable or incomplete)'
            } else {
                'unexpected explorer scroll envelope'
            }
            Add-Judgement -Id 'J4' -Claim 'below-fold Explorer auto-scroll or honest unsupported' `
                -Target 'Explorer listitem/treeitem' `
                -Result $(if ($j4Ok) { 'pass' } else { 'fail' }) `
                -Verdict $verdict `
                -Shape $scrollShape `
                -Notes ('candidates=' + @($listRefs).Count + ' code=' + $scrollShape.code +
                    ' delivery=' + $scrollShape.disposition_delivery + ' kind=' + $scrollShape.details_kind +
                    ' names_execute_action=' + $scrollShape.message_names_execute_action +
                    ' names_scroll_into_view=' + $scrollShape.message_names_scroll_into_view)
        }
    } catch {
        Add-Judgement -Id 'J4' -Claim 'below-fold Explorer auto-scroll or honest unsupported' `
            -Target 'Explorer' -Result 'skipped' -Verdict 'harness error' -Notes $_.Exception.Message
    }

    # -------------------------------------------------------------------------
    # J5: Chromium / Obsidian — U6 measurement of record for A18-3
    # -------------------------------------------------------------------------
    $obsidianExe = Join-Path $env:LOCALAPPDATA 'Programs\Obsidian\Obsidian.exe'
    try {
        if (-not (Test-Path -LiteralPath $obsidianExe)) {
            Add-Judgement -Id 'J5' -Claim 'Chromium hit-test branch (A18-3)' `
                -Target 'Obsidian' -Result 'skipped' `
                -Verdict 'Obsidian not installed' -Notes $obsidianExe
        } else {
            $obs = Start-DogfoodProcess -FilePath $obsidianExe
            Start-Sleep -Seconds 12
            $obsWid = $null
            for ($i = 0; $i -lt 16 -and -not $obsWid; $i++) {
                $obsWid = Find-WindowIdFor 'Obsidian'
                if (-not $obsWid) { Start-Sleep -Seconds 2 }
            }
            if (-not $obsWid) { throw 'no Obsidian window resolved' }
            $obsSnap = Invoke-Ad -Arguments @('snapshot', '--window-id', $obsWid, '--timeout-ms', '45000')
            if (-not $obsSnap.Envelope.ok) {
                $code = $obsSnap.Envelope.error.code
                if ($code -eq 'TIMEOUT') {
                    Add-Judgement -Id 'J5' -Claim 'Chromium hit-test behaves as A18-3 branch' `
                        -Target 'Obsidian Chromium/Electron' `
                        -Result 'ran' `
                        -Verdict 'target_absent_or_shell_bound' `
                        -Notes 'snapshot TIMEOUT on cold Obsidian (A16-11/A18-3 shell branch; U6 measurement of record)'
                    throw 'obsidian-timeout-recorded'
                }
                throw ('obsidian snapshot failed: ' + $code)
            }
            $refCount = $obsSnap.Envelope.data.ref_count
            $complete = $obsSnap.Envelope.data.complete
            $obsRefs = @([regex]::Matches($obsSnap.Raw, '"ref(?:_id)?"\s*:\s*"(?<r>@[A-Za-z0-9_:-]+)"') |
                    ForEach-Object { $_.Groups['r'].Value } | Select-Object -Unique)
            $positiveLeaf = $null
            $leafBoundsOk = 0
            foreach ($r in ($obsRefs | Select-Object -First 12)) {
                $b = Get-BoundsCenter -Ref $r
                if ($null -ne $b -and $b.width -gt 0 -and $b.height -gt 0) {
                    $leafBoundsOk++
                    if (-not $positiveLeaf) { $positiveLeaf = $r }
                }
            }
            $branch = 'target_absent_or_shell_bound'
            $clickShape = $null
            if ($positiveLeaf) {
                $obsClick = Invoke-HeadedClick -Ref $positiveLeaf -TimeoutMs 0
                $clickShape = Get-EnvelopeShape -Envelope $obsClick.Envelope
                Add-EnvelopeRecord -Id 'J5-chromium-headed-click' -Shape $clickShape
                $named = @($clickShape.checks | Where-Object {
                        $_.name -eq 'receives_events' -and $_.reason_shape -eq 'occluded by <role>'
                    }).Count -gt 0
                if ($named) {
                    $branch = 'same_root_intercepted_by'
                } elseif (Test-DispatchReached -Shape $clickShape) {
                    # Gate passed: ReachesTarget or Unknown (ancestor/pane) — both fail-open to dispatch.
                    $branch = 'gate_pass_reaches_or_unknown_ancestor'
                } elseif (Test-UnsupportedSeamBeforeDispatch -Shape $clickShape) {
                    # Not a gate pass: a seam short of dispatch answered, so this
                    # run measured nothing about the Chromium hit-test branch.
                    $branch = 'unsupported_seam_before_dispatch'
                } else {
                    $branch = 'other_envelope_' + $clickShape.code
                }
            }
            $notes = ('refs=' + $refCount + ' complete=' + $complete +
                ' positive_area_sampled=' + $leafBoundsOk + ' branch=' + $branch)
            Add-Judgement -Id 'J5' -Claim 'Chromium hit-test behaves as A18-3 branch' `
                -Target 'Obsidian Chromium/Electron' `
                -Result 'ran' `
                -Verdict $branch `
                -Shape $clickShape `
                -Notes $notes
        }
    } catch {
        if ($_.Exception.Message -ne 'obsidian-timeout-recorded') {
            Add-Judgement -Id 'J5' -Claim 'Chromium hit-test branch (A18-3)' `
                -Target 'Obsidian' -Result 'skipped' -Verdict 'harness error' -Notes $_.Exception.Message
        }
    }

    # Optional WPF same-root corroboration (does not replace J3 WinForms).
    $wpfScript = Join-Path $script:ScratchDir 'ScratchWpf.ps1'
    $wpf = $null
    try {
        $wpf = Start-DogfoodProcess -FilePath 'powershell.exe' -WindowStyle 'Hidden' -ArgumentList @(
            '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $wpfScript,
            '-Tag', 'u6', '-Left', '500', '-Top', '80', '-TimeoutSeconds', '120'
        )
        $wpfHwnd = Wait-MainWindow -Process $wpf -TimeoutSec 25
        if ($wpfHwnd -eq [IntPtr]::Zero) { throw 'WPF fixture never presented a window' }
        Start-Sleep -Seconds 2
        $wpfWid = 'w-' + $wpfHwnd.ToInt64()
        $wpfSnap = Invoke-Ad -Arguments @('snapshot', '--window-id', $wpfWid)
        if (-not $wpfSnap.Envelope.ok) { throw ('wpf snapshot failed: ' + $wpfSnap.Envelope.error.code) }
        $wpfCovered = Find-RefByNativeId -WindowId $wpfWid -NativeId 'btnCovered' -Role 'button'
        if ($wpfCovered) {
            $wpfClick = Invoke-HeadedClick -Ref $wpfCovered -TimeoutMs 0
            $wpfShape = Get-EnvelopeShape -Envelope $wpfClick.Envelope
            Add-EnvelopeRecord -Id 'J3b-wpf-same-root' -Shape $wpfShape
            $wpfOk = (-not $wpfClick.Envelope.ok) -and ($wpfShape.occluder_role_present -eq $true)
            Add-Judgement -Id 'J3b' -Claim 'WPF same-root overlay corroboration' `
                -Target 'ScratchWpf btnCovered' `
                -Result $(if ($wpfOk) { 'pass' } else { 'ran' }) `
                -Verdict $(if ($wpfOk) { 'in-window occluder named' } else { 'envelope recorded' }) `
                -Shape $wpfShape -Notes ('code=' + $wpfShape.code)
        } else {
            Add-Judgement -Id 'J3b' -Claim 'WPF same-root overlay corroboration' `
                -Target 'ScratchWpf' -Result 'skipped' -Verdict 'btnCovered ref absent' `
                -Notes ('ref_count=' + $wpfSnap.Envelope.data.ref_count)
        }
    } catch {
        Add-Judgement -Id 'J3b' -Claim 'WPF same-root overlay corroboration' `
            -Target 'ScratchWpf' -Result 'skipped' -Verdict 'harness error' -Notes $_.Exception.Message
    } finally {
        if ($null -ne $wpf) { Stop-Process -Id $wpf.Id -Force -ErrorAction SilentlyContinue }
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
    if (Test-Path -LiteralPath $explorerDir) {
        Remove-Item -LiteralPath $explorerDir -Recurse -Force -ErrorAction SilentlyContinue
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

$summaryPath = Join-Path $script:OutDir 'actionability-dogfood-run.json'
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
