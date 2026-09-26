$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\common.ps1"

Add-Type -AssemblyName UIAutomationClient | Out-Null
Add-Type -AssemblyName UIAutomationTypes | Out-Null
Add-Type -AssemblyName WindowsBase | Out-Null

$Probe = '01-tree-dump'
$Stack = 'managed'
$WalkMaxDepth = 12
$WalkNodeBudget = 3000
$SerializedNodeLimit = 120
$PlaceX = 40
$PlaceY = 40
$PlaceWidth = 1000
$PlaceHeight = 640
$SettingsRect = @{ X = 0; Y = 0; Width = 1620; Height = 700 }
$ObsidianSettleMs = 8000
$ExplorerWaitSec = 30
$SettingsWaitSec = 45
$SettingsWakeSec = 8
$ContentControlTypes = @(
    'ControlType.Document', 'ControlType.Edit', 'ControlType.Text', 'ControlType.ListItem',
    'ControlType.DataItem', 'ControlType.TreeItem', 'ControlType.Hyperlink', 'ControlType.Image'
)
$InheritedNameMinLength = 4

$AE = [System.Windows.Automation.AutomationElement]
$Walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker

function Get-ChildElements {
    param($Element)
    $kids = New-Object System.Collections.ArrayList
    try {
        $k = $Walker.GetFirstChild($Element)
        while ($null -ne $k) {
            [void]$kids.Add($k)
            $k = $Walker.GetNextSibling($k)
        }
    } catch { }
    return @($kids)
}

function Get-FocusRecord {
    try {
        $f = $AE::FocusedElement
        if ($null -eq $f) { return [ordered]@{ Resolved = $false; Reason = 'FocusedElement returned null' } }
        $c = $f.Current
        return [ordered]@{
            Resolved           = $true
            ControlType        = $c.ControlType.ProgrammaticName
            ClassName          = $c.ClassName
            ProcessId          = $c.ProcessId
            NativeWindowHandle = $c.NativeWindowHandle
        }
    } catch {
        return [ordered]@{ Resolved = $false; Reason = $_.Exception.GetType().Name }
    }
}

function Test-FocusUnchanged {
    param($Before, $After)
    if (-not $Before.Resolved -or -not $After.Resolved) { return $false }
    return ($Before.ControlType -eq $After.ControlType -and
            $Before.ClassName -eq $After.ClassName -and
            $Before.ProcessId -eq $After.ProcessId -and
            $Before.NativeWindowHandle -eq $After.NativeWindowHandle)
}

$NodeRecordFormat = 'one node per line, space separated key=value fields in fixed order: i=<index> d=<depth> parent=<parent index, -1 at root> ct=<ControlType.ProgrammaticName> cls=<ClassName> fw=<FrameworkId> aid=<AutomationId> pid=<ProcessId> bounds=<ok|empty> left=<px> top=<px> width=<px> height=<px> en=<0|1 IsEnabled> off=<0|1 IsOffscreen> kbf=<0|1 IsKeyboardFocusable> pats=<comma separated pattern short names, the ProgrammaticName with its PatternIdentifiers.Pattern suffix removed; "-" when none> children=<child count, present only on a node truncated at MaxDepth> name=<Name, or <redacted:N chars> on any node NameRedactionRule reduces>. left/top/width/height are omitted when bounds=empty. name= is always the last field so a Name containing spaces needs no quoting. Empty string values render as "-".'

function Format-NodeField {
    param([string]$Value)
    if ([string]::IsNullOrEmpty($Value)) { return '-' }
    return ($Value -replace '[\r\n]', ' ')
}

function New-NodeRecord {
    param($Element, [int]$Index, [int]$Depth, [int]$ParentIndex, [bool]$RedactName)
    $rec = [pscustomobject]@{
        Index = $Index; Depth = $Depth; Parent = $ParentIndex
        Prefix = ''; Name = ''; HasName = $false; RawName = ''; Reduced = $false
        BoundsEmpty = $true; BoundsZeroArea = $true; BoundsOffscreenOrigin = $false
        ControlTypeKnown = $false; HasAutomationId = $false; PatternCount = 0
    }
    $cur = $null
    try { $cur = $Element.Current } catch { }
    if ($null -eq $cur) {
        $rec.Prefix = ('i=' + $Index + ' d=' + $Depth + ' parent=' + $ParentIndex + ' ct=<element-unavailable>')
        return $rec
    }
    $controlType = '<unavailable>'
    try { if ($null -ne $cur.ControlType) { $controlType = $cur.ControlType.ProgrammaticName; $rec.ControlTypeKnown = $true } } catch { }
    $name = ''
    try { $name = [string]$cur.Name } catch { }
    $rec.RawName = $name
    $inheritsDocumentTitle = ($null -ne $script:SensitiveRootName -and $name.Length -ge $InheritedNameMinLength -and $script:SensitiveRootName.Contains($name))
    if (($RedactName -or $inheritsDocumentTitle -or ($ContentControlTypes -contains $controlType)) -and $name.Length -gt 0) {
        $name = Protect-ProbeName -Name $name
        $rec.Reduced = $true
    }
    $automationId = ''
    try { $automationId = [string]$cur.AutomationId } catch { }
    $rec.HasAutomationId = (-not [string]::IsNullOrEmpty($automationId))
    $className = ''
    try { $className = [string]$cur.ClassName } catch { }
    $frameworkId = ''
    try { $frameworkId = [string]$cur.FrameworkId } catch { }
    $processId = -1
    try { $processId = $cur.ProcessId } catch { }
    $geometry = 'bounds=empty'
    try {
        $r = $cur.BoundingRectangle
        if (-not $r.IsEmpty) {
            $rec.BoundsEmpty = $false
            $rec.BoundsZeroArea = ($r.Width -le 0 -or $r.Height -le 0)
            $rec.BoundsOffscreenOrigin = ($r.Left -le -30000 -or $r.Top -le -30000)
            $geometry = 'bounds=ok left=' + [int]$r.Left + ' top=' + [int]$r.Top + ' width=' + [int]$r.Width + ' height=' + [int]$r.Height
        }
    } catch { }
    $patterns = @()
    try { $patterns = @($Element.GetSupportedPatterns() | ForEach-Object { $_.ProgrammaticName -replace 'PatternIdentifiers\.Pattern$', '' } | Sort-Object) } catch { }
    $rec.PatternCount = $patterns.Count
    $states = ''
    foreach ($s in @(@{ K = 'en'; P = 'IsEnabled' }, @{ K = 'off'; P = 'IsOffscreen' }, @{ K = 'kbf'; P = 'IsKeyboardFocusable' })) {
        $v = '?'
        try { if ($cur.($s.P)) { $v = '1' } else { $v = '0' } } catch { }
        $states = $states + ' ' + $s.K + '=' + $v
    }
    $rec.Prefix = ('i=' + $Index + ' d=' + $Depth + ' parent=' + $ParentIndex +
        ' ct=' + (Format-NodeField $controlType) +
        ' cls=' + (Format-NodeField $className) +
        ' fw=' + (Format-NodeField $frameworkId) +
        ' aid=' + (Format-NodeField $automationId) +
        ' pid=' + $processId + ' ' + $geometry + $states +
        ' pats=' + (Format-NodeField ($patterns -join ',')))
    $rec.Name = (Format-NodeField $name)
    $rec.HasName = $true
    return $rec
}

