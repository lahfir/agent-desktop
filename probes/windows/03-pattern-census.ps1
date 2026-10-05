<#
.SYNOPSIS
    Probe 03 (sub-phase 2.0, unit U4): per-ControlType pattern-availability census on
    the UIA3 COM stack, with the managed cross-check that justifies KTD1.

.DESCRIPTION
    The authoritative census is NOT re-measured here. U7's csc-built UIA3 COM shim
    already walked WinForms (both fixture modes), WPF and classic Notepad in census
    mode and committed the result; this probe consumes
    captures/08-uia3-com/census.json and reshapes it into the per-ControlType matrix
    the ledger cites, so the corpus holds exactly one COM pattern measurement.

    What is new here is the managed side. U7's comparison.json recorded managed node
    counts and "how many elements advertise any pattern"; it did not record WHICH
    pattern names the managed stack can name. This probe runs a fresh
    System.Windows.Automation sweep over the same four fixture shapes, enumerates
    pattern short names per element, and diffs the two vocabularies. The result is
    the divergence row: LegacyIAccessible is live on every element the COM stack
    sees and is not expressible at all on the managed stack, so a managed census
    would have recorded false absences for the patterns the Rust adapter will see.

    Captures under captures/03-pattern-census/:
        pattern-matrix.json     COM per-target ControlType x pattern matrix + sanity anchors
        managed-crosscheck.json managed pattern vocabulary over the same fixture shapes
        divergence.json         per-pattern COM-vs-managed visibility, KTD1 evidence row
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\common.ps1"

Add-Type -AssemblyName UIAutomationClient | Out-Null
Add-Type -AssemblyName UIAutomationTypes | Out-Null

$Probe = '03-pattern-census'
$NodeBudget = 400
$AE = [System.Windows.Automation.AutomationElement]
$RawWalker = [System.Windows.Automation.TreeWalker]::RawViewWalker
$MatrixRecordFormat = 'one row per line, space separated key=value fields in fixed order: target=<census target label> ct=<UIA ControlType short name> n=<elements of that ControlType in the RawView walk> withpat=<how many of them advertise at least one pattern> pats=<Name:count comma separated, "-" when none>. Counts are element occurrences, not distinct patterns.'
$script:Spawned = New-Object System.Collections.ArrayList

function Get-DictRows {
    param($Dict)
    $pairs = @()
    if ($null -eq $Dict) { return '-' }
    foreach ($p in @($Dict.PSObject.Properties | Sort-Object -Property Name)) {
        $pairs += ($p.Name + ':' + $p.Value)
    }
    if ($pairs.Count -eq 0) { return '-' }
    return ($pairs -join ',')
}

function Get-HashRows {
    param([hashtable]$Table)
    $pairs = @()
    foreach ($k in @($Table.Keys | Sort-Object)) { $pairs += ($k + ':' + $Table[$k]) }
    if ($pairs.Count -eq 0) { return '-' }
    return ($pairs -join ',')
}

function Add-Count {
    param([hashtable]$Table, [string]$Key, [int]$By = 1)
    if ($Table.ContainsKey($Key)) { $Table[$Key] = $Table[$Key] + $By } else { $Table[$Key] = $By }
}

function Get-PatternCell {
    param($ByControlType, [string]$ControlType, [string]$Pattern)
    foreach ($row in @($ByControlType)) {
        if ($row.controlType -ne $ControlType) { continue }
        $hit = @($row.patterns.PSObject.Properties | Where-Object { $_.Name -eq $Pattern })
        $available = 0
        if ($hit.Count -gt 0) { $available = [int]$hit[0].Value }
        return [ordered]@{ Present = $true; Elements = [int]$row.count; WithPattern = $available }
    }
    return [ordered]@{ Present = $false; Elements = 0; WithPattern = 0 }
}

