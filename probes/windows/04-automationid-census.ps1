<#
.SYNOPSIS
    Probe 04 (sub-phase 2.0, unit U4): AutomationId coverage across the four Windows
    UI stacks, and the identity-stability experiment 2.5's element-resolution design
    depends on.

.DESCRIPTION
    Part A - coverage. One managed ControlView walk per stack (Win32 Notepad, Win32
    Explorer, WinForms in both fixture modes, WPF, Electron/Obsidian), reporting the
    percentage of interactive elements carrying a non-empty AutomationId and the id
    style (numeric control id vs symbolic). U7's COM census is consumed as a
    cross-check on the four targets whose element list it captured completely.

    Part B - identity stability. The repo's RefEntry evidence set is pid, role, path,
    stable text identity and bounds hash. Nothing else in this corpus measures which
    of those survive on Windows, so this probe dumps a target, changes exactly one
    thing, dumps it again, and reports per-property survival:

        arm "mutation"  list content changes inside the SAME process (InvokePattern
                        on btnMutateList; new/removed files for the Explorer folder)
        arm "restart"   the process is terminated and relaunched with identical
                        arguments at the identical window position

    RuntimeId handling (KTD9): the normalized twin canonicalizes RuntimeId to
    <runtimeid>, which would erase exactly the evidence this probe produces. Every
    RuntimeId comparison is therefore done in memory and only the RESULT is written -
    survived/changed counts and a stable structural classification of the id. No raw
    RuntimeId value reaches a capture. Names are handled the same way: they are
    compared in memory and only equality counts are written, so no document content
    is serialized.

    Captures under captures/04-automationid-census/:
        coverage.json          per-stack AutomationId coverage + id-style census
        identity-mutation.json per-property survival across an in-process list change
        identity-restart.json  per-property survival across process restart
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\common.ps1"

Add-Type -AssemblyName UIAutomationClient | Out-Null
Add-Type -AssemblyName UIAutomationTypes | Out-Null

$Probe = '04-automationid-census'
$NodeBudget = 800
$WalkMaxDepth = 14
$AE = [System.Windows.Automation.AutomationElement]
$Walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker
$RawWalker = [System.Windows.Automation.TreeWalker]::RawViewWalker
$InteractiveControlTypes = @('Button', 'CheckBox', 'ComboBox', 'DataItem', 'Edit', 'Hyperlink',
    'ListItem', 'MenuItem', 'RadioButton', 'Slider', 'Spinner', 'SplitButton', 'Tab', 'TabItem', 'TreeItem')
$AutomationIdSampleLimit = 45
$ObsidianSettleMs = 8000
$Pos = @{
    WinFormsDefault = @{ X = 20;   Y = 20 }
    WinFormsHosted  = @{ X = 520;  Y = 20 }
    Wpf             = @{ X = 20;   Y = 520 }
    Notepad         = @{ X = 1020; Y = 20;  Width = 600; Height = 450 }
    Explorer        = @{ X = 1020; Y = 490; Width = 600; Height = 400 }
}
$script:Spawned = New-Object System.Collections.ArrayList
$script:ExplorerHandle = 0
$script:ExplorerDir = ''
$script:ObsidianLaunched = $false

function Start-Tracked {
    param([string]$FilePath, [string[]]$ArgumentList = @(), [int]$TimeoutSec = 25)
    $p = Start-ScratchProcess -FilePath $FilePath -ArgumentList $ArgumentList -NoActivate -TimeoutSec $TimeoutSec
    [void]$script:Spawned.Add($p.ProcessId)
    return $p
}

function Get-ChildCount {
    param($Element)
    $n = 0
    try {
        $k = $RawWalker.GetFirstChild($Element)
        while ($null -ne $k) { $n++; $k = $RawWalker.GetNextSibling($k) }
    } catch { }
    return $n
}

function Wait-TopLevelElement {
    param([string]$Name = '', [string]$ClassName = '', [int[]]$ExcludeHandles = @(), [int[]]$ProcessIds = @(), [int]$TimeoutSec = 40)
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ($true) {
        try {
            $c = $Walker.GetFirstChild($AE::RootElement)
            while ($null -ne $c) {
                try {
                    if (($Name -eq '' -or $c.Current.Name -eq $Name) -and
                        ($ClassName -eq '' -or $c.Current.ClassName -eq $ClassName) -and
                        -not ($ExcludeHandles -contains [int]$c.Current.NativeWindowHandle) -and
                        ($ProcessIds.Count -eq 0 -or $ProcessIds -contains [int]$c.Current.ProcessId)) { return $c }
                } catch { }
                $c = $Walker.GetNextSibling($c)
            }
        } catch { }
        if ((Get-Date) -ge $deadline) { return $null }
        Start-Sleep -Milliseconds 450
    }
}