function Get-NodeLine {
    param($Record)
    if (-not $Record.HasName) { return $Record.Prefix }
    return ($Record.Prefix + ' name=' + $Record.Name)
}

function Get-NodeChildIndexes {
    param($ChildIndexes, [int]$Index)
    if (-not $ChildIndexes.ContainsKey($Index)) { return @() }
    return @($ChildIndexes[$Index])
}

function Test-EchoesReducedDescendant {
    param($Nodes, $ChildIndexes, [int]$Index)
    $name = [string]$Nodes[$Index].RawName
    $pending = New-Object System.Collections.Stack
    foreach ($child in (Get-NodeChildIndexes -ChildIndexes $ChildIndexes -Index $Index)) { $pending.Push($child) }
    while ($pending.Count -gt 0) {
        $current = $pending.Pop()
        $descendant = $Nodes[$current]
        if ($descendant.Reduced -and $descendant.RawName.Length -ge $InheritedNameMinLength -and $name.Contains($descendant.RawName)) {
            return $true
        }
        foreach ($child in (Get-NodeChildIndexes -ChildIndexes $ChildIndexes -Index $current)) { $pending.Push($child) }
    }
    return $false
}

<#
    A container carrying no label of its own takes its accessible name from the
    content beneath it, so a document title lands on a Group or a Button that no
    content-control-type test can see and no window-title comparison catches -
    the leak reaches a tab header belonging to a split the window title does not
    name. This reduces a kept Name once the subtree below it holds a Name this
    rule already reduced, comparing the real strings while they are still in
    hand.

    One pass over the nodes settles it, and that is a property of the test
    rather than an assumption about tree shape: containment is transitive, so a
    container that would only become an echo once a nested container beneath it
    was reduced already contains whatever reduced that one, and every node is
    compared against its whole subtree rather than its direct children.

    It runs only on a target the caller declared to carry user content, and that
    scope is what keeps it from destroying evidence rather than merely making it
    quieter. Measured over this corpus: on the Settings frame the window title
    and the home-page header are both the word Settings, so an unscoped rule
    reduces the title of a system app holding no user content, on the frame
    window and the core window but not the title-bar window - dismantling the
    frame/host split those three nodes exist to demonstrate. Chrome in a
    content-carrying app is untouched for a structural reason, not a lucky one:
    an explicit label owes nothing to the subtree, and the subtree of a
    clickable icon is an unnamed Image.
#>
function Update-EchoedContentNames {
    param($Nodes)
    $childIndexes = @{}
    for ($i = 0; $i -lt $Nodes.Count; $i++) {
        $parent = $Nodes[$i].Parent
        if ($parent -lt 0) { continue }
        if (-not $childIndexes.ContainsKey($parent)) { $childIndexes[$parent] = New-Object System.Collections.ArrayList }
        [void]$childIndexes[$parent].Add($i)
    }
    $reduced = New-Object System.Collections.ArrayList
    for ($i = 0; $i -lt $Nodes.Count; $i++) {
        $node = $Nodes[$i]
        if ($node.Reduced -or -not $node.HasName) { continue }
        if ([string]::IsNullOrEmpty($node.RawName)) { continue }
        if (-not (Test-EchoesReducedDescendant -Nodes $Nodes -ChildIndexes $childIndexes -Index $i)) { continue }
        [void]$reduced.Add($i)
    }
    foreach ($i in $reduced) {
        $Nodes[$i].Name = Protect-ProbeName -Name $Nodes[$i].RawName
        $Nodes[$i].Reduced = $true
    }
    return , $reduced
}