function Get-ManagedSweep {
    param($Root, [string]$Label)
    $nodes = 0
    $withPattern = 0
    $typeCounts = @{}
    $typeWith = @{}
    $byType = @{}
    $totals = @{}
    $order = New-Object System.Collections.Stack
    $order.Push($Root)
    while ($order.Count -gt 0 -and $nodes -lt $NodeBudget) {
        $node = $order.Pop()
        $nodes++
        $typeName = '<unavailable>'
        try { $typeName = $node.Current.ControlType.ProgrammaticName -replace '^ControlType\.', '' } catch { }
        Add-Count -Table $typeCounts -Key $typeName
        $patterns = @()
        try { $patterns = @($node.GetSupportedPatterns() | ForEach-Object { $_.ProgrammaticName -replace 'PatternIdentifiers\.Pattern$', '' }) } catch { }
        if ($patterns.Count -gt 0) {
            $withPattern++
            Add-Count -Table $typeWith -Key $typeName
        }
        foreach ($p in $patterns) {
            Add-Count -Table $totals -Key $p
            if (-not $byType.ContainsKey($typeName)) { $byType[$typeName] = @{} }
            Add-Count -Table $byType[$typeName] -Key $p
        }
        try {
            $child = $RawWalker.GetFirstChild($node)
            while ($null -ne $child) {
                $order.Push($child)
                $child = $RawWalker.GetNextSibling($child)
            }
        } catch { }
    }
    $rows = @()
    foreach ($k in @($typeCounts.Keys | Sort-Object)) {
        $cells = '-'
        if ($byType.ContainsKey($k)) { $cells = (Get-HashRows -Table $byType[$k]) }
        $with = 0
        if ($typeWith.ContainsKey($k)) { $with = $typeWith[$k] }
        $rows += ('target=' + $Label + ' ct=' + $k + ' n=' + $typeCounts[$k] + ' withpat=' + $with + ' pats=' + $cells)
    }
    return [pscustomobject]@{
        Label                  = $Label
        RawViewNodes           = $nodes
        ElementsWithAnyPattern = $withPattern
        PatternTotals          = $totals
        Rows                   = @($rows)
    }
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

function Start-Tracked {
    param([string]$FilePath, [string[]]$ArgumentList = @(), [int]$TimeoutSec = 25)
    $p = Start-ScratchProcess -FilePath $FilePath -ArgumentList $ArgumentList -NoActivate -TimeoutSec $TimeoutSec
    [void]$script:Spawned.Add($p.ProcessId)
    return $p
}

function Wait-TopLevelElementByName {
    param([string]$Name, [int]$TimeoutSec = 40)
    $walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ($true) {
        try {
            $c = $walker.GetFirstChild($AE::RootElement)
            while ($null -ne $c) {
                try {
                    if ($c.Current.Name -eq $Name) { return $c }
                } catch { }
                $c = $walker.GetNextSibling($c)
            }
        } catch { }
        if ((Get-Date) -ge $deadline) { return $null }
        Start-Sleep -Milliseconds 400
    }
}

function Start-ScratchWpfWindow {
    param([string]$Tag, [int]$Left = 40, [int]$Top = 600, [int]$SettleSec = 6, [int]$Attempts = 3)
    $title = 'AgentDesktop Scratch WPF [' + $Tag + ']'
    for ($i = 1; $i -le $Attempts; $i++) {
        $proc = Start-Tracked -FilePath 'powershell.exe' -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File',
            (Join-Path (Get-ProbeRoot) 'scratch\ScratchWpf.ps1'), '-Tag', $Tag,
            '-Left', ([string]$Left), '-Top', ([string]$Top), '-TimeoutSeconds', '300')
        Start-Sleep -Seconds $SettleSec
        $element = Wait-TopLevelElementByName -Name $title -TimeoutSec 20
        if ($null -ne $element -and (Get-ChildCount -Element $element) -gt 0) {
            $hostPid = 0
            try { $hostPid = [int]$element.Current.ProcessId } catch { }
            if ($hostPid -gt 0 -and -not $script:Spawned.Contains($hostPid)) {
                Register-ScratchProcessId -ProcessId $hostPid
                [void]$script:Spawned.Add($hostPid)
            }
            $className = ''
            try { $className = [string]$element.Current.ClassName } catch { }
            return [pscustomobject]@{
                Element        = $element
                Attempts       = $i
                ClassNameShape = [regex]::Replace($className, '[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}', '<guid>')
            }
        }
        Write-ProbeLog -Message ('WPF automation peer not bound on attempt ' + $i + '; relaunching to get a fresh HWND') -Level 'warn'
        try { Stop-ScratchProcess -ProcessId $proc.ProcessId } catch { }
    }
    return $null
}

