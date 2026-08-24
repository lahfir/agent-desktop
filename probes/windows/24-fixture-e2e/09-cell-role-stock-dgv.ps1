#Requires -Version 5.1
<#
.SYNOPSIS
    Stock WinForms DataGridView reconfirmation (area 24, sub-phase 2.12, U13).

.DESCRIPTION
    A24-9's stock-DataGridView leg did not measure on this box
    (`stock_dgv.branch: dgvRows_element_not_found`) and left the reconfirmation
    A16-10 owes as still open. U13's approach item 4 requires this box to be
    tried again before the `cell` role arm decides whether the refinement
    rests on A16-10's 2.4 reading alone.

    Unlike 06-cell-role-provider.ps1's single fixed-name AutomationId lookup,
    this probe polls the actual UIA search itself rather than sleeping once
    and trying once, and it runs three independent searches so a negative
    result is diagnosed rather than merely restated:

      1. FindFirst by AutomationId 'dgvRows' (the exact A24-9 leg).
      2. FindFirst by ControlType.DataGrid, regardless of AutomationId - tells
         whether *any* DataGrid-typed element reaches UIA at all.
      3. A bounded ControlView tree dump (depth 8, 200 nodes) recording a
         ControlType histogram - tells whether the grid surfaces under some
         other ControlType (Custom, Pane, ...) that the first two searches
         would miss entirely.

    If a grid element is found by either of the first two searches, its
    GridItem/TableItem pattern shape is read at the grid, row and cell levels
    with the same Get-ElementRoleShape probe already uses, so a positive
    result is directly comparable to A16-10's WPF reading.

    Corpus safety: only ControlType names (UIA vocabulary, not target text),
    the one fixture-authored AutomationId constant this probe searches for
    ('dgvRows', authored by this corpus's own ScratchForms.cs), booleans and
    small integers ever reach the capture - no window titles, file paths,
    pids, machine names, user names, or message text. The scratch process is
    killed in a `finally` and swept again from the scratch-process ledger.

    Run: powershell -NoProfile -ExecutionPolicy Bypass -File .\probes\windows\24-fixture-e2e\09-cell-role-stock-dgv.ps1 -Label <devbox|ci>
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'ci')][string]$Label = 'devbox'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

. (Join-Path (Split-Path -Parent $PSCommandPath) '..\common.ps1')
Initialize-ProbeRedaction
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$script:Probe = '24-fixture-e2e-09-cell-role-stock-dgv'
$script:ProbeDir = Split-Path -Parent $PSCommandPath
$script:CaptureDir = Join-Path $script:ProbeDir 'captures'
if (-not (Test-Path -LiteralPath $script:CaptureDir)) {
    New-Item -ItemType Directory -Path $script:CaptureDir -Force | Out-Null
}

$AE = [System.Windows.Automation.AutomationElement]
$TS = [System.Windows.Automation.TreeScope]
$ControlWalker = [System.Windows.Automation.TreeWalker]::ControlViewWalker

Register-MandatoryCapture -Name @("cell-role-stock-dgv-$Label.json")

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

<#
    Polls the search itself rather than sleeping a fixed duration and trying
    once: the observable condition is "this element is now in the tree", and
    the only way to wait on it is to keep asking. Each attempt is counted so
    the capture shows whether a positive result needed retries at all.
#>
function Wait-DescendantElement {
    param(
        [Parameter(Mandatory = $true)]$Root,
        [Parameter(Mandatory = $true)]$Condition,
        [int]$TimeoutMs = 5000,
        [int]$PollMs = 150
    )
    $deadline = (Get-Date).AddMilliseconds($TimeoutMs)
    $found = $null
    $attempts = 0
    while ((Get-Date) -lt $deadline) {
        $attempts++
        try { $found = $Root.FindFirst($TS::Descendants, $Condition) } catch { $found = $null }
        if ($null -ne $found) { break }
        Start-Sleep -Milliseconds $PollMs
    }
    return [ordered]@{ element = $found; attempts = $attempts; found = ($null -ne $found) }
}

function Get-ElementRoleShape {
    param([Parameter(Mandatory = $true)]$Element)
    $controlType = 'unread'
    try { $controlType = ($Element.Current.ControlType.ProgrammaticName -replace '^ControlType\.', '') } catch { }
    $automationIdPresent = $false
    try { $automationIdPresent = -not [string]::IsNullOrEmpty($Element.Current.AutomationId) } catch { }
    $supported = @()
    try { $supported = @($Element.GetSupportedPatterns() | ForEach-Object { $_.ProgrammaticName -replace 'PatternIdentifiers\.Pattern$', '' } | Sort-Object) } catch { }

    $gridItemObj = $null
    $hasGridItem = $false
    try { $hasGridItem = $Element.TryGetCurrentPattern([System.Windows.Automation.GridItemPattern]::Pattern, [ref]$gridItemObj) } catch { }
    $tableItemObj = $null
    $hasTableItem = $false
    try { $hasTableItem = $Element.TryGetCurrentPattern([System.Windows.Automation.TableItemPattern]::Pattern, [ref]$tableItemObj) } catch { }

    return [ordered]@{
        control_type               = $controlType
        automation_id_present      = $automationIdPresent
        supported_patterns         = @($supported)
        grid_item_pattern_try_get  = $hasGridItem
        table_item_pattern_try_get = $hasTableItem
    }
}