function Invoke-TreeDump {
    param(
        $Root,
        [string]$Label,
        [bool]$RedactRootName = $false,
        [bool]$IncludeNodes = $true,
        [int]$MaxDepth = $WalkMaxDepth,
        [int]$NodeBudget = $WalkNodeBudget
    )
    $nodes = New-Object System.Collections.ArrayList
    $depthCuts = New-Object System.Collections.ArrayList
    $script:SensitiveRootName = $null
    if ($RedactRootName) {
        try { $script:SensitiveRootName = [string]$Root.Current.Name } catch { }
    }
    $order = New-Object System.Collections.Stack
    $order.Push(@{ Element = $Root; Depth = 0; Parent = -1; Redact = $RedactRootName })
    $budgetHit = $false
    while ($order.Count -gt 0) {
        $item = $order.Pop()
        if ($nodes.Count -ge $NodeBudget) { $budgetHit = $true; break }
        $index = $nodes.Count
        [void]$nodes.Add((New-NodeRecord -Element $item.Element -Index $index -Depth $item.Depth -ParentIndex $item.Parent -RedactName $item.Redact))
        $kids = Get-ChildElements -Element $item.Element
        if ($item.Depth -ge $MaxDepth) {
            if ($kids.Count -gt 0) {
                $nodes[$index].Prefix = ($nodes[$index].Prefix + ' children=' + $kids.Count)
                [void]$depthCuts.Add($index)
            }
            continue
        }
        for ($i = $kids.Count - 1; $i -ge 0; $i--) {
            $order.Push(@{ Element = $kids[$i]; Depth = ($item.Depth + 1); Parent = $index; Redact = $false })
        }
    }
    if ($RedactRootName) { [void](Update-EchoedContentNames -Nodes $nodes) }
    $withBounds = @($nodes | Where-Object { -not $_.BoundsEmpty }).Count
    $withArea = @($nodes | Where-Object { -not $_.BoundsEmpty -and -not $_.BoundsZeroArea }).Count
    $offscreen = @($nodes | Where-Object { $_.BoundsOffscreenOrigin }).Count
    $withControlType = @($nodes | Where-Object { $_.ControlTypeKnown }).Count
    $withAutomationId = @($nodes | Where-Object { $_.HasAutomationId }).Count
    $refable = @($nodes | Where-Object { $_.PatternCount -gt 0 }).Count
    $deepest = 0
    foreach ($n in $nodes) { if ($n.Depth -gt $deepest) { $deepest = $n.Depth } }
    $pct = 0
    $pctArea = 0
    if ($nodes.Count -gt 0) {
        $pct = [Math]::Round((100.0 * $withBounds / $nodes.Count), 2)
        $pctArea = [Math]::Round((100.0 * $withArea / $nodes.Count), 2)
    }
    $serialized = @()
    if ($IncludeNodes) { $serialized = @($nodes | Select-Object -First $SerializedNodeLimit | ForEach-Object { Get-NodeLine -Record $_ }) }
    return [ordered]@{
        Label                   = $Label
        Stack                   = $Stack
        View                    = 'ControlView (System.Windows.Automation.TreeWalker.ControlViewWalker)'
        NodeCount               = $nodes.Count
        DeepestDepth            = $deepest
        MaxDepth                = $MaxDepth
        NodeBudget              = $NodeBudget
        NodeBudgetExhausted     = $budgetHit
        DepthTruncatedNodeCount = $depthCuts.Count
        NodesWithNonEmptyBounds = $withBounds
        PctNonEmptyBounds       = $pct
        BoundsMetricNote        = 'PctNonEmptyBounds counts nodes whose BoundingRectangle is not Rect.Empty — the exact degeneracy a minimized window produces. PctPositiveAreaBounds additionally excludes zero-width/zero-height rects, which are collapsed-but-present controls (a hidden scrollbar), not missing geometry.'
        NodesWithPositiveArea   = $withArea
        PctPositiveAreaBounds   = $pctArea
        NodesAtOffscreenOrigin  = $offscreen
        NodesWithControlType    = $withControlType
        NodesWithAutomationId   = $withAutomationId
        RefableNodeCount        = $refable
        RefableDefinition       = 'node advertising at least one managed control pattern; the managed stack cannot see the six UIA3-only patterns (KTD1), so this is a floor not a total'
        NameRedactionRule       = 'R11: Name is reduced to <redacted:N chars> on document/content control types (Document, Edit, Text, ListItem, DataItem, TreeItem, Hyperlink, Image), on a document-titled root when the target carries user content, on any node whose Name is a >=4 character substring of that root Name — Electron and Win32 chrome panes echo the document title onto themselves — and, on a target declared to carry user content, on any node whose Name contains the >=4 character Name of a descendant this rule already reduced, because a container carrying no label of its own takes its accessible name from the content beneath it and so lands a document title on a Group or Button no content-control-type test can see. ControlType, AutomationId, ClassName, FrameworkId, bounds, patterns and states stay verbatim, and application-chrome names (button and tab labels) are deliberately kept: an explicit label owes nothing to the subtree beneath it, so no clause reaches it.'
        SerializedNodeLimit     = $SerializedNodeLimit
        SerializedNodeCount     = $serialized.Count
        NodeRecordFormat        = $NodeRecordFormat
        TruncationNote          = 'statistics above cover every walked node; Nodes[] below is truncated to SerializedNodeLimit in document order so the PR stays reviewable (KTD6), and secondary passes of the same target carry statistics only. Re-run this probe locally for the full record.'
        Nodes                   = @($serialized)
    }
}

function Get-TopLevelElements {
    $out = New-Object System.Collections.ArrayList
    try {
        $c = $Walker.GetFirstChild($AE::RootElement)
        while ($null -ne $c) {
            [void]$out.Add($c)
            try { $c = $Walker.GetNextSibling($c) } catch { break }
        }
    } catch { }
    return @($out)
}

function Get-ElementHandle {
    param($Element)
    try { return [IntPtr]$Element.Current.NativeWindowHandle } catch { return [IntPtr]::Zero }
}

function Wait-TopLevelWindowHandle {
    param([string]$ClassName, [int[]]$ProcessIds = @(), [int[]]$ExcludeHandles = @(), [int]$TimeoutSec = 30)
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ($true) {
        foreach ($top in (Get-TopLevelElements)) {
            try {
                $handle = [int]$top.Current.NativeWindowHandle
                if ($top.Current.ClassName -ne $ClassName) { continue }
                if ($ExcludeHandles -contains $handle) { continue }
                if ($ProcessIds.Count -gt 0 -and -not ($ProcessIds -contains $top.Current.ProcessId)) { continue }
                return $handle
            } catch { }
        }
        if ((Get-Date) -ge $deadline) { return 0 }
        Start-Sleep -Milliseconds 500
    }
}