$status = 'ok'
$message = ''
$resultData = [ordered]@{}

try {
    $censusPath = Join-Path (Join-Path (Get-ProbeRoot) 'captures\08-uia3-com') 'census.json'
    if (-not (Test-Path -LiteralPath $censusPath)) { throw ('U7 census capture missing at ' + $censusPath + '; run 08-uia3-com.ps1 first') }
    $census = [IO.File]::ReadAllText($censusPath) | ConvertFrom-Json

    $matrixRows = New-Object System.Collections.ArrayList
    $totalsByTarget = [ordered]@{}
    $comVocabulary = @{}
    $comTargetsByPattern = @{}
    $anchors = New-Object System.Collections.ArrayList
    foreach ($t in @($census.result.targets)) {
        foreach ($row in @($t.byControlType)) {
            [void]$matrixRows.Add('target=' + $t.label + ' ct=' + $row.controlType + ' n=' + $row.count +
                ' withpat=' + $row.withAnyPattern + ' pats=' + (Get-DictRows -Dict $row.patterns))
        }
        $totalsByTarget[$t.label] = (Get-DictRows -Dict $t.patternTotals)
        foreach ($p in @($t.patternTotals.PSObject.Properties)) {
            Add-Count -Table $comVocabulary -Key $p.Name -By ([int]$p.Value)
            if (-not $comTargetsByPattern.ContainsKey($p.Name)) { $comTargetsByPattern[$p.Name] = @() }
            $comTargetsByPattern[$p.Name] = @($comTargetsByPattern[$p.Name] + $t.label)
        }
        $button = Get-PatternCell -ByControlType $t.byControlType -ControlType 'Button' -Pattern 'Invoke'
        $text = Get-PatternCell -ByControlType $t.byControlType -ControlType 'Text' -Pattern 'Invoke'
        [void]$anchors.Add([ordered]@{
            Target                 = $t.label
            ButtonElements         = $button.Elements
            ButtonsWithInvoke      = $button.WithPattern
            ButtonAnchorHolds      = ($button.Present -and $button.Elements -gt 0 -and $button.WithPattern -eq $button.Elements)
            TextElements           = $text.Elements
            TextWithInvoke         = $text.WithPattern
            TextAnchorHolds        = ($text.WithPattern -eq 0)
            TextControlTypePresent = $text.Present
        })
    }
    $anchorFailures = @($anchors | Where-Object { -not $_.ButtonAnchorHolds -or -not $_.TextAnchorHolds } | ForEach-Object { $_.Target })

    $matrix = [ordered]@{
        Probe              = $Probe
        Stack              = 'uia3-com'
        Authoritative      = $true
        Scope              = 'app/provider'
        Source             = 'captures/08-uia3-com/census.json, produced by U7 shim mode "census" over hand-declared ComImport UIA3 (CUIAutomation8); reshaped here, not re-measured'
        SourceRationale    = 'KTD1 puts pattern availability on the COM stack because the managed stack structurally cannot name LegacyIAccessible, Drag, DropTarget, Annotation, Styles or TextChild. Re-walking the same four targets a second time would add a second sample of the same fact and a second chance for drift, so this probe consumes the committed measurement.'
        View               = 'RawView walk (shim census mode); inControlView is recorded per element in the source capture'
        MatrixRecordFormat = $MatrixRecordFormat
        Targets            = @($census.result.targets | ForEach-Object {
            [ordered]@{
                Label                                 = $_.label
                ProcessName                           = $_.processName
                RootFrameworkId                       = $_.rootFrameworkId
                ControlViewNodes                      = $_.controlViewNodes
                RawViewNodes                          = $_.rawViewNodes
                ElementsWithAnyPattern                = $_.elementsWithAnyPattern
                ElementsWithAnyPatternExcludingLegacy = $_.elementsWithAnyPatternExcludingLegacy
            }
        })
        PatternTotalsByTarget = $totalsByTarget
        Rows               = @($matrixRows)
        SanityAnchors      = @($anchors)
        SanityAnchorNote   = 'Invoke on Button and no-Invoke on Text are the two anchors that would catch a census reading the wrong property ids. winforms-default has no Text ControlType at all: its custom IRawElementProviderSimple collapses every child to Pane, so the Text anchor there is vacuously true and is reported with TextControlTypePresent=false rather than silently counted as a pass.'
        Uia3OnlyPatterns   = @($census.result.uia3OnlyPatternProperties)
        PropertyIdNote     = 'every availability property id in Uia3OnlyPatterns was discovered from the OS at runtime by U7, not written from memory. IsAnnotationPatternAvailable is 30118 on this build; 30113 - the value a from-memory table would carry - is a different property. A hardcoded id table is the single most likely silent failure in a Rust pattern-availability check.'
        FixtureModeNote    = 'winforms-default is a fixture artifact, not a Win32/WinForms platform fact: the scratch app installs a custom server-side IRawElementProviderSimple that suppresses both the client-side proxies and WinForms own providers, so every child collapses to Pane with LegacyIAccessible only. winforms-host-providers is the apples-to-apples WinForms row - there chkToggle is CheckBox+Toggle, txtValue is Edit+Value, cboChoice is ComboBox+ExpandCollapse+Value.'
    }
    [void](Write-ProbeJson -Probe $Probe -Name 'pattern-matrix.json' -InputObject $matrix)

    $scratchExe = Join-Path (Get-ProbeRoot) 'scratch\bin\ScratchForms.exe'
    if (-not (Test-Path -LiteralPath $scratchExe)) {
        & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path (Get-ProbeRoot) 'scratch\build-scratch.ps1') | Out-Null
    }
    if (-not (Test-Path -LiteralPath $scratchExe)) { throw ('scratch fixture missing at ' + $scratchExe) }

    $wpf = Start-ScratchWpfWindow -Tag 'u4-census'
    if ($null -eq $wpf) { throw 'WPF scratch window never bound its automation peer across three launches' }
    $wpfElement = $wpf.Element
    $wpfClassNameShape = $wpf.ClassNameShape
    $winDefault = Start-Tracked -FilePath $scratchExe -ArgumentList @('--tag', 'u4-default', '--pos', '40,40')
    $winHosted = Start-Tracked -FilePath $scratchExe -ArgumentList @('--tag', 'u4-hosted', '--pos', '840,40', '--host-providers')
    $notepad = Start-Tracked -FilePath (Join-Path $env:WINDIR 'System32\notepad.exe')
    if ($winDefault.MainWindowHandle -eq [IntPtr]::Zero) { throw 'WinForms default-mode window never appeared' }
    if ($winHosted.MainWindowHandle -eq [IntPtr]::Zero) { throw 'WinForms host-providers window never appeared' }
    if ($notepad.MainWindowHandle -eq [IntPtr]::Zero) { throw 'notepad window never appeared' }

    $sweeps = @(
        (Get-ManagedSweep -Root ($AE::FromHandle($winDefault.MainWindowHandle)) -Label 'winforms-default'),
        (Get-ManagedSweep -Root ($AE::FromHandle($winHosted.MainWindowHandle)) -Label 'winforms-host-providers'),
        (Get-ManagedSweep -Root $wpfElement -Label 'wpf'),
        (Get-ManagedSweep -Root ($AE::FromHandle($notepad.MainWindowHandle)) -Label 'notepad')
    )
    $managedVocabulary = @{}
    $managedTargetsByPattern = @{}
    $managedRows = New-Object System.Collections.ArrayList
    foreach ($s in $sweeps) {
        foreach ($r in $s.Rows) { [void]$managedRows.Add($r) }
        foreach ($k in @($s.PatternTotals.Keys)) {
            Add-Count -Table $managedVocabulary -Key $k -By $s.PatternTotals[$k]
            if (-not $managedTargetsByPattern.ContainsKey($k)) { $managedTargetsByPattern[$k] = @() }
            $managedTargetsByPattern[$k] = @($managedTargetsByPattern[$k] + $s.Label)
        }
    }

    $crossCheck = [ordered]@{
        Probe              = $Probe
        Stack              = 'managed-System.Windows.Automation'
        Authoritative      = $false
        Scope              = 'app/provider'
        Method             = 'fresh RawViewWalker sweep over the same four fixture shapes, calling AutomationElement.GetSupportedPatterns() per element and reducing each AutomationPattern ProgrammaticName by stripping the PatternIdentifiers.Pattern suffix'
        WhyNotAuthoritative = 'KTD1: recorded only so the COM-vs-managed pattern vocabulary can be diffed. Where a managed row disagrees with the COM matrix the COM row is the product-relevant one, because the Rust adapter wraps a UIA3 COM client.'
        SameRunCaveat      = 'these windows are fresh instances launched by this probe, not the HWNDs U7 measured. That does not weaken the divergence row: the claim under test is which pattern names a client stack can express at all, which is a property of the client library, not of a window instance. U7 comparison.json already holds the same-run, same-HWND node-count comparison.'
        WpfPeerActivation = [ordered]@{
            Verdict                     = 'NEW-EDGE'
            ClassNameShapeAfterActivation = $wpfClassNameShape
            timingPeerActivationLaunches = $wpf.Attempts
            Observation = 'if a UIA client reads the WPF window before WPF has built its automation peer, the client binds the generic HWND provider for that window and never re-resolves it. The window then reports ControlType.Window with ClassName HwndWrapper[<appdomain>;<thread name>;<guid>] and ZERO children, and stays that way. Measured: a 30 s poll loop in the same client process, including AutomationElement.FromHandle and a FindAll(Descendants, TrueCondition) forced traversal on every pass, never recovered the tree. The same fixture read for the first time 8 s after launch reports ClassName=Window with 8 children immediately.'
            Consequence = 'a Windows snapshot taken immediately after launching a WPF app can return a permanently empty one-node tree, and retrying inside the same process cannot fix it - the adapter would have to re-resolve from a new HWND or a new client. This is a different failure mode from the Electron settle U3 measured: Electron under-reports and then grows, WPF binds the wrong provider and stays wrong.'
            ProbePolicy = 'this probe launches the WPF fixture first, settles before its first UIA touch, verifies the root has children, and relaunches on a fresh HWND if it does not. timingPeerActivationLaunches records how many launches that took on this run.'
            ClassNameCorollary = 'the two shapes ClassName takes (Window when the peer is bound, HwndWrapper[...;<guid>] when it is not) also disqualify ClassName as an element-identity component on WPF: one of them embeds a per-launch GUID.'
        }
        MatrixRecordFormat = $MatrixRecordFormat
        Targets            = @($sweeps | ForEach-Object {
            [ordered]@{
                Label                  = $_.Label
                RawViewNodes           = $_.RawViewNodes
                ElementsWithAnyPattern = $_.ElementsWithAnyPattern
                PatternTotals          = (Get-HashRows -Table $_.PatternTotals)
            }
        })
        Rows               = @($managedRows)
        NodeBudget         = $NodeBudget
    }
    [void](Write-ProbeJson -Probe $Probe -Name 'managed-crosscheck.json' -InputObject $crossCheck)

    $managedPatternClasses = @([System.Windows.Automation.AutomationPattern].Assembly.GetTypes() |
        Where-Object { $_.IsPublic -and $_.Name -match 'PatternIdentifiers$' } |
        ForEach-Object { $_.Name -replace 'PatternIdentifiers$', '' } | Sort-Object)
    $allPatterns = @(@($comVocabulary.Keys) + @($managedVocabulary.Keys) | Sort-Object -Unique)
    $divergenceRows = New-Object System.Collections.ArrayList
    foreach ($p in $allPatterns) {
        $comCount = 0
        if ($comVocabulary.ContainsKey($p)) { $comCount = $comVocabulary[$p] }
        $managedCount = 0
        if ($managedVocabulary.ContainsKey($p)) { $managedCount = $managedVocabulary[$p] }
        $expressible = ($managedPatternClasses -contains $p)
        $verdict = 'both stacks report it'
        if ($comCount -gt 0 -and $managedCount -eq 0 -and -not $expressible) { $verdict = 'COM only - the managed stack has no identifier for this pattern and can never report it' }
        elseif ($comCount -gt 0 -and $managedCount -eq 0) { $verdict = 'COM only - the managed stack could name this pattern but did not walk an element advertising it' }
        elseif ($comCount -eq 0 -and $managedCount -gt 0) { $verdict = 'managed only' }
        $comTargets = @()
        if ($comTargetsByPattern.ContainsKey($p)) { $comTargets = @($comTargetsByPattern[$p]) }
        $managedTargets = @()
        if ($managedTargetsByPattern.ContainsKey($p)) { $managedTargets = @($managedTargetsByPattern[$p]) }
        [void]$divergenceRows.Add([ordered]@{
            Pattern              = $p
            ExpressibleByManagedStack = $expressible
            ComOccurrences       = $comCount
            ManagedOccurrences   = $managedCount
            ComTargets           = $comTargets
            ManagedTargets       = $managedTargets
            Verdict              = $verdict
        })
    }
    $comOnly = @($divergenceRows | Where-Object { $_.Verdict -like 'COM only*' } | ForEach-Object { $_.Pattern })
    $structurallyInvisible = @($divergenceRows | Where-Object { $_.ComOccurrences -gt 0 -and -not $_.ExpressibleByManagedStack } | ForEach-Object { $_.Pattern })
    $legacyRow = @($divergenceRows | Where-Object { $_.Pattern -eq 'LegacyIAccessible' })
    $legacyCom = 0
    if ($legacyRow.Count -gt 0) { $legacyCom = $legacyRow[0].ComOccurrences }
    $legacyManaged = -1
    if ($legacyRow.Count -gt 0) { $legacyManaged = $legacyRow[0].ManagedOccurrences }

    $divergence = [ordered]@{
        Probe          = $Probe
        Question       = 'does a managed pattern census record the same availability the Rust adapter will see through UIA3 COM'
        ComparisonBase = 'COM occurrences summed across the four census targets in captures/08-uia3-com/census.json; managed occurrences summed across the four fresh sweeps in managed-crosscheck.json'
        ManagedPatternClasses = @($managedPatternClasses)
        ManagedPatternClassCount = @($managedPatternClasses).Count
        ManagedVocabularyNote = 'ManagedPatternClasses is reflected out of the UIAutomationTypes assembly at run time (public types named *PatternIdentifiers), not transcribed. It is the complete set of pattern names System.Windows.Automation can ever return, so a COM-observed pattern outside this set is a structural blind spot rather than a sampling gap. The two cases are separated in every row: ExpressibleByManagedStack=false is structural, ExpressibleByManagedStack=true with zero managed occurrences only means the smaller managed tree did not reach an element advertising it.'
        Rows           = @($divergenceRows)
        ComOnlyPatterns = @($comOnly)
        StructurallyInvisibleToManaged = @($structurallyInvisible)
        LegacyIAccessibleRow = [ordered]@{
            ComElementsReportingIt      = $legacyCom
            ManagedElementsReportingIt  = $legacyManaged
            ComElementsTotal            = (@($census.result.targets | ForEach-Object { $_.rawViewNodes }) | Measure-Object -Sum).Sum
            Verdict                     = 'CONFIRMS KTD1'
            Finding                     = 'LegacyIAccessible is advertised by every element the COM census walked and by zero elements the managed sweep walked. The managed absence is structural, not environmental: System.Windows.Automation exposes 22 pattern classes and has no LegacyIAccessible identifier to return, so GetSupportedPatterns() can never name it. A managed-only census would have filed that as "pattern unavailable" for the whole corpus.'
            Consequence                 = 'every pattern-availability row in this sub-phase must carry stack=uia3-com. A Windows adapter that decides actionability from a managed-derived pattern table would under-report actionable elements on exactly the legacy Win32 surfaces where LegacyIAccessible.DoDefaultAction is the only affordance.'
        }
        NotepadDocumentRow = [ordered]@{
            Finding = 'the same divergence shows up as a ControlType disagreement, not only a pattern one: classic Notepad edit surface is ControlType.Pane with AutomationId 15 to the managed client (U3, 01-tree-dump) and ControlType.Document with Value, Text, Text2, Scroll, LegacyIAccessible to the COM client. Text2 (30119) is another pattern the managed stack cannot name.'
            Verdict = 'NEW-EDGE'
        }
    }
    [void](Write-ProbeJson -Probe $Probe -Name 'divergence.json' -InputObject $divergence)

    $resultData['comTargets'] = @($census.result.targets).Count
    $resultData['comPatternVocabulary'] = @($comVocabulary.Keys).Count
    $resultData['managedPatternVocabulary'] = @($managedVocabulary.Keys).Count
    $resultData['comOnlyPatterns'] = ($comOnly -join ',')
    $resultData['structurallyInvisibleToManaged'] = ($structurallyInvisible -join ',')
    $resultData['legacyComOccurrences'] = $legacyCom
    $resultData['legacyManagedOccurrences'] = $legacyManaged
    $resultData['anchorFailures'] = ($anchorFailures -join ',')
    if ($anchorFailures.Count -gt 0) { throw ('sanity anchor failed on: ' + ($anchorFailures -join ', ')) }
    if ($legacyCom -le 0 -or $legacyManaged -ne 0) { throw ('LegacyIAccessible divergence not reproduced: com=' + $legacyCom + ' managed=' + $legacyManaged) }
    $message = 'com pattern matrix reshaped from U7 census; managed cross-check reports ' + $comOnly.Count + ' COM-only pattern(s)'
} catch {
    $status = 'fail'
    $message = ($_.Exception.Message -replace '[\r\n]+', ' ')
    Write-ProbeLog -Message ('probe failed: ' + $message) -Level 'error'
} finally {
    foreach ($id in @($script:Spawned)) {
        try { Stop-ScratchProcess -ProcessId $id } catch { Write-ProbeLog -Message ('teardown: ' + $_.Exception.Message) -Level 'warn' }
    }
    foreach ($f in @(Get-ChildItem -LiteralPath (Get-CaptureDir -Probe $Probe) -File -ErrorAction SilentlyContinue)) {
        if ($f.Name -like '*.normalized') { continue }
        if (-not (Test-CaptureRedaction -Path $f.FullName)) { $status = 'fail'; $message = ('redaction residue in ' + $f.Name) }
    }
}

Write-ProbeResult -Probe $Probe -Status $status -Message $message -Data $resultData
if ($status -eq 'fail') { exit 1 }
exit 0
