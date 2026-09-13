#Requires -Version 5.1
<#
.SYNOPSIS
    Sub-phase 2.5 live-loop dogfood runner (U7).

.DESCRIPTION
    Drives the release binary's live loop against repo-controlled targets -
    classic Notepad on a scratch file, Explorer on a scratch directory, the
    WinForms and WPF scratch fixtures, and Obsidian (Chromium/Electron) when
    present - reading the binary's JSON, never the suite's opinion of itself.
    Per target it snapshots, round-trips a find, and reads live values and
    state off sampled refs (get/is). The WinForms fixture also runs an
    A7-3-shaped identity-stability judgement: a stored ref's live `value` is
    captured before and after a fixture content swap and compared, not just
    re-resolved with an `ok:true` on a stored-field read - a value that
    drifted after a successful resolve is the silent-neighbour signature
    `ok:true` alone cannot see. A17-2 measured this fixture's ListBox exposes
    zero ListItems to a COM client, so the swapped rows are never in the
    walked/reffed tree; the judgement therefore checks whether the sampled
    (non-list) ref's identity survives an unrelated content-changing message,
    not A7-3's index-keyed wrong-target shape itself. The swap is driven
    through the control the fixture actually implements, `btnMutateList`, and
    is witnessed independently on the fixture's own status text: a judgement
    taken across a swap that did not happen would report stability no matter
    what the resolver did, so an unwitnessed swap is reported as such instead.
    Obsidian is read shape-only, and its refs' re-resolution STALE_REF rate is
    reported honestly. Every capture is redacted through the corpus gate.
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
if (-not $OutDir) { $OutDir = Join-Path $script:RepoRoot 'docs\dogfood-reports\2026-08-03-001-captures' }
New-Item -ItemType Directory -Path $OutDir -Force | Out-Null
$script:OutDir = (Resolve-Path -LiteralPath $OutDir).ProviderPath
$utf8NoBom = New-Object System.Text.UTF8Encoding $false

$script:LaunchedPids = New-Object System.Collections.Generic.List[int]
function Start-LiveProcess {
    param([string]$FilePath, [string[]]$ArgumentList = @())
    if ($ArgumentList.Count -gt 0) {
        $proc = Start-Process -FilePath $FilePath -ArgumentList $ArgumentList -PassThru
    } else {
        $proc = Start-Process -FilePath $FilePath -PassThru
    }
    [void]$script:LaunchedPids.Add($proc.Id)
    return $proc
}
function Invoke-Ad {
    param([string[]]$Arguments)
    $raw = (& $script:Binary @Arguments 2>$null | Out-String)
    $exitCode = $LASTEXITCODE
    $parsed = $null
    if ($raw -and $raw.Trim()) {
        try { $parsed = ($raw | ConvertFrom-Json) } catch { $parsed = $null }
    }
    # A non-zero exit still prints the error envelope, and that envelope carries
    # the code the caller needs. Returning a bare placeholder instead loses it
    # and, under this script's strict mode, turns every caller's `.ok` read into
    # a PowerShell property error that is then recorded as the target's reason -
    # so a run where the binary failed on every target reads as a harness bug.
    if ($null -ne $parsed) { return $parsed }
    return [pscustomobject]@{
        ok    = $false
        error = [pscustomobject]@{
            code    = 'BINARY_NO_JSON'
            message = ('agent-desktop exited ' + $exitCode + ' with no JSON for: ' + ($Arguments -join ' '))
        }
    }
}
function Find-WindowIdFor {
    param([Parameter(Mandatory = $true)][string]$AppNamePattern)
    $lw = Invoke-Ad -Arguments @('list-windows')
    $rec = @($lw.data | Where-Object { $_.app_name -match $AppNamePattern } | Select-Object -First 1)
    if ($rec.Count -eq 0) { return $null }
    return $rec[0].id
}
function Get-FirstRef {
    param([Parameter(Mandatory = $true)][string]$WindowId)
    $snapText = (& $script:Binary snapshot --window-id $WindowId 2>$null | Out-String)
    $snapObj = $snapText | ConvertFrom-Json
    if (-not $snapObj.ok) { return $null }
    $json = $snapObj | ConvertTo-Json -Depth 40
    $m = [regex]::Match($json, '"ref(?:_id)?"\s*:\s*"(?<r>@[A-Za-z0-9_:-]+)"')
    if (-not $m.Success) {
        $m = [regex]::Match($snapText, '"(?<r>@[A-Za-z0-9_:-]+)"')
    }
    return $m.Groups['r'].Value
}

