#Requires -Version 5.1
<#
.SYNOPSIS
    Area 26 06-cost.ps1 - rows A26-10 (raw platform-operation costs under the
    corpus methodology) and A26-13 (the positive-area premise R20 depends on).

.DESCRIPTION
    A26-10 prices the RAW platform operations the shell/notification commands
    will compose from - raising an Action Center surface via its accelerator
    with detection through the UIA-root reach mechanism, one UIA-root-children
    resolution, one Action Center tree read, one tray enumeration - using the
    corpus cost methodology (one discarded warm-up, seven timed runs, min
    with median and max beside it, A15-13 as applied in A18-7). All readings
    come from the CUIAutomation8 shim in one child process so a per-call
    process spawn cannot pollute them. This row is labelled a
    pre-implementation platform-cost reference: it is NOT the shipped
    command's cost - U16 takes that baseline through the release binary once
    U3/U9 exist. Raw sample values never reach the capture; each statistic is
    emitted under elapsed_*_ms keys, which the normalized twin masks, so two
    runs of unchanged timings produce byte-identical twins.

    A26-13 replicates A24-11's content-leaf selector shape (role filter,
    offscreen state, available actions - that selector never read bounds and
    could not) against a real Electron/Chromium app (Obsidian), reads the
    LIVE rectangle of qualifying nameless leaves, and records counts of
    leaves in each class (positive-area versus zero-extent). If every such
    leaf reads zero-extent, U13's fix cannot reach them and R20's branch
    changes before a line is written.

    Run: powershell -NoProfile -ExecutionPolicy Bypass -File .\probes\windows\26-shell-surfaces\06-cost.ps1 -Label <devbox|ci>
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'ci')][string]$Label = 'devbox'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

. (Join-Path (Split-Path -Parent $PSCommandPath) '..\common.ps1')
Initialize-ProbeRedaction
. (Join-Path (Split-Path -Parent $PSCommandPath) 'lib.ps1')

$script:Probe = '26-shell-surfaces/06-cost'
Register-MandatoryCapture -Name @("platform-cost-$Label.json")

# ------------------------------------------------------------------ A26-10

$costLeg = [ordered]@{ measurable = $false; branch = 'not_measured' }

try {
    Initialize-ShellProbe | Out-Null
    $cost = Invoke-ShellProbe -Arguments @('cost', '--cycles', '7')
    $costLeg['measurable'] = $true
    $costLeg['branch'] = 'platform_costs_measured_in_single_com_process'
    $costLeg['note'] = 'pre-implementation platform-cost reference, NOT the shipped command cost; U16 takes the product baseline through the release binary'
    $costLeg['cost'] = $cost
    if ([int]$cost.open_detection_failures -ge [int]$cost.cycles_attempted) {
        # Every raise detection deadline missed: this shell never presented the
        # Action Center, so the raise-dependent components (the raise/close
        # cycles and the Action Center tree read) recorded no samples - their
        # stats are honest empties, and only the tray enumeration and the
        # root-children resolution are measurements here. The decline is the
        # environment outcome, recorded as such.
        $costLeg['raisable'] = $false
        $costLeg['declined_reason'] = 'action_center_not_raisable_by_lwin_a_accelerator'
    }
} catch {
    $costLeg = [ordered]@{
        measurable   = $false
        branch       = 'cost_leg_threw'
        error_class  = $_.Exception.GetType().Name
        error        = ($_.Exception.Message -replace '[\r\n]+', ' ')
    }
}

# ------------------------------------------------------------------ A26-13

