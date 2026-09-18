#Requires -Version 5.1
<#
.SYNOPSIS
    Signals and menu-detection gap probe (area 23, sub-phase 2.11 U1).

.DESCRIPTION
    Measures menu-open detection per UI stack (classic-menu-mode flags, UIA
    ControlType.Menu family, #32768 popup presence, and whether that popup
    would pass the shipped agent-facing window filter), the mid-walk
    identity-race rate and the bounded re-walk's efficacy, how many enumerated
    windows are durably versus intermittently unidentifiable, modal-dialog
    classification, and GetGUIThreadInfo's cross-process reach.

    Captures under captures\ as signals-*-{devbox,ci}.json (+ .normalized
    twins). Corpus safety: scratch-only windows and known fixtures; shapes,
    counts and booleans only - no titles, paths, pids, machine names, or
    message text. Every raw HWND/PID this probe reads stays inside the C#
    helper process boundary or this PowerShell process; only booleans and
    counts are ever handed to ConvertTo-Json.
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

Register-MandatoryCapture -Name @(
    "signals-menu-detection-$Label.json",
    "signals-race-$Label.json",
    "signals-unidentifiable-$Label.json",
    "signals-modal-$Label.json",
    "signals-guithreadinfo-$Label.json",
    "signals-cost-$Label.json"
)

function Write-A23Capture {
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

# Built once: every place that asks "is a Menu/MenuBar/MenuItem reachable"
# reuses this condition rather than re-declaring it, so the three control
# types checked never drift apart between legs.
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
    One instant's answer to all three UIA-reachable questions for `pid`:
    is a Menu/MenuBar itself a root child of the desktop (the shape a WPF
    submenu popup takes), and is a Menu-family element reachable as a
    descendant of one of that pid's top-level windows (the shape the plan's
    KTD6 calls "under_app_window"). Both are checked because U5 needs to know
    which shape to scan, not just whether the element exists somewhere.
#>
function Get-UiaMenuFamilyPresence {
    param([Parameter(Mandatory = $true)][int]$ProcessId)
    $root = [System.Windows.Automation.AutomationElement]::RootElement
    $cond = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ProcessIdProperty, $ProcessId)
    $children = $root.FindAll([System.Windows.Automation.TreeScope]::Children, $cond)
    $rootChildFound = $false
    $underAppWindowFound = $false
    foreach ($c in $children) {
        if ($c.Current.ControlType -eq [System.Windows.Automation.ControlType]::Menu -or
            $c.Current.ControlType -eq [System.Windows.Automation.ControlType]::MenuBar) {
            $rootChildFound = $true
        }
        try {
            $descendant = $c.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $script:MenuFamilyCondition)
            if ($null -ne $descendant) { $underAppWindowFound = $true }
        } catch { }
    }
    return [ordered]@{
        present           = ($rootChildFound -or $underAppWindowFound)
        root_child        = $rootChildFound
        under_app_window  = $underAppWindowFound
        root_children_for_pid = $children.Count
    }
}

function Get-ClassPopupSummary {
    param([Parameter(Mandatory = $true)][int]$ProcessId)
    $popups = [AgentDesktopProbe.A23.Signals23]::ClassPopupsForPid($ProcessId)
    $anyPasses = $false
    $anyVisible = $false
    $anyCloaked = $false
    $anyToolWindow = $false
    $anyNonZeroRect = $false
    foreach ($p in $popups) {
        if ($p.PassesFilter) { $anyPasses = $true }
        if ($p.Visible) { $anyVisible = $true }
        if ($p.Cloaked) { $anyCloaked = $true }
        if ($p.ToolWindow) { $anyToolWindow = $true }
        if ($p.NonZeroRect) { $anyNonZeroRect = $true }
    }
    return [ordered]@{
        present                        = ($popups.Count -gt 0)
        count                          = $popups.Count
        any_visible                    = $anyVisible
        any_cloaked                    = $anyCloaked
        any_tool_window                = $anyToolWindow
        any_nonzero_rect               = $anyNonZeroRect
        any_passes_agent_facing_filter = $anyPasses
    }
}

