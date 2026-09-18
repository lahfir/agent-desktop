#Requires -Version 5.1
<#
.SYNOPSIS
    Explorer Items View below-fold realization probe (area 24, sub-phase 2.12).

.DESCRIPTION
    phases.md's Explorer residual (2.7 dogfood J3): the shipped ancestor-scroll
    ladder (A19-7, `crates/windows/src/actions/scroll_ladder.rs`) was never
    forced through a below-fold geometry change on Explorer, because the
    realized option set at 40 synthetic files had no off-screen row (24 of 40
    visible). §2.12 owns staging a denser or unrealized surface that forces
    it.

    This probe stages exactly that: a scratch folder of N zero-byte synthetic
    files (N = 40, 200, 1000), a folder window opened on it and pinned to a
    deliberately small height, forced into Details (single-column) view with
    a real Ctrl+Shift+6 keystroke (SendInput, not PostMessage — Engineering
    Invariant #5) so "below fold" means vertical position and nothing else.
    For each N it reports: files actually on disk, how many UIA children the
    items container realizes (`ControlView` direct children of the first
    List/DataGrid container found whose first child is a ListItem/DataItem),
    how many of those intersect the container's own `BoundingRectangle`
    (unclipped provider rects — A18-2 — so a realized-but-invisible row still
    carries positive-area bounds), and how many are realized with positive
    area entirely below the container's visible bottom edge —
    `realized_below_fold`. That count is the number that matters: greater
    than zero is the exact condition `ladder_judged_for` needs a direction to
    walk toward. It also reports whether the container exposes ScrollPattern
    and whether any realized item exposes ScrollItemPattern, since the
    product ladder branches on precisely that (absent/no-op ScrollItemPattern
    falls through to the ancestor ladder).

    A geometry self-check (`single_column_layout_observed`, counting distinct
    left-edge x positions among realized children) is reported alongside the
    view-switch attempt so a reader does not have to trust the keystroke
    blindly — a true single-column Details layout puts every realized child
    at the same left edge.

    Corpus safety: only counts, booleans and small derived integers ever
    reach the capture — no raw screen coordinates, window titles, file
    paths, pids, machine names, user names or message text. Every window
    this probe opens is closed via `Shell.Application` on every exit path
    (explorer.exe itself is never terminated — killing the shared shell host
    would take out the desktop); every scratch folder is removed in a
    `finally`.

    Run: powershell -NoProfile -ExecutionPolicy Bypass -File .\probes\windows\24-fixture-e2e\04-explorer-below-fold.ps1 -Label <devbox|ci>
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'ci')][string]$Label = 'devbox',
    [int[]]$FileCounts = @(40, 200, 1000)
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

. (Join-Path (Split-Path -Parent $PSCommandPath) '..\common.ps1')
. (Join-Path (Split-Path -Parent $PSCommandPath) '..\23-signals-menus\native.ps1')
Initialize-ProbeRedaction
Initialize-ProbeNative
Initialize-Signals23Native
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$script:Probe = '24-fixture-e2e-04-explorer-below-fold'
$script:ProbeDir = Split-Path -Parent $PSCommandPath
$script:CaptureDir = Join-Path $script:ProbeDir 'captures'
if (-not (Test-Path -LiteralPath $script:CaptureDir)) {
    New-Item -ItemType Directory -Path $script:CaptureDir -Force | Out-Null
}
$script:OpenedHandles = New-Object System.Collections.Generic.List[int]
$script:ScratchDirs = New-Object System.Collections.Generic.List[string]

$AE = [System.Windows.Automation.AutomationElement]
$CT = [System.Windows.Automation.ControlType]
$TS = [System.Windows.Automation.TreeScope]
$ControlWalker = [System.Windows.Automation.TreeWalker]::ControlViewWalker

# Deliberately small: enough room for the address bar/ribbon/column headers
# plus a handful of Details rows, nowhere near enough to hold 40+ rows.
$script:PlacementX = 20
$script:PlacementY = 20
$script:PlacementWidth = 760
$script:PlacementHeight = 220

Register-MandatoryCapture -Name @("explorer-below-fold-$Label.json")