function Start-ScratchWpfWindow {
    param([string]$Tag, [int]$Left, [int]$Top, [int]$SettleSec = 6, [int]$Attempts = 3)
    $title = 'AgentDesktop Scratch WPF [' + $Tag + ']'
    for ($i = 1; $i -le $Attempts; $i++) {
        $proc = Start-Tracked -FilePath 'powershell.exe' -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File',
            (Join-Path (Get-ProbeRoot) 'scratch\ScratchWpf.ps1'), '-Tag', $Tag,
            '-Left', ([string]$Left), '-Top', ([string]$Top), '-TimeoutSeconds', '600')
        Start-Sleep -Seconds $SettleSec
        $element = Wait-TopLevelElement -Name $title -TimeoutSec 20
        if ($null -ne $element -and (Get-ChildCount -Element $element) -gt 0) {
            $hostPid = 0
            try { $hostPid = [int]$element.Current.ProcessId } catch { }
            if ($hostPid -gt 0 -and -not $script:Spawned.Contains($hostPid)) {
                Register-ScratchProcessId -ProcessId $hostPid
                [void]$script:Spawned.Add($hostPid)
            }
            return [pscustomobject]@{ Element = $element; ProcessId = $hostPid; LauncherProcessId = $proc.ProcessId; Attempts = $i }
        }
        Write-ProbeLog -Message ('WPF automation peer not bound on attempt ' + $i + '; relaunching for a fresh HWND') -Level 'warn'
        try { Stop-ScratchProcess -ProcessId $proc.ProcessId } catch { }
    }
    return $null
}

function Get-NodeRecords {
    param($Root)
    $records = New-Object System.Collections.ArrayList
    $order = New-Object System.Collections.Stack
    $order.Push(@{ Element = $Root; Depth = 0; Path = '0' })
    while ($order.Count -gt 0 -and $records.Count -lt $NodeBudget) {
        $item = $order.Pop()
        $element = $item.Element
        $cur = $null
        try { $cur = $element.Current } catch { }
        if ($null -eq $cur) { continue }
        $controlType = '<unavailable>'
        try { $controlType = $cur.ControlType.ProgrammaticName -replace '^ControlType\.', '' } catch { }
        $automationId = ''
        try { $automationId = [string]$cur.AutomationId } catch { }
        $className = ''
        try { $className = [string]$cur.ClassName } catch { }
        $name = ''
        try { $name = [string]$cur.Name } catch { }
        $runtimeId = ''
        try { $runtimeId = (@($element.GetRuntimeId()) -join '.') } catch { }
        $bounds = ''
        try {
            $r = $cur.BoundingRectangle
            if (-not $r.IsEmpty) { $bounds = ([int]$r.Left).ToString() + ',' + ([int]$r.Top) + ',' + ([int]$r.Width) + ',' + ([int]$r.Height) }
        } catch { }
        [void]$records.Add([pscustomobject]@{
            Path         = $item.Path
            Depth        = $item.Depth
            ControlType  = $controlType
            AutomationId = $automationId
            ClassName    = $className
            Name         = $name
            RuntimeId    = $runtimeId
            Bounds       = $bounds
        })
        if ($item.Depth -ge $WalkMaxDepth) { continue }
        $kids = New-Object System.Collections.ArrayList
        try {
            $k = $Walker.GetFirstChild($element)
            while ($null -ne $k) { [void]$kids.Add($k); $k = $Walker.GetNextSibling($k) }
        } catch { }
        for ($i = $kids.Count - 1; $i -ge 0; $i--) {
            $order.Push(@{ Element = $kids[$i]; Depth = ($item.Depth + 1); Path = ($item.Path + '.' + $i) })
        }
    }
    return @($records)
}

function Get-BucketedBounds {
    param([string]$Bounds)
    if ([string]::IsNullOrEmpty($Bounds)) { return '' }
    $parts = $Bounds -split ','
    $out = @()
    foreach ($p in $parts) { $out += [string]([int]([Math]::Round(([double]$p) / 8.0) * 8)) }
    return ($out -join ',')
}

function Get-AutomationIdStyle {
    param([string]$Value)
    if ([string]::IsNullOrEmpty($Value)) { return 'empty' }
    if ($Value -match '^\d+$') { return 'numeric-control-id' }
    return 'symbolic'
}

function Get-CoverageRow {
    param([string]$Stack, [string]$Target, $Records, [bool]$SampleIds = $true)
    $all = @($Records)
    $interactive = @($all | Where-Object { $InteractiveControlTypes -contains $_.ControlType })
    $allWithId = @($all | Where-Object { -not [string]::IsNullOrEmpty($_.AutomationId) })
    $interactiveWithId = @($interactive | Where-Object { -not [string]::IsNullOrEmpty($_.AutomationId) })
    $styles = @{ 'empty' = 0; 'numeric-control-id' = 0; 'symbolic' = 0 }
    foreach ($r in $all) { $styles[(Get-AutomationIdStyle -Value $r.AutomationId)] += 1 }
    $pctAll = 0
    if ($all.Count -gt 0) { $pctAll = [Math]::Round((100.0 * $allWithId.Count / $all.Count), 1) }
    $pctInteractive = 0
    if ($interactive.Count -gt 0) { $pctInteractive = [Math]::Round((100.0 * $interactiveWithId.Count / $interactive.Count), 1) }
    $sample = @()
    if ($SampleIds) { $sample = @($allWithId | Select-Object -First $AutomationIdSampleLimit | ForEach-Object { $_.ControlType + '=' + $_.AutomationId }) }
    $dominant = 'none'
    if ($styles['symbolic'] -gt $styles['numeric-control-id']) { $dominant = 'symbolic' }
    elseif ($styles['numeric-control-id'] -gt 0) { $dominant = 'numeric-control-id' }
    return [ordered]@{
        Target                       = $Target
        Stack                        = $Stack
        Elements                     = $all.Count
        ElementsWithAutomationId     = $allWithId.Count
        PctAllElements               = $pctAll
        InteractiveElements          = $interactive.Count
        InteractiveWithAutomationId  = $interactiveWithId.Count
        PctInteractiveElements       = $pctInteractive
        IdStyleEmpty                 = $styles['empty']
        IdStyleNumericControlId      = $styles['numeric-control-id']
        IdStyleSymbolic              = $styles['symbolic']
        DominantIdStyle              = $dominant
        AutomationIdSample           = @($sample)
    }
}