<#
    A bounded ControlView breadth-first walk from root, capped at 200 visited
    nodes and depth 8 - generous for a fixture window with a handful of
    controls, far short of anything that could turn a negative result into a
    long-running walk. Only a ControlType histogram and the deepest depth
    reached are recorded, per the shapes-and-counts corpus safety rule.
#>
function Get-ControlTypeHistogram {
    param([Parameter(Mandatory = $true)]$Root, [int]$MaxNodes = 200, [int]$MaxDepth = 8)
    $histogram = [ordered]@{}
    $queue = New-Object System.Collections.Generic.Queue[object]
    $queue.Enqueue(@{ element = $Root; depth = 0 })
    $visited = 0
    $maxDepthReached = 0
    $dataGridSeen = $false
    while ($queue.Count -gt 0 -and $visited -lt $MaxNodes) {
        $entry = $queue.Dequeue()
        $visited++
        if ($entry.depth -gt $maxDepthReached) { $maxDepthReached = $entry.depth }
        $controlType = 'unread'
        try { $controlType = ($entry.element.Current.ControlType.ProgrammaticName -replace '^ControlType\.', '') } catch { }
        if ($controlType -eq 'DataGrid') { $dataGridSeen = $true }
        if ($histogram.Contains($controlType)) { $histogram[$controlType] = [int]$histogram[$controlType] + 1 } else { $histogram[$controlType] = 1 }
        if ($entry.depth -ge $MaxDepth) { continue }
        $child = $null
        try { $child = $ControlWalker.GetFirstChild($entry.element) } catch { $child = $null }
        while ($null -ne $child) {
            $queue.Enqueue(@{ element = $child; depth = ($entry.depth + 1) })
            try { $child = $ControlWalker.GetNextSibling($child) } catch { $child = $null }
        }
    }
    return [ordered]@{
        nodes_visited      = $visited
        truncated          = ($queue.Count -gt 0)
        max_depth_reached  = $maxDepthReached
        data_grid_seen     = $dataGridSeen
        control_type_counts = $histogram
    }
}

function Measure-StockDataGridView {
    $scratchExe = Join-Path (Get-ProbeRoot) 'scratch\bin\ScratchForms.exe'
    if (-not (Test-Path -LiteralPath $scratchExe)) {
        try {
            & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path (Get-ProbeRoot) 'scratch\build-scratch.ps1') | Out-Null
        } catch { }
    }
    if (-not (Test-Path -LiteralPath $scratchExe)) {
        return [ordered]@{ measurable = $false; branch = 'scratch_forms_exe_unavailable' }
    }

    $procId = 0
    try {
        $started = Start-ScratchProcess -FilePath $scratchExe -ArgumentList @('--tag', 'a24u13-stock', '--pos', '0,0', '--host-providers') -NoActivate
        $procId = $started.ProcessId
        if ($started.MainWindowHandle -eq [IntPtr]::Zero) {
            return [ordered]@{ measurable = $false; branch = 'scratch_forms_window_not_found' }
        }

        $root = $null
        $rootDeadline = (Get-Date).AddSeconds(5)
        while ((Get-Date) -lt $rootDeadline -and $null -eq $root) {
            try { $root = $AE::FromHandle($started.MainWindowHandle) } catch { $root = $null }
            if ($null -eq $root) { Start-Sleep -Milliseconds 150 }
        }
        if ($null -eq $root) {
            return [ordered]@{ measurable = $false; branch = 'uia_from_handle_failed' }
        }

        $byAutomationId = New-Object System.Windows.Automation.PropertyCondition($AE::AutomationIdProperty, 'dgvRows')
        $searchByAutomationId = Wait-DescendantElement -Root $root -Condition $byAutomationId

        $byControlType = New-Object System.Windows.Automation.PropertyCondition(
            $AE::ControlTypeProperty, [System.Windows.Automation.ControlType]::DataGrid)
        $searchByControlType = Wait-DescendantElement -Root $root -Condition $byControlType

        $grid = $null
        $foundVia = 'neither'
        if ($searchByAutomationId.found) { $grid = $searchByAutomationId.element; $foundVia = 'automation_id' }
        elseif ($searchByControlType.found) { $grid = $searchByControlType.element; $foundVia = 'control_type' }

        if ($null -eq $grid) {
            $dump = Get-ControlTypeHistogram -Root $root
            return [ordered]@{
                measurable                    = $false
                branch                        = 'dgvRows_element_not_found'
                fixture_mode                  = 'host-providers: fixture custom collapse-to-Pane provider is switched off, dgvRows exposes its real WinForms UIA automation peer'
                search_by_automation_id       = [ordered]@{ found = $false; attempts = $searchByAutomationId.attempts }
                search_by_control_type        = [ordered]@{ found = $false; attempts = $searchByControlType.attempts }
                control_type_dump             = $dump
            }
        }

        $gridInfo = Get-ElementRoleShape -Element $grid

        $rows = New-Object System.Collections.Generic.List[object]
        $child = $null
        try { $child = $ControlWalker.GetFirstChild($grid) } catch { }
        $visited = 0
        while ($null -ne $child -and $visited -lt 12) {
            $visited++
            $rowInfo = Get-ElementRoleShape -Element $child
            $cellInfo = $null
            $grandchild = $null
            try { $grandchild = $ControlWalker.GetFirstChild($child) } catch { }
            if ($null -ne $grandchild) { $cellInfo = Get-ElementRoleShape -Element $grandchild }
            [void]$rows.Add([ordered]@{ row = $rowInfo; cell = $cellInfo })
            try { $child = $ControlWalker.GetNextSibling($child) } catch { $child = $null }
        }

        $rowHasShape = $false
        $cellHasShape = $false
        foreach ($entry in $rows) {
            if ($null -ne $entry.row -and ($entry.row.grid_item_pattern_try_get -or $entry.row.table_item_pattern_try_get)) { $rowHasShape = $true }
            if ($null -ne $entry.cell -and ($entry.cell.grid_item_pattern_try_get -or $entry.cell.table_item_pattern_try_get)) { $cellHasShape = $true }
        }
        $matchesA1610 = (-not ($rowHasShape -or $cellHasShape))

        return [ordered]@{
            measurable                                  = $true
            branch                                       = if ($matchesA1610) { 'stock_dgv_matches_a16_10_no_griditem_tableitem' } else { 'stock_dgv_contradicts_a16_10_shape_present' }
            fixture_mode                                 = 'host-providers: fixture custom collapse-to-Pane provider is switched off, dgvRows exposes its real WinForms UIA automation peer'
            found_via                                    = $foundVia
            search_by_automation_id                      = [ordered]@{ found = $true; attempts = $searchByAutomationId.attempts }
            search_by_control_type                       = [ordered]@{ found = $searchByControlType.found; attempts = $searchByControlType.attempts }
            grid_children_walked                         = $rows.Count
            matches_a16_10_no_griditem_tableitem_on_dgv  = $matchesA1610
            grid                                         = $gridInfo
            rows                                         = @($rows)
        }
    } finally {
        if ($procId -gt 0) { try { Stop-ScratchProcess -ProcessId $procId } catch { } }
    }
}