function Get-FixtureElement {
    param([Parameter(Mandatory = $true)]$Root, [Parameter(Mandatory = $true)][string]$AutomationId)
    $condition = New-Object System.Windows.Automation.PropertyCondition ([System.Windows.Automation.AutomationElement]::AutomationIdProperty), $AutomationId
    return $Root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condition)
}

function Get-FixtureStatusText {
    param([Parameter(Mandatory = $true)]$Root)
    $status = Get-FixtureElement -Root $Root -AutomationId 'lblStatus'
    if ($null -eq $status) { return '' }
    return [string]$status.Current.Name
}

$script:Results = New-Object System.Collections.Generic.List[object]
function New-FindRow {
    param([string]$Name, [string]$Stack, [string]$Result, [object]$Snapshot = $null,
          [string]$FindResult = '', [string]$RefReads = '', [string]$Reason = '')
    [void]$script:Results.Add([ordered]@{
        name = $Name; ui_stack = $Stack; result = $Result
        snapshot_ok = if ($null -ne $Snapshot) { $Snapshot.ok } else { $null }
        ref_count = if ($null -ne $Snapshot) { $Snapshot.data.ref_count } else { $null }
        complete = if ($null -ne $Snapshot) { $Snapshot.data.complete } else { $null }
        find_result = $FindResult; ref_reads = $RefReads; reason = $Reason
    })
}

