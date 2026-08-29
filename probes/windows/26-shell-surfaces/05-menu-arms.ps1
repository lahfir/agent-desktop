#Requires -Version 5.1
<#
.SYNOPSIS
    Area 26 05-menu-arms.ps1 - rows A26-11 (WinUI/UWP menu-detector arm) and
    A26-12 (Chromium/Electron menu-detector arm), owned by plan unit U11.

.DESCRIPTION
    Evaluates both open arms of the two-source menu detector
    (crates/windows/src/system/menu_state.rs) against real hosts, reading the
    shipped sources DIRECTLY rather than through the product, the way
    24-fixture-e2e/08-chromium-content.ps1 reads them:

      - Source A (classic_menu_mode_active): a TH32CS_SNAPTHREAD walk filtered
        to the target pid with GetGUIThreadInfo per thread, testing
        GUI_INMENUMODE | GUI_SYSTEMMENUMODE | GUI_POPUPMENUMODE.
      - Source B (uia_menu_reachable): the target pid's root-level
        WS_EX_TOOLWINDOW windows, each resolved through ElementFromHandle and
        searched with find_first over TreeScope::Subtree under an OR of
        ControlType Menu / MenuBar / MenuItem (the exact menu-family condition
        menu_family_condition builds), pump-gated before the cross-process read.

    WINUI ARM: launches the UWP Settings host the way 04-frame-identity.ps1
    does (ms-settings: URI shell dispatch - a direct SystemSettings.exe spawn
    exits silently on this box, A26-8), CONFIRMS activation and foreground
    first, then stages a XAML context menu inside its CoreWindow if the app
    exposes any generic route (Alt tap, Shift+F10, content-area right-click).
    Each staging is evaluated through both sources, with an at-rest control
    reading, and an ESC-restore re-read. If no menu surface can be staged the
    arm records KTD9 branch B: measurable:false narrowed to WinUI3/MSIX with
    the staging attempts and host population enumerated - never a closure claim.

    CHROMIUM ARM: targets Cursor (VS Code fork = Chromium/Electron), activation
    confirmed first, then an Alt tap and a content-area right-click, each
    evaluated through both sources with an at-rest control and ESC restore.
    Also records the corrected host-population search - A24-12's needle list
    PLUS cursor - as counts of Chromium-family running executable images, never
    paths, and the Cursor subtree's Menu/MenuBar/MenuItem element counts at
    rest and after stagings (counts only).

    Run: powershell -NoProfile -ExecutionPolicy Bypass -File .\probes\windows\26-shell-surfaces\05-menu-arms.ps1 -Label <devbox|ci>
    Self-test (needles constant only, no desktop interaction):
    powershell -NoProfile -ExecutionPolicy Bypass -File .\probes\windows\26-shell-surfaces\05-menu-arms.ps1 -SelfTest
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'ci')][string]$Label = 'devbox',
    [switch]$SelfTest
)

$ErrorActionPreference = 'Stop'

$script:Probe = '26-shell-surfaces/05-menu-arms'

# The corrected host-population search constant: A24-12's needle list plus
# cursor, whose sixteen running processes the original search missed (KTD10).
$script:ChromiumNeedles = @('edge', 'chrome', 'chrome_x86', 'brave', 'teams', 'vscode', 'slack', 'cursor')

if ($SelfTest) {
    $failures = New-Object System.Collections.ArrayList
    if ($script:ChromiumNeedles -notcontains 'cursor') {
        [void]$failures.Add('needle list must include cursor - the A24-12 search that missed it is the exact correction this constant pins')
    }
    if ($script:ChromiumNeedles.Count -ne 8) {
        [void]$failures.Add(('expected the 8-needle list (A24-12 plus cursor), found {0}' -f $script:ChromiumNeedles.Count))
    }
    $duplicates = @(@($script:ChromiumNeedles | Group-Object) | Where-Object { $_.Count -gt 1 })
    if ($duplicates.Count -gt 0) {
        [void]$failures.Add(('duplicate needles: {0}' -f (($duplicates | ForEach-Object { $_.Name }) -join ',')))
    }
    foreach ($n in $script:ChromiumNeedles) {
        if ($n -notmatch '^[a-z0-9_]+$') { [void]$failures.Add(('needle is not a lowercase token: ' + $n)) }
    }
    if ($failures.Count -gt 0) {
        Write-Host ('SELFTEST FAIL: ' + ($failures -join '; '))
        exit 1
    }
    Write-Host ('SELFTEST OK: {0} needles, cursor included, no desktop interaction' -f $script:ChromiumNeedles.Count)
    exit 0
}

