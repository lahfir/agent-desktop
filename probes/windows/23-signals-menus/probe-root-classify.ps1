#Requires -Version 5.1
<#
.SYNOPSIS
    Stateless-predicate follow-up to signals-menu-detection: root-level UIA
    child classification (area 23, sub-phase 2.11).

.DESCRIPTION
    signals-menu-detection-devbox.json (A23-2) established that
    uia_menu_family.present is TRUE AT IDLE for WPF, because the static menu
    bar is reachable under the main window at rest - so raw reachability
    cannot answer "is a menu open right now" from a single observation. The
    only signal that moved was root_children_for_pid, a count, not a
    stateless predicate.

    This script tests the hypothesis that a WPF popup (submenu or
    ContextMenu) is a genuinely NEW root-level window rather than a
    descendant of the app window, by CLASSIFYING every root-level UIA child
    of the target pid rather than counting them: its ControlType; whether it
    is the app's main window; whether a Menu/MenuItem element is reachable
    inside it; its ClassName; whether its bounding rectangle is non-empty;
    and whether it passes the shipped agent-facing window filter
    (window_ops.rs passes_filter). It also measures two other plausible
    stateless discriminators: ExpandCollapsePattern.ExpandCollapseState ==
    Expanded on any MenuItem of the pid, and whether any reachable
    Menu-family element has IsOffscreen false and a non-empty bounding
    rectangle. Both stacks are measured (win32_classic via notepad,
    wpf via the existing menu-wpf-host.ps1 fixture, state confirmed
    independently via its own state file) so the answer covers both.

    Captures signals-menu-root-classify-{devbox,ci}.json (+ .normalized
    twin) under captures\. Corpus safety: shapes, counts, booleans,
    ControlType/ClassName tokens and branch names only - no titles, paths,
    pids, machine names, AutomationIds or message text. Raw HWND/PID values
    never leave this PowerShell process or the native.ps1 C# helper; only
    booleans, counts and ControlType/ClassName tokens are ever handed to
    ConvertTo-Json.
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
$script:paths = @{}

Register-MandatoryCapture -Name @("signals-menu-root-classify-$Label.json")

function Write-RootClassifyCapture {
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

# Same condition probe.ps1 builds for its own menu-family reachability
# checks, redeclared here rather than shared across processes: each script
# runs in its own PowerShell process, so there is no live object to share.
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

<#
    Every root-level UIA child of `ProcessId`, classified rather than
    counted: ControlType, whether it is the app's known main window (matched
    by NativeWindowHandle against `MainWindowHandle`, captured once by the
    caller from an independent Win32 source - never inferred from this same
    UIA walk), whether a Menu/MenuBar/MenuItem element is reachable at or
    under it, its ClassName, whether its rect is non-empty, whether it
    carries WS_EX_TOOLWINDOW, and whether it passes the shipped
    agent-facing filter.
