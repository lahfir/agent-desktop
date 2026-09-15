#Requires -Version 5.1
<#
.SYNOPSIS
    Chromium/Electron target discovery and live measurement probe (area 24,
    sub-phase 2.12).

.DESCRIPTION
    §2.12 inherits two open questions that every earlier probe deferred
    because they need a live, session-interactive Chromium/Electron target:

      A17-8 successor - phases.md states the tree is wholly present once a
      Chromium/Electron window is restored and wholly absent while it is
      minimized (a restore/minimize effect, not a settle-time effect: A17-8
      itself only ever measured a first-contact shell). This probe controls
      window state directly - minimized, then restored-but-not-activated -
      and reports node count and max depth for each, against the same hwnd,
      after one fixed content-settle wait taken in the window's default
      (restored) state so settle time cannot be confused with the
      restore/minimize effect under test.

      A23-3 successor - no corpus row stages a menu surface on a live
      Chromium/Electron target (A23-3 tried Obsidian with no vault and found
      no generically-reachable surface). This probe stages an app menu via a
      real Alt keystroke (SendInput, not PostMessage - A6/Engineering
      Invariant #5) and a context menu via a real right-click on the
      window's own content area, reporting a branch per attempt.

    Before either measurement, this probe answers a corpus gap of its own:
    every existing probe that reaches for a Chromium/Electron target hardcodes
    Obsidian's known LOCALAPPDATA path and stops there. This probe searches
    properly - both Program Files trees, the per-user AppData\Local install
    location, the HKLM/HKCU App Paths registry key (native and Wow6432Node
    views), a known-Electron-app fallback path, and a bounded recursive scan
    for the app.asar/electron.asar packaging signature under
    LOCALAPPDATA\Programs and both Program Files trees - and reports every
    Chromium-family target found as a redacted shape: product family and
    version only. No install path is ever placed in the capture, by
    instruction; discovery internally resolves real paths to launch the
    target, but every returned row carries a symbolic `source` category
    (e.g. `program_files`, `app_paths_registry`) instead.

    If a real browser (Edge/Chrome/Brave/Vivaldi/Opera) is found, the
    measured target is that browser launched against a repo-authored
    `data:` URL (many list items and buttons) under a throwaway
    `--user-data-dir` scratch profile, so the "real web content" half of
    A17-8's successor is answered with content this probe owns, not a
    website. If only a generic Electron app is found, it is launched under
    the same throwaway `--user-data-dir` and measured as-is; the run is
    recorded as `generic_electron_app_default_content` so a reader knows
    "content" there is not repo-controlled web content.

    Captures under captures\ as chromium-target-{devbox,ci}.json (+
    .normalized twin). Corpus safety: node counts, depths, booleans and
    symbolic category/branch strings only - no window titles, file paths,
    pids, machine names, user names or message text ever reach the capture.
    Every process this probe spawns (diffed against the pre-launch process
    list for the target's own exe name, so a browser's multi-process tree is
    fully captured without touching a pre-existing user session of the same
    app) and every scratch profile directory it creates is removed on every
    exit path.
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'ci')][string]$Label = 'devbox'
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

$script:Probe = '24-fixture-e2e-03-chromium-target'
$script:ProbeDir = Split-Path -Parent $PSCommandPath
$script:CaptureDir = Join-Path $script:ProbeDir 'captures'
if (-not (Test-Path -LiteralPath $script:CaptureDir)) {
    New-Item -ItemType Directory -Path $script:CaptureDir -Force | Out-Null
}
$script:Spawned = New-Object System.Collections.ArrayList

Register-MandatoryCapture -Name @("chromium-target-$Label.json")

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

function Register-SpawnedPid {
    param([int]$ProcessId)
    if ($ProcessId -gt 0 -and -not $script:Spawned.Contains($ProcessId)) { [void]$script:Spawned.Add($ProcessId) }
}

function Stop-AllSpawned {
    foreach ($id in @($script:Spawned)) {
        try { Stop-ScratchProcess -ProcessId $id } catch { }
    }
    $script:Spawned.Clear()
}