function Write-A24Capture {
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

# ------------------------------------------------------------ scratch folder

function New-SyntheticFolder {
    param([Parameter(Mandatory = $true)][int]$Count)
    $dir = Join-Path $env:TEMP ('agent-desktop-probe24-explorer-' + $Count + '-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $dir -Force | Out-Null
    $script:ScratchDirs.Add($dir)
    $utf8NoBom = New-Object System.Text.UTF8Encoding $false
    for ($i = 1; $i -le $Count; $i++) {
        $leaf = 'row-' + $i.ToString('0000') + '.txt'
        [IO.File]::WriteAllText((Join-Path $dir $leaf), '', $utf8NoBom)
    }
    return $dir
}

function Remove-SyntheticFolder {
    param([Parameter(Mandatory = $true)][string]$Dir)
    if (Test-Path -LiteralPath $Dir) {
        try { Remove-Item -LiteralPath $Dir -Recurse -Force -ErrorAction Stop } catch { }
    }
    [void]$script:ScratchDirs.Remove($Dir)
}

# ------------------------------------------------------------ window lifecycle

function Get-CabinetHandles {
    $handles = New-Object System.Collections.Generic.HashSet[int]
    try {
        $c = $ControlWalker.GetFirstChild($AE::RootElement)
        while ($null -ne $c) {
            try {
                if ($c.Current.ClassName -eq 'CabinetWClass') { [void]$handles.Add([int]$c.Current.NativeWindowHandle) }
            } catch { }
            $c = $ControlWalker.GetNextSibling($c)
        }
    } catch { }
    return $handles
}

function Wait-NewCabinetElement {
    param([Parameter(Mandatory = $true)]$Baseline, [int]$TimeoutSec = 30)
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ((Get-Date) -lt $deadline) {
        try {
            $c = $ControlWalker.GetFirstChild($AE::RootElement)
            while ($null -ne $c) {
                try {
                    if ($c.Current.ClassName -eq 'CabinetWClass' -and -not $Baseline.Contains([int]$c.Current.NativeWindowHandle)) {
                        return $c
                    }
                } catch { }
                $c = $ControlWalker.GetNextSibling($c)
            }
        } catch { }
        Start-Sleep -Milliseconds 300
    }
    return $null
}

function Close-OpenedHandle {
    param([Parameter(Mandatory = $true)][int]$Handle)
    try {
        $shell = New-Object -ComObject Shell.Application
        foreach ($w in @($shell.Windows())) {
            try { if ([int]$w.HWND -eq $Handle) { $w.Quit() } } catch { }
        }
    } catch { }
    [void]$script:OpenedHandles.Remove($Handle)
}

# ------------------------------------------------------------ items container

<#
    BFS over ControlView (not RawView) for the first List/DataGrid whose
    first realized child is a ListItem/DataItem — Explorer's Items View
    container, whatever the current view mode turns out to be. Bounded so a
    surface with no such container at all cannot turn this into an unbounded
    walk; the cap is far above what any Explorer folder window's own chrome
    (ribbon, nav pane, address bar, status bar) contributes.
#>
function Find-ItemsContainer {
    param(
        [Parameter(Mandatory = $true)]$Root,
        [int]$MaxVisited = 3000
    )
    $queue = New-Object System.Collections.Generic.Queue[object]
    $queue.Enqueue($Root)
    $visited = 0
    $best = $null
    $bestCount = 0
    while ($queue.Count -gt 0 -and $visited -lt $MaxVisited) {
        $node = $queue.Dequeue()
        $visited++
        $cur = $null
        try { $cur = $node.Current } catch { continue }
        if ($null -eq $cur) { continue }
        if ($cur.ControlType -eq $CT::List -or $cur.ControlType -eq $CT::DataGrid) {
            # Details view puts a Header ahead of the rows, so a first-child-only
            # test rejects the very container this probe exists to measure. Scan
            # the child run for an item instead of assuming the item is first.
            $scan = $null
            try { $scan = $ControlWalker.GetFirstChild($node) } catch { }
            $scanned = 0
            $itemCount = 0
            while ($null -ne $scan -and $scanned -lt 4000) {
                $scanned++
                $childType = $null
                try { $childType = $scan.Current.ControlType } catch { }
                if ($childType -eq $CT::ListItem -or $childType -eq $CT::DataItem) { $itemCount++ }
                try { $scan = $ControlWalker.GetNextSibling($scan) } catch { $scan = $null }
            }
            # Returning the first matching list finds the navigation pane, whose
            # entry count is fixed and independent of the staged folder - a
            # reading that looks measurable and answers a different question.
            # Every candidate is scored and the richest one wins instead.
            if ($itemCount -gt 0 -and $itemCount -gt $bestCount) {
                $bestCount = $itemCount
                $best = $node
            }
        }
        $k = $null
        try { $k = $ControlWalker.GetFirstChild($node) } catch { }
        while ($null -ne $k) {
            $queue.Enqueue($k)
            try { $k = $ControlWalker.GetNextSibling($k) } catch { $k = $null }
        }
    }
    return $best
}

function Get-RealizedChildren {
    param([Parameter(Mandatory = $true)]$Container)
    $children = New-Object System.Collections.Generic.List[object]
    $k = $null
    try { $k = $ControlWalker.GetFirstChild($Container) } catch { }
    while ($null -ne $k) {
        $children.Add($k)
        try { $k = $ControlWalker.GetNextSibling($k) } catch { $k = $null }
    }
    return $children
}

function Test-RectIntersect {
    param($A, $B)
    if ($A.IsEmpty -or $B.IsEmpty) { return $false }
    $left = [Math]::Max($A.Left, $B.Left)
    $top = [Math]::Max($A.Top, $B.Top)
    $right = [Math]::Min($A.Right, $B.Right)
    $bottom = [Math]::Min($A.Bottom, $B.Bottom)
    return ($right -gt $left -and $bottom -gt $top)
}

<#
    One geometry+pattern reading over whatever the items container currently
    realizes. Called repeatedly by Measure-ItemsRealization to stabilize
    against a container that is still populating.
#>
function Read-ItemsRealization {
    param([Parameter(Mandatory = $true)]$Container)
    $containerBounds = $null
    try { $containerBounds = $Container.Current.BoundingRectangle } catch { }
    $hasContainerBounds = ($null -ne $containerBounds -and -not $containerBounds.IsEmpty)

    $hasScrollPattern = $false
    $scrollPatternObj = $null
    try { $hasScrollPattern = $Container.TryGetCurrentPattern([System.Windows.Automation.ScrollPattern]::Pattern, [ref]$scrollPatternObj) } catch { }

    $children = Get-RealizedChildren -Container $Container
    $realizedTotal = $children.Count
    $nonEmptyBounds = 0
    $intersecting = 0
    $belowFold = 0
    $partiallyVisible = 0
    $withScrollItemPattern = 0
    $leftXValues = New-Object System.Collections.Generic.HashSet[int]

    foreach ($child in $children) {
        $bounds = $null
        try { $bounds = $child.Current.BoundingRectangle } catch { }
        $nonEmpty = ($null -ne $bounds -and -not $bounds.IsEmpty)
        if ($nonEmpty) {
            $nonEmptyBounds++
            [void]$leftXValues.Add([int][Math]::Round($bounds.Left))
            if ($hasContainerBounds) {
                $intersects = Test-RectIntersect -A $bounds -B $containerBounds
                if ($intersects) {
                    $intersecting++
                    if ($bounds.Bottom -gt $containerBounds.Bottom) { $partiallyVisible++ }
                } elseif ($bounds.Top -ge $containerBounds.Bottom) {
                    $belowFold++
                }
            }
        }
        $sip = $null
        $hasSip = $false
        try { $hasSip = $child.TryGetCurrentPattern([System.Windows.Automation.ScrollItemPattern]::Pattern, [ref]$sip) } catch { }
        if ($hasSip) { $withScrollItemPattern++ }
    }

    return [ordered]@{
        has_container_bounds              = $hasContainerBounds
        has_scroll_pattern                = $hasScrollPattern
        realized_total                    = $realizedTotal
        realized_nonempty_bounds          = $nonEmptyBounds
        realized_intersecting_visible_rect = $intersecting
        realized_below_fold               = $belowFold
        realized_partially_visible         = $partiallyVisible
        items_with_scroll_item_pattern    = $withScrollItemPattern
        any_item_has_scroll_item_pattern  = ($withScrollItemPattern -gt 0)
        distinct_left_x_count             = $leftXValues.Count
        single_column_layout_observed     = ($nonEmptyBounds -gt 0 -and $leftXValues.Count -eq 1)
    }
}

<#
    Stabilizes the reading against a container still populating: reads up to
    5 times, 300 ms apart, and stops as soon as two consecutive reads agree
    on realized_total. The last reading is what gets reported either way, so
    a container that never stabilizes still yields a real measurement rather
    than nothing.
#>
function Measure-ItemsRealization {
    param([Parameter(Mandatory = $true)]$Container)
    $previous = $null
    $reading = $null
    $attempts = 0
    for ($i = 0; $i -lt 5; $i++) {
        $reading = Read-ItemsRealization -Container $Container
        $attempts++
        if ($null -ne $previous -and $previous -eq $reading.realized_total) { break }
        $previous = $reading.realized_total
        Start-Sleep -Milliseconds 300
    }
    $reading.stabilization_reads = $attempts
    return $reading
}

# ------------------------------------------------------------ per-N leg

function Measure-OneFileCount {
    param([Parameter(Mandatory = $true)][int]$Count)

    $dir = $null
    $handle = 0
    try {
        $dir = New-SyntheticFolder -Count $Count
        $filesOnDisk = @(Get-ChildItem -LiteralPath $dir -File -ErrorAction SilentlyContinue).Count

        $baseline = Get-CabinetHandles
        $launcher = Start-Process -FilePath (Join-Path $env:WINDIR 'explorer.exe') -ArgumentList @($dir) -PassThru
        Register-ScratchProcessId -ProcessId $launcher.Id

        $element = Wait-NewCabinetElement -Baseline $baseline -TimeoutSec 30
        if ($null -eq $element) {
            return [ordered]@{
                n_requested   = $Count
                files_on_disk = $filesOnDisk
                measurable    = $false
                branch        = 'explorer_window_not_found'
            }
        }
        $handle = [int]$element.Current.NativeWindowHandle
        $script:OpenedHandles.Add($handle)
        $hwnd = [IntPtr]$handle

        Show-WindowNoActivate -WindowHandle $hwnd -X $script:PlacementX -Y $script:PlacementY -Width $script:PlacementWidth -Height $script:PlacementHeight
        Start-Sleep -Milliseconds 1000

        $activated = [AgentDesktopProbe.A23.Signals23]::ActivateOnce($hwnd)
        Start-Sleep -Milliseconds 300
        $foregroundConfirmed = [AgentDesktopProbe.A23.Signals23]::ForegroundIs($hwnd)
        # Ctrl+Shift+6: force Details (single-column) view so "below fold"
        # is purely a vertical-position question, never a column-wrap one.
        [AgentDesktopProbe.A23.Signals23]::KeyChord(@(0x11, 0x10, 0x36))
        Start-Sleep -Milliseconds 900

        $root = $null
        try { $root = [System.Windows.Automation.AutomationElement]::FromHandle($hwnd) } catch { }
        if ($null -eq $root) {
            return [ordered]@{
                n_requested   = $Count
                files_on_disk = $filesOnDisk
                measurable    = $false
                branch        = 'uia_from_handle_failed'
            }
        }

        $container = Find-ItemsContainer -Root $root
        if ($null -eq $container) {
            return [ordered]@{
                n_requested          = $Count
                files_on_disk        = $filesOnDisk
                measurable           = $false
                branch               = 'items_container_not_found'
                activation_confirmed = $foregroundConfirmed
            }
        }

        $reading = Measure-ItemsRealization -Container $container
        $forcesLadder = ($reading.realized_below_fold -gt 0)

        $result = [ordered]@{
            n_requested          = $Count
            files_on_disk        = $filesOnDisk
            measurable           = $true
            branch               = if ($forcesLadder) { 'below_fold_realized_ladder_would_be_forced' } else { 'no_below_fold_realized_ladder_not_forced' }
            window_placement     = [ordered]@{ width = $script:PlacementWidth; height = $script:PlacementHeight }
            activation_confirmed = $activated -and $foregroundConfirmed
            view_switch          = [ordered]@{
                chord_sent                    = $true
                single_column_layout_observed = $reading.single_column_layout_observed
                distinct_left_x_count          = $reading.distinct_left_x_count
            }
            container            = [ordered]@{
                has_bounds         = $reading.has_container_bounds
                has_scroll_pattern = $reading.has_scroll_pattern
            }
            children             = [ordered]@{
                realized_total                     = $reading.realized_total
                realized_nonempty_bounds           = $reading.realized_nonempty_bounds
                realized_intersecting_visible_rect = $reading.realized_intersecting_visible_rect
                realized_partially_visible         = $reading.realized_partially_visible
                realized_below_fold                = $reading.realized_below_fold
                any_item_has_scroll_item_pattern   = $reading.any_item_has_scroll_item_pattern
                items_with_scroll_item_pattern     = $reading.items_with_scroll_item_pattern
            }
            stabilization_reads = $reading.stabilization_reads
        }
        return $result
    } finally {
        if ($handle -ne 0) { Close-OpenedHandle -Handle $handle }
        if ($null -ne $dir) { Remove-SyntheticFolder -Dir $dir }
    }
}

# ------------------------------------------------------------ main

$question = 'can an Explorer surface be staged that forces the ancestor-scroll ladder: for N synthetic files (40, 200, 1000) at a deliberately small window height in Details (single-column) view, how many UIA children does the items container realize, how many intersect the container''s own visible rect, how many are realized-but-below-fold (>0 is the condition ladder_judged_for needs), and does the container expose ScrollPattern / do realized items expose ScrollItemPattern'

$runs = New-Object System.Collections.Generic.List[object]
$overallError = $null
try {
    foreach ($n in $FileCounts) {
        try {
            $leg = Measure-OneFileCount -Count $n
        } catch {
            $leg = [ordered]@{
                n_requested = $n
                measurable  = $false
                branch      = 'n_leg_threw'
                error_class = $_.Exception.GetType().Name
            }
        }
        $runs.Add($leg)
    }
} catch {
    $overallError = $_.Exception.GetType().Name
} finally {
    foreach ($h in @($script:OpenedHandles)) { try { Close-OpenedHandle -Handle $h } catch { } }
    foreach ($d in @($script:ScratchDirs)) { try { Remove-SyntheticFolder -Dir $d } catch { } }
}

$measuredRuns = @($runs | Where-Object { $_.measurable })
$forcedRuns = @($measuredRuns | Where-Object { $_.children -and $_.children.realized_below_fold -gt 0 })

if ($null -ne $overallError) {
    $overall = [ordered]@{
        measurable  = $false
        branch      = 'probe_threw'
        error_class = $overallError
    }
} elseif ($measuredRuns.Count -eq 0) {
    $overall = [ordered]@{
        measurable = $false
        branch     = 'no_leg_measurable'
    }
} elseif ($forcedRuns.Count -gt 0) {
    $overall = [ordered]@{
        measurable                = $true
        branch                     = 'ladder_forced_by_at_least_one_n'
        any_n_forces_below_fold    = $true
        forced_at_n                = @($forcedRuns | ForEach-Object { $_.n_requested })
    }
} else {
    $overall = [ordered]@{
        measurable             = $true
        branch                  = 'ladder_never_forced_across_measured_ns'
        any_n_forces_below_fold = $false
    }
}

$result = [ordered]@{
    probe    = $script:Probe
    question = $question
    cites    = @('2.7-dogfood-J3', 'A19-7', 'A18-2')
    overall  = $overall
    runs     = $runs.ToArray()
}

$capturePath = Write-A24Capture -Name "explorer-below-fold-$Label.json" -Content (ConvertTo-Json -InputObject $result -Depth 18)
Register-MandatoryPass -Capture $capturePath -Result $result

Assert-MandatoryMeasurement -Probe $script:Probe -Label $Label

Write-ProbeResult -Probe $script:Probe -Status 'ok' -Message 'Explorer below-fold realization probe captured' -Data @{
    capture = Split-Path -Leaf $capturePath
}
exit 0