Set-StrictMode -Version 2.0

. (Join-Path (Split-Path -Parent $PSCommandPath) '..\common.ps1')
Initialize-ProbeRedaction
. (Join-Path (Split-Path -Parent $PSCommandPath) 'lib.ps1')

Register-MandatoryCapture -Name @("menu-arms-$Label.json")

$script:CursorPosBefore = $null
$script:FreshFrameHandles = New-Object System.Collections.ArrayList

function Invoke-MenuReadingSet {
    <# One evaluation of the shipped sources against the target pid - classic
       flags, tool-window UIA, and the third source this unit added after the
       first run measured both shipped sources silent under a demonstrably
       open Chromium DOM menu - plus the target window's own subtree
       menu-family counts (diagnostic beyond the shipped predicate, labelled
       by its keys). #>
    param([Parameter(Mandatory = $true)][int]$TargetPid, [Parameter(Mandatory = $true)][string]$SubtreeHwnd)
    $read = Invoke-ShellProbe -Arguments @('menuread', '--pid', ([string]$TargetPid))
    $sub = Invoke-ShellProbe -Arguments @('menusubtree', '--hwnd', $SubtreeHwnd)
    return [ordered]@{
        classic_source_fired                            = [bool]$read.classic_source_fired
        classic_source_thread_walk_reached_target       = [bool]$read.classic_source_thread_walk_reached_target
        classic_source_per_thread_reads_all_succeeded   = [bool]$read.classic_source_per_thread_reads_all_succeeded
        uia_source_fired                                = [bool]$read.uia_source_fired
        tool_window_candidates_present                  = [bool]$read.tool_window_candidates_present
        candidate_class_set                             = @($read.candidate_class_set)
        candidates_hung_gated_skipped                   = [int]$read.candidates_hung_gated_skipped
        chromium_source_fired                           = [bool]$read.chromium_source_fired
        chromium_source_presented_candidates_present    = [bool]$read.chromium_source_presented_candidates_present
        subtree_root_control_type                       = [string]$sub.root_control_type
        subtree_read_nonempty                           = [bool]$sub.subtree_read_nonempty
        subtree_menu_element_count                      = [int]$sub.menu_element_count
        subtree_menu_bar_element_count                  = [int]$sub.menu_bar_element_count
        subtree_menu_item_element_count                 = [int]$sub.menu_item_element_count
    }
}

function Test-ReadingBackAtRest {
    param($Baseline, $Restored)
    return (
        ([bool]$Baseline.classic_source_fired -eq [bool]$Restored.classic_source_fired) -and
        ([bool]$Baseline.uia_source_fired -eq [bool]$Restored.uia_source_fired) -and
        ([bool]$Baseline.chromium_source_fired -eq [bool]$Restored.chromium_source_fired) -and
        ([int]$Baseline.subtree_menu_element_count -eq [int]$Restored.subtree_menu_element_count) -and
        ([int]$Baseline.subtree_menu_item_element_count -eq [int]$Restored.subtree_menu_item_element_count)
    )
}

function Invoke-StagedReading {
    param(
        [Parameter(Mandatory = $true)][string]$Method,
        [Parameter(Mandatory = $true)][scriptblock]$Stage,
        [Parameter(Mandatory = $true)][int]$TargetPid,
        [Parameter(Mandatory = $true)][string]$SubtreeHwnd,
        [Parameter(Mandatory = $true)]$Baseline
    )
    & $Stage
    Start-Sleep -Milliseconds 900
    $after = Invoke-MenuReadingSet -TargetPid $TargetPid -SubtreeHwnd $SubtreeHwnd
    Invoke-ShellProbe -Arguments @('key', '--seq', 'esc') | Out-Null
    Start-Sleep -Milliseconds 500
    $restored = Invoke-MenuReadingSet -TargetPid $TargetPid -SubtreeHwnd $SubtreeHwnd
    return [ordered]@{
        method                  = $Method
        after_staging           = $after
        after_esc_restore       = $restored
        returned_to_rest        = (Test-ReadingBackAtRest -Baseline $Baseline -Restored $restored)
        menu_family_reached     = ([bool]$after.classic_source_fired -or [bool]$after.uia_source_fired -or [bool]$after.chromium_source_fired -or [int]$after.subtree_menu_element_count -gt 0 -or [int]$after.subtree_menu_item_element_count -gt 0)
    }
}