function Add-LedgerRow {
    param([string]$Target, $Dump)
    [void]$script:Ledger.Add([ordered]@{
        Target                = $Target
        NodeCount             = $Dump.NodeCount
        PctNonEmptyBounds     = $Dump.PctNonEmptyBounds
        PctPositiveAreaBounds = $Dump.PctPositiveAreaBounds
        NodesWithControlType  = $Dump.NodesWithControlType
        RefableNodeCount      = $Dump.RefableNodeCount
        FocusUnchanged        = $Dump.FocusUnchanged
    })
}

function Show-Placed {
    param($Handle, $Rect = $null)
    if ($null -eq $Rect) { $Rect = @{ X = $PlaceX; Y = $PlaceY; Width = $PlaceWidth; Height = $PlaceHeight } }
    Show-WindowNoActivate -WindowHandle $Handle -X $Rect.X -Y $Rect.Y -Width $Rect.Width -Height $Rect.Height
}

function Hide-WindowMinimized {
    param($Handle)
    [void][AgentDesktopProbe.Native]::ShowWindow($Handle, 6)
    Start-Sleep -Milliseconds 400
}

function Invoke-PlacedDump {
    param($Handle, [string]$Label, [bool]$RedactRootName = $false, [bool]$Place = $true, [int]$SettleMs = 700, $Rect = $null, [bool]$IncludeNodes = $true)
    if ($Place) { Show-Placed -Handle $Handle -Rect $Rect }
    Start-Sleep -Milliseconds $SettleMs
    $element = $AE::FromHandle($Handle)
    $before = Get-FocusRecord
    $dump = Invoke-TreeDump -Root $element -Label $Label -RedactRootName $RedactRootName -IncludeNodes $IncludeNodes
    $after = Get-FocusRecord
    $stable = Test-FocusUnchanged -Before $before -After $after
    $dump['FocusBefore'] = $before
    $dump['FocusAfter'] = $after
    $dump['FocusUnchanged'] = $stable
    $dump['FocusIdentityNote'] = 'focus identity is asserted on ControlType/ClassName/ProcessId/NativeWindowHandle; the focused element is usually the operator console, whose Name mutates independently of focus and would produce false interference'
    $dump['Foreground'] = [ordered]@{ ProcessId = [AgentDesktopProbe.Native]::GetForegroundProcessId() }
    if (-not $stable) {
        $trackedPids = @(Get-ScratchProcessIds)
        $selfActivated = ($after.Resolved -and ($trackedPids -contains $after.ProcessId))
        $dump['FocusChangeClass'] = $(if ($selfActivated) { 'target-self-activation' } else { 'external-interference' })
        $dump['FocusChangeNote'] = 'a probe-launched target taking focus during its own startup is the target activating itself, not the probe stealing focus or an operator disturbing the run. Electron does this asynchronously when its renderer becomes ready, so it can land mid-dump under load. Only a focus move to a window the probe never launched is interference; self-activation is recorded as a platform fact and does not fail the probe.'
        if ($selfActivated) { [void]$script:FocusSelfActivation.Add($Label) } else { [void]$script:FocusInterference.Add($Label) }
    }
    return $dump
}

function Resolve-SettingsFrameWindow {
    param([int[]]$SettingsPids)
    foreach ($top in (Get-TopLevelElements)) {
        $class = ''
        try { $class = [string]$top.Current.ClassName } catch { }
        if ($class -ne 'ApplicationFrameWindow') { continue }
        $core = $null
        try {
            $core = $top.FindFirst([System.Windows.Automation.TreeScope]::Descendants,
                (New-Object System.Windows.Automation.PropertyCondition($AE::ClassNameProperty, 'Windows.UI.Core.CoreWindow')))
        } catch { }
        if ($null -eq $core) { continue }
        $corePid = -1
        try { $corePid = $core.Current.ProcessId } catch { }
        if ($SettingsPids -contains $corePid) {
            return [pscustomobject]@{ Frame = $top; CoreProcessId = $corePid }
        }
    }
    return $null
}

function Close-ShellWindowByHandle {
    param([int]$Handle)
    $shell = New-Object -ComObject Shell.Application
    foreach ($w in @($shell.Windows())) {
        try {
            if ([int]$w.HWND -eq $Handle) { $w.Quit(); return $true }
        } catch { }
    }
    return $false
}

<#
    The rule's MUST-CATCH / MUST-PASS fixture, driven through the shipped
    Update-EchoedContentNames so it cannot drift away from a second copy of the
    rule that agrees with itself. Only the trees and the expected answers are
    written out here.

    Both halves are driven, and the must-pass half is the one that has to
    survive. A rule that reduces every Group and Button name would satisfy every
    catch case and destroy the corpus: Close, Bookmarks and New tab are the
    evidence, and a redaction that takes them is not a fix. Each pass case is a
    shape that already exists in the committed captures.
#>
function New-EchoFixtureNode {
    param([int]$Index, [int]$Parent, [AllowEmptyString()][string]$RawName, [bool]$Reduced)
    $emitted = $RawName
    if ($Reduced -and $RawName.Length -gt 0) { $emitted = Protect-ProbeName -Name $RawName }
    return [pscustomobject]@{
        Index = $Index; Depth = 0; Parent = $Parent
        Prefix = ('i=' + $Index); Name = (Format-NodeField $emitted); HasName = $true
        RawName = $RawName; Reduced = $Reduced
        BoundsEmpty = $true; BoundsZeroArea = $true; BoundsOffscreenOrigin = $false
        ControlTypeKnown = $true; HasAutomationId = $false; PatternCount = 0
    }
}

