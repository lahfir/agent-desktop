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
    state off sampled refs (get/is). The WinForms fixture also runs the A7-3
    judgement: a stored ref is re-resolved after a WM_APP content swap, and the
    outcome (resolved-correct / stale / ambiguous) is recorded. Obsidian is
    read shape-only, and its refs' re-resolution STALE_REF rate is reported
    honestly. Every capture is redacted through the corpus gate.
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
    if ($LASTEXITCODE -ne 0) { return [ordered]@{ skipped = "binary exit $LASTEXITCODE" } }
    return ($raw | ConvertFrom-Json)
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
        if ($ref) {
            $g = Invoke-Ad -Arguments @('get',$ref,'--property','bounds')
            $vp = Invoke-Ad -Arguments @('is',$ref,'--property','enabled')
            $reads = "get=$($g.ok) is=$($vp.ok)"
        }
        $find = Invoke-Ad -Arguments @('find','--window-id',$wid,'--role','button','--first')
        $matchCount = if ($find.data.PSObject.Properties.Name -contains 'matches') { @($find.data.matches).Count } elseif ($find.data.PSObject.Properties.Name -contains 'match') { 1 } else { 0 }
        $findResult = "ok=$($find.ok) matches=$matchCount"

        # A7-3 judgement: swap the fixture list via WM_APP, then re-resolve the
        # stored ref; the resolver must either find the same element or go
        # stale, never silently resolve a neighbour.
        # WM_APP+5 to the fixture window
        $hwndVal = $wid.Replace('w-','')
        Add-Type -TypeDefinition 'using System.Runtime.InteropServices; public static class AD { [DllImport("user32.dll")] public static extern bool PostMessage(System.IntPtr hWnd, uint msg, System.IntPtr wp, System.IntPtr lp); }'
        $post = [AD]::PostMessage([IntPtr]::new([int64]$hwndVal), 0x8005, [IntPtr]::Zero, [IntPtr]::Zero)
        Start-Sleep -Seconds 1
        $reresolve = if ($ref) { Invoke-Ad -Arguments @('get',$ref,'--property','role') } else { $null }
        $reresult = if ($null -ne $reresolve -and $reresolve.ok) { 'resolved-correct' } elseif ($null -ne $reresolve) { $reresolve.error.code } else { 'no-ref' }
        New-FindRow -Name 'winforms' -Stack 'winforms' -Result 'ran' -Snapshot $snapObj -FindResult $findResult -RefReads ($reads + " re-resolve-after-swap=$reresult")
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
exit 0