function Wait-ForForegroundHwnd {
    param([Parameter(Mandatory = $true)][string]$Hwnd, [int]$TimeoutSec = 6)
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ((Get-Date) -lt $deadline) {
        $fg = Invoke-ShellProbe -Arguments @('foregroundinfo')
        if ([bool]$fg.foreground_present -and ([string]$fg.nativewindowhandle -eq $Hwnd)) { return $fg }
        Start-Sleep -Milliseconds 400
    }
    return $null
}

function Get-ForegroundClass {
    param([Parameter(Mandatory = $true)][string[]]$ClassNames, [int]$TimeoutSec = 6)
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ((Get-Date) -lt $deadline) {
        $fg = Invoke-ShellProbe -Arguments @('foregroundinfo')
        if ([bool]$fg.foreground_present -and (@($ClassNames) -contains [string]$fg.foreground_class)) { return $fg }
        Start-Sleep -Milliseconds 400
    }
    return $null
}

function Get-FrameCandidateHandles {
    $found = New-Object System.Collections.ArrayList
    try {
        $hits = Invoke-ShellProbe -Arguments @('findbyclass', '--cls', 'ApplicationFrameWindow')
        foreach ($h in $hits.handles) { [void]$found.Add([string]$h) }
    } catch { }
    return $found.ToArray()
}