function Get-ClassicFlagsSummary {
    param([Parameter(Mandatory = $true)][int]$ProcessId)
    $threadsRead = 0
    $threadsTotal = 0
    $any = [AgentDesktopProbe.A23.Signals23]::AnyThreadInMenuMode($ProcessId, [ref]$threadsRead, [ref]$threadsTotal)
    return [ordered]@{
        any_thread_menu_mode = $any
        threads_read         = $threadsRead
        threads_total        = $threadsTotal
    }
}

function Get-MenuStackSample {
    param([Parameter(Mandatory = $true)][int]$ProcessId)
    $classic = Get-ClassicFlagsSummary -ProcessId $ProcessId
    $popup = Get-ClassPopupSummary -ProcessId $ProcessId
    $uia = Get-UiaMenuFamilyPresence -ProcessId $ProcessId
    return [ordered]@{
        classic_flags   = $classic
        class_32768     = $popup
        uia_menu_family = $uia
        any_source_fired = ($classic.any_thread_menu_mode -or $popup.present -or $uia.present)
    }
}

# ---------------------------------------------------------------- leg 1: menu detection per stack

function Measure-Win32MenuDetection {
    $note = Start-Process notepad.exe -PassThru
    Register-SpawnedPid -ProcessId $note.Id
    Start-Sleep -Milliseconds 800
    $note.Refresh()
    Assert-Foreground -ExpectedProcessId $note.Id -Stage 'a23-win32-menu-idle'
    $hwnd = $note.MainWindowHandle
    $edit = [AgentDesktopProbe.A23.Signals23]::FindEditChild($hwnd)

    $idle = Get-MenuStackSample -ProcessId $note.Id

    [void][AgentDesktopProbe.A23.Signals23]::PostSysKeyMenu($hwnd, 0)
    Start-Sleep -Milliseconds 400
    [AgentDesktopProbe.A23.Signals23]::PostKeyTap($hwnd, [AgentDesktopProbe.A23.Signals23]::VK_DOWN)
    Start-Sleep -Milliseconds 400
    $menuBar = Get-MenuStackSample -ProcessId $note.Id
    [AgentDesktopProbe.A23.Signals23]::PostKeyTap($hwnd, [AgentDesktopProbe.A23.Signals23]::VK_ESCAPE)
    [AgentDesktopProbe.A23.Signals23]::PostKeyTap($hwnd, [AgentDesktopProbe.A23.Signals23]::VK_ESCAPE)
    Start-Sleep -Milliseconds 300

    [void][AgentDesktopProbe.A23.Signals23]::PostContextMenuKeyboardInvoked($edit)
    Start-Sleep -Milliseconds 400
    $contextMenu = Get-MenuStackSample -ProcessId $note.Id
    [AgentDesktopProbe.A23.Signals23]::PostKeyTap($hwnd, [AgentDesktopProbe.A23.Signals23]::VK_ESCAPE)
    Start-Sleep -Milliseconds 300

    [void][AgentDesktopProbe.A23.Signals23]::PostSysKeyMenu($hwnd, [AgentDesktopProbe.A23.Signals23]::VK_SPACE)
    Start-Sleep -Milliseconds 400
    $systemMenu = Get-MenuStackSample -ProcessId $note.Id
    [AgentDesktopProbe.A23.Signals23]::PostKeyTap($hwnd, [AgentDesktopProbe.A23.Signals23]::VK_ESCAPE)
    Start-Sleep -Milliseconds 300

    try { Stop-ScratchProcess -ProcessId $note.Id } catch { }

    return [ordered]@{
        measurable        = $true
        branch            = 'notepad_classic_win32_staged'
        staging           = 'PostMessage WM_SYSCOMMAND/SC_KEYMENU + WM_CONTEXTMENU, no real input synthesis needed'
        idle              = $idle
        menu_bar_dropdown = $menuBar
        context_menu      = $contextMenu
        system_menu       = $systemMenu
    }
}

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