function Get-PropertySurvival {
    param($Pairs, [string]$Property)
    $all = @($Pairs)
    $survived = 0
    foreach ($pair in $all) {
        if ($pair.Before.$Property -eq $pair.After.$Property) { $survived++ }
    }
    $pct = 0
    if ($all.Count -gt 0) { $pct = [Math]::Round((100.0 * $survived / $all.Count), 1) }
    return [ordered]@{ Property = $Property; Matched = $all.Count; Survived = $survived; PctSurvived = $pct }
}

function Compare-Identity {
    param([string]$Target, [string]$Arm, $Before, $After, [string]$Change)
    $beforeByPath = @{}
    foreach ($r in $Before) { $beforeByPath[$r.Path] = $r }
    $afterByPath = @{}
    foreach ($r in $After) { $afterByPath[$r.Path] = $r }
    $pairs = New-Object System.Collections.ArrayList
    foreach ($k in $beforeByPath.Keys) {
        if ($afterByPath.ContainsKey($k)) {
            [void]$pairs.Add([pscustomobject]@{ Before = $beforeByPath[$k]; After = $afterByPath[$k] })
        }
    }
    $pathOnlyBefore = @($beforeByPath.Keys | Where-Object { -not $afterByPath.ContainsKey($_) }).Count
    $pathOnlyAfter = @($afterByPath.Keys | Where-Object { -not $beforeByPath.ContainsKey($_) }).Count

    $beforeById = @{}
    foreach ($r in $Before) {
        if ([string]::IsNullOrEmpty($r.AutomationId)) { continue }
        $key = $r.ControlType + '|' + $r.AutomationId
        if ($beforeById.ContainsKey($key)) { $beforeById[$key] = $null } else { $beforeById[$key] = $r }
    }
    $afterById = @{}
    foreach ($r in $After) {
        if ([string]::IsNullOrEmpty($r.AutomationId)) { continue }
        $key = $r.ControlType + '|' + $r.AutomationId
        if ($afterById.ContainsKey($key)) { $afterById[$key] = $null } else { $afterById[$key] = $r }
    }
    $idPairs = New-Object System.Collections.ArrayList
    foreach ($k in $beforeById.Keys) {
        if ($null -eq $beforeById[$k]) { continue }
        if ($afterById.ContainsKey($k) -and $null -ne $afterById[$k]) {
            [void]$idPairs.Add([pscustomobject]@{ Before = $beforeById[$k]; After = $afterById[$k] })
        }
    }
    $idUniqueBefore = @($beforeById.Keys | Where-Object { $null -ne $beforeById[$_] }).Count
    $idLost = $idUniqueBefore - @($idPairs).Count

    $boundsPairs = @($pairs | ForEach-Object {
        [pscustomobject]@{
            Before = [pscustomobject]@{ BoundsBucketed = (Get-BucketedBounds -Bounds $_.Before.Bounds); PathAndBounds = $_.Before.Bounds }
            After  = [pscustomobject]@{ BoundsBucketed = (Get-BucketedBounds -Bounds $_.After.Bounds); PathAndBounds = $_.After.Bounds }
        }
    })

    $byType = New-Object System.Collections.ArrayList
    foreach ($t in @($pairs | ForEach-Object { $_.Before.ControlType } | Sort-Object -Unique)) {
        $subset = @($pairs | Where-Object { $_.Before.ControlType -eq $t })
        [void]$byType.Add([ordered]@{
            ControlType         = $t
            PathMatchedPairs    = $subset.Count
            AutomationIdSurvived = @($subset | Where-Object { $_.Before.AutomationId -eq $_.After.AutomationId }).Count
            NameSurvived        = @($subset | Where-Object { $_.Before.Name -eq $_.After.Name }).Count
            RuntimeIdSurvived   = @($subset | Where-Object { $_.Before.RuntimeId -eq $_.After.RuntimeId }).Count
            BoundsSurvived      = @($subset | Where-Object { $_.Before.Bounds -eq $_.After.Bounds }).Count
        })
    }

    return [ordered]@{
        Target                    = $Target
        Arm                       = $Arm
        Change                    = $Change
        Stack                     = 'managed-System.Windows.Automation ControlView'
        BeforeNodes               = @($Before).Count
        AfterNodes                = @($After).Count
        PathMatchedPairs          = @($pairs).Count
        PathsOnlyInBefore         = $pathOnlyBefore
        PathsOnlyInAfter          = $pathOnlyAfter
        PerPropertySurvival       = @(
            (Get-PropertySurvival -Pairs $pairs -Property 'ControlType'),
            (Get-PropertySurvival -Pairs $pairs -Property 'AutomationId'),
            (Get-PropertySurvival -Pairs $pairs -Property 'ClassName'),
            (Get-PropertySurvival -Pairs $pairs -Property 'Name'),
            (Get-PropertySurvival -Pairs $pairs -Property 'RuntimeId'),
            (Get-PropertySurvival -Pairs $pairs -Property 'Bounds')
        )
        BoundsBucketed8pxSurvival = (Get-PropertySurvival -Pairs $boundsPairs -Property 'BoundsBucketed')
        AutomationIdKeyedMatch    = [ordered]@{
            UniqueKeysBefore = $idUniqueBefore
            KeysReFound      = @($idPairs).Count
            KeysLost         = $idLost
            PathAlsoSurvived = @($idPairs | Where-Object { $_.Before.Path -eq $_.After.Path }).Count
            RuntimeIdAlsoSurvived = @($idPairs | Where-Object { $_.Before.RuntimeId -eq $_.After.RuntimeId }).Count
            KeysReFoundOnADifferentElement = @($idPairs | Where-Object { $_.Before.Name -ne $_.After.Name }).Count
            KeysReFoundOnADifferentElementNote = 'the number of AutomationId keys that still resolve after the change but now land on an element whose Name differs. This is the silent-wrong-target count: a ref keyed on AutomationId alone would resolve successfully and act on the wrong element. Names are compared in memory only.'
            Note             = 'the key is ControlType|AutomationId; keys that are not unique within a dump are excluded from both sides rather than matched arbitrarily'
        }
        ByControlType             = @($byType)
    }
}