function Measure-WinuiArm {
    $leg = [ordered]@{ measurable = $false; ktd9_branch = 'not_measured' }
    if (-not (Test-Path -LiteralPath (Join-Path $env:WINDIR 'ImmersiveControlPanel\SystemSettings.exe'))) {
        $leg['ktd9_branch'] = 'system_settings_exe_not_found_at_immersive_control_panel_path'
        return $leg
    }

    $preExisting = @(Get-FrameCandidateHandles)
    $leg['preexisting_settings_frame_present'] = ($preExisting.Count -gt 0)
    $suspendedClosed = 0
    foreach ($w in $preExisting) {
        try {
            $pred = Invoke-ShellProbe -Arguments @('predicate', '--hwnd', $w)
            if ($pred.cloak_state -ne 'none' -and (@('frame_host', 'system_settings') -contains [string]$pred.host_token)) {
                Invoke-ShellProbe -Arguments @('closewindow', '--hwnd', $w) | Out-Null
                $suspendedClosed++
            }
        } catch { }
    }
    if ($suspendedClosed -gt 0) { Start-Sleep -Seconds 2 }
    $leg['suspended_frames_closed_before_launch'] = $suspendedClosed

    $preAtLaunch = @(Get-FrameCandidateHandles)
    $leg['launch_route'] = 'ms_settings_uri_shell_dispatch'
    Start-Process 'ms-settings:'

    $frameHwnd = $null
    $usedExisting = $false
    for ($i = 0; $i -lt 10 -and -not $frameHwnd; $i++) {
        Start-Sleep -Seconds 2
        $now = @(Get-FrameCandidateHandles)
        $fresh = @($now | Where-Object { $preAtLaunch -notcontains $_ })
        foreach ($w in $fresh) {
            if (-not $script:FreshFrameHandles.Contains($w)) { [void]$script:FreshFrameHandles.Add($w) }
            Invoke-ShellProbe -Arguments @('activate', '--hwnd', $w) | Out-Null
            Start-Sleep -Milliseconds 900
            if (Wait-ForForegroundHwnd -Hwnd $w -TimeoutSec 5) { $frameHwnd = $w; break }
        }
    }
    if (-not $frameHwnd) {
        foreach ($w in @(Get-FrameCandidateHandles)) {
            try {
                $pred = Invoke-ShellProbe -Arguments @('predicate', '--hwnd', $w)
                if ((@('frame_host', 'system_settings') -notcontains [string]$pred.host_token)) { continue }
            } catch { continue }
            Invoke-ShellProbe -Arguments @('activate', '--hwnd', $w) | Out-Null
            Start-Sleep -Milliseconds 900
            $fg = Wait-ForForegroundHwnd -Hwnd $w -TimeoutSec 5
            if ($fg) { $frameHwnd = $w; $usedExisting = $true; break }
        }
    }
    if (-not $frameHwnd) {
        $leg['ktd9_branch'] = 'application_frame_window_never_reached_foreground'
        return $leg
    }

    $leg['activation_confirmed'] = $true
    $leg['reused_existing_instance'] = $usedExisting
    $fgInfo = Invoke-ShellProbe -Arguments @('foregroundinfo')
    $leg['foreground_class'] = [string]$fgInfo.foreground_class
    $leg['foreground_owner_host_token'] = [string]$fgInfo.foreground_host_token

    $frameWalk = Invoke-ShellProbe -Arguments @('framewalk', '--frame', $frameHwnd)
    $coreChild = $null
    for ($i = 0; $i -lt 12 -and -not $coreChild; $i++) {
        try { $coreChild = Invoke-ShellProbe -Arguments @('childofclass', '--parent', $frameHwnd, '--childcls', 'Windows.UI.Core.CoreWindow', '--ownerdiffers', 'true') } catch { $coreChild = $null }
        if (-not $coreChild -or -not $coreChild.child_found) { $coreChild = $null; Start-Sleep -Seconds 1 }
    }
    $leg['core_window_child_present'] = [bool]($coreChild -and $coreChild.child_found)
    if (-not $coreChild -or -not $coreChild.child_found) {
        $leg['ktd9_branch'] = 'core_window_child_with_differing_owner_not_found'
        return $leg
    }
    $leg['hosted_owner_differs_from_frame'] = $true

    $coreOwner = Invoke-ShellProbe -Arguments @('ownerpidof', '--hwnd', ([string]$coreChild.nativewindowhandle))
    $hostedPid = [int]$coreOwner.pid
    $coreHwnd = [string]$coreChild.nativewindowhandle

    $leg['at_rest'] = Invoke-MenuReadingSet -TargetPid $hostedPid -SubtreeHwnd $coreHwnd

    $stagings = @()
    $stagings += Invoke-StagedReading -Method 'alt_tap' -Stage { Invoke-ShellProbe -Arguments @('key', '--seq', 'alt') | Out-Null } -TargetPid $hostedPid -SubtreeHwnd $coreHwnd -Baseline $leg['at_rest']
    $stagings += Invoke-StagedReading -Method 'shift_f10' -Stage { Invoke-ShellProbe -Arguments @('key', '--seq', 'shift_f10') | Out-Null } -TargetPid $hostedPid -SubtreeHwnd $coreHwnd -Baseline $leg['at_rest']
    $script:CursorPosBefore = Invoke-ShellProbe -Arguments @('mouse', '--action', 'cursorpos')
    $stagings += Invoke-StagedReading -Method 'right_click_center' -Stage { Invoke-ShellProbe -Arguments @('mouse', '--action', 'rightclickcenterof', '--hwnd', $coreHwnd) | Out-Null } -TargetPid $hostedPid -SubtreeHwnd $coreHwnd -Baseline $leg['at_rest']
    if ($script:CursorPosBefore) {
        Invoke-ShellProbe -Arguments @('mouse', '--action', 'move', '--x', ([string]$script:CursorPosBefore.x), '--y', ([string]$script:CursorPosBefore.y)) | Out-Null
    }
    $leg['stagings'] = $stagings
    $leg['staging_methods_attempted'] = @('alt_tap', 'shift_f10', 'right_click_center')
    $anyReached = @($stagings | Where-Object { $_.menu_family_reached }).Count -gt 0
    $leg['menu_family_reached_by_any_staging'] = $anyReached

    if ($anyReached) {
        $leg['measurable'] = $true
        $leg['ktd9_branch'] = 'A_menu_family_reached_and_both_sources_evaluated'
    } else {
        $leg['measurable'] = $false
        $leg['ktd9_branch'] = 'B_no_menu_surface_reachable_by_generic_staging'
        $leg['narrowed_to'] = 'WinUI3/MSIX'
        $leg['branch_note'] = 'the UWP CoreWindow shape was reached, activation confirmed, and all detector sources were evaluated directly against the host at rest and after each staging attempt; no XAML menu surface could be staged from an external process on this host, so the menu-open half of the arm stays unevaluated and narrows to the WinUI3/MSIX population this box does not carry'
    }
    return $leg
}