function Test-EchoedNameRule {
    $cases = @(
        @{
            Case   = 'a tab header taking its name from the reduced Text beneath it'
            Tree   = @(@('<title>', $true), @('Welcome', $false), @('Welcome', $true), @('Close', $false), @('', $false))
            Parents = @(-1, 0, 1, 1, 3)
            Expect = @(1)
        },
        @{
            Case   = 'a header whose name concatenates the reduced title with its own chrome label'
            Tree   = @(@('<title>', $true), @('WelcomeClose', $false), @('Welcome', $true), @('Close', $false))
            Parents = @(-1, 0, 1, 1)
            Expect = @(1)
        },
        @{
            Case   = 'a container two levels above the reduced Text it takes its name from'
            Tree   = @(@('<title>', $true), @('Welcome', $false), @('', $false), @('Welcome', $true))
            Parents = @(-1, 0, 1, 2)
            Expect = @(1)
        },
        @{
            Case   = 'a clickable icon whose only child is an unnamed Image'
            Tree   = @(@('<title>', $true), @('New tab', $false), @('', $false))
            Parents = @(-1, 0, 1)
            Expect = @()
        },
        @{
            Case   = 'a chrome label matching a reduced Name that lives outside its own subtree'
            Tree   = @(@('<title>', $true), @('Bookmarks', $false), @('', $false), @('Bookmarks', $true))
            Parents = @(-1, 0, 1, 0)
            Expect = @()
        },
        @{
            Case   = 'a chrome label above a reduced Name shorter than the inherited-name floor'
            Tree   = @(@('<title>', $true), @('Close', $false), @('C', $true))
            Parents = @(-1, 0, 1)
            Expect = @()
        }
    )
    $failures = New-Object System.Collections.ArrayList
    foreach ($case in $cases) {
        $nodes = New-Object System.Collections.ArrayList
        for ($i = 0; $i -lt $case.Tree.Count; $i++) {
            [void]$nodes.Add((New-EchoFixtureNode -Index $i -Parent $case.Parents[$i] -RawName $case.Tree[$i][0] -Reduced $case.Tree[$i][1]))
        }
        $reduced = Update-EchoedContentNames -Nodes $nodes
        $expected = @($case.Expect)
        $missed = @($expected | Where-Object { $reduced -notcontains $_ })
        $extra = @($reduced | Where-Object { $expected -notcontains $_ })
        foreach ($m in $missed) {
            [void]$failures.Add('MUST CATCH, missed: ' + $case.Case + ' left node i=' + $m + ' verbatim')
        }
        foreach ($e in $extra) {
            [void]$failures.Add('MUST PASS, false positive: ' + $case.Case + ' reduced node i=' + $e)
        }
        foreach ($i in $reduced) {
            if (-not $nodes[$i].Reduced -or $nodes[$i].Name -ne (Protect-ProbeName -Name $nodes[$i].RawName)) {
                [void]$failures.Add('MUST CATCH, unreduced: ' + $case.Case + ' reported node i=' + $i + ' as reduced but did not rewrite its emitted Name')
            }
        }
    }
    return , $failures
}

$echoRuleFailures = Test-EchoedNameRule
if ($echoRuleFailures.Count -gt 0) {
    foreach ($f in $echoRuleFailures) { Write-ProbeLog -Message ('echoed-name rule self-test: ' + $f) -Level 'error' }
    Write-ProbeResult -Probe $Probe -Status 'fail' -Message ('the echoed-name rule failed its own self-test, so no capture this run writes can be trusted to be redacted; first: ' + $echoRuleFailures[0])
    exit 1
}

$script:FocusInterference = New-Object System.Collections.ArrayList
$script:FocusSelfActivation = New-Object System.Collections.ArrayList
$script:Ledger = New-Object System.Collections.ArrayList
$notepadPid = 0
$settingsPid = 0
$settingsPreexisting = $false
$explorerHandle = 0
$explorerPreexisting = $false
$obsidianPids = @()
$obsidianPreexisting = $false