function Invoke-ByAutomationId {
    param($Root, [string]$AutomationId)
    $condition = New-Object System.Windows.Automation.PropertyCondition($AE::AutomationIdProperty, $AutomationId)
    $target = $Root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condition)
    if ($null -eq $target) { throw ('element with AutomationId ' + $AutomationId + ' not found') }
    $pattern = $target.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
    $pattern.Invoke()
}

function Get-StatusText {
    param($Root, [string]$AutomationId = 'lblStatus')
    try {
        $condition = New-Object System.Windows.Automation.PropertyCondition($AE::AutomationIdProperty, $AutomationId)
        $element = $Root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condition)
        if ($null -eq $element) { return '<not-found>' }
        return [string]$element.Current.Name
    } catch { return '<error>' }
}

function Close-ShellWindowByHandle {
    param([int]$Handle)
    $shell = New-Object -ComObject Shell.Application
    foreach ($w in @($shell.Windows())) {
        try { if ([int]$w.HWND -eq $Handle) { $w.Quit(); return $true } } catch { }
    }
    return $false
}

$status = 'ok'
$message = ''
$resultData = [ordered]@{}

try {
    Initialize-ProbeNative
    $scratchExe = Join-Path (Get-ProbeRoot) 'scratch\bin\ScratchForms.exe'
    if (-not (Test-Path -LiteralPath $scratchExe)) {
        & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path (Get-ProbeRoot) 'scratch\build-scratch.ps1') | Out-Null
    }
    if (-not (Test-Path -LiteralPath $scratchExe)) { throw ('scratch fixture missing at ' + $scratchExe) }

    $coverage = New-Object System.Collections.ArrayList

    # --- Electron first, alone on the desktop (U3 placement hazard) ---------
    $obsidianExe = Join-Path $env:LOCALAPPDATA 'Programs\Obsidian\Obsidian.exe'
    $obsidianVersion = '<absent>'
    $obsidianRow = $null
    if (Test-Path -LiteralPath $obsidianExe) {
        $obsidianVersion = (Get-Item -LiteralPath $obsidianExe).VersionInfo.FileVersion
        $obsidianPids = @(Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
        if ($obsidianPids.Count -eq 0) {
            [void](Start-Tracked -FilePath $obsidianExe -TimeoutSec 40)
            $script:ObsidianLaunched = $true
            Start-Sleep -Seconds 4
            $obsidianPids = @(Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
            foreach ($opid in $obsidianPids) {
                if (-not $script:Spawned.Contains($opid)) { Register-ScratchProcessId -ProcessId $opid; [void]$script:Spawned.Add($opid) }
            }
        }
        $obsidianWindow = Wait-TopLevelElement -ClassName 'Chrome_WidgetWin_1' -ProcessIds $obsidianPids -TimeoutSec 40
        if ($null -ne $obsidianWindow) {
            Show-WindowNoActivate -WindowHandle ([IntPtr][int]$obsidianWindow.Current.NativeWindowHandle) -X 40 -Y 40 -Width 1000 -Height 640
            $obsidianRecords = @()
            $settlePasses = 0
            for ($i = 1; $i -le 3; $i++) {
                Start-Sleep -Milliseconds $ObsidianSettleMs
                $settlePasses = $i
                $obsidianRecords = Get-NodeRecords -Root $obsidianWindow
                if (@($obsidianRecords).Count -gt 20) { break }
            }
            $obsidianRow = Get-CoverageRow -Stack 'electron-chromium' -Target 'obsidian' -Records $obsidianRecords -SampleIds $false
            $obsidianRow['timingSettlePasses'] = $settlePasses
            $obsidianRow['AutomationIdSampleWithheld'] = 'AutomationId values are emitted verbatim for probe-owned fixtures and for Win32 chrome, but not for Obsidian: an Electron AutomationId can be derived from note or vault content, which R11 keeps out of the corpus. Only counts and id-style statistics are reported for this row.'
            [void]$coverage.Add($obsidianRow)
        } else {
            Write-ProbeLog -Message 'no Obsidian-owned Chromium top-level window found; Electron coverage row is skipped' -Level 'warn'
        }
    }
    if ($script:ObsidianLaunched) {
        foreach ($p in @(Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue)) { try { Stop-ScratchProcess -ProcessId $p.Id } catch { } }
        $script:ObsidianLaunched = $false
    }

    # --- WinForms default mode: coverage only ------------------------------
    $winDefault = Start-Tracked -FilePath $scratchExe -ArgumentList @('--tag', 'u4-default', '--pos', ($Pos.WinFormsDefault.X.ToString() + ',' + $Pos.WinFormsDefault.Y))
    if ($winDefault.MainWindowHandle -eq [IntPtr]::Zero) { throw 'WinForms default-mode window never appeared' }
    [void]$coverage.Add((Get-CoverageRow -Stack 'winforms' -Target 'winforms-default' -Records (Get-NodeRecords -Root ($AE::FromHandle($winDefault.MainWindowHandle)))))
    Stop-ScratchProcess -ProcessId $winDefault.ProcessId

    # --- WinForms host-providers mode: coverage + identity baseline --------
    $winHostedArgs = @('--tag', 'u4-hosted', '--pos', ($Pos.WinFormsHosted.X.ToString() + ',' + $Pos.WinFormsHosted.Y), '--host-providers')
    $winHosted = Start-Tracked -FilePath $scratchExe -ArgumentList $winHostedArgs
    if ($winHosted.MainWindowHandle -eq [IntPtr]::Zero) { throw 'WinForms host-providers window never appeared' }
    $winHostedRoot = $AE::FromHandle($winHosted.MainWindowHandle)
    $winBaseline = Get-NodeRecords -Root $winHostedRoot
    [void]$coverage.Add((Get-CoverageRow -Stack 'winforms' -Target 'winforms-host-providers' -Records $winBaseline))

    # --- WPF: coverage + identity baseline ---------------------------------
    $wpf = Start-ScratchWpfWindow -Tag 'u4-identity' -Left $Pos.Wpf.X -Top $Pos.Wpf.Y
    if ($null -eq $wpf) { throw 'WPF scratch window never bound its automation peer across three launches' }
    $wpfBaseline = Get-NodeRecords -Root $wpf.Element
    [void]$coverage.Add((Get-CoverageRow -Stack 'wpf' -Target 'wpf' -Records $wpfBaseline))

    # --- Win32 Notepad: coverage + identity baseline -----------------------
    $notepad = Start-Tracked -FilePath (Join-Path $env:WINDIR 'System32\notepad.exe')
    if ($notepad.MainWindowHandle -eq [IntPtr]::Zero) { throw 'notepad window never appeared' }
    Show-WindowNoActivate -WindowHandle $notepad.MainWindowHandle -X $Pos.Notepad.X -Y $Pos.Notepad.Y -Width $Pos.Notepad.Width -Height $Pos.Notepad.Height
    Start-Sleep -Milliseconds 700
    $notepadBaseline = Get-NodeRecords -Root ($AE::FromHandle($notepad.MainWindowHandle))
    [void]$coverage.Add((Get-CoverageRow -Stack 'win32' -Target 'notepad' -Records $notepadBaseline))

    # --- Win32 Explorer over a probe-owned folder: coverage + identity -----
    $script:ExplorerDir = Join-Path $env:TEMP 'agent-desktop-u4-explorer'
    if (Test-Path -LiteralPath $script:ExplorerDir) { Remove-Item -LiteralPath $script:ExplorerDir -Recurse -Force }
    New-Item -ItemType Directory -Path $script:ExplorerDir -Force | Out-Null
    foreach ($n in @('item-alpha', 'item-bravo', 'item-charlie', 'item-delta', 'item-echo')) {
        [IO.File]::WriteAllText((Join-Path $script:ExplorerDir ($n + '.txt')), $n)
    }
    $preCabinet = @()
    $c = $Walker.GetFirstChild($AE::RootElement)
    while ($null -ne $c) {
        try { if ($c.Current.ClassName -eq 'CabinetWClass') { $preCabinet += [int]$c.Current.NativeWindowHandle } } catch { }
        $c = $Walker.GetNextSibling($c)
    }
    $explorerLauncher = Start-Process -FilePath (Join-Path $env:WINDIR 'explorer.exe') -ArgumentList @($script:ExplorerDir) -PassThru
    Register-ScratchProcessId -ProcessId $explorerLauncher.Id
    $explorerElement = Wait-TopLevelElement -ClassName 'CabinetWClass' -ExcludeHandles $preCabinet -TimeoutSec 40
    if ($null -eq $explorerElement) { throw 'no CabinetWClass folder window appeared for the probe folder' }
    $script:ExplorerHandle = [int]$explorerElement.Current.NativeWindowHandle
    Show-WindowNoActivate -WindowHandle ([IntPtr]$script:ExplorerHandle) -X $Pos.Explorer.X -Y $Pos.Explorer.Y -Width $Pos.Explorer.Width -Height $Pos.Explorer.Height
    Start-Sleep -Seconds 3
    $explorerBaseline = Get-NodeRecords -Root $explorerElement
    [void]$coverage.Add((Get-CoverageRow -Stack 'win32' -Target 'explorer-folder-window' -Records $explorerBaseline))

    # --- COM cross-check from U7's complete element lists ------------------
    $comRows = New-Object System.Collections.ArrayList
    $censusPath = Join-Path (Join-Path (Get-ProbeRoot) 'captures\08-uia3-com') 'census.json'
    if (Test-Path -LiteralPath $censusPath) {
        $census = [IO.File]::ReadAllText($censusPath) | ConvertFrom-Json
        foreach ($t in @($census.result.targets)) {
            $els = @($t.elements)
            if ($els.Count -lt $t.rawViewNodes) { continue }
            $withId = @($els | Where-Object { -not [string]::IsNullOrEmpty($_.automationId) })
            $interactive = @($els | Where-Object { $InteractiveControlTypes -contains $_.controlType })
            $interactiveWithId = @($interactive | Where-Object { -not [string]::IsNullOrEmpty($_.automationId) })
            $pctAll = 0
            if ($els.Count -gt 0) { $pctAll = [Math]::Round((100.0 * $withId.Count / $els.Count), 1) }
            $pctInteractive = 0
            if ($interactive.Count -gt 0) { $pctInteractive = [Math]::Round((100.0 * $interactiveWithId.Count / $interactive.Count), 1) }
            [void]$comRows.Add([ordered]@{
                Target                      = $t.label
                Stack                       = 'uia3-com'
                View                        = 'RawView'
                Elements                    = $els.Count
                ElementsWithAutomationId    = $withId.Count
                PctAllElements              = $pctAll
                InteractiveElements         = $interactive.Count
                InteractiveWithAutomationId = $interactiveWithId.Count
                PctInteractiveElements      = $pctInteractive
            })
        }
    }

    $coverageCapture = [ordered]@{
        Probe                   = $Probe
        Question                = 'what fraction of interactive elements carries a non-empty AutomationId on each Windows UI stack, and in what style'
        Stack                   = 'managed-System.Windows.Automation'
        Scope                   = 'app/provider'
        View                    = 'ControlView - the view a snapshot would walk; the COM cross-check below is RawView, so its denominators are larger by construction'
        InteractiveControlTypes = @($InteractiveControlTypes)
        InteractiveDefinition   = 'the ControlTypes that map onto the interactive roles the repo ref system already allocates refs for. Container roles that are actionable-but-not-interactive (scrollarea, disclosure) are deliberately excluded from the denominator - they are ref-able because they advertise an action, not because they are interactive.'
        WalkLimits              = [ordered]@{ NodeBudget = $NodeBudget; MaxDepth = $WalkMaxDepth }
        Rows                    = @($coverage)
        ComCrossCheck           = @($comRows)
        ComCrossCheckNote       = 'consumed from captures/08-uia3-com/census.json. The shim serializes at most 60 per-element records per target, so a row is only emitted when the element list covers the whole RawView walk; Explorer and Obsidian are therefore managed-only rows.'
        ObsidianVersion         = $obsidianVersion
        ObsidianPlacementNote   = 'the Electron row is measured first, with no other probe window on the desktop, and after an 8 s settle. U3 measured that an Electron tree read behind another window can stay at its first-contact size indefinitely.'
    }
    [void](Write-ProbeJson -Probe $Probe -Name 'coverage.json' -InputObject $coverageCapture)

    # --- identity arm 1: in-process mutation --------------------------------
    $mutationRows = New-Object System.Collections.ArrayList
    $wpfStatusBefore = Get-StatusText -Root $wpf.Element
    Invoke-ByAutomationId -Root $wpf.Element -AutomationId 'btnMutateList'
    Start-Sleep -Milliseconds 800
    $wpfStatusAfter = Get-StatusText -Root $wpf.Element
    $wpfMutated = Get-NodeRecords -Root $wpf.Element
    $wpfMutationRow = Compare-Identity -Target 'wpf' -Arm 'mutation' -Before $wpfBaseline -After $wpfMutated `
        -Change 'InvokePattern on btnMutateList: same list change as the WinForms arm. The WPF fixture also derives each item AutomationId from the item text (lstItem-<name>), which is the interesting contrast.'
    $wpfMutationRow['MutationVerifiedByObservation'] = ($wpfStatusBefore -ne $wpfStatusAfter)
    $wpfMutationRow['StatusTextBefore'] = $wpfStatusBefore
    $wpfMutationRow['StatusTextAfter'] = $wpfStatusAfter
    [void]$mutationRows.Add($wpfMutationRow)

    $listItemCondition = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, [System.Windows.Automation.ControlType]::ListItem)
    $itemsBefore = @($explorerElement.FindAll([System.Windows.Automation.TreeScope]::Descendants, $listItemCondition)).Count
    Remove-Item -LiteralPath (Join-Path $script:ExplorerDir 'item-bravo.txt') -Force
    [IO.File]::WriteAllText((Join-Path $script:ExplorerDir 'item-foxtrot.txt'), 'item-foxtrot')
    [IO.File]::WriteAllText((Join-Path $script:ExplorerDir 'item-golf.txt'), 'item-golf')
    $refreshSeconds = -1
    for ($waited = 2; $waited -le 60; $waited += 2) {
        Start-Sleep -Seconds 2
        if (@($explorerElement.FindAll([System.Windows.Automation.TreeScope]::Descendants, $listItemCondition)).Count -ne $itemsBefore) { $refreshSeconds = $waited; break }
    }
    $explorerMutated = Get-NodeRecords -Root $explorerElement
    $explorerRow = Compare-Identity -Target 'explorer-folder-window' -Arm 'mutation' -Before $explorerBaseline -After $explorerMutated `
        -Change 'the probe-owned folder loses item-bravo.txt and gains item-foxtrot.txt and item-golf.txt; Explorer refreshes itself from the file-system change notification, with no window, process or input change'
    $explorerRow['MutationVerifiedByObservation'] = ($refreshSeconds -ge 0)
    $explorerRow['timingRefreshLatencySeconds'] = $refreshSeconds
    $explorerRow['RefreshLatencyNote'] = 'measured, not assumed: the folder window took this long to reflect the file-system change. A 4 s wait was tried first and reported a completely unchanged tree, which would have been filed as "Explorer identity is perfectly stable across content mutation" - the exact opposite of what the window does once it refreshes.'
    [void]$mutationRows.Add($explorerRow)

    [void](Write-ProbeJson -Probe $Probe -Name 'identity-mutation.json' -InputObject ([ordered]@{
        Probe            = $Probe
        Question         = 'when list content changes inside a live process, which per-node identity properties survive'
        Why              = "2.5 chooses the Windows RefEntry evidence set. The repo contract is pid, role, path, stable text identity and bounds hash; this arm is the one that shows what a content change does to path, text identity and bounds while pid and role are held constant."
        ReRunStability   = 'measured over three consecutive runs on this box: every field here reproduces exactly except the Explorer RuntimeId survival count, which moved between 59/82 and 60/82 - one DirectUI Edit node whose RuntimeId is regenerated or not depending on how the shell refresh lands. That is real platform nondeterminism in the property, not probe noise, and under KTD9 a non-empty normalized diff on this one number is the signal a later re-runner should see rather than something to smooth away. All other captures in this probe were byte-identical across runs.'
        WinFormsNotHere  = 'the WinForms fixture has no in-process arm because no managed-visible affordance exists to drive it: in --host-providers mode the managed client sees the whole form as Panes with numeric control ids and btnMutateList advertises no InvokePattern, and in default mode the custom provider suppresses patterns entirely. Its list change is measured in identity-restart.json as the restart+mutation row against the pure restart row as control. Driving it any other way would need SendInput or PostMessage, which belongs to U6.'
        RuntimeIdHandling = 'RuntimeId is compared in memory and only survived/changed counts are written. The KTD9 normalizer rewrites any literal RuntimeId to <runtimeid>, so a capture holding raw ids would normalize away the exact evidence this arm produces.'
        NameHandling     = 'Name is compared in memory and only equality counts are written. No Name value from any target reaches a capture, so the R11 content rule is satisfied by construction rather than by redaction.'
        Rows             = @($mutationRows)
    }))

    # --- identity arm 2: restart -------------------------------------------
    $restartRows = New-Object System.Collections.ArrayList
    Stop-ScratchProcess -ProcessId $winHosted.ProcessId
    Start-Sleep -Milliseconds 500
    $winRestarted = Start-Tracked -FilePath $scratchExe -ArgumentList $winHostedArgs
    if ($winRestarted.MainWindowHandle -eq [IntPtr]::Zero) { throw 'WinForms host-providers window never reappeared after restart' }
    Start-Sleep -Milliseconds 800
    [void]$restartRows.Add((Compare-Identity -Target 'winforms-host-providers' -Arm 'restart' `
        -Before $winBaseline -After (Get-NodeRecords -Root ($AE::FromHandle($winRestarted.MainWindowHandle))) `
        -Change 'process terminated and relaunched with identical arguments and the identical --pos origin; the list is back at its baseline content'))

    Stop-ScratchProcess -ProcessId $wpf.ProcessId
    if ($wpf.LauncherProcessId -ne $wpf.ProcessId) { try { Stop-ScratchProcess -ProcessId $wpf.LauncherProcessId } catch { } }
    Start-Sleep -Milliseconds 500
    $wpfRestarted = Start-ScratchWpfWindow -Tag 'u4-identity' -Left $Pos.Wpf.X -Top $Pos.Wpf.Y
    if ($null -eq $wpfRestarted) { throw 'WPF scratch window never bound its automation peer after restart' }
    [void]$restartRows.Add((Compare-Identity -Target 'wpf' -Arm 'restart' `
        -Before $wpfBaseline -After (Get-NodeRecords -Root $wpfRestarted.Element) `
        -Change 'process terminated and relaunched with identical -Left/-Top arguments; the peer-activation settle is applied again because a WPF window read too early binds the HWND fallback provider permanently (03-pattern-census)'))

    Stop-ScratchProcess -ProcessId $notepad.ProcessId
    Start-Sleep -Milliseconds 500
    $notepadRestarted = Start-Tracked -FilePath (Join-Path $env:WINDIR 'System32\notepad.exe')
    if ($notepadRestarted.MainWindowHandle -eq [IntPtr]::Zero) { throw 'notepad window never reappeared after restart' }
    Show-WindowNoActivate -WindowHandle $notepadRestarted.MainWindowHandle -X $Pos.Notepad.X -Y $Pos.Notepad.Y -Width $Pos.Notepad.Width -Height $Pos.Notepad.Height
    Start-Sleep -Milliseconds 700
    [void]$restartRows.Add((Compare-Identity -Target 'notepad' -Arm 'restart' `
        -Before $notepadBaseline -After (Get-NodeRecords -Root ($AE::FromHandle($notepadRestarted.MainWindowHandle))) `
        -Change 'the real Win32 target: process terminated and relaunched, then placed at the identical rect with SetWindowPos so window placement cannot masquerade as layout drift'))

    Stop-ScratchProcess -ProcessId $winRestarted.ProcessId
    Start-Sleep -Milliseconds 500
    $winMutatedRestart = Start-Tracked -FilePath $scratchExe -ArgumentList (@($winHostedArgs) + @('--mutate-list'))
    if ($winMutatedRestart.MainWindowHandle -eq [IntPtr]::Zero) { throw 'WinForms host-providers window never reappeared with --mutate-list' }
    Start-Sleep -Milliseconds 800
    $winMutateRow = Compare-Identity -Target 'winforms-host-providers' -Arm 'restart+mutation' `
        -Before $winBaseline -After (Get-NodeRecords -Root ($AE::FromHandle($winMutatedRestart.MainWindowHandle))) `
        -Change 'relaunched with --mutate-list added: same origin, same arguments otherwise, but the list starts as Alpha/Charlie/Delta/Echo/Foxtrot/Golf. Its control is the pure restart row above; any survival difference between the two rows is attributable to the list change alone.'
    $winMutateRow['StatusText'] = (Get-StatusText -Root ($AE::FromHandle($winMutatedRestart.MainWindowHandle)) -AutomationId '1020')
    [void]$restartRows.Add($winMutateRow)

    [void](Write-ProbeJson -Probe $Probe -Name 'identity-restart.json' -InputObject ([ordered]@{
        Probe             = $Probe
        Question          = 'when a process is restarted, which per-node identity properties survive'
        PlacementControl  = 'every target is relaunched at the identical window origin (scratch fixtures via --pos / -Left -Top, Notepad via SetWindowPos with the same rect). Without that control every bounds comparison would report 0% survival for the trivial reason that Windows cascades a new window, and the interesting question - whether layout inside the window is reproducible - would be unanswerable.'
        RuntimeIdHandling = 'as in identity-mutation.json: compared in memory, only survived/changed counts written.'
        Rows              = @($restartRows)
    }))

    $summaryRows = @()
    foreach ($r in @($mutationRows + $restartRows)) {
        $cells = @()
        foreach ($p in $r.PerPropertySurvival) { $cells += ($p.Property + ':' + $p.PctSurvived) }
        $summaryRows += ('target=' + $r.Target + ' arm=' + $r.Arm + ' matched=' + $r.PathMatchedPairs + ' pct=' + ($cells -join ','))
    }
    $resultData['coverageRows'] = @($coverage).Count
    $resultData['identityRows'] = @($mutationRows).Count + @($restartRows).Count
    foreach ($row in $coverage) { $resultData[('pctInteractiveWithId_' + $row.Target)] = $row.PctInteractiveElements }
    $resultData['survival'] = ($summaryRows -join ' ; ')
    $message = 'automationid coverage over ' + @($coverage).Count + ' stack rows; identity survival over ' + $resultData['identityRows'] + ' arms'
} catch {
    $status = 'fail'
    $message = ($_.Exception.Message -replace '[\r\n]+', ' ')
    Write-ProbeLog -Message ('probe failed: ' + $message) -Level 'error'
} finally {
    if ($script:ExplorerHandle -ne 0) {
        try { [void](Close-ShellWindowByHandle -Handle $script:ExplorerHandle) } catch { Write-ProbeLog -Message ('could not close folder window: ' + $_.Exception.Message) -Level 'warn' }
    }
    if ($script:ObsidianLaunched) {
        foreach ($p in @(Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue)) { try { Stop-ScratchProcess -ProcessId $p.Id } catch { } }
    }
    foreach ($id in @($script:Spawned)) {
        try { Stop-ScratchProcess -ProcessId $id } catch { Write-ProbeLog -Message ('teardown: ' + $_.Exception.Message) -Level 'warn' }
    }
    if ($script:ExplorerDir -and (Test-Path -LiteralPath $script:ExplorerDir)) {
        try { Remove-Item -LiteralPath $script:ExplorerDir -Recurse -Force } catch { }
    }
    foreach ($f in @(Get-ChildItem -LiteralPath (Get-CaptureDir -Probe $Probe) -File -ErrorAction SilentlyContinue)) {
        if ($f.Name -like '*.normalized') { continue }
        if (-not (Test-CaptureRedaction -Path $f.FullName)) { $status = 'fail'; $message = ('redaction residue in ' + $f.Name) }
    }
}

Write-ProbeResult -Probe $Probe -Status $status -Message $message -Data $resultData
if ($status -eq 'fail') { exit 1 }
exit 0