function Measure-ChromiumArm {
    $leg = [ordered]@{ measurable = $false; ktd10_branch = 'not_measured' }
    $main = Invoke-ShellProbe -Arguments @('mainwindowofimage', '--leaf', 'cursor', '--cls', 'Chrome_WidgetWin_1')
    $leg['target_present'] = [bool]$main.found
    $leg['target_top_level_class'] = [string]$main.top_level_class_matched
    if (-not $main.found) {
        $leg['ktd10_branch'] = 'cursor_top_level_window_not_found'
        $leg['population_search'] = Get-PopulationSearch
        return $leg
    }
    $leg['target_image_is_needle_leaf'] = [bool]$main.owner_image_is_needle_leaf

    $mainHwnd = [string]$main.nativewindowhandle
    $owner = Invoke-ShellProbe -Arguments @('ownerpidof', '--hwnd', $mainHwnd)
    $mainPid = [int]$owner.pid

    Invoke-ShellProbe -Arguments @('activate', '--hwnd', $mainHwnd) | Out-Null
    Start-Sleep -Milliseconds 900
    $fg = Wait-ForForegroundHwnd -Hwnd $mainHwnd -TimeoutSec 6
    $leg['activation_confirmed'] = [bool]$fg
    if (-not $fg) {
        $leg['ktd10_branch'] = 'cursor_window_never_reached_foreground'
        $leg['population_search'] = Get-PopulationSearch
        return $leg
    }
    $leg['foreground_class'] = [string]$fg.foreground_class

    $leg['population_search'] = Get-PopulationSearch
    $leg['at_rest'] = Invoke-MenuReadingSet -TargetPid $mainPid -SubtreeHwnd $mainHwnd

    $stagings = @()
    $stagings += Invoke-StagedReading -Method 'alt_tap' -Stage { Invoke-ShellProbe -Arguments @('key', '--seq', 'alt') | Out-Null } -TargetPid $mainPid -SubtreeHwnd $mainHwnd -Baseline $leg['at_rest']
    if (-not $script:CursorPosBefore) {
        $script:CursorPosBefore = Invoke-ShellProbe -Arguments @('mouse', '--action', 'cursorpos')
    }
    $stagings += Invoke-StagedReading -Method 'right_click_center' -Stage { Invoke-ShellProbe -Arguments @('mouse', '--action', 'rightclickcenterof', '--hwnd', $mainHwnd) | Out-Null } -TargetPid $mainPid -SubtreeHwnd $mainHwnd -Baseline $leg['at_rest']
    if ($script:CursorPosBefore) {
        Invoke-ShellProbe -Arguments @('mouse', '--action', 'move', '--x', ([string]$script:CursorPosBefore.x), '--y', ([string]$script:CursorPosBefore.y)) | Out-Null
    }
    $leg['stagings'] = $stagings
    $leg['staging_methods_attempted'] = @('alt_tap', 'right_click_center')

    $firedClassic = (@($stagings | Where-Object { $_.after_staging.classic_source_fired }).Count -gt 0)
    $firedUia = (@($stagings | Where-Object { $_.after_staging.uia_source_fired }).Count -gt 0)
    $firedChromium = (@($stagings | Where-Object { $_.after_staging.chromium_source_fired }).Count -gt 0)
    $menuFamilySeen = (@($stagings | Where-Object { $_.menu_family_reached }).Count -gt 0)
    $leg['classic_source_fired_by_any_staging'] = $firedClassic
    $leg['uia_source_fired_by_any_staging'] = $firedUia
    $leg['chromium_source_fired_by_any_staging'] = $firedChromium
    $leg['menu_family_reached_by_any_staging'] = $menuFamilySeen

    if ($firedClassic -or $firedUia -or $firedChromium -or $menuFamilySeen) {
        $leg['measurable'] = $true
        $leg['ktd10_branch'] = 'A_menu_staged_and_sources_evaluated'
        if ($firedChromium) {
            if ($firedClassic -or $firedUia) { $leg['source_that_fired'] = 'chromium_and_other' }
            else { $leg['source_that_fired'] = 'chromium_only' }
        }
        elseif ($firedClassic -and $firedUia) { $leg['source_that_fired'] = 'classic_and_uia' }
        elseif ($firedClassic) { $leg['source_that_fired'] = 'classic' }
        elseif ($firedUia) { $leg['source_that_fired'] = 'uia' }
        else { $leg['source_that_fired'] = 'menu_family_subtree_only' }
    } else {
        $leg['measurable'] = $false
        $leg['ktd10_branch'] = 'B_no_menu_staged_measurable_false_with_search'
    }
    return $leg
}