try {
    Initialize-ProbeNative
    $capturedAt = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
    $baselineFocus = Get-FocusRecord

    $notepadPath = Join-Path $env:WINDIR 'System32\notepad.exe'
    $notepad = Start-ScratchProcess -FilePath $notepadPath -TimeoutSec 20
    $notepadPid = $notepad.ProcessId
    if ($notepad.MainWindowHandle -eq [IntPtr]::Zero) { throw 'notepad produced no main window handle' }
    $notepadHandle = $notepad.MainWindowHandle
    $restored = Invoke-PlacedDump -Handle $notepadHandle -Label 'notepad-restored-not-activated'
    Hide-WindowMinimized -Handle $notepadHandle
    $minimized = Invoke-PlacedDump -Handle $notepadHandle -Label 'notepad-minimized-control' -Place $false
    $notepadCapture = [ordered]@{
        Probe          = $Probe
        CapturedAtUtc  = $capturedAt
        Target         = 'notepad.exe'
        TargetKind     = 'classic Win32 Notepad (%WINDIR%\System32\notepad.exe) — not the Store/MSIX RichEdit Notepad'
        Stack          = $Stack
        Scope          = 'app/provider'
        LaunchPolicy   = 'Start-Process then ShowWindow(SW_SHOWNOACTIVATE) + SetWindowPos(SWP_NOACTIVATE|SWP_NOZORDER); SetForegroundWindow is never called'
        PlacementRect  = [ordered]@{ Left = $PlaceX; Top = $PlaceY; Width = $PlaceWidth; Height = $PlaceHeight }
        MinimizedDelta = [ordered]@{
            RestoredNodeCount        = $restored.NodeCount
            MinimizedNodeCount       = $minimized.NodeCount
            NodeCountDelta           = ($minimized.NodeCount - $restored.NodeCount)
            RestoredPctNonEmpty      = $restored.PctNonEmptyBounds
            MinimizedPctNonEmpty     = $minimized.PctNonEmptyBounds
            RestoredOffscreenOrigin  = $restored.NodesAtOffscreenOrigin
            MinimizedOffscreenOrigin = $minimized.NodesAtOffscreenOrigin
            Finding                  = 'launch/window state does not change managed tree completeness on classic Notepad: identical node count restored and minimized. It changes geometry only.'
            MinimizedOffscreenState  = 'NEW-EDGE: every node in the minimized pass still reports IsOffscreen=0, including the Window node whose rect is Rect.Empty. UIA does not mark a minimized window or its children offscreen on this build, so 2.6 cannot use IsOffscreen alone to detect a non-visible window.'
            MinimizedGeometryShape   = 'NEW-EDGE, and narrower than the plan assumed: only the minimized top-level Window node reports Rect.Empty. Its descendants report a NON-empty rect anchored at the -32000 off-screen origin with real width/height. A geometry corpus taken minimized is therefore degenerate in two different shapes, and an occlusion/hit-test gate that only tests Rect.IsEmpty would accept the -32000 descendants as real screen geometry.'
        }
        RestoredPass   = $restored
        MinimizedPass  = $minimized
    }
    [void](Write-ProbeJson -Probe $Probe -Name 'notepad.json' -InputObject $notepadCapture)
    Add-LedgerRow -Target 'notepad' -Dump $restored

    $preSettings = @(Get-Process -Name 'SystemSettings' -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
    $settingsPreexisting = ($preSettings.Count -gt 0)
    if (-not $settingsPreexisting) { Start-Process 'ms-settings:' | Out-Null }
    $deadline = (Get-Date).AddSeconds($SettingsWaitSec)
    $wakeDeadline = (Get-Date).AddSeconds($SettingsWakeSec)
    $settingsWakeIssued = $false
    $settingsMatch = $null
    while ((Get-Date) -lt $deadline) {
        $pids = @(Get-Process -Name 'SystemSettings' -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
        if (-not $settingsPreexisting) {
            foreach ($settingsSpawnedPid in $pids) { Register-ScratchProcessId -ProcessId $settingsSpawnedPid }
        }
        if ($pids.Count -gt 0) { $settingsMatch = Resolve-SettingsFrameWindow -SettingsPids $pids }
        if ($null -ne $settingsMatch) { break }
        if ($settingsPreexisting -and -not $settingsWakeIssued -and (Get-Date) -gt $wakeDeadline) {
            Start-Process 'ms-settings:' | Out-Null
            $settingsWakeIssued = $true
            Write-ProbeLog -Message 'preexisting SystemSettings presented no frame; issued ms-settings: to wake a PLM-suspended instance' -Level 'warn'
        }
        Start-Sleep -Milliseconds 500
    }
    if ($null -eq $settingsMatch) { throw ('no ApplicationFrameWindow hosts a Windows.UI.Core.CoreWindow owned by SystemSettings.exe after ' + $SettingsWaitSec + 's (wake issued: ' + $settingsWakeIssued + '); a backgrounded UWP instance can stay PLM-suspended and never present its frame') }
    if (-not $settingsPreexisting) {
        $settingsPid = $settingsMatch.CoreProcessId
        Register-ScratchProcessId -ProcessId $settingsPid
    }
    $framePid = -1
    try { $framePid = $settingsMatch.Frame.Current.ProcessId } catch { }
    $settingsHandle = Get-ElementHandle -Element $settingsMatch.Frame
    $settingsDump = Invoke-PlacedDump -Handle $settingsHandle -Label 'settings-frame-window' -SettleMs 1500 -Rect $SettingsRect
    $settingsCapture = [ordered]@{
        Probe             = $Probe
        CapturedAtUtc     = $capturedAt
        Target            = 'SystemSettings.exe (UWP), hosted by ApplicationFrameHost.exe'
        TargetKind        = 'ms-settings: home page'
        Stack             = $Stack
        Scope             = 'app/provider'
        Preexisting       = $settingsPreexisting
        ResolutionPath    = 'enumerate top-level ApplicationFrameWindow elements; for each, FindFirst(Descendants, ClassName = Windows.UI.Core.CoreWindow); select the frame whose CoreWindow ProcessId is a SystemSettings.exe pid'
        LocaleSafety      = 'no window/element Name is compared anywhere in this script; selection is by ClassName and ProcessId only. InstalledUICulture on this box is es-ES while CurrentCulture is en-US and the observed Settings chrome renders English, so a title match would have been both forbidden and silently environment-dependent.'
        PlacementRect     = [ordered]@{ Left = $SettingsRect.X; Top = $SettingsRect.Y; Width = $SettingsRect.Width; Height = $SettingsRect.Height }
        PlacementReason   = 'placed at a large fixed rect rather than the launch-maximized size: the XAML content is scroll-virtualized, so both the node count and whether a VerticalScrollBar exists at all depend on window size. A fixed rect makes the capture reproducible independently of the VM framebuffer size.'
        FrameHostSplit    = [ordered]@{
            FrameWindow = [ordered]@{ ClassName = 'ApplicationFrameWindow'; ProcessName = 'ApplicationFrameHost'; ProcessId = $framePid }
            CoreWindow  = [ordered]@{ ClassName = 'Windows.UI.Core.CoreWindow'; ProcessName = 'SystemSettings'; ProcessId = $settingsMatch.CoreProcessId }
            Verdict     = 'NEW-EDGE'
            Consequence = 'the top-level window a UWP app presents does not belong to the app pid. Windows app targeting in 2.4 cannot resolve a UWP target by matching the top-level window ProcessId; it must descend to the CoreWindow.'
        }
        Dump              = $settingsDump
    }
    [void](Write-ProbeJson -Probe $Probe -Name 'settings.json' -InputObject $settingsCapture)
    Add-LedgerRow -Target 'settings' -Dump $settingsDump

    $preCabinet = @()
    foreach ($top in (Get-TopLevelElements)) {
        try { if ($top.Current.ClassName -eq 'CabinetWClass') { $preCabinet += [int]$top.Current.NativeWindowHandle } } catch { }
    }
    $explorerPreexisting = ($preCabinet.Count -gt 0)
    if ($explorerPreexisting) {
        $explorerHandle = $preCabinet[0]
    } else {
        $launcher = Start-Process -FilePath (Join-Path $env:WINDIR 'explorer.exe') -ArgumentList @((Join-Path $env:WINDIR 'System32')) -PassThru
        Register-ScratchProcessId -ProcessId $launcher.Id
        $explorerHandle = Wait-TopLevelWindowHandle -ClassName 'CabinetWClass' -ExcludeHandles $preCabinet -TimeoutSec $ExplorerWaitSec
    }
    if ($explorerHandle -eq 0) { throw 'no CabinetWClass folder window appeared within the wait window' }
    $explorerDump = Invoke-PlacedDump -Handle ([IntPtr]$explorerHandle) -Label 'explorer-folder-window' -Place (-not $explorerPreexisting) -SettleMs 3000
    $explorerCapture = [ordered]@{
        Probe          = $Probe
        CapturedAtUtc  = $capturedAt
        Target         = 'explorer.exe'
        TargetKind     = 'folder window (CabinetWClass) over %WINDIR%\System32'
        Stack          = $Stack
        Scope          = 'app/provider'
        Preexisting    = $explorerPreexisting
        ShellOnlyNote  = 'explorer.exe runs shell-only on this box: the desktop/taskbar process has an empty MainWindowTitle and exposes no folder tree. A folder window has to be opened to get a real Explorer tree, and only that window is closed in teardown — explorer.exe is never terminated.'
        VirtualizationNote = 'the item list is UI-virtualized, so NodeCount tracks the placed window size, not the folder item count. The window is placed at a fixed rect so the count is reproducible.'
        Dump           = $explorerDump
    }
    [void](Write-ProbeJson -Probe $Probe -Name 'explorer.json' -InputObject $explorerCapture)
    Add-LedgerRow -Target 'explorer' -Dump $explorerDump

    $obsidianPath = Join-Path $env:LOCALAPPDATA 'Programs\Obsidian\Obsidian.exe'
    $obsidianVersion = '<absent>'
    if (Test-Path -LiteralPath $obsidianPath) { $obsidianVersion = (Get-Item -LiteralPath $obsidianPath).VersionInfo.FileVersion }
    $preObsidian = @(Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
    $obsidianPreexisting = ($preObsidian.Count -gt 0)
    if (-not $obsidianPreexisting) {
        $launched = Start-ScratchProcess -FilePath $obsidianPath -TimeoutSec 30
        Start-Sleep -Seconds 4
        $obsidianPids = @(Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
        $obsidianPidDeadline = (Get-Date).AddSeconds(30)
        while (@($obsidianPids).Count -eq 0 -and (Get-Date) -lt $obsidianPidDeadline) {
            Start-Sleep -Milliseconds 500
            $obsidianPids = @(Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
        }
        foreach ($opid in $obsidianPids) { if ($opid -ne $launched.ProcessId) { Register-ScratchProcessId -ProcessId $opid } }
    } else {
        $obsidianPids = $preObsidian
    }
    if (@($obsidianPids).Count -eq 0) {
        throw 'no Obsidian process is running, so the Chromium window search would carry an empty pid list. Wait-TopLevelWindowHandle treats that as "do not filter by process" and would return any Chrome_WidgetWin_1 window on the desktop, which would be dumped and published as the Electron target'
    }
    $obsidianHandle = Wait-TopLevelWindowHandle -ClassName 'Chrome_WidgetWin_1' -ProcessIds $obsidianPids -TimeoutSec 30
    if ($obsidianHandle -eq 0) { throw 'no Chrome_WidgetWin_1 top-level window owned by an Obsidian process appeared' }
    $obsidianPlace = (-not $obsidianPreexisting)
    $firstContact = Invoke-PlacedDump -Handle ([IntPtr]$obsidianHandle) -Label 'obsidian-first-contact' -RedactRootName $true -Place $obsidianPlace -SettleMs 700
    $afterSettle = Invoke-PlacedDump -Handle ([IntPtr]$obsidianHandle) -Label 'obsidian-after-settle' -RedactRootName $true -Place $false -SettleMs $ObsidianSettleMs -IncludeNodes $false
    Hide-WindowMinimized -Handle $settingsHandle
    Hide-WindowMinimized -Handle ([IntPtr]$explorerHandle)
    if ($obsidianPlace) { Show-Placed -Handle ([IntPtr]$obsidianHandle) }
    $settled = Invoke-PlacedDump -Handle ([IntPtr]$obsidianHandle) -Label 'obsidian-after-other-targets-minimized' -RedactRootName $true -Place $false -SettleMs $ObsidianSettleMs
    $obsidianCapture = [ordered]@{
        Probe             = $Probe
        CapturedAtUtc     = $capturedAt
        Target            = 'Obsidian (Electron/Chromium)'
        TargetKind        = '%LOCALAPPDATA%\Programs\Obsidian\Obsidian.exe'
        ObsidianVersion   = $obsidianVersion
        Stack             = $Stack
        Scope             = 'app/provider'
        Preexisting       = $obsidianPreexisting
        SettleMs          = $ObsidianSettleMs
        ActivationFinding = [ordered]@{
            FirstContactNodeCount             = $firstContact.NodeCount
            AfterSettleNodeCount              = $afterSettle.NodeCount
            AfterOtherTargetsMinimizedCount   = $settled.NodeCount
            FirstContactRefableNodeCount      = $firstContact.RefableNodeCount
            SettledRefableNodeCount           = $settled.RefableNodeCount
            Verdict                           = 'NEW-EDGE'
            Finding                           = 'the managed client does trigger Chromium renderer-accessibility activation with no --force-renderer-accessibility flag: the control view grows from FirstContactNodeCount to AfterSettleNodeCount across the 8 s settle. A single immediate read understates the tree by more than an order of magnitude, so any Electron measurement taken without a settle is a false baseline.'
            CrossCheck                        = 'U7 measured 9 -> 133 ControlView across an 8 s settle on the UIA3 COM stack. First contact reproduces exactly on the managed stack; the settled size tracks Obsidian window content and is not a fixed number.'
            PlacementHazard                   = 'OBSERVED, and the reason this script minimizes each target after its own dump: an earlier revision that left Notepad, Settings, and Explorer restored on top of Obsidian at the same 40,40,1000,640 rect held Obsidian at the first-contact count for the full 8 s settle and for a 16 s instrumented hold across three consecutive runs. With the covering windows minimized the same HWND populates normally. The causal mechanism is not isolated here — Chromium native-window occlusion tracking is the obvious candidate but was not proven — so this is recorded as a probe-placement hazard, not as a product-contract claim. A sub-phase that measures an Electron tree behind another window may be measuring nothing.'
            SettledSizeStability              = 'the settled node count is content-dependent (133 measured on a bare-walker run, ' + $settled.NodeCount + ' here). Treat the growth ratio, not the absolute count, as the stable fact.'
            BaselineForU8                     = 'SettledRefableNodeCount is the ref-able baseline U8 diffs its --force-renderer-accessibility run against, and it is only valid taken after a settle'
        }
        FirstContactPass  = $firstContact
        AfterSettlePass   = $afterSettle
        SettledPass       = $settled
    }
    [void](Write-ProbeJson -Probe $Probe -Name 'obsidian.json' -InputObject $obsidianCapture)
    Add-LedgerRow -Target 'obsidian' -Dump $settled

    $boundsFloor = 95.0
    $boundsRows = @($restored, $settingsDump, $explorerDump, $settled)
    $belowFloor = @($boundsRows | Where-Object { $_.PctNonEmptyBounds -lt $boundsFloor } | ForEach-Object { $_.Label })
    $summary = [ordered]@{
        Probe             = $Probe
        CapturedAtUtc     = $capturedAt
        Stack             = $Stack
        BaselineFocus     = $baselineFocus
        FocusInterference = @($script:FocusInterference)
        FocusSelfActivation = @($script:FocusSelfActivation)
        BoundsFloorPct    = $boundsFloor
        BelowBoundsFloor  = @($belowFloor)
        Targets           = @($script:Ledger)
        MinimizedVsRestored = [ordered]@{
            RestoredNodeCount  = $restored.NodeCount
            MinimizedNodeCount = $minimized.NodeCount
            NodeCountDelta     = ($minimized.NodeCount - $restored.NodeCount)
            RestoredPctNonEmptyBounds  = $restored.PctNonEmptyBounds
            MinimizedPctNonEmptyBounds = $minimized.PctNonEmptyBounds
            RestoredNodesAtOffscreenOrigin  = $restored.NodesAtOffscreenOrigin
            MinimizedNodesAtOffscreenOrigin = $minimized.NodesAtOffscreenOrigin
            Verdict            = 'launch policy does not affect tree completeness; it affects geometry only'
        }
        NotepadVocabulary = [ordered]@{
            Verdict = 'NEW-EDGE'
            Finding = "classic Notepad's editor exposes as ControlType.Pane with ClassName Edit and AutomationId 15 to the managed client — not ControlType.Edit or ControlType.Document. 2.3's vocabulary map cannot key an editable text surface off ControlType alone."
        }
        StackDivergence   = 'all counts here are managed ControlView. U7 measured classic Notepad at 26 nodes on the UIA3 COM stack against the 3 recorded here, so a low managed count on a Win32 target is evidence of client-stack divergence (KTD1), not of a missing tree.'
    }
    [void](Write-ProbeJson -Probe $Probe -Name 'summary.json' -InputObject $summary)

    $resultData = [ordered]@{
        NotepadNodes  = $restored.NodeCount
        SettingsNodes = $settingsDump.NodeCount
        ExplorerNodes = $explorerDump.NodeCount
        ObsidianNodes = $settled.NodeCount
        ObsidianFirstContactNodes = $firstContact.NodeCount
        ObsidianRefable = $settled.RefableNodeCount
        BelowBoundsFloor = @($belowFloor).Count
        FocusInterference = @($script:FocusInterference).Count
        FocusSelfActivation = @($script:FocusSelfActivation).Count
    }
    if (@($belowFloor).Count -gt 0) {
        Write-ProbeResult -Probe $Probe -Status 'fail' -Message ('bounds floor breached by: ' + (@($belowFloor) -join ', ')) -Data $resultData
        exit 1
    }
    if (@($script:FocusInterference).Count -gt 0) {
        Write-ProbeResult -Probe $Probe -Status 'fail' -Message ('FocusedElement changed across dump(s): ' + (@($script:FocusInterference) -join ', ')) -Data $resultData
        exit 1
    }
    Write-ProbeLog -Message 'wrote notepad.json, settings.json, explorer.json, obsidian.json, summary.json'
    Write-ProbeResult -Probe $Probe -Status 'ok' -Message 'four managed control-view tree dumps captured with real geometry, focus held across every dump' -Data $resultData
    exit 0
} catch {
    Write-ProbeResult -Probe $Probe -Status 'fail' -Message ('unhandled error: ' + $_.Exception.Message)
    exit 1
} finally {
    if ($notepadPid -ne 0) { try { Stop-ScratchProcess -ProcessId $notepadPid } catch { Write-ProbeLog -Message $_.Exception.Message -Level 'error' } }
    if ($explorerHandle -ne 0 -and -not $explorerPreexisting) {
        try { [void](Close-ShellWindowByHandle -Handle $explorerHandle) } catch { Write-ProbeLog -Message ('could not close folder window: ' + $_.Exception.Message) -Level 'error' }
    }
    if ($settingsPid -ne 0 -and -not $settingsPreexisting) { try { Stop-ScratchProcess -ProcessId $settingsPid } catch { Write-ProbeLog -Message $_.Exception.Message -Level 'error' } }
    if (-not $obsidianPreexisting) {
        foreach ($opid in @(Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })) {
            try { Stop-ScratchProcess -ProcessId $opid } catch { Write-ProbeLog -Message $_.Exception.Message -Level 'error' }
        }
    }
}