function Measure-WpfMenuDetection {
    $hostScript = Join-Path $script:ProbeDir 'menu-wpf-host.ps1'
    if (-not (Test-Path -LiteralPath $hostScript)) {
        return [ordered]@{ measurable = $false; branch = 'menu_wpf_host_missing' }
    }
    $stateFile = Join-Path $env:TEMP ('a23-wpf-state-' + [guid]::NewGuid().ToString('N') + '.txt')
    $proc = Start-Process -FilePath 'powershell.exe' -ArgumentList @(
        '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $hostScript,
        '-StateFile', $stateFile, '-Left', '60', '-Top', '60',
        '-MenuOpenAtMs', '1200', '-MenuOpenDurationMs', '2600',
        '-ContextOpenAtMs', '4300', '-ContextOpenDurationMs', '2600', '-ExitAtMs', '9500'
    ) -PassThru -RedirectStandardOutput ([System.IO.Path]::Combine($env:TEMP, 'a23-wpf-stdout.txt')) -RedirectStandardError ([System.IO.Path]::Combine($env:TEMP, 'a23-wpf-stderr.txt'))
    Register-SpawnedPid -ProcessId $proc.Id
    $targetPid = $proc.Id
    Start-Sleep -Milliseconds 700

    $idleOk = Wait-MenuHostState -StateFile $stateFile -Expected 'idle' -TimeoutMs 3000
    $idle = Get-MenuStackSample -ProcessId $targetPid
    $idle.fixture_confirmed = $idleOk

    $menuOk = Wait-MenuHostState -StateFile $stateFile -Expected 'menu-open' -TimeoutMs 3000
    $menuBar = Get-MenuStackSample -ProcessId $targetPid
    $menuBar.fixture_confirmed = $menuOk

    $ctxOk = Wait-MenuHostState -StateFile $stateFile -Expected 'context-open' -TimeoutMs 4000
    $contextMenu = Get-MenuStackSample -ProcessId $targetPid
    $contextMenu.fixture_confirmed = $ctxOk

    try { Stop-ScratchProcess -ProcessId $proc.Id } catch { }
    Remove-Item -LiteralPath $stateFile -Force -ErrorAction SilentlyContinue

    return [ordered]@{
        measurable        = $true
        branch            = 'wpf_scratch_menu_host_staged'
        staging           = 'MenuItem.IsSubmenuOpen / ContextMenu.IsOpen set programmatically by the fixture on its own DispatcherTimer schedule; state confirmed via an independent state file the probe polls'
        idle              = $idle
        menu_bar_dropdown = $menuBar
        context_menu      = $contextMenu
    }
}

function Stop-NamedProcesses {
    param([Parameter(Mandatory = $true)][string]$NameLike)
    foreach ($proc in @(Get-Process -Name $NameLike -ErrorAction SilentlyContinue)) {
        try { Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue } catch { }
    }
}