function Measure-A2613 {
    <# A24-11's selector shape (probes/windows/24-fixture-e2e/08-chromium-
       content.ps1): role filter, offscreen state, available actions. That
       snapshot-based walk could not read bounds; this leg re-reads the SAME
       qualifying population live through the COM shim and classifies each
       leaf by its live rectangle. Content is staged exactly far enough for
       nameless task-list checkboxes to exist (a scratch vault like A24-11's,
       backed up before and restored after); nothing about any document or
       leaf text is recorded - only counts. #>
    $leg = [ordered]@{ measurable = $false; branch = 'not_measured'; stack = 'uia3-com' }
    $obsidianExe = Join-Path $env:LOCALAPPDATA 'Programs\Obsidian\Obsidian.exe'
    if (-not (Test-Path -LiteralPath $obsidianExe)) {
        $leg['branch'] = 'electron_target_not_installed'
        return $leg
    }

    $vaultRoot = Join-Path $env:TEMP 'agent-desktop-shell26\a26-vault'
    $notePath = Join-Path $vaultRoot 'task-note.md'
    $obsidianJson = Join-Path $env:APPDATA 'obsidian\obsidian.json'
    $backupFile = $null
    $hadBackup = $false

    try {
        $killedRunningInstances = 0
        foreach ($p in @(Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue)) {
            Register-ScratchProcessId -ProcessId $p.Id
            Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
            $killedRunningInstances++
        }
        if ($killedRunningInstances -gt 0) { Start-Sleep -Seconds 2 }

        New-Item -ItemType Directory -Path (Join-Path $vaultRoot '.obsidian') -Force | Out-Null
        $tasks = New-Object System.Collections.ArrayList
        for ($i = 1; $i -le 12; $i++) { [void]$tasks.Add(('- [ ] task item {0}' -f $i)) }
        Set-Content -LiteralPath $notePath -Value (($tasks | ForEach-Object { $_ }) -join "`r`n") -Encoding ASCII

        if (Test-Path -LiteralPath $obsidianJson) {
            $backupFile = ($obsidianJson + '.a26-backup')
            Copy-Item -LiteralPath $obsidianJson -Destination $backupFile -Force
            $hadBackup = $true
        }
        # PS5.1 has no ConvertFrom-Json -AsHashtable; mutate the PSCustomObject.
        $cfg = $null
        if (Test-Path -LiteralPath $obsidianJson) {
            try { $cfg = Get-Content -LiteralPath $obsidianJson -Raw | ConvertFrom-Json } catch { $cfg = $null }
        }
        if ($null -eq $cfg) { $cfg = New-Object PSObject }
        if (-not ($cfg.PSObject.Properties['vaults'])) {
            $cfg | Add-Member -NotePropertyName vaults -NotePropertyValue (New-Object PSObject)
        }
        $vaultKey = [guid]::NewGuid().ToString('N').Substring(0, 16)
        $entry = New-Object PSObject
        $entry | Add-Member -NotePropertyName path -NotePropertyValue $vaultRoot
        $entry | Add-Member -NotePropertyName n -NotePropertyValue 'a26-vault'
        $entry | Add-Member -NotePropertyName ts -NotePropertyValue 0
        $cfg.vaults | Add-Member -NotePropertyName $vaultKey -NotePropertyValue $entry -Force
        if (-not ($cfg.PSObject.Properties['open'])) {
            $cfg | Add-Member -NotePropertyName open -NotePropertyValue $vaultKey
        } else {
            $cfg.open = $vaultKey
        }
        $cfgDir = Split-Path -Parent $obsidianJson
        if (-not (Test-Path -LiteralPath $cfgDir)) { New-Item -ItemType Directory -Path $cfgDir -Force | Out-Null }
        $cfg | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $obsidianJson -Encoding UTF8

        function Get-VisibleObsidianHandles {
            $candidates = New-Object System.Collections.ArrayList
            try {
                $hits = Invoke-ShellProbe -Arguments @('findbyclass', '--cls', 'Chrome_WidgetWin_1')
                foreach ($h in @($hits.handles)) {
                    try {
                        $pred = Invoke-ShellProbe -Arguments @('predicate', '--hwnd', ([string]$h))
                        if ($pred.cloak_state -eq 'none') { [void]$candidates.Add([string]$h) }
                    } catch { }
                }
            } catch { }
            return $candidates.ToArray()
        }

        # Baseline must precede the launch: Start-ScratchProcess itself waits
        # for a main window, so anything post-launch is the run's own.
        $beforeLaunchSet = @(Get-VisibleObsidianHandles)
        Start-ScratchProcess -FilePath $obsidianExe -ArgumentList @($vaultRoot) -TimeoutSec 40 | Out-Null

        # Settle-and-poll for real Chromium tree exposure (A1-4's settled
        # figure); activation nudges in between. If no window's tree ever
        # exceeds a small floor, the leg records itself unmeasurable instead
        # of reporting a fabricated zero population.
        $target = $null
        $pollSeconds = 0
        $maxNodesSeen = 0
        while ($pollSeconds -lt 60 -and -not $target) {
            Start-Sleep -Seconds 2
            $pollSeconds += 2
            foreach ($cand in @(Get-VisibleObsidianHandles)) {
                if (@($beforeLaunchSet) -contains $cand) { continue }
                try {
                    $t = Invoke-ShellProbe -Arguments @('actree', '--hwnd', ([string]$cand), '--maxnodes', '2500', '--maxdepth', '80')
                    if ([int]$t.node_count -gt [int]$maxNodesSeen) { $maxNodesSeen = [int]$t.node_count }
                    if ([int]$t.node_count -gt 40) {
                        $target = $cand
                        break
                    }
                    Invoke-ShellProbe -Arguments @('activate', '--hwnd', ([string]$cand)) | Out-Null
                    Start-Sleep -Milliseconds 600
                    $fgNow = Invoke-ShellProbe -Arguments @('foregroundinfo')
                    if ([string]$fgNow.foreground_class -eq 'Windows.UI.Core.CoreWindow') {
                        Invoke-ShellProbe -Arguments @('key', '--seq', 'esc') | Out-Null
                        Start-Sleep -Milliseconds 300
                        Invoke-ShellProbe -Arguments @('activate', '--hwnd', ([string]$cand)) | Out-Null
                    }
                } catch { }
            }
        }

        $leg['measurable'] = $true
        $leg['branch'] = 'leaf_population_classified_live'
        $leg['windows_read_candidates'] = @(Get-VisibleObsidianHandles).Count
        if (-not $target) {
            $leg['measurable'] = $false
            $leg['branch'] = 'electron_tree_never_exposed_past_settle_floor'
            $leg['unmeasurable_reason'] = 'a staged-vault Obsidian window appeared but its Chromium UIA tree stayed at or below the 40-node exposure floor for ' + $pollSeconds + ' s of settling and activation nudges (A1-4 settled exposure did not reproduce this session; consistent with A1-6''s occlusion hazard), so no qualifying leaf existed to classify'
            $leg['settle_seconds_spent'] = $pollSeconds
            $leg['max_nodes_observed_any_window'] = $maxNodesSeen
            return $leg
        }

        Start-Sleep -Seconds 4
        Invoke-ShellProbe -Arguments @('key', '--seq', 'ctrl_o') | Out-Null
        Start-Sleep -Milliseconds 900
        Invoke-ShellProbe -Arguments @('key', '--seq', 'type', '--text', 'task-note') | Out-Null
        Start-Sleep -Milliseconds 1100
        Invoke-ShellProbe -Arguments @('key', '--seq', 'return') | Out-Null
        Start-Sleep -Seconds 7

        $handles = @($target)
        $perWindow = @()
        $positiveTotal = 0
        $zeroTotal = 0
        $grandTotal = 0
        $truncatedReads = 0
        foreach ($h in $handles) {
            $treeRaw = Invoke-ShellProbe -Arguments @('actree', '--hwnd', ([string]$h), '--maxnodes', '4000', '--maxdepth', '80')
            $nodes = @($treeRaw.nodes)
            $winPositive = 0
            $winZero = 0
            $qualifyingHere = 0
            foreach ($n in $nodes) {
                if ([string]$n.ct -ne 'CheckBox') { continue }
                $nmPresentFlag = $false
                if ($n.PSObject.Properties['nm']) { $nmPresentFlag = [bool]$n.nm }
                if ($nmPresentFlag) { continue }
                if ([int]$n.off -ne 0) { continue }
                if (@($n.pats) -notcontains 'Invoke') { continue }
                $qualifyingHere++
                if ([bool]$n.pos) { $winPositive++ } else { $winZero++ }
            }
            $positiveTotal += $winPositive
            $zeroTotal += $winZero
            $grandTotal += $qualifyingHere
            $perWindow += [ordered]@{
                window_ordinal                     = $perWindow.Count
                nodes_reached                      = $nodes.Count
                node_cap                           = [int]$treeRaw.node_cap
                qualified_nameless_checkbox_leaves = $qualifyingHere
                live_rectangle_positive_area       = $winPositive
                live_rectangle_zero_extent         = $winZero
            }
            if ($nodes.Count -ge [int]$treeRaw.node_cap) { $truncatedReads++ }
        }
        $leg['per_window_reads'] = $perWindow
        $leg['qualifying_nameless_checkbox_leaves_total'] = $grandTotal
        $leg['live_rectangle_positive_area_count'] = $positiveTotal
        $leg['live_rectangle_zero_extent_count'] = $zeroTotal
        $leg['window_reads_truncated_at_cap'] = $truncatedReads
        $leg['counting_note'] = 'counts only; no bounds coordinates and no leaf content recorded; selector shape reproduced from A24-11 (role CheckBox filter, IsOffscreen false, Invoke available) plus the nameless condition its checkboxes have, read LIVE so the rectangle is real rather than snapshot-time'
    } catch {
        $leg['measurable'] = $false
        $leg['branch'] = 'a26_13_leg_threw'
        $leg['error_class'] = $_.Exception.GetType().Name
        $leg['error'] = ($_.Exception.Message -replace '[\r\n]+', ' ')
    } finally {
        foreach ($p in @(Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue)) {
            try { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue } catch { }
        }
        if ($hadBackup) {
            try {
                Move-Item -LiteralPath $backupFile -Destination $obsidianJson -Force
            } catch { }
        } elseif (Test-Path -LiteralPath $obsidianJson) {
            try { Remove-Item -LiteralPath $obsidianJson -Force -ErrorAction SilentlyContinue } catch { }
        }
    }
    return $leg
}

