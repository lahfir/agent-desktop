#Requires -Version 5.1
<#
.SYNOPSIS
    Hot-path signal-capture cost baseline (A15-13/A18-7/A20-6 methodology).

.DESCRIPTION
    Measures the single-pass cached capture (R2's shipped composition: one
    ToolHelp process snapshot + one EnumWindows pass, pid-token reads cached),
    the same pass filtered to one process, the naive two-walk composition it
    replaces, the same cached pass with the mid-walk re-walk forced to its
    maximum attempt count, and the menu predicate's two sources measured
    separately. Min-of-seven after a discarded warm-up (n=8 samples, first
    discarded), min reported with median and max beside it. Writes
    signals-cost-{Label}.json. Absolute milliseconds are environment-sensitive;
    CI asserts relative shape only (min<=median<=max, n=7, warmup_discarded).
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'ci')][string]$Label = 'devbox'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

. (Join-Path (Split-Path -Parent $PSCommandPath) '..\common.ps1')
. (Join-Path (Split-Path -Parent $PSCommandPath) 'native.ps1')
Initialize-ProbeRedaction
Initialize-Signals23Native
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$script:ProbeDir = Split-Path -Parent $PSCommandPath
$script:CaptureDir = Join-Path $script:ProbeDir 'captures'
if (-not (Test-Path -LiteralPath $script:CaptureDir)) {
    New-Item -ItemType Directory -Path $script:CaptureDir -Force | Out-Null
}
$script:Spawned = New-Object System.Collections.ArrayList

function Write-CostCapture {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Content
    )
    $redacted = Protect-ProbeText -Text $Content
    $path = Join-Path $script:CaptureDir $Name
    $utf8NoBom = New-Object System.Text.UTF8Encoding $false
    [IO.File]::WriteAllText($path, $redacted, $utf8NoBom)
    $normalized = Get-NormalizedCapture -Text $redacted
    [IO.File]::WriteAllText(($path + '.normalized'), $normalized, $utf8NoBom)
    if (-not (Test-CaptureRedaction -Path $path)) {
        throw "redaction residue in $path"
    }
    return $path
}

function Register-SpawnedPid {
    param([int]$ProcessId)
    if ($ProcessId -gt 0) { [void]$script:Spawned.Add($ProcessId) }
}

function Stop-AllSpawned {
    foreach ($id in @($script:Spawned)) {
        try { Stop-ScratchProcess -ProcessId $id } catch { }
    }
    $script:Spawned.Clear()
}

# Copied verbatim from 21-system-lifecycle/measure-cost.ps1 (A15-13 methodology):
# n=8 samples, first discarded as warm-up, min/median/max over the remaining 7.
function Summarize-Samples {
    param([object[]]$Samples)
    $sorted = @($Samples | Sort-Object)
    $used = @($sorted | Select-Object -Skip 1)
    $medianIdx = [int][Math]::Floor($used.Count / 2)
    return [ordered]@{
        samples_ms       = @($Samples)
        min_ms           = ($used | Measure-Object -Minimum).Minimum
        median_ms        = ($used | Sort-Object)[$medianIdx]
        max_ms           = ($used | Measure-Object -Maximum).Maximum
        n                = $used.Count
        warmup_discarded = $true
    }
}