function Measure-ChromiumMenuDetection {
    $exe = Join-Path $env:LOCALAPPDATA 'Programs\Obsidian\Obsidian.exe'
    if (-not (Test-Path -LiteralPath $exe)) {
        return [ordered]@{ measurable = $false; branch = 'obsidian_not_installed' }
    }
    $proc = Start-Process -FilePath $exe -PassThru
    Register-SpawnedPid -ProcessId $proc.Id
    Start-Sleep -Milliseconds 6000

    $allProcs = @(Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue)
    foreach ($op in $allProcs) { Register-SpawnedPid -ProcessId $op.Id }
    $targetHwnd = [IntPtr]::Zero
    $targetPid = 0
    foreach ($op in $allProcs) {
        $handle = [AgentDesktopProbe.A23.Signals23]::FindVisibleTitledTopLevelForPid($op.Id)
        if ($handle -ne [IntPtr]::Zero) { $targetHwnd = $handle; $targetPid = $op.Id; break }
    }
    if ($targetHwnd -eq [IntPtr]::Zero) {
        Stop-NamedProcesses -NameLike 'Obsidian'
        return [ordered]@{ measurable = $false; branch = 'no_visible_titled_top_level_window_found' }
    }

    $activated = [AgentDesktopProbe.A23.Signals23]::ActivateOnce($targetHwnd)
    Start-Sleep -Milliseconds 300
    $foregroundConfirmed = [AgentDesktopProbe.A23.Signals23]::ForegroundIs($targetHwnd)

    $attempts = New-Object System.Collections.ArrayList
    $anyStagingSignal = $false

    [AgentDesktopProbe.A23.Signals23]::TapAlt()
    Start-Sleep -Milliseconds 700
    $r1 = Get-MenuStackSample -ProcessId $targetPid
    [void]$attempts.Add(([ordered]@{ method = 'alt_tap'; result = $r1 }))
    if ($r1.any_source_fired) { $anyStagingSignal = $true }

    [AgentDesktopProbe.A23.Signals23]::KeyChord(@(0x12, 0x46))
    Start-Sleep -Milliseconds 700
    $r2 = Get-MenuStackSample -ProcessId $targetPid
    [void]$attempts.Add(([ordered]@{ method = 'alt_f_accelerator'; result = $r2 }))
    if ($r2.any_source_fired) { $anyStagingSignal = $true }

    $rect = [AgentDesktopProbe.A23.Signals23]::WindowRect($targetHwnd)
    $cx = [int](($rect.Left + $rect.Right) / 2)
    $cy = [int](($rect.Top + $rect.Bottom) / 2)
    [AgentDesktopProbe.A23.Signals23]::RightClickAt($cx, $cy)
    Start-Sleep -Milliseconds 700
    $r3 = Get-MenuStackSample -ProcessId $targetPid
    [void]$attempts.Add(([ordered]@{ method = 'right_click_window_center'; result = $r3 }))
    if ($r3.any_source_fired) { $anyStagingSignal = $true }

    Stop-NamedProcesses -NameLike 'Obsidian'

    $activation = [ordered]@{ activate_once_reported = $activated; foreground_confirmed = $foregroundConfirmed }
    if (-not $anyStagingSignal) {
        return [ordered]@{
            measurable = $false
            branch     = 'no_menu_surface_reached_by_generic_staging_this_app_version'
            activation = $activation
            attempts   = @($attempts)
        }
    }
    return [ordered]@{
        measurable = $true
        branch     = 'staged_via_generic_input'
        activation = $activation
        attempts   = @($attempts)
    }
}

function Measure-MenuDetection {
    return [ordered]@{
        probe    = '23-signals-menus'
        question = 'per-stack menu-open detection: classic GUITHREADINFO menu-mode flags, UIA Menu/MenuBar/MenuItem reachability, #32768 popup presence, and whether that popup passes the shipped agent-facing window filter (window_ops.rs passes_filter)'
        stacks   = [ordered]@{
            win32_classic     = Measure-Win32MenuDetection
            wpf               = Measure-WpfMenuDetection
            chromium_electron = Measure-ChromiumMenuDetection
            winui_uwp         = [ordered]@{
                measurable = $false
                branch     = 'no_modern_shell_host_available_this_session'
                cites      = 'A10-7'
            }
        }
    }
}

# ---------------------------------------------------------------- leg 2: mid-walk identity race

function Invoke-CachedAgentFacingPass {
    $windowCount = 0
    $records = [AgentDesktopProbe.A23.Signals23]::SingleWalkCachedPass([ref]$windowCount)
    $raceHits = 0
    foreach ($record in $records) {
        if (-not $record.PidNonZero -or -not $record.TokenReadOk) { $raceHits++ }
    }
    return [ordered]@{ WindowCount = $windowCount; RaceHits = $raceHits; Records = $records }
}

function Measure-RaceSeries {
    param([Parameter(Mandatory = $true)][int]$Iterations, [scriptblock]$BetweenIteration = $null)
    $totalWindows = 0
    $totalRaceHits = 0
    $iterationsWithRace = 0
    $rewalkCleared = @(0, 0, 0, 0, 0)
    $rewalkUncleared = 0
    for ($i = 0; $i -lt $Iterations; $i++) {
        if ($BetweenIteration) { & $BetweenIteration }
        $pass = Invoke-CachedAgentFacingPass
        $totalWindows += $pass.WindowCount
        $totalRaceHits += $pass.RaceHits
        if ($pass.RaceHits -gt 0) {
            $iterationsWithRace++
            $cleared = $false
            for ($attempt = 1; $attempt -le 5; $attempt++) {
                $retry = Invoke-CachedAgentFacingPass
                if ($retry.RaceHits -eq 0) {
                    $rewalkCleared[$attempt - 1]++
                    $cleared = $true
                    break
                }
            }
            if (-not $cleared) { $rewalkUncleared++ }
        }
    }
    return [ordered]@{
        iterations                    = $Iterations
        windows_examined_total        = $totalWindows
        race_hits_total               = $totalRaceHits
        iterations_with_race          = $iterationsWithRace
        rewalk_cleared_by_attempt     = $rewalkCleared
        rewalk_uncleared_after_budget = $rewalkUncleared
    }
}