#>
function Get-RootLevelClassification {
    param(
        [Parameter(Mandatory = $true)][int]$ProcessId,
        [Parameter(Mandatory = $true)][IntPtr]$MainWindowHandle
    )
    $root = [System.Windows.Automation.AutomationElement]::RootElement
    $cond = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ProcessIdProperty, $ProcessId)
    $children = $root.FindAll([System.Windows.Automation.TreeScope]::Children, $cond)
    $records = New-Object System.Collections.ArrayList
    $nonMainPasses = $false
    $nonMainMenuReachable = $false
    $nonMainToolWindowMenuChild = $false
    foreach ($c in $children) {
        $hwndRaw = 0
        try { $hwndRaw = $c.Current.NativeWindowHandle } catch { $hwndRaw = 0 }
        $hwnd = [IntPtr]$hwndRaw
        $hasHandle = ($hwnd -ne [IntPtr]::Zero)
        $controlType = $c.Current.ControlType.ProgrammaticName
        $isMain = ($hasHandle -and $hwnd -eq $MainWindowHandle)
        $selfIsMenuFamily = (
            $c.Current.ControlType -eq [System.Windows.Automation.ControlType]::Menu -or
            $c.Current.ControlType -eq [System.Windows.Automation.ControlType]::MenuBar
        )
        $descendantMenuFamily = $false
        try {
            $descendant = $c.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $script:MenuFamilyCondition)
            if ($null -ne $descendant) { $descendantMenuFamily = $true }
        } catch { }
        $menuReachable = ($selfIsMenuFamily -or $descendantMenuFamily)
        $className = $null
        $nonZeroRect = $false
        $toolWindow = $false
        $passesFilter = $false
        if ($hasHandle) {
            $className = [AgentDesktopProbe.A23.Signals23]::ClassNameOf($hwnd)
            $detail = $null
            $passesFilter = [AgentDesktopProbe.A23.Signals23]::PassesAgentFacingFilter($hwnd, [ref]$detail)
            $nonZeroRect = $detail.NonZeroRect
            $toolWindow = $detail.ToolWindow
        }
        [void]$records.Add([ordered]@{
                control_type               = $controlType
                is_main_window             = $isMain
                has_native_window_handle   = $hasHandle
                menu_family_reachable      = $menuReachable
                class_name                 = $className
                non_zero_rect              = $nonZeroRect
                is_tool_window             = $toolWindow
                passes_agent_facing_filter = $passesFilter
            })
        if (-not $isMain) {
            if ($passesFilter) { $nonMainPasses = $true }
            if ($menuReachable) { $nonMainMenuReachable = $true }
            if ($toolWindow -and $menuReachable) { $nonMainToolWindowMenuChild = $true }
        }
    }
    return [ordered]@{
        root_children_count                              = $children.Count
        children                                          = @($records)
        non_main_root_child_present                       = @($records | Where-Object { -not $_.is_main_window }).Count -gt 0
        non_main_root_child_menu_family_reachable          = $nonMainMenuReachable
        non_main_root_child_passes_agent_facing_filter     = $nonMainPasses
        # The compound candidate: a non-main root child that BOTH carries
        # WS_EX_TOOLWINDOW and has a Menu/MenuBar/MenuItem reachable at or
        # under it. Kept distinct from non_main_root_child_menu_family_reachable
        # alone because a companion window with no menu content of its own
        # (this leg's own PowerShell console host, present at idle) is
        # observed to sometimes report menu-family-reachable without being a
        # tool window - the exact UIA mechanism is not pinned down here - and
        # is_tool_window alone does not imply menu content either; only the
        # conjunction is specific to a menu popup.
        non_main_tool_window_menu_child_present            = $nonMainToolWindowMenuChild
    }
}

<#
    Second candidate stateless predicate: any MenuItem belonging to the pid
    (searched from every root-level child, not just the main window, so a
    root-promoted popup's own MenuItems are covered too) reporting
    ExpandCollapseState == Expanded.
#>
function Get-ExpandCollapseMenuItemExpanded {
    param([Parameter(Mandatory = $true)][int]$ProcessId)
    $root = [System.Windows.Automation.AutomationElement]::RootElement
    $pidCond = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ProcessIdProperty, $ProcessId)
    $topLevels = $root.FindAll([System.Windows.Automation.TreeScope]::Children, $pidCond)
    $menuItemCond = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
        [System.Windows.Automation.ControlType]::MenuItem)
    $anyExpanded = $false
    $anyMenuItemSeen = $false
    foreach ($top in $topLevels) {
        $items = $null
        try { $items = $top.FindAll([System.Windows.Automation.TreeScope]::Subtree, $menuItemCond) } catch { continue }
        if ($null -eq $items) { continue }
        foreach ($item in $items) {
            $anyMenuItemSeen = $true
            $pattern = $null
            $has = $false
            try { $has = $item.TryGetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern, [ref]$pattern) } catch { $has = $false }
            if ($has -and $null -ne $pattern) {
                $state = ([System.Windows.Automation.ExpandCollapsePattern]$pattern).Current.ExpandCollapseState
                if ($state -eq [System.Windows.Automation.ExpandCollapseState]::Expanded) { $anyExpanded = $true }
            }
        }
    }
    return [ordered]@{ any_menu_item_seen = $anyMenuItemSeen; any_expanded = $anyExpanded }
}

<#
    Third candidate stateless predicate: any Menu-family element belonging
    to the pid with IsOffscreen == false AND a non-empty bounding
    rectangle, versus the same read at idle.
#>
function Get-MenuFamilyOffscreenFalseVisible {
    param([Parameter(Mandatory = $true)][int]$ProcessId)
    $root = [System.Windows.Automation.AutomationElement]::RootElement
    $pidCond = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ProcessIdProperty, $ProcessId)
    $topLevels = $root.FindAll([System.Windows.Automation.TreeScope]::Children, $pidCond)
    $anySeen = $false
    $anyVisible = $false
    foreach ($top in $topLevels) {
        $elements = $null
        try { $elements = $top.FindAll([System.Windows.Automation.TreeScope]::Subtree, $script:MenuFamilyCondition) } catch { continue }
        if ($null -eq $elements) { continue }
        foreach ($el in $elements) {
            $anySeen = $true
            $offscreen = $true
            try { $offscreen = [bool]$el.Current.IsOffscreen } catch { }
            $rect = $el.Current.BoundingRectangle
            $nonEmpty = ($rect.Width -gt 0 -and $rect.Height -gt 0)
            if ((-not $offscreen) -and $nonEmpty) { $anyVisible = $true }
        }
    }
    return [ordered]@{ any_menu_family_seen = $anySeen; any_visible_onscreen_nonempty_rect = $anyVisible }
}