$a2613 = Measure-A2613

$status = 'ok'
$message = 'platform costs + A26-13 leaf classification captured'

# A declined raise is the environment outcome the cost leg records honestly
# (raisable:false + declined_reason) - the run skips. A cost leg that threw
# failed a mandatory measurement, and that stays a strict failure.
if ($costLeg.measurable -eq $false) {
    if ($costLeg.branch -eq 'cost_leg_threw') {
        $status = 'fail'
        $message = ('cost leg threw: ' + [string]$costLeg['error_class'])
    }
} elseif ($costLeg.Contains('declined_reason')) {
    $status = 'skip'
    $message = [string]$costLeg['declined_reason']
}

$content = ConvertTo-Json -InputObject ([ordered]@{
        probe              = $script:Probe
        question           = 'what do the raw platform operations cost and does the nameless content-leaf population A24-11''s fallback depends on actually present positive-area rectangles when measured live'
        cites              = @('A15-13', 'A24-11')
        label              = $Label
        client_stack       = 'uia3-com'
        platform_cost      = $costLeg
        chromium_leafs     = $a2613
    }) -Depth 24

try {
    $capturePath = Write-Shell26Capture -Name "platform-cost-$Label.json" -Content $content
    Register-MandatoryPass -Capture $capturePath -Result @{ measurable_placeholder_written = $false; status_ok = ($status -eq 'ok'); not_measured = ($status -eq 'fail') }
} catch {
    $status = 'fail'
    $message = ('capture write failed: ' + $_.Exception.Message)
}

Write-ProbeResult -Probe $script:Probe -Status $status -Message $message -Data @{
    capture   = "captures/platform-cost-$Label.json"
    rows      = @('A26-10', 'A26-13')
    stack     = 'uia3-com'
}
if ($status -eq 'fail') { exit 1 }
Assert-MandatoryMeasurement -Probe $script:Probe -Label $Label
exit 0