function Measure-MidWalkRace {
    $idle = Measure-RaceSeries -Iterations 60

    $helper = Join-Path (Get-ProbeRoot) 'scratch\lifecycle-helpers\bin\LifecycleHelpers.exe'
    if (-not (Test-Path -LiteralPath $helper)) {
        $build = Join-Path (Get-ProbeRoot) 'scratch\lifecycle-helpers\build.ps1'
        if (Test-Path -LiteralPath $build) { & $build | Out-Null }
    }
    $churn = $null
    if (Test-Path -LiteralPath $helper) {
        $churnQueue = New-Object System.Collections.ArrayList
        $churn = Measure-RaceSeries -Iterations 60 -BetweenIteration {
            $spawned = Start-Process -FilePath $helper -ArgumentList @('--mode', 'window') -PassThru
            Register-SpawnedPid -ProcessId $spawned.Id
            [void]$churnQueue.Add($spawned.Id)
            if ($churnQueue.Count -gt 2) {
                $oldest = $churnQueue[0]
                $churnQueue.RemoveAt(0)
                try { Stop-Process -Id $oldest -Force -ErrorAction SilentlyContinue } catch { }
            }
        }
        foreach ($remaining in @($churnQueue)) {
            try { Stop-Process -Id $remaining -Force -ErrorAction SilentlyContinue } catch { }
        }
    }

    return [ordered]@{
        probe                 = '23-signals-menus'
        question              = 'mid-walk owning-process identity race rate on an idle desktop and while a fixture spawns/terminates windows, and how many re-walks (budget 5, per KTD3/LISTING_RACE_ATTEMPTS) clear it'
        methodology_cites     = @('KTD3', 'window_ops.rs LISTING_RACE_ATTEMPTS')
        idle_desktop          = $idle
        churn_spawn_terminate = $(if ($null -ne $churn) { $churn } else { [ordered]@{ measurable = $false; branch = 'lifecycle_helper_unavailable' } })
    }
}

# ---------------------------------------------------------------- leg 4: durably unidentifiable windows

function Measure-DurablyUnidentifiable {
    param([int]$Repeats = 15, [int]$IntervalMs = 200)
    $seen = @{}
    for ($i = 0; $i -lt $Repeats; $i++) {
        $windowCount = 0
        $records = [AgentDesktopProbe.A23.Signals23]::SingleWalkCachedPass([ref]$windowCount)
        foreach ($record in $records) {
            $key = $record.Handle.ToInt64()
            if (-not $seen.ContainsKey($key)) { $seen[$key] = [ordered]@{ total = 0; fail = 0 } }
            $seen[$key].total = $seen[$key].total + 1
            if (-not $record.PidNonZero -or -not $record.TokenReadOk) { $seen[$key].fail = $seen[$key].fail + 1 }
        }
        Start-Sleep -Milliseconds $IntervalMs
    }
    $persistent = 0
    $intermittent = 0
    $neverFailed = 0
    foreach ($key in $seen.Keys) {
        $entry = $seen[$key]
        if ($entry.fail -eq 0) { $neverFailed++ }
        elseif ($entry.fail -eq $entry.total) { $persistent++ }
        else { $intermittent++ }
    }
    return [ordered]@{
        probe                    = '23-signals-menus'
        question                 = 'across repeated agent-facing captures, how many distinct windows fail the owning-process identity read every time versus intermittently (sizes the R3 exclusion; grounds R11)'
        environment_dependency   = 'measured while running at High integrity as a local Administrator (00-environment.ps1); token reads succeed against more processes here than on a standard-user desktop, so the persistent-failure count below is a floor, not an upper bound, for a lower-privilege caller'
        repeats                  = $Repeats
        interval_ms              = $IntervalMs
        distinct_windows_seen    = $seen.Count
        never_failed             = $neverFailed
        persistent_failures      = $persistent
        intermittent_failures    = $intermittent
    }
}