function Get-PopulationSearch {
    $scan = Invoke-ShellProbe -Arguments @('imagesscan', '--needles', ($script:ChromiumNeedles -join ','))
    $installRows = @()
    $installHits = 0
    $pf = $env:ProgramFiles
    $pfx86 = ${env:ProgramFiles(x86)}
    $lad = $env:LOCALAPPDATA
    $locations = @{
        'edge'       = (Join-Path $pf 'Microsoft\Edge\Application\msedge.exe')
        'chrome'     = (Join-Path $pf 'Google\Chrome\Application\chrome.exe')
        'chrome_x86' = (Join-Path $pfx86 'Google\Chrome\Application\chrome.exe')
        'brave'      = (Join-Path $pf 'BraveSoftware\Brave-Browser\Application\brave.exe')
        'teams'      = (Join-Path $lad 'Microsoft\Teams\current\Teams.exe')
        'vscode'     = (Join-Path $lad 'Programs\Microsoft VS Code\Code.exe')
        'slack'      = (Join-Path $lad 'slack\slack.exe')
        'cursor'     = (Join-Path $lad 'Programs\cursor\Cursor.exe')
    }
    foreach ($needle in ($script:ChromiumNeedles | Sort-Object)) {
        $present = $false
        if ($locations.ContainsKey($needle) -and $locations[$needle] -and (Test-Path -LiteralPath $locations[$needle] -PathType Leaf)) { $present = $true; $installHits++ }
        $installRows += [ordered]@{ needle = $needle; installed_location_present = $present }
    }
    return [ordered]@{
        scan_mode                          = [string]$scan.scan_mode
        needles_searched                   = @($scan.needles_searched)
        matched_needles                    = @($scan.matched_needles)
        matched_processes_total            = [int]$scan.matched_processes_total
        needles_hit_count                  = [int]$scan.needles_hit_count
        hits_by_needle                     = @($scan.hits_by_needle)
        install_location_hits_total        = $installHits
        install_location_hits_by_needle    = $installRows
        chromium_needles_includes_cursor   = ($script:ChromiumNeedles -contains 'cursor')
        needle_count                       = $script:ChromiumNeedles.Count
        counting_note                      = 'counts and flags only; no paths, no window titles, no pids'
    }
}

$winuiLeg = [ordered]@{ measurable = $false; ktd9_branch = 'not_measured' }
$chromiumLeg = [ordered]@{ measurable = $false; ktd10_branch = 'not_measured' }
$status = 'ok'
$message = 'menu-detector arms captured'

try {
    Initialize-ShellProbe | Out-Null
    $winuiLeg = Measure-WinuiArm
    $chromiumLeg = Measure-ChromiumArm
} catch {
    $status = 'fail'
    $message = $_.Exception.Message -replace '[\r\n]+', ' '
} finally {
    try {
        if ($script:CursorPosBefore) {
            Invoke-ShellProbe -Arguments @('mouse', '--action', 'move', '--x', ([string]$script:CursorPosBefore.x), '--y', ([string]$script:CursorPosBefore.y)) | Out-Null
        }
    } catch { }
    for ($i = 0; $i -lt 3; $i++) {
        try { Invoke-ShellProbe -Arguments @('key', '--seq', 'esc') | Out-Null } catch { }
        Start-Sleep -Milliseconds 300
    }
}

$content = ConvertTo-Json -InputObject ([ordered]@{
        probe         = $script:Probe
        question      = 'which source of the two-source menu detector fires, if any, on a real UWP Settings host and on a real Chromium/Electron host under generic staging, with the corrected Chromium host population (A24-12 needles plus cursor) counted as running executable images'
        cites         = @('KTD9', 'KTD10', 'A24-12')
        label         = $Label
        client_stack  = 'uia3-com'
        winui_arm     = $winuiLeg
        chromium_arm  = $chromiumLeg
    }) -Depth 24

if ($status -ne 'ok') {
    $placeholder = New-NotMeasuredResult -Reason $message
    $content = ConvertTo-Json -InputObject ([ordered]@{
            probe         = $script:Probe
            label         = $Label
            not_measured  = $placeholder.not_measured
            skipped       = $placeholder.skipped
            partial_facts = ([ordered]@{ winui_arm = $winuiLeg; chromium_arm = $chromiumLeg })
        }) -Depth 14
}

try {
    $capturePath = Write-Shell26Capture -Name "menu-arms-$Label.json" -Content $content
    Register-MandatoryPass -Capture $capturePath -Result @{ measurable_placeholder_written = $false; status_ok = ($status -eq 'ok'); not_measured = ($status -ne 'ok') }
} catch {
    $status = 'fail'
    $message = ('capture write failed: ' + $_.Exception.Message)
}

Write-ProbeResult -Probe $script:Probe -Status $status -Message $message -Data @{
    capture   = "captures/menu-arms-$Label.json"
    rows      = @('A26-11', 'A26-12')
    stack     = 'uia3-com'
}
if ($status -eq 'fail') { exit 1 }

Assert-MandatoryMeasurement -Probe $script:Probe -Label $Label
exit 0