# ------------------------------------------------------------ target discovery

function Get-ExeVersionInfo {
    param([Parameter(Mandatory = $true)][string]$Path)
    try {
        $vi = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($Path)
        return [ordered]@{
            product_name      = [string]$vi.ProductName
            product_version   = [string]$vi.ProductVersion
            file_description  = [string]$vi.FileDescription
            company_name      = [string]$vi.CompanyName
        }
    } catch {
        return $null
    }
}

<#
    A packaged Electron app carries its app code and/or the Electron
    framework's own default assets as an .asar archive directly under
    `resources\` next to the main exe. `app.asar` (the packed app) is the
    common case; `electron.asar` appears on some builds. Either is treated
    as a positive signature - this is a discovery heuristic, not a security
    boundary, so a false negative just means a real Electron app is reported
    under the weaker `Chromium-family:<ProductName>` branch below rather
    than not found at all.
#>
function Test-ElectronSignature {
    param([Parameter(Mandatory = $true)][string]$ExePath)
    $dir = Split-Path -Parent $ExePath
    foreach ($leaf in @('app.asar', 'electron.asar')) {
        if (Test-Path -LiteralPath (Join-Path $dir ('resources\' + $leaf))) { return $true }
    }
    return $false
}

function Get-ChromiumFamily {
    param(
        [Parameter(Mandatory = $true)][string]$ExePath,
        [AllowNull()]$VersionInfo,
        [Parameter(Mandatory = $true)][bool]$ElectronSignature
    )
    $leaf = (Split-Path -Leaf $ExePath).ToLowerInvariant()
    switch ($leaf) {
        'msedge.exe' { return 'Microsoft Edge' }
        'chrome.exe' { return 'Google Chrome' }
        'brave.exe' { return 'Brave' }
        'vivaldi.exe' { return 'Vivaldi' }
        'opera.exe' { return 'Opera' }
    }
    if ($ElectronSignature) {
        $name = $null
        if ($null -ne $VersionInfo -and $VersionInfo.product_name) { $name = $VersionInfo.product_name }
        if (-not $name) { $name = (Split-Path -Leaf (Split-Path -Parent $ExePath)) }
        return ('Electron:' + $name)
    }
    if ($null -ne $VersionInfo -and $VersionInfo.product_name -and
        ($VersionInfo.product_name -match 'Chromium|Electron|Chrome|Edge')) {
        return ('Chromium-family:' + $VersionInfo.product_name)
    }
    return $null
}

function Test-BrowserLike {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Family)
    return @('Microsoft Edge', 'Google Chrome', 'Brave', 'Vivaldi', 'Opera') -contains $Family
}

function Add-CandidateExe {
    param($Map, [AllowNull()][string]$Path, [Parameter(Mandatory = $true)][string]$Source)
    if ([string]::IsNullOrWhiteSpace($Path)) { return }
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { return }
    $resolved = (Resolve-Path -LiteralPath $Path).ProviderPath
    $key = $resolved.ToLowerInvariant()
    if ($Map.Contains($key)) { return }
    $Map[$key] = [ordered]@{ Path = $resolved; Source = $Source }
}

<#
    App Paths values may or may not be quoted; PowerShell's registry
    provider returns the default value as a bare string in either case, so
    trimming a leading/trailing quote is the only normalization needed
    before checking whether it is a rooted path at all.
#>
function Get-AppPathsCandidates {
    param($Map, [Parameter(Mandatory = $true)][string]$RootPath, [Parameter(Mandatory = $true)][string]$RootLabel)
    $examined = [ordered]@{ root = $RootLabel; exists = $false; subkeys_examined = 0 }
    if (-not (Test-Path -LiteralPath $RootPath)) { return $examined }
    $examined.exists = $true
    $subkeys = @(Get-ChildItem -LiteralPath $RootPath -ErrorAction SilentlyContinue)
    $examined.subkeys_examined = $subkeys.Count
    foreach ($sub in $subkeys) {
        $default = $null
        try { $default = (Get-ItemProperty -LiteralPath $sub.PSPath -ErrorAction SilentlyContinue).'(default)' } catch { }
        if ([string]::IsNullOrWhiteSpace($default)) { continue }
        $exePath = ([string]$default).Trim().Trim('"')
        if (-not [System.IO.Path]::IsPathRooted($exePath)) { continue }
        Add-CandidateExe -Map $Map -Path $exePath -Source 'app_paths_registry'
    }
    return $examined
}

<#
    A bounded recursive scan for the Electron packaging signature, capped at
    a fixed depth so a large Program Files tree cannot turn this probe into
    a full-disk walk. `resources\app.asar` sits one level below the app
    directory that holds the exe, so the exe search looks at the asar's
    grandparent (the common single-level layout every Electron app in this
    corpus so far uses) as well as its parent, and filters out the
    installer/updater/crash-handler helper binaries every Electron app ships
    alongside its real one.
#>
function Find-ElectronAsarExes {
    param($Map, [Parameter(Mandatory = $true)][string]$ScanRoot, [int]$MaxDepth = 4)
    $performed = [ordered]@{ root = 'redacted_root'; attempted = $false; asar_hits = 0 }
    if (-not (Test-Path -LiteralPath $ScanRoot)) { return $performed }
    $performed.attempted = $true
    $hits = New-Object System.Collections.ArrayList
    foreach ($leaf in @('app.asar', 'electron.asar')) {
        try {
            foreach ($f in (Get-ChildItem -LiteralPath $ScanRoot -Recurse -Depth $MaxDepth -Filter $leaf -ErrorAction SilentlyContinue -Force)) {
                [void]$hits.Add($f)
            }
        } catch { }
    }
    $performed.asar_hits = $hits.Count
    $helperPattern = 'unins|uninstall|crashpad|updater|update\.exe|elevate|squirrel'
    foreach ($asar in $hits) {
        $resourcesDir = $asar.DirectoryName
        $appDirs = @($resourcesDir, (Split-Path -Parent $resourcesDir)) | Where-Object { $_ -and (Test-Path -LiteralPath $_) } | Select-Object -Unique
        foreach ($appDir in $appDirs) {
            $exeCandidates = @()
            try { $exeCandidates = Get-ChildItem -LiteralPath $appDir -Filter '*.exe' -ErrorAction SilentlyContinue -Force } catch { }
            foreach ($exeCandidate in $exeCandidates) {
                if ($exeCandidate.Name -match $helperPattern) { continue }
                Add-CandidateExe -Map $Map -Path $exeCandidate.FullName -Source 'electron_asar_scan'
            }
        }
    }
    return $performed
}

<#
    Every prior probe that reaches for a live Electron target hardcodes this
    exact path (04-automationid-census.ps1, 11-electron-activation.ps1,
    23-signals-menus/probe.ps1). Kept as an explicit fallback alongside the
    generic asar scan above so a directory-layout assumption in the scan
    heuristic cannot silently lose the one Electron app this corpus already
    depends on elsewhere.
#>
function Get-KnownElectronAppCandidates {
    param($Map)
    Add-CandidateExe -Map $Map -Path (Join-Path $env:LOCALAPPDATA 'Programs\Obsidian\Obsidian.exe') -Source 'known_electron_app'
}

function Find-ChromiumFamilyTargets {
    $candidates = [ordered]@{}

    $standardEntries = New-Object System.Collections.ArrayList
    function Add-Standard {
        param([AllowNull()][string]$Base, [string]$Suffix, [string]$Source)
        if ([string]::IsNullOrWhiteSpace($Base)) { return }
        [void]$standardEntries.Add(@{ Path = (Join-Path $Base $Suffix); Source = $Source })
    }
    $pf = $env:ProgramFiles
    $pfx86 = ${env:ProgramFiles(x86)}
    $lad = $env:LOCALAPPDATA
    Add-Standard -Base $pf -Suffix 'Microsoft\Edge\Application\msedge.exe' -Source 'program_files'
    Add-Standard -Base $pfx86 -Suffix 'Microsoft\Edge\Application\msedge.exe' -Source 'program_files_x86'
    Add-Standard -Base $lad -Suffix 'Microsoft\Edge\Application\msedge.exe' -Source 'localappdata_direct'
    Add-Standard -Base $pf -Suffix 'Google\Chrome\Application\chrome.exe' -Source 'program_files'
    Add-Standard -Base $pfx86 -Suffix 'Google\Chrome\Application\chrome.exe' -Source 'program_files_x86'
    Add-Standard -Base $lad -Suffix 'Google\Chrome\Application\chrome.exe' -Source 'localappdata_direct'
    Add-Standard -Base $pf -Suffix 'BraveSoftware\Brave-Browser\Application\brave.exe' -Source 'program_files'
    Add-Standard -Base $pfx86 -Suffix 'BraveSoftware\Brave-Browser\Application\brave.exe' -Source 'program_files_x86'
    Add-Standard -Base $lad -Suffix 'BraveSoftware\Brave-Browser\Application\brave.exe' -Source 'localappdata_direct'
    Add-Standard -Base $pf -Suffix 'Vivaldi\Application\vivaldi.exe' -Source 'program_files'
    Add-Standard -Base $lad -Suffix 'Vivaldi\Application\vivaldi.exe' -Source 'localappdata_direct'
    foreach ($entry in $standardEntries) { Add-CandidateExe -Map $candidates -Path $entry.Path -Source $entry.Source }

    $registryExamined = New-Object System.Collections.ArrayList
    [void]$registryExamined.Add((Get-AppPathsCandidates -Map $candidates -RootPath 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths' -RootLabel 'HKLM'))
    [void]$registryExamined.Add((Get-AppPathsCandidates -Map $candidates -RootPath 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\App Paths' -RootLabel 'HKLM_WOW6432Node'))
    [void]$registryExamined.Add((Get-AppPathsCandidates -Map $candidates -RootPath 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths' -RootLabel 'HKCU'))

    Get-KnownElectronAppCandidates -Map $candidates

    $scanPerformed = New-Object System.Collections.ArrayList
    if (-not [string]::IsNullOrWhiteSpace($lad)) {
        $r = Find-ElectronAsarExes -Map $candidates -ScanRoot (Join-Path $lad 'Programs')
        $r.root = 'localappdata_programs'
        [void]$scanPerformed.Add($r)
    }
    if (-not [string]::IsNullOrWhiteSpace($pf)) {
        $r = Find-ElectronAsarExes -Map $candidates -ScanRoot $pf -MaxDepth 3
        $r.root = 'program_files'
        [void]$scanPerformed.Add($r)
    }
    if (-not [string]::IsNullOrWhiteSpace($pfx86)) {
        $r = Find-ElectronAsarExes -Map $candidates -ScanRoot $pfx86 -MaxDepth 3
        $r.root = 'program_files_x86'
        [void]$scanPerformed.Add($r)
    }

    $rows = New-Object System.Collections.ArrayList
    foreach ($key in $candidates.Keys) {
        $entry = $candidates[$key]
        $versionInfo = Get-ExeVersionInfo -Path $entry.Path
        $electronSig = Test-ElectronSignature -ExePath $entry.Path
        $family = Get-ChromiumFamily -ExePath $entry.Path -VersionInfo $versionInfo -ElectronSignature $electronSig
        if (-not $family) { continue }
        [void]$rows.Add([ordered]@{
                family             = $family
                product_version    = if ($versionInfo) { $versionInfo.product_version } else { $null }
                electron_signature = $electronSig
                source             = $entry.Source
                is_browser_like    = (Test-BrowserLike -Family $family)
                path_internal_only = $entry.Path
            })
    }

    return [ordered]@{
        rows                    = @($rows)
        standard_locations_checked = @($standardEntries | ForEach-Object { $_.Source }) | Select-Object -Unique
        registry_app_paths_examined = @($registryExamined)
        electron_asar_scan_performed = @($scanPerformed | ForEach-Object { [ordered]@{ root = $_.root; attempted = $_.attempted; asar_hits = $_.asar_hits } })
        known_electron_app_checked = $true
    }
}

# ------------------------------------------------------------ leg a: window-state tree effect

<#
    node_count/max_depth over a ControlView walk from the window's own root
    element - the same view every other probe in this corpus (01, 04, 23)
    already measures Electron/Chromium trees with. Capped so a genuinely
    deep content tree cannot turn this probe into an unbounded walk; the
    cap is far above the ~18-node shell this leg exists to distinguish from
    real content, so hitting it is itself evidence of a populated tree.
#>
function Get-BoundedControlViewWalk {
    param($Root, [int]$MaxNodes = 4000, [int]$MaxDepth = 40)
    $walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker
    $visited = 0
    $maxDepthSeen = 0
    $truncated = $false
    $stack = New-Object System.Collections.Stack
    $stack.Push(@{ Element = $Root; Depth = 0 })
    while ($stack.Count -gt 0) {
        if ($visited -ge $MaxNodes) { $truncated = $true; break }
        $item = $stack.Pop()
        $element = $item.Element
        $cur = $null
        try { $cur = $element.Current } catch { continue }
        if ($null -eq $cur) { continue }
        $visited++
        if ($item.Depth -gt $maxDepthSeen) { $maxDepthSeen = $item.Depth }
        if ($item.Depth -ge $MaxDepth) { continue }
        $kids = New-Object System.Collections.ArrayList
        try {
            $k = $walker.GetFirstChild($element)
            while ($null -ne $k) { [void]$kids.Add($k); $k = $walker.GetNextSibling($k) }
        } catch { }
        for ($i = $kids.Count - 1; $i -ge 0; $i--) {
            $stack.Push(@{ Element = $kids[$i]; Depth = ($item.Depth + 1) })
        }
    }
    return [ordered]@{ measurable = $true; branch = 'walk_completed'; view = 'ControlView'; node_count = $visited; max_depth = $maxDepthSeen; truncated = $truncated }
}

function Get-WindowStateWalk {
    param([Parameter(Mandatory = $true)][IntPtr]$Hwnd, [Parameter(Mandatory = $true)][int]$ShowCmd, [Parameter(Mandatory = $true)][string]$StateLabel)
    [void][AgentDesktopProbe.Native]::ShowWindow($Hwnd, $ShowCmd)
    Start-Sleep -Milliseconds 900
    try {
        $root = [System.Windows.Automation.AutomationElement]::FromHandle($Hwnd)
        $walk = Get-BoundedControlViewWalk -Root $root
        $walk.state = $StateLabel
        return $walk
    } catch {
        return [ordered]@{ measurable = $false; branch = 'uia_from_handle_threw'; state = $StateLabel; error_class = $_.Exception.GetType().Name }
    }
}

# ------------------------------------------------------------ leg b: menu staging

<#
    Mirrors 23-signals-menus's three-source menu signal (classic
    GUITHREADINFO menu-mode flags, the #32768 popup class, UIA
    Menu/MenuBar/MenuItem reachability) via the Signals23 native helper this
    probe dot-sources, so a positive here means the same thing a positive
    means everywhere else in the corpus.
#>
function Get-MenuSignalSample {
    param([Parameter(Mandatory = $true)][int]$ProcessId)
    $threadsRead = 0
    $threadsTotal = 0
    $classicAny = [AgentDesktopProbe.A23.Signals23]::AnyThreadInMenuMode($ProcessId, [ref]$threadsRead, [ref]$threadsTotal)

    $popups = [AgentDesktopProbe.A23.Signals23]::ClassPopupsForPid($ProcessId)
    $popupPresent = ($popups.Count -gt 0)
    $popupPassesFilter = $false
    foreach ($p in $popups) { if ($p.PassesFilter) { $popupPassesFilter = $true } }

    $uiaPresent = $false
    try {
        $root = [System.Windows.Automation.AutomationElement]::RootElement
        $cond = New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::ProcessIdProperty, $ProcessId)
        $children = $root.FindAll([System.Windows.Automation.TreeScope]::Children, $cond)
        $menuFamily = New-Object System.Windows.Automation.OrCondition(@(
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
        foreach ($c in $children) {
            if ($c.Current.ControlType -eq [System.Windows.Automation.ControlType]::Menu -or
                $c.Current.ControlType -eq [System.Windows.Automation.ControlType]::MenuBar) { $uiaPresent = $true }
            try {
                $descendant = $c.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $menuFamily)
                if ($null -ne $descendant) { $uiaPresent = $true }
            } catch { }
        }
    } catch { }

    return [ordered]@{
        classic_any_thread_menu_mode = $classicAny
        class_32768_popup_present    = $popupPresent
        class_32768_passes_filter    = $popupPassesFilter
        uia_menu_family_present      = $uiaPresent
        any_source_fired             = ($classicAny -or $popupPresent -or $uiaPresent)
    }
}

function Invoke-EscapeClose {
    [AgentDesktopProbe.A23.Signals23]::KeyChord(@(0x1B))
    Start-Sleep -Milliseconds 400
}

function Measure-MenuStaging {
    param([Parameter(Mandatory = $true)][int]$ProcessId, [Parameter(Mandatory = $true)][IntPtr]$Hwnd)

    $activated = [AgentDesktopProbe.A23.Signals23]::ActivateOnce($Hwnd)
    Start-Sleep -Milliseconds 400
    $foregroundConfirmed = [AgentDesktopProbe.A23.Signals23]::ForegroundIs($Hwnd)

    $idle = Get-MenuSignalSample -ProcessId $ProcessId
    $attempts = New-Object System.Collections.ArrayList

    [AgentDesktopProbe.A23.Signals23]::TapAlt()
    Start-Sleep -Milliseconds 700
    $afterAlt = Get-MenuSignalSample -ProcessId $ProcessId
    $altBranch = if ($afterAlt.any_source_fired) { 'app_menu_via_alt_tap_staged' } else { 'app_menu_via_alt_tap_no_signal' }
    [void]$attempts.Add([ordered]@{ method = 'app_menu_alt_tap'; branch = $altBranch; result = $afterAlt })
    Invoke-EscapeClose

    $rect = [AgentDesktopProbe.A23.Signals23]::WindowRect($Hwnd)
    $cx = [int](($rect.Left + $rect.Right) / 2)
    $cy = [int](($rect.Top + $rect.Bottom) / 2)
    [AgentDesktopProbe.A23.Signals23]::RightClickAt($cx, $cy)
    Start-Sleep -Milliseconds 700
    $afterRightClick = Get-MenuSignalSample -ProcessId $ProcessId
    $rcBranch = if ($afterRightClick.any_source_fired) { 'context_menu_via_right_click_content_area_staged' } else { 'context_menu_via_right_click_content_area_no_signal' }
    [void]$attempts.Add([ordered]@{ method = 'context_menu_right_click_content_area'; branch = $rcBranch; result = $afterRightClick })
    Invoke-EscapeClose

    $anyStaged = ($afterAlt.any_source_fired -or $afterRightClick.any_source_fired)

    return [ordered]@{
        measurable              = $true
        branch                  = if ($anyStaged) { 'at_least_one_menu_surface_staged' } else { 'no_menu_surface_staged_by_either_method' }
        cites                   = 'A23-3'
        activation               = [ordered]@{ activate_once_reported = $activated; foreground_confirmed = $foregroundConfirmed }
        idle_before_any_attempt = $idle
        attempts                = @($attempts)
    }
}

# ------------------------------------------------------------ combined launch + measure

<#
    One launch serves both legs: leg a (window-state tree effect) needs the
    window minimized then restored-not-activated, and leg b (menu staging)
    needs it activated - running both against the same process avoids a
    second multi-process browser launch and a second cleanup pass.

    `--user-data-dir` under a throwaway scratch directory is the isolation
    mechanism: without it, launching an already-running single-instance
    browser just forwards the command line to the existing process and exits
    the new one immediately, which would stage everything below inside the
    operator's real browser session instead of a scratch one. With a fresh
    profile directory the browser can never be an already-running instance,
    so every process this leg spawns is provably its own.
#>
function Measure-ChromiumTarget {
    param(
        [Parameter(Mandatory = $true)][string]$ExePath,
        [Parameter(Mandatory = $true)][string]$Family,
        [Parameter(Mandatory = $true)][string]$TargetKind
    )

    $scratchProfileDir = Join-Path $env:TEMP ('agent-desktop-probe24-chromium-profile-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $scratchProfileDir -Force | Out-Null
    $exeLeaf = [System.IO.Path]::GetFileNameWithoutExtension($ExePath)

    try {
        $baseline = @(Get-Process -Name $exeLeaf -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)

        $launchArgs = @(('--user-data-dir=' + $scratchProfileDir), '--no-first-run', '--no-default-browser-check')
        $contentStaged = $false
        if ($TargetKind -eq 'browser_with_controlled_content') {
            $items = 1..40 | ForEach-Object { '<li>probe24 row ' + $_ + ' <button>b' + $_ + '</button></li>' }
            $html = '<html><body><h1>agent-desktop probe24 content</h1><ul>' + ($items -join '') + '</ul></body></html>'
            $dataUrl = 'data:text/html,' + [Uri]::EscapeDataString($html)
            $launchArgs += @('--new-window', $dataUrl)
            $contentStaged = $true
        }

        try {
            $launched = Start-Process -FilePath $ExePath -ArgumentList $launchArgs -PassThru
        } catch {
            return [ordered]@{ measurable = $false; branch = 'launch_threw'; error_class = $_.Exception.GetType().Name }
        }
        Register-SpawnedPid -ProcessId $launched.Id

        $hwnd = [IntPtr]::Zero
        $ownerPid = 0
        $ours = @()
        $findDeadline = (Get-Date).AddSeconds(15)
        while ((Get-Date) -lt $findDeadline -and $hwnd -eq [IntPtr]::Zero) {
            $afterAll = @(Get-Process -Name $exeLeaf -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
            $ours = @($afterAll | Where-Object { $baseline -notcontains $_ })
            foreach ($id in $ours) {
                $candidate = [AgentDesktopProbe.A23.Signals23]::FindVisibleTitledTopLevelForPid($id)
                if ($candidate -ne [IntPtr]::Zero) { $hwnd = $candidate; $ownerPid = $id; break }
            }
            if ($hwnd -eq [IntPtr]::Zero) { Start-Sleep -Milliseconds 500 }
        }
        foreach ($id in $ours) { Register-SpawnedPid -ProcessId $id }

        if ($hwnd -eq [IntPtr]::Zero) {
            return [ordered]@{ measurable = $false; branch = 'no_top_level_window_found_after_settle'; own_processes_spawned = $ours.Count }
        }

        # Fixed content-settle wait taken in the window's default (restored)
        # state, before this probe forces any window-state transition, so
        # settle time and the restore/minimize effect under test cannot be
        # confounded (01-tree-dump.ps1's 8s Electron settle finding).
        Start-Sleep -Milliseconds 8000

        $minimizedWalk = Get-WindowStateWalk -Hwnd $hwnd -ShowCmd 7 -StateLabel 'minimized' # SW_SHOWMINNOACTIVE
        $restoredWalk = Get-WindowStateWalk -Hwnd $hwnd -ShowCmd 4 -StateLabel 'restored_not_activated' # SW_SHOWNOACTIVATE
        $foregroundAfterRestoreNotActivate = [AgentDesktopProbe.A23.Signals23]::ForegroundIs($hwnd)

        $windowStateEffect = [ordered]@{
            measurable                           = $true
            branch                                = 'both_states_measured'
            cites                                  = 'A17-8'
            content_staged                         = $contentStaged
            settle_ms_before_first_walk            = 8000
            foreground_after_restore_not_activate  = $foregroundAfterRestoreNotActivate
            minimized                              = $minimizedWalk
            restored_not_activated                 = $restoredWalk
        }
        if ($minimizedWalk.Contains('node_count') -and $restoredWalk.Contains('node_count')) {
            $windowStateEffect.node_count_delta = ($restoredWalk.node_count - $minimizedWalk.node_count)
            $windowStateEffect.restored_reaches_deep_content = (($restoredWalk.node_count -gt 30) -and ($restoredWalk.node_count -gt ($minimizedWalk.node_count * 2)))
        }

        $menuStaging = Measure-MenuStaging -ProcessId $ownerPid -Hwnd $hwnd

        return [ordered]@{
            measurable                = $true
            branch                     = 'target_launched_and_measured'
            owner_pid_is_launcher_pid  = ($ownerPid -eq $launched.Id)
            own_processes_spawned      = $ours.Count
            window_state_effect        = $windowStateEffect
            menu_staging               = $menuStaging
        }
    } finally {
        Stop-AllSpawned
        if (Test-Path -LiteralPath $scratchProfileDir) {
            Remove-Item -LiteralPath $scratchProfileDir -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
}

# ------------------------------------------------------------ main

$question = 'is there a Chromium/Electron target on this box, and if so does it answer 2.12''s two inherited questions: (a) A17-8 successor - with the window restored-but-not-activated does a UIA walk reach real content versus only a shell while minimized, measured as node count and max depth in both states; (b) A23-3 successor - can an app menu (keyboard) or context menu (right-click on a content area) be staged on it'

$result = $null
try {
    $discovery = Find-ChromiumFamilyTargets
    $foundRows = @($discovery.rows)

    if ($foundRows.Count -eq 0) {
        $result = [ordered]@{
            probe      = $script:Probe
            question   = $question
            measurable = $false
            branch     = 'no_chromium_or_electron_target_found'
            searched   = [ordered]@{
                standard_locations_checked   = @($discovery.standard_locations_checked)
                registry_app_paths_examined  = @($discovery.registry_app_paths_examined)
                electron_asar_scan_performed = @($discovery.electron_asar_scan_performed)
                known_electron_app_checked   = $discovery.known_electron_app_checked
            }
        }
    } else {
        $foundSummary = @($foundRows | ForEach-Object {
                [ordered]@{
                    family             = $_.family
                    product_version    = $_.product_version
                    electron_signature = $_.electron_signature
                    source             = $_.source
                    is_browser_like    = $_.is_browser_like
                }
            })

        $browserRows = @($foundRows | Where-Object { $_.is_browser_like })
        $target = $null
        $targetKind = $null
        if ($browserRows.Count -gt 0) {
            $target = $browserRows[0]
            $targetKind = 'browser_with_controlled_content'
        } else {
            $target = $foundRows[0]
            $targetKind = 'generic_electron_app_default_content'
        }

        $measurement = Measure-ChromiumTarget -ExePath $target.path_internal_only -Family $target.family -TargetKind $targetKind

        $result = [ordered]@{
            probe            = $script:Probe
            question         = $question
            measurable       = $true
            branch           = 'chromium_family_target_found'
            targets_found    = @($foundSummary)
            target_selected  = [ordered]@{ family = $target.family; kind = $targetKind }
            measurement      = $measurement
        }
    }
} catch {
    $result = [ordered]@{
        probe       = $script:Probe
        question    = $question
        measurable  = $false
        branch      = 'probe_threw'
        error_class = $_.Exception.GetType().Name
    }
} finally {
    Stop-AllSpawned
}

$capturePath = Write-A24Capture -Name "chromium-target-$Label.json" -Content (ConvertTo-Json -InputObject $result -Depth 18)
Register-MandatoryPass -Capture $capturePath -Result $result

Assert-MandatoryMeasurement -Probe $script:Probe -Label $Label

Write-ProbeResult -Probe $script:Probe -Status 'ok' -Message 'Chromium/Electron target discovery and live measurement probe captured' -Data @{
    capture = Split-Path -Leaf $capturePath
}
exit 0