# ---------------------------------------------------------------- leg 5: modal classification

function Measure-ModalClassification {
    $exe = Join-Path (Get-ProbeRoot) 'scratch\lifecycle-helpers\bin\LifecycleHelpers.exe'
    if (-not (Test-Path -LiteralPath $exe)) {
        $build = Join-Path (Get-ProbeRoot) 'scratch\lifecycle-helpers\build.ps1'
        if (Test-Path -LiteralPath $build) { & $build | Out-Null }
    }
    $win32Result = [ordered]@{ measurable = $false; branch = 'lifecycle_helper_missing' }
    if (Test-Path -LiteralPath $exe) {
        $proc = Start-Process -FilePath $exe -ArgumentList @('--mode', 'modal') -PassThru
        Register-SpawnedPid -ProcessId $proc.Id
        Start-Sleep -Milliseconds 1200
        $dialog = [AgentDesktopProbe.A23.Signals23]::FindWindowByClassForPid($proc.Id, '#32770')
        if ($dialog -ne [IntPtr]::Zero) {
            $hasOwner = [AgentDesktopProbe.A23.Signals23]::HasOwner($dialog)
            $hasDlgModalFrame = [AgentDesktopProbe.A23.Signals23]::HasDlgModalFrame($dialog)
            $controlType = 'unread'
            $isModal = $null
            try {
                $element = [System.Windows.Automation.AutomationElement]::FromHandle($dialog)
                $controlType = $element.Current.ControlType.ProgrammaticName
                $isModal = $element.GetCurrentPropertyValue([System.Windows.Automation.WindowPattern]::IsModalProperty)
            } catch { }
            $win32Result = [ordered]@{
                measurable            = $true
                branch                = 'win32_messagebox_staged'
                owner_present          = $hasOwner
                ws_ex_dlgmodalframe    = $hasDlgModalFrame
                uia_control_type       = $controlType
                uia_window_is_modal    = $isModal
            }
            [AgentDesktopProbe.A23.Signals23]::CloseWindow($dialog)
            Start-Sleep -Milliseconds 500
        } else {
            $win32Result = [ordered]@{ measurable = $false; branch = 'dialog_window_not_found' }
        }
        try { Stop-ScratchProcess -ProcessId $proc.Id } catch { }
    }

    return [ordered]@{
        probe            = '23-signals-menus'
        question         = 'what distinguishes a modal dialog from an ordinary top-level window: owner window, WS_EX_DLGMODALFRAME, UIA ControlType, UIA WindowIsModal (grounds U6 Sheet classification)'
        win32_messagebox = $win32Result
        chromium_modal   = [ordered]@{
            measurable = $false
            branch     = 'no_generic_trigger_available_this_session'
            stack      = 'obsidian'
            note       = 'no vault-independent, app-version-independent action was found in this session that reliably opens a native modal in Obsidian; staging one would require app-specific UI knowledge outside this leg is scope'
        }
    }
}

# ---------------------------------------------------------------- leg 6: GetGUIThreadInfo reach

function Measure-GuiThreadInfoReach {
    $note = Start-Process notepad.exe -PassThru
    Register-SpawnedPid -ProcessId $note.Id
    Start-Sleep -Milliseconds 800
    $crossThreads = [AgentDesktopProbe.A23.Signals23]::ThreadsForPid($note.Id)
    $crossAnyOk = $false
    foreach ($tid in $crossThreads) {
        $result = [AgentDesktopProbe.A23.Signals23]::ReadThreadFlags($tid)
        if ($result.Ok) { $crossAnyOk = $true; break }
    }
    try { Stop-ScratchProcess -ProcessId $note.Id } catch { }

    # A bare cmd.exe with no /c argument sits at its prompt indefinitely rather
    # than running one command and exiting, so it stays alive long enough to
    # read. It never calls a user32 window API, so it is expected to carry no
    # GUI thread at all - the negative case this leg exists to ground.
    $cmdProc = Start-Process -FilePath 'cmd.exe' -WindowStyle Hidden -PassThru
    Register-SpawnedPid -ProcessId $cmdProc.Id
    Start-Sleep -Milliseconds 400
    $noGuiThreads = [AgentDesktopProbe.A23.Signals23]::ThreadsForPid($cmdProc.Id)
    $noGuiAnyOk = $false
    $noGuiFlags = $null
    foreach ($tid in $noGuiThreads) {
        $result = [AgentDesktopProbe.A23.Signals23]::ReadThreadFlags($tid)
        if ($result.Ok) { $noGuiAnyOk = $true; $noGuiFlags = $result.Flags }
    }
    try { Stop-ScratchProcess -ProcessId $cmdProc.Id } catch { }

    return [ordered]@{
        probe                         = '23-signals-menus'
        question                      = 'does GetGUIThreadInfo succeed against another process''s threads at the same integrity level, and what does it return for a process with no GUI threads'
        cross_process_same_integrity  = [ordered]@{
            threads_total    = $crossThreads.Length
            any_thread_read_ok = $crossAnyOk
        }
        no_gui_thread_process         = [ordered]@{
            threads_total       = $noGuiThreads.Length
            any_thread_read_ok  = $noGuiAnyOk
            flags_when_read_ok  = $noGuiFlags
        }
    }
}