$script:MenuFamilyCondition = New-Object System.Windows.Automation.OrCondition(@(
        (New-Object System.Windows.Automation.PropertyCondition(
                [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
                [System.Windows.Automation.ControlType]::Menu)),
        (New-Object System.Windows.Automation.PropertyCondition(
                [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
                [System.Windows.Automation.ControlType]::MenuBar)),
        (New-Object System.Windows.Automation.PropertyCondition(
                [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
                [System.Windows.Automation.ControlType]::MenuItem))
    ))

function Measure-UiaMenuScanCost {
    param([Parameter(Mandatory = $true)][int]$ProcessId)
    $root = [System.Windows.Automation.AutomationElement]::RootElement
    $cond = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ProcessIdProperty, $ProcessId)
    $children = $root.FindAll([System.Windows.Automation.TreeScope]::Children, $cond)
    foreach ($c in $children) {
        try { [void]$c.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $script:MenuFamilyCondition) } catch { }
    }
}

$singlePassSamples = New-Object System.Collections.ArrayList
$singlePassFilteredSamples = New-Object System.Collections.ArrayList
$twoWalkSamples = New-Object System.Collections.ArrayList
$rewalkMaxSamples = New-Object System.Collections.ArrayList
$menuClassicSamples = New-Object System.Collections.ArrayList
$menuUiaSamples = New-Object System.Collections.ArrayList

try {
    Initialize-ProbeNative

    $note = Start-Process notepad.exe -PassThru
    Register-SpawnedPid -ProcessId $note.Id
    Start-Sleep -Milliseconds 800
    $notePid = $note.Id

    for ($i = 0; $i -lt 8; $i++) {
        $windowCount = 0
        $sw = [Diagnostics.Stopwatch]::StartNew()
        [void][AgentDesktopProbe.A23.Signals23]::SingleWalkCachedPass([ref]$windowCount)
        $sw.Stop()
        [void]$singlePassSamples.Add([Math]::Round($sw.Elapsed.TotalMilliseconds, 4))
    }

    for ($i = 0; $i -lt 8; $i++) {
        $windowCount = 0
        $sw = [Diagnostics.Stopwatch]::StartNew()
        $records = [AgentDesktopProbe.A23.Signals23]::SingleWalkCachedPass([ref]$windowCount)
        $filtered = 0
        foreach ($r in $records) { if ($r.PidNonZero) { $filtered++ } }
        $sw.Stop()
        [void]$singlePassFilteredSamples.Add([Math]::Round($sw.Elapsed.TotalMilliseconds, 4))
    }

    # The naive composition KTD2 replaces: list_windows_live's own EnumWindows
    # (AgentFacingPass, one token_for_pid open per window - no cross-check
    # against a ToolHelp snapshot) plus list_apps_live's *separate* EnumWindows
    # inside owning_processes() (SingleWalkCachedPass models that walk plus
    # its ToolHelp snapshot and cached per-pid token reads). Two full walks
    # against the same desktop, exactly as app_ops.rs:88-97 performs today.
    for ($i = 0; $i -lt 8; $i++) {
        $wc1 = 0; $wc2 = 0
        $sw = [Diagnostics.Stopwatch]::StartNew()
        [void][AgentDesktopProbe.A23.Signals23]::AgentFacingPass([ref]$wc1)
        [void][AgentDesktopProbe.A23.Signals23]::SingleWalkCachedPass([ref]$wc2)
        $sw.Stop()
        [void]$twoWalkSamples.Add([Math]::Round($sw.Elapsed.TotalMilliseconds, 4))
    }

    for ($i = 0; $i -lt 8; $i++) {
        $sw = [Diagnostics.Stopwatch]::StartNew()
        for ($attempt = 0; $attempt -lt 5; $attempt++) {
            $wc = 0
            [void][AgentDesktopProbe.A23.Signals23]::SingleWalkCachedPass([ref]$wc)
        }
        $sw.Stop()
        [void]$rewalkMaxSamples.Add([Math]::Round($sw.Elapsed.TotalMilliseconds, 4))
    }

    for ($i = 0; $i -lt 8; $i++) {
        $threadsRead = 0
        $threadsTotal = 0
        $sw = [Diagnostics.Stopwatch]::StartNew()
        [void][AgentDesktopProbe.A23.Signals23]::AnyThreadInMenuMode($notePid, [ref]$threadsRead, [ref]$threadsTotal)
        $sw.Stop()
        [void]$menuClassicSamples.Add([Math]::Round($sw.Elapsed.TotalMilliseconds, 4))
    }

    for ($i = 0; $i -lt 8; $i++) {
        $sw = [Diagnostics.Stopwatch]::StartNew()
        Measure-UiaMenuScanCost -ProcessId $notePid
        $sw.Stop()
        [void]$menuUiaSamples.Add([Math]::Round($sw.Elapsed.TotalMilliseconds, 4))
    }

    try { Stop-ScratchProcess -ProcessId $note.Id } catch { }

    $cost = [ordered]@{
        probe                        = '23-signals-menus'
        question                     = 'hot-path cost for the single-pass cached capture, the same pass filtered to one process, the naive two-walk composition it replaces, the re-walk forced to its maximum attempt count, and the two menu-predicate sources scoped to one process (min-of-seven, warm-up discarded per A15-13)'
        methodology_cites            = @('A15-13', 'A18-7', 'A20-6')
        single_pass_cached_unfiltered = (Summarize-Samples -Samples @($singlePassSamples))
        single_pass_cached_filtered_one_process = (Summarize-Samples -Samples @($singlePassFilteredSamples))
        two_walk_composition_naive   = (Summarize-Samples -Samples @($twoWalkSamples))
        rewalk_forced_to_max_attempts = (Summarize-Samples -Samples @($rewalkMaxSamples))
        menu_predicate_classic_flags = (Summarize-Samples -Samples @($menuClassicSamples))
        menu_predicate_uia_scan      = (Summarize-Samples -Samples @($menuUiaSamples))
        note                         = 'absolute ms are environment-sensitive; CI asserts min<=median<=max, n=7, warmup_discarded only. filtering to one process does not shrink the walk itself (the enumeration still visits every agent-facing window before any pid comparison), so single_pass_cached_unfiltered and _filtered_one_process are expected to read close to each other'
    }
    $path = Write-CostCapture -Name "signals-cost-$Label.json" -Content (ConvertTo-Json -InputObject $cost -Depth 12)
    Write-Host "wrote $path"
} finally {
    Stop-AllSpawned
}