function Get-RootClassifySample {
    param(
        [Parameter(Mandatory = $true)][int]$ProcessId,
        [Parameter(Mandatory = $true)][IntPtr]$MainWindowHandle
    )
    return [ordered]@{
        root_level      = Get-RootLevelClassification -ProcessId $ProcessId -MainWindowHandle $MainWindowHandle
        expand_collapse = Get-ExpandCollapseMenuItemExpanded -ProcessId $ProcessId
        offscreen_rect  = Get-MenuFamilyOffscreenFalseVisible -ProcessId $ProcessId
    }
}

# ---------------------------------------------------------------- win32 classic leg

function Measure-Win32RootClassify {
    $note = Start-Process notepad.exe -PassThru
    Register-SpawnedPid -ProcessId $note.Id
    Start-Sleep -Milliseconds 800
    $note.Refresh()
    Assert-Foreground -ExpectedProcessId $note.Id -Stage 'a23-rootclassify-win32-idle'
    $hwnd = $note.MainWindowHandle
    $edit = [AgentDesktopProbe.A23.Signals23]::FindEditChild($hwnd)

    $idle = Get-RootClassifySample -ProcessId $note.Id -MainWindowHandle $hwnd

    [void][AgentDesktopProbe.A23.Signals23]::PostSysKeyMenu($hwnd, 0)
    Start-Sleep -Milliseconds 400
    [AgentDesktopProbe.A23.Signals23]::PostKeyTap($hwnd, [AgentDesktopProbe.A23.Signals23]::VK_DOWN)
    Start-Sleep -Milliseconds 400
    $menuBar = Get-RootClassifySample -ProcessId $note.Id -MainWindowHandle $hwnd
    [AgentDesktopProbe.A23.Signals23]::PostKeyTap($hwnd, [AgentDesktopProbe.A23.Signals23]::VK_ESCAPE)
    [AgentDesktopProbe.A23.Signals23]::PostKeyTap($hwnd, [AgentDesktopProbe.A23.Signals23]::VK_ESCAPE)
    Start-Sleep -Milliseconds 300

    [void][AgentDesktopProbe.A23.Signals23]::PostContextMenuKeyboardInvoked($edit)
    Start-Sleep -Milliseconds 400
    $contextMenu = Get-RootClassifySample -ProcessId $note.Id -MainWindowHandle $hwnd
    [AgentDesktopProbe.A23.Signals23]::PostKeyTap($hwnd, [AgentDesktopProbe.A23.Signals23]::VK_ESCAPE)
    Start-Sleep -Milliseconds 300

    # Closes the loop `wait --menu-closed` needs: every sample so far only
    # shows the predicate turning true, never returning to false again after
    # a real close (idle is a *pre-open* false, not a *post-close* one).
    $contextMenuClosed = Get-RootClassifySample -ProcessId $note.Id -MainWindowHandle $hwnd

    try { Stop-ScratchProcess -ProcessId $note.Id } catch { }

    return [ordered]@{
        measurable          = $true
        branch               = 'notepad_classic_win32_staged'
        idle                 = $idle
        menu_bar_dropdown    = $menuBar
        context_menu         = $contextMenu
        context_menu_closed  = $contextMenuClosed
    }
}

# ---------------------------------------------------------------- wpf leg

function Wait-MenuHostState {
    param([Parameter(Mandatory = $true)][string]$StateFile, [Parameter(Mandatory = $true)][string]$Expected, [int]$TimeoutMs = 4000)
    $deadline = [Diagnostics.Stopwatch]::StartNew()
    while ($deadline.ElapsedMilliseconds -lt $TimeoutMs) {
        if (Test-Path -LiteralPath $StateFile) {
            $value = Get-Content -LiteralPath $StateFile -Raw -ErrorAction SilentlyContinue
            if ($value -eq $Expected) { return $true }
        }
        Start-Sleep -Milliseconds 100
    }
    return $false
}