# ---------------------------------------------------------------- main

$question = 'does the stock WinForms DataGridView (dgvRows) resolve on this box under --host-providers, reconfirming or contradicting A24-9''s dgvRows_element_not_found leg - and if it still does not resolve, what does an independent ControlType-based search and a bounded tree dump show about why'

$stock = $null
try { $stock = Measure-StockDataGridView } catch {
    $stock = [ordered]@{ measurable = $false; branch = 'stock_leg_threw'; error_class = $_.Exception.GetType().Name }
}

if ($stock.measurable -eq $true) {
    $overall = [ordered]@{
        measurable = $true
        branch     = 'stock_dgv_reconfirmed'
        conclusion = 'the stock WinForms DataGridView resolved on this box; see stock_dgv.branch for whether its shape matches or contradicts A16-10'
    }
} else {
    $overall = [ordered]@{
        measurable = $false
        branch     = 'stock_dgv_still_not_measurable'
        conclusion = 'A24-9''s dgvRows_element_not_found leg reproduces on this box even with an independent ControlType search and a bounded tree dump; the cell role arm''s real-application grounding rests on A16-10''s 2.4 WPF DataGrid reading alone, not on a reconfirmed WinForms DataGridView reading'
    }
}

$result = [ordered]@{
    probe     = $script:Probe
    question  = $question
    cites     = @('A24-9', 'A16-10')
    overall   = $overall
    stock_dgv = $stock
}

$overallError = $null
$capturePath = $null
try {
    $capturePath = Write-A24Capture -Name "cell-role-stock-dgv-$Label.json" -Content (ConvertTo-Json -InputObject $result -Depth 18)
    Register-MandatoryPass -Capture $capturePath -Result $result
} catch {
    $overallError = $_.Exception.GetType().Name
} finally {
    foreach ($leftoverId in (Get-ScratchProcessIds)) {
        try { Stop-Process -Id $leftoverId -Force -ErrorAction SilentlyContinue } catch { }
    }
}

if ($null -ne $overallError) {
    Write-ProbeResult -Probe $script:Probe -Status 'fail' -Message ('probe threw while writing capture: ' + $overallError) -Data @{ error_class = $overallError }
    exit 1
}

Assert-MandatoryMeasurement -Probe $script:Probe -Label $Label

Write-ProbeResult -Probe $script:Probe -Status 'ok' -Message 'stock DataGridView reconfirmation probe captured' -Data @{
    capture = Split-Path -Leaf $capturePath
}
exit 0