try {
    Initialize-ProbeNative

    $menuDetection = Measure-MenuDetection
    $script:paths.menu = Write-A23Capture -Name "signals-menu-detection-$Label.json" -Content (ConvertTo-Json -InputObject $menuDetection -Depth 14)
    Register-MandatoryPass -Capture $script:paths.menu -Result $menuDetection

    $race = Measure-MidWalkRace
    $script:paths.race = Write-A23Capture -Name "signals-race-$Label.json" -Content (ConvertTo-Json -InputObject $race -Depth 10)
    Register-MandatoryPass -Capture $script:paths.race -Result $race

    $unidentifiable = Measure-DurablyUnidentifiable
    $script:paths.unidentifiable = Write-A23Capture -Name "signals-unidentifiable-$Label.json" -Content (ConvertTo-Json -InputObject $unidentifiable -Depth 8)
    Register-MandatoryPass -Capture $script:paths.unidentifiable -Result $unidentifiable

    $modal = Measure-ModalClassification
    $script:paths.modal = Write-A23Capture -Name "signals-modal-$Label.json" -Content (ConvertTo-Json -InputObject $modal -Depth 10)
    Register-MandatoryPass -Capture $script:paths.modal -Result $modal

    $guiReach = Measure-GuiThreadInfoReach
    $script:paths.guireach = Write-A23Capture -Name "signals-guithreadinfo-$Label.json" -Content (ConvertTo-Json -InputObject $guiReach -Depth 8)
    Register-MandatoryPass -Capture $script:paths.guireach -Result $guiReach

    $measureCost = Join-Path $script:ProbeDir 'measure-cost.ps1'
    & $measureCost -Label $Label
    $costPath = Join-Path $script:CaptureDir "signals-cost-$Label.json"
    if (-not (Test-Path -LiteralPath $costPath)) {
        throw "signals-cost-$Label.json was not written by measure-cost.ps1"
    }
    $costObj = Get-Content -LiteralPath $costPath -Raw | ConvertFrom-Json
    $script:paths.cost = $costPath
    Register-MandatoryPass -Capture $script:paths.cost -Result $costObj
} finally {
    Stop-AllSpawned
}

Assert-MandatoryMeasurement -Probe '23-signals-menus' -Label $Label

Write-ProbeResult -Probe '23-signals-menus' -Status 'ok' -Message 'signals and menu-detection gap probes captured' -Data @{
    menu           = if ($script:paths.menu) { Split-Path -Leaf $script:paths.menu } else { '<none>' }
    race           = if ($script:paths.race) { Split-Path -Leaf $script:paths.race } else { '<none>' }
    unidentifiable = if ($script:paths.unidentifiable) { Split-Path -Leaf $script:paths.unidentifiable } else { '<none>' }
    modal          = if ($script:paths.modal) { Split-Path -Leaf $script:paths.modal } else { '<none>' }
    guireach       = if ($script:paths.guireach) { Split-Path -Leaf $script:paths.guireach } else { '<none>' }
    cost           = if ($script:paths.cost) { Split-Path -Leaf $script:paths.cost } else { '<none>' }
}
exit 0