function Measure-WpfRootClassify {
    $hostScript = Join-Path $script:ProbeDir 'menu-wpf-host.ps1'
    if (-not (Test-Path -LiteralPath $hostScript)) {
        return [ordered]@{ measurable = $false; branch = 'menu_wpf_host_missing' }
    }
    $stateFile = Join-Path $env:TEMP ('a23-rootclassify-wpf-state-' + [guid]::NewGuid().ToString('N') + '.txt')
    $proc = Start-Process -FilePath 'powershell.exe' -ArgumentList @(
        '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $hostScript,
        '-StateFile', $stateFile, '-Left', '60', '-Top', '60',
        '-MenuOpenAtMs', '1200', '-MenuOpenDurationMs', '2600',
        '-ContextOpenAtMs', '4300', '-ContextOpenDurationMs', '2600', '-ExitAtMs', '9500'
    ) -PassThru -RedirectStandardOutput ([System.IO.Path]::Combine($env:TEMP, 'a23-rootclassify-wpf-stdout.txt')) -RedirectStandardError ([System.IO.Path]::Combine($env:TEMP, 'a23-rootclassify-wpf-stderr.txt'))
    Register-SpawnedPid -ProcessId $proc.Id
    $targetPid = $proc.Id
    Start-Sleep -Milliseconds 700

    $idleOk = Wait-MenuHostState -StateFile $stateFile -Expected 'idle' -TimeoutMs 3000
    # Main window identified once, from an independent Win32 source (visible +
    # titled top-level for this pid) rather than from the UIA walk under
    # test - the host's WPF popups never carry a window title, so this stays
    # stable as the same handle across all four samples (idle, menu-open,
    # context-open, context-closed).
    $mainHwnd = [AgentDesktopProbe.A23.Signals23]::FindVisibleTitledTopLevelForPid($targetPid)
    $idle = Get-RootClassifySample -ProcessId $targetPid -MainWindowHandle $mainHwnd
    $idle.fixture_confirmed = $idleOk

    $menuOk = Wait-MenuHostState -StateFile $stateFile -Expected 'menu-open' -TimeoutMs 3000
    $menuBar = Get-RootClassifySample -ProcessId $targetPid -MainWindowHandle $mainHwnd
    $menuBar.fixture_confirmed = $menuOk

    $ctxOk = Wait-MenuHostState -StateFile $stateFile -Expected 'context-open' -TimeoutMs 4000
    $contextMenu = Get-RootClassifySample -ProcessId $targetPid -MainWindowHandle $mainHwnd
    $contextMenu.fixture_confirmed = $ctxOk

    # Closes the loop `wait --menu-closed` needs: the fixture's own schedule
    # sets 'context-closed' ~2.6s after 'context-open' and only exits ~2.6s
    # after that, so there is a real post-close window to sample - confirmed
    # independently through the fixture's own state file, same as every
    # other sample, never through the predicate under test.
    $closedOk = Wait-MenuHostState -StateFile $stateFile -Expected 'context-closed' -TimeoutMs 4000
    $contextMenuClosed = Get-RootClassifySample -ProcessId $targetPid -MainWindowHandle $mainHwnd
    $contextMenuClosed.fixture_confirmed = $closedOk

    try { Stop-ScratchProcess -ProcessId $proc.Id } catch { }
    Remove-Item -LiteralPath $stateFile -Force -ErrorAction SilentlyContinue

    return [ordered]@{
        measurable          = $true
        branch               = 'wpf_scratch_menu_host_staged'
        idle                 = $idle
        menu_bar_dropdown    = $menuBar
        context_menu         = $contextMenu
        context_menu_closed  = $contextMenuClosed
    }
}

function Measure-RootClassify {
    return [ordered]@{
        probe    = '23-signals-menus'
        question = 'is there a stateless predicate - one evaluable from a single observation, no baseline, no prior sample - that distinguishes "this pid has a menu open" from "this pid merely has a menu bar": root-level UIA child classification (ControlType, is-main-window, menu-family reachability, ClassName, non-zero rect, passes_filter) per KTD6''s new-root-level-window hypothesis, plus two other candidates (ExpandCollapsePattern.ExpandCollapseState == Expanded on any MenuItem; any reachable Menu-family element with IsOffscreen false and a non-empty rect), for both win32_classic and wpf'
        stacks   = [ordered]@{
            win32_classic = Measure-Win32RootClassify
            wpf           = Measure-WpfRootClassify
        }
    }
}

try {
    Initialize-ProbeNative

    $result = Measure-RootClassify
    $script:paths.classify = Write-RootClassifyCapture -Name "signals-menu-root-classify-$Label.json" -Content (ConvertTo-Json -InputObject $result -Depth 16)
    Register-MandatoryPass -Capture $script:paths.classify -Result $result
} finally {
    Stop-AllSpawned
}

Assert-MandatoryMeasurement -Probe '23-signals-menus' -Label $Label

Write-ProbeResult -Probe '23-signals-menus' -Status 'ok' -Message 'root-level UIA child classification (stateless WPF/win32 menu-open predicate) captured' -Data @{
    classify = if ($script:paths.classify) { Split-Path -Leaf $script:paths.classify } else { '<none>' }
}
exit 0