try {
    # 1. classic Notepad on a scratch file
    $scratchFile = Join-Path $env:TEMP ('agent-desktop-live-' + [guid]::NewGuid() + '.txt')
    [IO.File]::WriteAllText($scratchFile, "synthetic dogfood alpha`r`n", $utf8NoBom)
    try {
        $notepad = Start-LiveProcess -FilePath 'notepad.exe' -ArgumentList @($scratchFile)
        Start-Sleep -Seconds 3
        $wid = Find-WindowIdFor 'notepad'
        if (-not $wid) { throw 'no notepad window resolved' }
        $snap = Invoke-Ad -Arguments @('snapshot','--window-id',$wid)
        $snapObj = if ($snap.ok) { $snap } else { $null }
        $ref = Get-FirstRef -WindowId $wid
        $reads = ''
        if ($ref) {
            $g = Invoke-Ad -Arguments @('get',$ref,'--property','bounds')
            $vp = Invoke-Ad -Arguments @('is',$ref,'--property','visible')
            $reads = "get=$($g.ok) is=$($vp.ok)"
        }
        $find = Invoke-Ad -Arguments @('find','--window-id',$wid,'--role','button','--first')
        $findResult = "ok=$($find.ok)"
        New-FindRow -Name 'notepad' -Stack 'win32' -Result 'ran' -Snapshot $snapObj -FindResult $findResult -RefReads $reads
    } catch {
        New-FindRow -Name 'notepad' -Stack 'win32' -Result 'skipped' -Reason $_.Exception.Message
    } finally {
        Stop-Process -Id $notepad.Id -Force -ErrorAction SilentlyContinue
    }

    # 2. Explorer on a scratch directory
    $explorerDir = Join-Path $env:TEMP ('agent-desktop-live-dir-' + [guid]::NewGuid())
    New-Item -ItemType Directory -Path $explorerDir -Force | Out-Null
    foreach ($name in @('alpha.txt','bravo.txt','charlie.txt')) {
        [IO.File]::WriteAllText((Join-Path $explorerDir $name), "synthetic $name`r`n", $utf8NoBom)
    }
    try {
        [void](Start-LiveProcess -FilePath 'explorer.exe' -ArgumentList @($explorerDir))
        Write-Host 'dogfood: waiting 22s for Explorer to reflect the filesystem change (A7-4)'
        Start-Sleep -Seconds 22
        $wid = Find-WindowIdFor 'explorer'
        if (-not $wid) { throw 'no explorer window resolved' }
        $snap = Invoke-Ad -Arguments @('snapshot','--window-id',$wid)
        $snapObj = if ($snap.ok) { $snap } else { $null }
        $ref = Get-FirstRef -WindowId $wid
        $reads = ''
        if ($ref) {
            $g = Invoke-Ad -Arguments @('get',$ref,'--property','bounds')
            $reads = "get=$($g.ok)"
        }
        New-FindRow -Name 'explorer' -Stack 'shell (DirectUI)' -Result 'ran' -Snapshot $snapObj -FindResult '' -RefReads $reads
    } catch {
        New-FindRow -Name 'explorer' -Stack 'shell (DirectUI)' -Result 'skipped' -Reason $_.Exception.Message
    }

    # 3. WinForms fixture: snapshot, live reads, and the A7-3 mutation judgement
    try {
        & (Join-Path $script:ScratchDir 'build-scratch.ps1') | Out-Null
        $winforms = Start-LiveProcess -FilePath (Join-Path $script:ScratchDir 'bin\ScratchForms.exe') -ArgumentList @('--tag','live')
        Start-Sleep -Seconds 4
        $wid = Find-WindowIdFor 'ScratchForms'
        if (-not $wid) { throw 'no winforms window resolved' }
        $snap = Invoke-Ad -Arguments @('snapshot','--window-id',$wid)
        $snapObj = if ($snap.ok) { $snap } else { $null }
        $ref = Get-FirstRef -WindowId $wid
        $reads = ''
        $preValue = $null
        if ($ref) {
            $g = Invoke-Ad -Arguments @('get',$ref,'--property','bounds')
            $vp = Invoke-Ad -Arguments @('is',$ref,'--property','enabled')
            $preValue = Invoke-Ad -Arguments @('get',$ref,'--property','value')
            $reads = "get=$($g.ok) is=$($vp.ok)"
        }
        $find = Invoke-Ad -Arguments @('find','--window-id',$wid,'--role','button','--first')
        $matchCount = if ($find.data.PSObject.Properties.Name -contains 'matches') { @($find.data.matches).Count } elseif ($find.data.PSObject.Properties.Name -contains 'match') { 1 } else { 0 }
        $findResult = "ok=$($find.ok) matches=$matchCount"

        # A7-3-shaped judgement, scoped honestly (see the header comment and
        # A17-2): swap the fixture's list contents, then re-resolve the stored
        # ref and compare its LIVE `value` (read off the resolved live element
        # via get_live_value, not the stored RefEntry) against the value
        # captured before the swap. A bare `get --property role` success is not
        # enough - `role` is served from the stored entry and cannot tell a
        # correct resolve from a silently-resolved neighbour; comparing the
        # live value can, because a neighbour would very likely carry different
        # content.
        #
        # The swap is driven with BM_CLICK on btnMutateList, the only mutation
        # path this fixture implements, and is witnessed on the fixture's own
        # status text rather than assumed from the PostMessage return, which is
        # true for a message nothing handles. Without that witness the whole
        # judgement degenerates: with no swap the two value reads are the same
        # read twice and 'stable' is guaranteed whatever the resolver does.
        $hwndVal = $wid.Replace('w-','')
        Add-Type -AssemblyName UIAutomationClient
        Add-Type -AssemblyName UIAutomationTypes
        Add-Type -TypeDefinition 'using System.Runtime.InteropServices; public static class AD { [DllImport("user32.dll")] public static extern bool PostMessage(System.IntPtr hWnd, uint msg, System.IntPtr wp, System.IntPtr lp); }'
        $fixtureRoot = [System.Windows.Automation.AutomationElement]::FromHandle([IntPtr]::new([int64]$hwndVal))
        $statusBefore = Get-FixtureStatusText -Root $fixtureRoot
        $mutateButton = Get-FixtureElement -Root $fixtureRoot -AutomationId 'btnMutateList'
        $posted = $false
        if ($null -ne $mutateButton -and $mutateButton.Current.NativeWindowHandle -ne 0) {
            $posted = [AD]::PostMessage([IntPtr]::new([int64]$mutateButton.Current.NativeWindowHandle), 0x00F5, [IntPtr]::Zero, [IntPtr]::Zero)
        }
        Start-Sleep -Seconds 1
        $statusAfter = Get-FixtureStatusText -Root $fixtureRoot
        $swapObserved = ($posted -and $statusBefore -ne $statusAfter)
        $reresolve = if ($ref) { Invoke-Ad -Arguments @('get',$ref,'--property','role') } else { $null }
        $postValue = if ($ref) { Invoke-Ad -Arguments @('get',$ref,'--property','value') } else { $null }
        $reresult = if ($null -ne $reresolve -and $reresolve.ok) {
            $preOk = ($null -ne $preValue) -and $preValue.ok
            $postOk = ($null -ne $postValue) -and $postValue.ok
            if (-not $swapObserved) {
                'resolved-but-swap-not-observed'
            } elseif ($preOk -and $postOk -and ($preValue.data.value -eq $postValue.data.value)) {
                'resolved-identity-stable'
            } elseif ($preOk -and $postOk) {
                'resolved-identity-drifted'
            } else {
                'resolved-unverified'
            }
        } elseif ($null -ne $reresolve) { $reresolve.error.code } else { 'no-ref' }
        New-FindRow -Name 'winforms' -Stack 'winforms' -Result 'ran' -Snapshot $snapObj -FindResult $findResult -RefReads ($reads + " swap-witness=$statusBefore->$statusAfter re-resolve-after-swap=$reresult")
    } catch {
        New-FindRow -Name 'winforms' -Stack 'winforms' -Result 'skipped' -Reason $_.Exception.Message
    }

    # 4. WPF fixture: snapshot + live reads
    $wpfScript = Join-Path $script:ScratchDir 'ScratchWpf.ps1'
    $wpf = $null
    try {
        if (-not (Test-Path -LiteralPath $wpfScript)) { throw 'no ScratchWpf.ps1' }
        $wpf = Start-LiveProcess -FilePath 'powershell.exe' -ArgumentList @('-NoProfile','-ExecutionPolicy','Bypass','-File',$wpfScript,'-Tag','live','-TimeoutSeconds','120')
        $deadline = (Get-Date).AddSeconds(20)
        while ((Get-Date) -lt $deadline) {
            $wpf.Refresh()
            if ($wpf.HasExited) { break }
            if ($wpf.MainWindowHandle -ne [IntPtr]::Zero) { break }
            Start-Sleep -Milliseconds 500
        }
        if ($wpf.HasExited -or $wpf.MainWindowHandle -eq [IntPtr]::Zero) { throw 'the WPF fixture never presented a window' }
        Start-Sleep -Seconds 2
        $wid = 'w-' + $wpf.MainWindowHandle.ToString()
        $snap = Invoke-Ad -Arguments @('snapshot','--window-id',$wid)
        $snapObj = if ($snap.ok) { $snap } else { $null }
        New-FindRow -Name 'wpf' -Stack 'wpf' -Result 'ran' -Snapshot $snapObj
    } catch {
        New-FindRow -Name 'wpf' -Stack 'wpf' -Result 'skipped' -Reason $_.Exception.Message
    } finally {
        if ($null -ne $wpf) { Stop-Process -Id $wpf.Id -Force -ErrorAction SilentlyContinue }
    }

    # 5. Obsidian (Chromium/Electron), shape-only, if present
    $obsidianExe = Join-Path $env:LOCALAPPDATA 'Programs\Obsidian\Obsidian.exe'
    try {
        if (-not (Test-Path -LiteralPath $obsidianExe)) { throw 'Obsidian is not installed' }
        $obs = Start-LiveProcess -FilePath $obsidianExe
        Start-Sleep -Seconds 12
        $wid = $null
        for ($i = 0; $i -lt 14 -and -not $wid; $i++) {
            $wid = Find-WindowIdFor 'Obsidian'
            if (-not $wid) { Start-Sleep -Seconds 2 }
        }
        if (-not $wid) { throw 'no obsidian window resolved' }
        $snapText = (& $script:Binary snapshot --window-id $wid --timeout-ms 15000 2>$null | Out-String)
        $snapObj = try { $snapText | ConvertFrom-Json } catch { $null }
        # Attempt ref re-resolution on a few sampled refs; report the honest
        # STALE_REF rate over counts only - never element content.
        $json = $snapText
        $refs = @([regex]::Matches($json, '"ref(?:_id)?"\s*:\s*"(@[A-Za-z0-9_:-]+)"') | ForEach-Object { $_.Groups[1].Value } | Select-Object -Unique -First 6)
        $stale = 0; $ok = 0
        foreach ($r in $refs) {
            $g = Invoke-Ad -Arguments @('get', $r, '--property', 'role')
            if ($null -ne $g -and $g.PSObject.Properties.Name -contains 'ok') {
                if ($g.ok) { $ok++ }
                else {
                    $code = if ($g.PSObject.Properties.Name -contains 'error') { $g.error.code } else { '' }
                    if ($code -eq 'STALE_REF') { $stale++ }
                }
            } else { $stale++ }
        }
        $judgement = "refs_sampled=$(@($refs).Count) stale=$stale resolved=$ok"
        New-FindRow -Name 'obsidian' -Stack 'chromium/electron' -Result 'ran' -Snapshot $snapObj -RefReads $judgement
    } catch {
        New-FindRow -Name 'obsidian' -Stack 'chromium/electron' -Result 'skipped' -Reason $_.Exception.Message
    }
} finally {
    foreach ($pid_ in $script:LaunchedPids) {
        $proc = Get-Process -Id $pid_ -ErrorAction SilentlyContinue
        if ($proc) { Stop-Process -Id $pid_ -Force -ErrorAction SilentlyContinue }
    }
    Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue | ForEach-Object { Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue }
}

$summaryPath = Join-Path $script:OutDir 'live-dogfood-run.json'
$summaryJson = ConvertTo-Json -InputObject ([ordered]@{
    generated = (Get-Date).ToString('o')
    binary = Split-Path -Leaf $script:Binary
    targets = $script:Results
}) -Depth 10
$redacted = Protect-ProbeText -Text $summaryJson
[IO.File]::WriteAllText($summaryPath, $redacted, $utf8NoBom)
if (-not (Test-CaptureRedaction -Path $summaryPath)) { throw "redaction residue in $summaryPath" }
Write-Output "wrote $summaryPath"
$script:Results | ForEach-Object { Write-Output ("  " + $_.name + ": " + $_.result + " ref=" + $_.ref_count) }
$measuredTargets = @($script:Results | Where-Object { $_.result -eq 'ran' -and $_.snapshot_ok -eq $true })
if ($measuredTargets.Count -eq 0) {
    Write-Output 'no target produced a snapshot, so this run asserted nothing about the live loop'
    exit 1
}
exit 0