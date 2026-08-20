<#
.SYNOPSIS
    Probe 05 (sub-phase 2.0, unit U5): every 2.0(3) interaction driven through UIA
    patterns against probe-owned fixtures, each verified by an independent re-read.

.DESCRIPTION
    Stack: managed System.Windows.Automation. Scope: app/provider.

    Every row is pre-state -> action -> independently re-read post-state. The pattern
    call's own return value is never the evidence: state is re-read from the element
    and, wherever the fixture offers one, from a separate observable sink control
    (lblStatus / lblScrollPos), which is a different UIA element than the one acted on.

    Target choice is measured, not assumed. U3 established that the WinForms fixture in
    default mode exposes ZERO patterns to a managed client (its server-side
    IRawElementProviderSimple suppresses both the client-side proxies and WinForms' own
    providers, collapsing every node to Pane) and that --host-providers restores only
    two interactive elements. The WPF fixture carries the full managed pattern surface,
    so it drives the pattern interactions and the WinForms modes are recorded as
    comparison rows - a fixture artifact filed as such, not as a platform fact.

    A failed interaction is a verdict row, not a script error.

    Captures under captures/05-interactions/:
        interactions.json  one row per interaction with pre/post evidence
        text-pattern.json  TextPattern exposure on classic Notepad's Edit vs WPF's TextBox
        focus.json         SetFocus vs foreground, headless-invariant evidence for 2.7
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\common.ps1"

Add-Type -AssemblyName UIAutomationClient | Out-Null
Add-Type -AssemblyName UIAutomationTypes | Out-Null

$Probe = '05-interactions'
$AE = [System.Windows.Automation.AutomationElement]
$Walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker
$Descendants = [System.Windows.Automation.TreeScope]::Descendants
$ControlIds = @{
    chkToggle = '1001'; txtValue = '1003'; cboChoice = '1004'; btnAction = '1005'
    btnMutateList = '1006'; tbSlider = '1008'; lstItems = '1010'; pnlScroll = '1011'
    lblStatus = '1020'; lblScrollPos = '1022'
}
$script:Spawned = New-Object System.Collections.ArrayList
$script:Rows = New-Object System.Collections.ArrayList

function Initialize-InteractNative {
    if ('AgentDesktopProbe.Interact' -as [type]) { return }
    $src = @'
using System;
using System.Runtime.InteropServices;
using System.Text;
namespace AgentDesktopProbe {
    public static class Interact {
        [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        public static extern IntPtr FindWindowExW(IntPtr parent, IntPtr child, string cls, string title);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        public static extern int GetWindowTextW(IntPtr hWnd, StringBuilder text, int count);
        [DllImport("user32.dll")]
        public static extern IntPtr GetForegroundWindow();
        public static string WindowTitle(IntPtr h) {
            StringBuilder sb = new StringBuilder(512);
            GetWindowTextW(h, sb, sb.Capacity);
            return sb.ToString();
        }
        public static string ForegroundTitle() {
            return WindowTitle(GetForegroundWindow());
        }
    }
}
'@
    Add-Type -TypeDefinition $src -Language CSharp | Out-Null
}

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
        $k = $Walker.GetFirstChild($Element)
        while ($null -ne $k) { $n++; $k = $Walker.GetNextSibling($k) }
    } catch { }
    return $n
}

function Wait-TopLevelByTitle {
    param([string]$Title, [int]$TimeoutSec = 25)
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ($true) {
        try {
            $c = $Walker.GetFirstChild($AE::RootElement)
            while ($null -ne $c) {
                try { if ($c.Current.Name -eq $Title) { return $c } } catch { }
                $c = $Walker.GetNextSibling($c)
            }
        } catch { }
        if ((Get-Date) -ge $deadline) { return $null }
        Start-Sleep -Milliseconds 400
    }
}

function Start-ScratchWpfWindow {
    param([string]$Tag, [int]$Left, [int]$Top, [int]$SettleSec = 8, [int]$Attempts = 3)
    $title = 'AgentDesktop Scratch WPF [' + $Tag + ']'
    for ($i = 1; $i -le $Attempts; $i++) {
        $proc = Start-Tracked -FilePath 'powershell.exe' -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File',
            (Join-Path (Get-ProbeRoot) 'scratch\ScratchWpf.ps1'), '-Tag', $Tag,
            '-Left', ([string]$Left), '-Top', ([string]$Top), '-TimeoutSeconds', '600')
        Start-Sleep -Seconds $SettleSec
        $element = Wait-TopLevelByTitle -Title $title -TimeoutSec 20
        if ($null -ne $element -and (Get-ChildCount -Element $element) -gt 0) {
            $hostPid = 0
            try { $hostPid = [int]$element.Current.ProcessId } catch { }
            if ($hostPid -gt 0 -and -not $script:Spawned.Contains($hostPid)) {
                Register-ScratchProcessId -ProcessId $hostPid
                [void]$script:Spawned.Add($hostPid)
            }
            return [pscustomobject]@{ Element = $element; ProcessId = $hostPid; timingLaunches = $i }
        }
        Write-ProbeLog -Message ('WPF automation peer not bound on attempt ' + $i + '; relaunching for a fresh HWND') -Level 'warn'
        try { Stop-ScratchProcess -ProcessId $proc.ProcessId } catch { }
    }
    return $null
}

function Find-ScratchElement {
    param($Root, [string]$Symbolic, [string]$Numeric = '')
    foreach ($aid in @($Symbolic, $Numeric)) {
        if ([string]::IsNullOrEmpty($aid)) { continue }
        try {
            $c = New-Object System.Windows.Automation.PropertyCondition($AE::AutomationIdProperty, $aid)
            $e = $Root.FindFirst($Descendants, $c)
            if ($null -ne $e) { return $e }
        } catch { }
    }
    return $null
}

function Find-Scratch {
    param($Root, [string]$Name)
    $numeric = ''
    if ($ControlIds.ContainsKey($Name)) { $numeric = $ControlIds[$Name] }
    return (Find-ScratchElement -Root $Root -Symbolic $Name -Numeric $numeric)
}

function Get-SinkText {
    param($Root, [string]$Name = 'lblStatus')
    $e = Find-Scratch -Root $Root -Name $Name
    if ($null -eq $e) { return '<sink-not-found>' }
    try { return [string]$e.Current.Name } catch { return '<sink-read-failed>' }
}

function Get-SupportedPatternNames {
    param($El)
    $names = @()
    try {
        foreach ($p in @($El.GetSupportedPatterns())) {
            $names += ($p.ProgrammaticName -replace 'PatternIdentifiers\.Pattern$', '')
        }
    } catch { }
    return @($names | Sort-Object)
}

function Get-ElementPattern {
    param($El, $PatternIdentifier)
    $obj = $null
    try { if ($El.TryGetCurrentPattern($PatternIdentifier, [ref]$obj)) { return $obj } } catch { }
    return $null
}

function Get-TextShape {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][AllowNull()][string]$Text)
    if ($null -eq $Text) { $Text = '' }
    $sha = [System.Security.Cryptography.SHA256]::Create()
    $hash = (($sha.ComputeHash([System.Text.Encoding]::Unicode.GetBytes($Text))) | ForEach-Object { $_.ToString('x2') }) -join ''
    $sha.Dispose()
    $codepoints = 0
    $pairs = 0
    $replacement = 0
    for ($i = 0; $i -lt $Text.Length; $i++) {
        $c = $Text[$i]
        if ($c -eq [char]0xFFFD) { $replacement++ }
        $codepoints++
        if ([char]::IsHighSurrogate($c) -and ($i + 1) -lt $Text.Length -and [char]::IsLowSurrogate($Text[$i + 1])) {
            $pairs++
            $i++
        }
    }
    return [ordered]@{
        utf16Units       = $Text.Length
        codepoints       = $codepoints
        surrogatePairs   = $pairs
        replacementChars = $replacement
        sha256Utf16      = $hash
    }
}

function Add-InteractionRow {
    param(
        [string]$Interaction, [string]$Target, [string]$AutomationId,
        [string]$Pattern, $Evidence
    )
    $row = [ordered]@{
        interaction  = $Interaction
        target       = $Target
        stack        = 'managed-System.Windows.Automation'
        automationId = $AutomationId
        pattern      = $Pattern
    }
    foreach ($k in $Evidence.Keys) { $row[$k] = $Evidence[$k] }
    [void]$script:Rows.Add($row)
}

function Add-PatternAbsentRow {
    param([string]$Interaction, [string]$Target, [string]$AutomationId, [string]$Pattern, $El, [string]$Detail)
    $supported = @()
    $controlType = '<element-not-found>'
    if ($null -ne $El) {
        $supported = Get-SupportedPatternNames -El $El
        try { $controlType = $El.Current.ControlType.ProgrammaticName -replace '^ControlType\.', '' } catch { }
    }
    Add-InteractionRow -Interaction $Interaction -Target $Target -AutomationId $AutomationId -Pattern $Pattern -Evidence ([ordered]@{
        controlType      = $controlType
        patternAcquired  = $false
        supportedPatterns = @($supported)
        verdict          = 'pattern-unavailable'
        detail           = $Detail
    })
}

$status = 'ok'
$message = ''
$resultData = [ordered]@{}

try {
    Initialize-ProbeNative
    Initialize-InteractNative
    $scratchExe = Join-Path (Get-ProbeRoot) 'scratch\bin\ScratchForms.exe'
    if (-not (Test-Path -LiteralPath $scratchExe)) {
        & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path (Get-ProbeRoot) 'scratch\build-scratch.ps1') | Out-Null
    }
    if (-not (Test-Path -LiteralPath $scratchExe)) { throw ('scratch fixture missing at ' + $scratchExe) }

    $asciiPayload = 'probe-value-ascii-01'
    $cjkPayload = [char]::ConvertFromUtf32(0x4E2D) + [char]::ConvertFromUtf32(0x6587) + [char]::ConvertFromUtf32(0x30C6)
    $astralPayload = 'a' + [char]::ConvertFromUtf32(0x1F600) + 'z'
    $mixedPayload = $asciiPayload + '-' + $cjkPayload + '-' + $astralPayload

    # --- WPF first: its automation peer binds once and never re-resolves ----
    $wpf = Start-ScratchWpfWindow -Tag 'u5' -Left 400 -Top 250
    if ($null -eq $wpf) { throw 'WPF scratch window never bound its automation peer across three launches' }
    $wpfRoot = $wpf.Element
    $wpfHandle = [IntPtr][int]$wpfRoot.Current.NativeWindowHandle

    $wfDefault = Start-Tracked -FilePath $scratchExe -ArgumentList @('--tag', 'u5-default', '--pos', '0,0')
    if ($wfDefault.MainWindowHandle -eq [IntPtr]::Zero) { throw 'WinForms default-mode window never appeared' }
    $wfDefaultRoot = $AE::FromHandle($wfDefault.MainWindowHandle)

    $wfHosted = Start-Tracked -FilePath $scratchExe -ArgumentList @('--tag', 'u5-hosted', '--pos', '860,0', '--host-providers')
    if ($wfHosted.MainWindowHandle -eq [IntPtr]::Zero) { throw 'WinForms host-providers window never appeared' }
    $wfHostedRoot = $AE::FromHandle($wfHosted.MainWindowHandle)

    $targets = @(
        [pscustomobject]@{ Label = 'wpf'; Root = $wpfRoot },
        [pscustomobject]@{ Label = 'winforms-host-providers'; Root = $wfHostedRoot },
        [pscustomobject]@{ Label = 'winforms-default'; Root = $wfDefaultRoot }
    )

    # --- invoke -------------------------------------------------------------
    foreach ($t in $targets) {
        $el = Find-Scratch -Root $t.Root -Name 'btnAction'
        $ip = $null
        if ($null -ne $el) { $ip = Get-ElementPattern -El $el -PatternIdentifier ([System.Windows.Automation.InvokePattern]::Pattern) }
        if ($null -eq $ip) {
            Add-PatternAbsentRow -Interaction 'invoke' -Target $t.Label -AutomationId 'btnAction' -Pattern 'Invoke' -El $el `
                -Detail 'no InvokePattern on this element in this fixture mode; recorded as a verdict row'
            continue
        }
        $before = Get-SinkText -Root $t.Root
        $ip.Invoke()
        Start-Sleep -Milliseconds 500
        $after = Get-SinkText -Root $t.Root
        Add-InteractionRow -Interaction 'invoke' -Target $t.Label -AutomationId 'btnAction' -Pattern 'Invoke' -Evidence ([ordered]@{
            controlType       = ($el.Current.ControlType.ProgrammaticName -replace '^ControlType\.', '')
            patternAcquired   = $true
            supportedPatterns = @(Get-SupportedPatternNames -El $el)
            sinkElement       = 'lblStatus (a different element than the one invoked)'
            sinkBefore        = $before
            sinkAfter         = $after
            changed           = ($before -ne $after)
            verdict           = $(if ($after -eq 'action:1') { 'ok' } else { 'unexpected-sink-value' })
        })
    }

    # --- toggle -------------------------------------------------------------
    foreach ($t in $targets) {
        $el = Find-Scratch -Root $t.Root -Name 'chkToggle'
        $tp = $null
        if ($null -ne $el) { $tp = Get-ElementPattern -El $el -PatternIdentifier ([System.Windows.Automation.TogglePattern]::Pattern) }
        if ($null -eq $tp) {
            Add-PatternAbsentRow -Interaction 'toggle' -Target $t.Label -AutomationId 'chkToggle' -Pattern 'Toggle' -El $el `
                -Detail 'no TogglePattern on this element in this fixture mode; recorded as a verdict row'
            continue
        }
        $pre = [string]$tp.Current.ToggleState
        $sinkBefore = Get-SinkText -Root $t.Root
        $tp.Toggle()
        Start-Sleep -Milliseconds 500
        $post = [string](Get-ElementPattern -El (Find-Scratch -Root $t.Root -Name 'chkToggle') `
                -PatternIdentifier ([System.Windows.Automation.TogglePattern]::Pattern)).Current.ToggleState
        $sinkAfter = Get-SinkText -Root $t.Root
        Add-InteractionRow -Interaction 'toggle' -Target $t.Label -AutomationId 'chkToggle' -Pattern 'Toggle' -Evidence ([ordered]@{
            controlType     = ($el.Current.ControlType.ProgrammaticName -replace '^ControlType\.', '')
            patternAcquired = $true
            preState        = $pre
            postState       = $post
            reReadMethod    = 'the element is re-found by AutomationId and its TogglePattern re-acquired, so the post-state is not read off the same pattern object the call returned'
            sinkBefore      = $sinkBefore
            sinkAfter       = $sinkAfter
            sinkChanged     = ($sinkBefore -ne $sinkAfter)
            sinkNote        = 'the WPF fixture updates lblStatus from a Click handler. TogglePattern.Toggle flips ToggleState without raising Click, so the sink is expected to stay silent here while the element state changes - which is exactly why the element is re-read instead of the sink being trusted as the only observable.'
            changed         = ($pre -ne $post)
            verdict         = $(if ($pre -eq 'Off' -and $post -eq 'On') { 'ok' } else { 'unexpected-state' })
        })
    }

    # --- set value (ASCII, CJK, astral plane) -------------------------------
    $valueTargets = @(
        [pscustomobject]@{ Label = 'wpf'; Root = $wpfRoot },
        [pscustomobject]@{ Label = 'winforms-host-providers'; Root = $wfHostedRoot }
    )
    foreach ($t in $valueTargets) {
        $el = Find-Scratch -Root $t.Root -Name 'txtValue'
        $vp = $null
        if ($null -ne $el) { $vp = Get-ElementPattern -El $el -PatternIdentifier ([System.Windows.Automation.ValuePattern]::Pattern) }
        if ($null -eq $vp) {
            Add-PatternAbsentRow -Interaction 'set-value' -Target $t.Label -AutomationId 'txtValue' -Pattern 'Value' -El $el `
                -Detail 'no ValuePattern on this element in this fixture mode; recorded as a verdict row'
            continue
        }
        foreach ($payload in @(
                [pscustomobject]@{ Kind = 'ascii'; Text = $asciiPayload },
                [pscustomobject]@{ Kind = 'cjk'; Text = $cjkPayload },
                [pscustomobject]@{ Kind = 'astral-plane'; Text = $astralPayload },
                [pscustomobject]@{ Kind = 'mixed'; Text = $mixedPayload })) {
            $preValue = [string]$vp.Current.Value
            $vp.SetValue($payload.Text)
            Start-Sleep -Milliseconds 300
            $fresh = Find-Scratch -Root $t.Root -Name 'txtValue'
            $freshVp = Get-ElementPattern -El $fresh -PatternIdentifier ([System.Windows.Automation.ValuePattern]::Pattern)
            $observed = ''
            if ($null -ne $freshVp) { $observed = [string]$freshVp.Current.Value }
            $expectedShape = Get-TextShape -Text $payload.Text
            $observedShape = Get-TextShape -Text $observed
            Add-InteractionRow -Interaction ('set-value/' + $payload.Kind) -Target $t.Label -AutomationId 'txtValue' -Pattern 'Value' -Evidence ([ordered]@{
                controlType     = ($el.Current.ControlType.ProgrammaticName -replace '^ControlType\.', '')
                patternAcquired = $true
                payloadKind     = $payload.Kind
                payloadOrigin   = 'built with [char]::ConvertFromUtf32 so payload integrity does not depend on this file''s encoding (R12)'
                preValueShape   = (Get-TextShape -Text $preValue)
                expectedShape   = $expectedShape
                observedShape   = $observedShape
                exactRoundTrip  = ($expectedShape.sha256Utf16 -eq $observedShape.sha256Utf16)
                reReadMethod    = 'element re-found by AutomationId and ValuePattern re-acquired before reading'
                payloadHandling = 'only length, codepoint counts and a SHA-256 of the UTF-16 bytes are recorded; no payload text reaches the capture'
                verdict         = $(if ($expectedShape.sha256Utf16 -eq $observedShape.sha256Utf16) { 'ok' } else { 'round-trip-mismatch' })
            })
        }
        $vp.SetValue('seed-value')
    }

    # --- expand / collapse --------------------------------------------------
    foreach ($t in $targets) {
        $el = Find-Scratch -Root $t.Root -Name 'cboChoice'
        $ep = $null
        if ($null -ne $el) { $ep = Get-ElementPattern -El $el -PatternIdentifier ([System.Windows.Automation.ExpandCollapsePattern]::Pattern) }
        if ($null -eq $ep) {
            Add-PatternAbsentRow -Interaction 'expand-collapse' -Target $t.Label -AutomationId 'cboChoice' -Pattern 'ExpandCollapse' -El $el `
                -Detail 'no ExpandCollapsePattern on this element in this fixture mode; recorded as a verdict row'
            continue
        }
        $pre = [string]$ep.Current.ExpandCollapseState
        $ep.Expand()
        Start-Sleep -Milliseconds 600
        $expanded = [string](Get-ElementPattern -El (Find-Scratch -Root $t.Root -Name 'cboChoice') `
                -PatternIdentifier ([System.Windows.Automation.ExpandCollapsePattern]::Pattern)).Current.ExpandCollapseState
        $ep.Collapse()
        Start-Sleep -Milliseconds 600
        $collapsed = [string](Get-ElementPattern -El (Find-Scratch -Root $t.Root -Name 'cboChoice') `
                -PatternIdentifier ([System.Windows.Automation.ExpandCollapsePattern]::Pattern)).Current.ExpandCollapseState
        Add-InteractionRow -Interaction 'expand-collapse' -Target $t.Label -AutomationId 'cboChoice' -Pattern 'ExpandCollapse' -Evidence ([ordered]@{
            controlType      = ($el.Current.ControlType.ProgrammaticName -replace '^ControlType\.', '')
            patternAcquired  = $true
            preState         = $pre
            stateAfterExpand = $expanded
            stateAfterCollapse = $collapsed
            reReadMethod     = 'state re-read after each half of the round trip from a freshly found element'
            roundTripped     = ($expanded -eq 'Expanded' -and $collapsed -eq 'Collapsed')
            verdict          = $(if ($expanded -eq 'Expanded' -and $collapsed -eq 'Collapsed') { 'ok' } else { 'round-trip-incomplete' })
        })
    }

    # --- select -------------------------------------------------------------
    foreach ($t in $targets) {
        $list = Find-Scratch -Root $t.Root -Name 'lstItems'
        $item = $null
        if ($null -ne $list) {
            $item = Find-ScratchElement -Root $list -Symbolic 'lstItem-Item-Charlie'
            if ($null -eq $item) {
                $itemCondition = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, [System.Windows.Automation.ControlType]::ListItem)
                $found = @($list.FindAll($Descendants, $itemCondition))
                if ($found.Count -ge 3) { $item = $found[2] }
            }
        }
        $sip = $null
        if ($null -ne $item) { $sip = Get-ElementPattern -El $item -PatternIdentifier ([System.Windows.Automation.SelectionItemPattern]::Pattern) }
        if ($null -eq $sip) {
            Add-PatternAbsentRow -Interaction 'select' -Target $t.Label -AutomationId 'lstItems/item[2]' -Pattern 'SelectionItem' -El $item `
                -Detail 'no ListItem with SelectionItemPattern reachable under lstItems in this fixture mode; recorded as a verdict row'
            continue
        }
        $selPattern = Get-ElementPattern -El $list -PatternIdentifier ([System.Windows.Automation.SelectionPattern]::Pattern)
        $preCount = -1
        if ($null -ne $selPattern) { $preCount = @($selPattern.Current.GetSelection()).Count }
        $sinkBefore = Get-SinkText -Root $t.Root
        $sip.Select()
        Start-Sleep -Milliseconds 500
        $postCount = -1
        $postIsSelected = $false
        $freshSel = Get-ElementPattern -El (Find-Scratch -Root $t.Root -Name 'lstItems') -PatternIdentifier ([System.Windows.Automation.SelectionPattern]::Pattern)
        if ($null -ne $freshSel) { $postCount = @($freshSel.Current.GetSelection()).Count }
        $freshItem = Get-ElementPattern -El $item -PatternIdentifier ([System.Windows.Automation.SelectionItemPattern]::Pattern)
        if ($null -ne $freshItem) { $postIsSelected = [bool]$freshItem.Current.IsSelected }
        $sinkAfter = Get-SinkText -Root $t.Root
        Add-InteractionRow -Interaction 'select' -Target $t.Label -AutomationId 'lstItems/item[2]' -Pattern 'SelectionItem' -Evidence ([ordered]@{
            controlType         = ($item.Current.ControlType.ProgrammaticName -replace '^ControlType\.', '')
            patternAcquired     = $true
            selectionCountBefore = $preCount
            selectionCountAfter = $postCount
            isSelectedAfter     = $postIsSelected
            sinkBefore          = $sinkBefore
            sinkAfter           = $sinkAfter
            reReadMethod        = 'container SelectionPattern re-acquired from a freshly found list, plus the item IsSelected re-read, plus the independent lblStatus sink'
            verdict             = $(if ($postIsSelected -and $postCount -eq 1) { 'ok' } else { 'unexpected-selection-state' })
        })
    }

    # --- scroll via pattern -------------------------------------------------
    foreach ($t in @(
            [pscustomobject]@{ Label = 'winforms-host-providers'; Root = $wfHostedRoot },
            [pscustomobject]@{ Label = 'winforms-default'; Root = $wfDefaultRoot })) {
        $panel = Find-Scratch -Root $t.Root -Name 'pnlScroll'
        $sp = $null
        if ($null -ne $panel) { $sp = Get-ElementPattern -El $panel -PatternIdentifier ([System.Windows.Automation.ScrollPattern]::Pattern) }
        if ($null -eq $sp) {
            Add-PatternAbsentRow -Interaction 'scroll-via-pattern' -Target $t.Label -AutomationId 'pnlScroll' -Pattern 'Scroll' -El $panel `
                -Detail 'the UIA3 COM census (captures/08-uia3-com/census.json) records Scroll on this exact provider in BOTH fixture modes; the managed client cannot acquire it. Divergence row, not a platform verdict.'
            continue
        }
        $sinkBefore = Get-SinkText -Root $t.Root -Name 'lblScrollPos'
        $percentBefore = [double]$sp.Current.VerticalScrollPercent
        $sp.Scroll([System.Windows.Automation.ScrollAmount]::NoAmount, [System.Windows.Automation.ScrollAmount]::LargeIncrement)
        Start-Sleep -Milliseconds 700
        $freshSp = Get-ElementPattern -El (Find-Scratch -Root $t.Root -Name 'pnlScroll') -PatternIdentifier ([System.Windows.Automation.ScrollPattern]::Pattern)
        $percentAfter = $percentBefore
        if ($null -ne $freshSp) { $percentAfter = [double]$freshSp.Current.VerticalScrollPercent }
        $sinkAfter = Get-SinkText -Root $t.Root -Name 'lblScrollPos'
        Add-InteractionRow -Interaction 'scroll-via-pattern' -Target $t.Label -AutomationId 'pnlScroll' -Pattern 'Scroll' -Evidence ([ordered]@{
            controlType          = ($panel.Current.ControlType.ProgrammaticName -replace '^ControlType\.', '')
            patternAcquired      = $true
            supportedPatterns    = @(Get-SupportedPatternNames -El $panel)
            verticalPercentBefore = [Math]::Round($percentBefore, 2)
            verticalPercentAfter = [Math]::Round($percentAfter, 2)
            sinkElement          = 'lblScrollPos, driven by a 100 ms timer inside the fixture, so it registers pattern, wheel and keyboard scrolling alike'
            sinkBefore           = $sinkBefore
            sinkAfter            = $sinkAfter
            changed              = ($sinkBefore -ne $sinkAfter)
            wheelCounterpart     = 'the SendInput wheel arm on this same control and this same sink is recorded separately in captures/06-input-synthesis/mouse.json'
            verdict              = $(if ($sinkBefore -ne $sinkAfter) { 'ok' } else { 'no-observed-scroll' })
        })
    }

    $wpfList = Find-Scratch -Root $wpfRoot -Name 'lstItems'
    $wpfScroll = $null
    if ($null -ne $wpfList) { $wpfScroll = Get-ElementPattern -El $wpfList -PatternIdentifier ([System.Windows.Automation.ScrollPattern]::Pattern) }
    if ($null -eq $wpfScroll) {
        Add-PatternAbsentRow -Interaction 'scroll-via-pattern' -Target 'wpf' -AutomationId 'lstItems' -Pattern 'Scroll' -El $wpfList -Detail 'ScrollPattern not acquirable'
    } else {
        $shrinkSteps = @(240, 190, 160)
        $stepUsed = 0
        foreach ($h in $shrinkSteps) {
            if ([bool]$wpfScroll.Current.VerticallyScrollable) { break }
            Show-WindowNoActivate -WindowHandle $wpfHandle -X 400 -Y 250 -Width 460 -Height $h
            Start-Sleep -Milliseconds 700
            $wpfList = Find-Scratch -Root $wpfRoot -Name 'lstItems'
            $wpfScroll = Get-ElementPattern -El $wpfList -PatternIdentifier ([System.Windows.Automation.ScrollPattern]::Pattern)
            $stepUsed = $h
        }
        $scrollable = [bool]$wpfScroll.Current.VerticallyScrollable
        $percentBefore = [double]$wpfScroll.Current.VerticalScrollPercent
        $verdict = 'not-scrollable'
        $percentAfter = $percentBefore
        if ($scrollable) {
            $wpfScroll.SetScrollPercent([System.Windows.Automation.ScrollPattern]::NoScroll, 100.0)
            Start-Sleep -Milliseconds 600
            $fresh = Get-ElementPattern -El (Find-Scratch -Root $wpfRoot -Name 'lstItems') -PatternIdentifier ([System.Windows.Automation.ScrollPattern]::Pattern)
            if ($null -ne $fresh) { $percentAfter = [double]$fresh.Current.VerticalScrollPercent }
            $verdict = $(if ($percentAfter -gt $percentBefore) { 'ok' } else { 'no-observed-scroll' })
        }
        Add-InteractionRow -Interaction 'scroll-via-pattern' -Target 'wpf' -AutomationId 'lstItems' -Pattern 'Scroll' -Evidence ([ordered]@{
            controlType           = 'List'
            patternAcquired       = $true
            makeScrollableMethod  = 'the window is shrunk with SetWindowPos(SWP_NOACTIVATE) until the ListBox viewport is smaller than its five items; the client-height step that achieved it is recorded'
            shrinkStepUsed        = $stepUsed
            verticallyScrollable  = $scrollable
            verticalPercentBefore = [Math]::Round($percentBefore, 2)
            verticalPercentAfter  = [Math]::Round($percentAfter, 2)
            reReadMethod          = 'percent re-read from a freshly acquired ScrollPattern on a freshly found element'
            verdict               = $verdict
        })
    }
    Show-WindowNoActivate -WindowHandle $wpfHandle -X 400 -Y 250 -Width 460 -Height 430
    Start-Sleep -Milliseconds 500

    [void](Write-ProbeJson -Probe $Probe -Name 'interactions.json' -InputObject ([ordered]@{
        probe    = $Probe
        question = 'does every 2.0(3) interaction actually take effect through a UIA pattern, verified by independent re-read rather than by the call return'
        stack    = 'managed-System.Windows.Automation'
        scope    = 'app/provider'
        verificationDiscipline = 'no row trusts the pattern call return. Element state is re-read after re-finding the element by AutomationId, and where the fixture exposes an independent sink control (lblStatus, lblScrollPos) that separate element is read as well.'
        fixtureArtifactNote = 'winforms-default rows report pattern-unavailable because the fixture installs a server-side IRawElementProviderSimple that suppresses the client-side proxies and WinForms'' own providers. That is a property of this fixture, not of WinForms or of Windows. winforms-host-providers rows show what WinForms itself exposes to a managed client.'
        managedVsComNote = 'several winforms-host-providers rows report pattern-unavailable for patterns the UIA3 COM census in captures/08-uia3-com/census.json records on the same providers (Invoke and Toggle on chkToggle, Value on txtValue, SelectionItem on the ListItems, Scroll on pnlScroll). Under KTD1 the COM row is the product-relevant one because the Rust adapter wraps a UIA3 COM client; these rows are filed as managed-client divergence, not as absent affordances. ExpandCollapse on cboChoice is the one WinForms pattern both stacks agree on, which is why it is the only non-WPF ok row here.'
        wheelCounterpartNote = 'the SendInput wheel arm is deliberately not in this file. It runs in 06-input-synthesis against the WinForms pnlScroll and its lblScrollPos sink, because that is the control this probe could NOT drive by pattern - the pair of the two captures is the pattern-vs-physical comparison 2.6 needs.'
        rows     = @($script:Rows)
    }))

    # --- text pattern -------------------------------------------------------
    $textRows = New-Object System.Collections.ArrayList
    $wpfEdit = Find-Scratch -Root $wpfRoot -Name 'txtValue'
    $wpfText = Get-ElementPattern -El $wpfEdit -PatternIdentifier ([System.Windows.Automation.TextPattern]::Pattern)
    if ($null -ne $wpfText) {
        $wpfValue = Get-ElementPattern -El $wpfEdit -PatternIdentifier ([System.Windows.Automation.ValuePattern]::Pattern)
        $wpfValue.SetValue($mixedPayload)
        Start-Sleep -Milliseconds 300
        $documentText = $wpfText.DocumentRange.GetText(-1)
        $selectionBefore = @($wpfText.GetSelection())
        $range = $wpfText.DocumentRange.Clone()
        [void]$range.MoveEndpointByUnit([System.Windows.Automation.Text.TextPatternRangeEndpoint]::Start, [System.Windows.Automation.Text.TextUnit]::Character, 2)
        [void]$range.MoveEndpointByRange([System.Windows.Automation.Text.TextPatternRangeEndpoint]::End, $range, [System.Windows.Automation.Text.TextPatternRangeEndpoint]::Start)
        [void]$range.MoveEndpointByUnit([System.Windows.Automation.Text.TextPatternRangeEndpoint]::End, [System.Windows.Automation.Text.TextUnit]::Character, 5)
        $range.Select()
        Start-Sleep -Milliseconds 300
        $selectionAfter = @($wpfText.GetSelection())
        $selectedText = ''
        if ($selectionAfter.Count -gt 0) { $selectedText = $selectionAfter[0].GetText(-1) }
        $caret = $wpfText.DocumentRange.Clone()
        [void]$caret.MoveEndpointByUnit([System.Windows.Automation.Text.TextPatternRangeEndpoint]::Start, [System.Windows.Automation.Text.TextUnit]::Character, 4)
        [void]$caret.MoveEndpointByRange([System.Windows.Automation.Text.TextPatternRangeEndpoint]::End, $caret, [System.Windows.Automation.Text.TextPatternRangeEndpoint]::Start)
        $caret.Select()
        Start-Sleep -Milliseconds 300
        $caretSelection = @($wpfText.GetSelection())
        $caretText = ''
        if ($caretSelection.Count -gt 0) { $caretText = $caretSelection[0].GetText(-1) }
        [void]$textRows.Add([ordered]@{
            target              = 'wpf-txtValue'
            stack               = 'managed-System.Windows.Automation'
            controlType         = 'Edit'
            className           = [string]$wpfEdit.Current.ClassName
            supportedPatterns   = @(Get-SupportedPatternNames -El $wpfEdit)
            textPatternExposed  = $true
            getText             = [ordered]@{
                expectedShape = (Get-TextShape -Text $mixedPayload)
                observedShape = (Get-TextShape -Text $documentText)
                exactMatch    = ((Get-TextShape -Text $mixedPayload).sha256Utf16 -eq (Get-TextShape -Text $documentText).sha256Utf16)
                note          = 'the value was set through ValuePattern and read back through TextPattern.DocumentRange.GetText - a cross-pattern read, not the same call returning its own argument'
            }
            selection           = [ordered]@{
                rangesBefore      = $selectionBefore.Count
                rangesAfter       = $selectionAfter.Count
                requestedUnits    = 5
                selectedShape     = (Get-TextShape -Text $selectedText)
                matchesSubstring  = ($selectedText -eq $mixedPayload.Substring(2, 5))
            }
            caret               = [ordered]@{
                method           = 'a degenerate range (End collapsed onto Start) is Selected at character offset 4 and the selection is re-read. Emptiness of the read-back text only means a collapsed caret if a range was read back at all: with no ranges the text is empty because nothing was measured, so the range count is part of the assertion rather than beside it.'
                rangesAfter      = $caretSelection.Count
                degenerate       = (($caretSelection.Count -gt 0) -and ($caretText.Length -eq 0))
            }
            insert              = [ordered]@{
                method       = 'TextPattern is read-only by contract; insertion is done through ValuePattern.SetValue and verified through the TextPattern read above'
                verified     = ((Get-TextShape -Text $mixedPayload).sha256Utf16 -eq (Get-TextShape -Text $documentText).sha256Utf16)
            }
        })
        $wpfValue.SetValue('seed-value')
    }

    $notepad = Start-Tracked -FilePath (Join-Path $env:WINDIR 'System32\notepad.exe')
    if ($notepad.MainWindowHandle -ne [IntPtr]::Zero) {
        Show-WindowNoActivate -WindowHandle $notepad.MainWindowHandle -X 900 -Y 300 -Width 600 -Height 400
        Start-Sleep -Milliseconds 900
        $notepadRoot = $AE::FromHandle($notepad.MainWindowHandle)
        $editHandle = [AgentDesktopProbe.Interact]::FindWindowExW($notepad.MainWindowHandle, [IntPtr]::Zero, 'Edit', $null)
        $probes = New-Object System.Collections.ArrayList
        $documentCondition = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, [System.Windows.Automation.ControlType]::Document)
        [void]$probes.Add([pscustomobject]@{ How = 'FindFirst(Descendants, ControlType=Document)'; El = $notepadRoot.FindFirst($Descendants, $documentCondition) })
        if ($editHandle -ne [IntPtr]::Zero) {
            [void]$probes.Add([pscustomobject]@{ How = 'AutomationElement.FromHandle(the Edit child HWND found with FindWindowEx)'; El = $AE::FromHandle($editHandle) })
        }
        foreach ($p in $probes) {
            $row = [ordered]@{
                target      = 'notepad-edit'
                stack       = 'managed-System.Windows.Automation'
                lookup      = $p.How
                resolved    = ($null -ne $p.El)
            }
            if ($null -ne $p.El) {
                $row['controlType'] = ($p.El.Current.ControlType.ProgrammaticName -replace '^ControlType\.', '')
                $row['className'] = [string]$p.El.Current.ClassName
                $row['supportedPatterns'] = @(Get-SupportedPatternNames -El $p.El)
                foreach ($pattern in @(
                        [pscustomobject]@{ Name = 'Text'; Id = [System.Windows.Automation.TextPattern]::Pattern },
                        [pscustomobject]@{ Name = 'Value'; Id = [System.Windows.Automation.ValuePattern]::Pattern },
                        [pscustomobject]@{ Name = 'Scroll'; Id = [System.Windows.Automation.ScrollPattern]::Pattern })) {
                    $row[('tryGetCurrentPattern_' + $pattern.Name)] = ($null -ne (Get-ElementPattern -El $p.El -PatternIdentifier $pattern.Id))
                }
            }
            [void]$textRows.Add($row)
        }
    }

    [void](Write-ProbeJson -Probe $Probe -Name 'text-pattern.json' -InputObject ([ordered]@{
        probe    = $Probe
        question = 'is TextPattern exposed on a classic Win32 Edit on Server 2019 at all, and does the managed stack agree with the COM stack about it'
        stack    = 'managed-System.Windows.Automation'
        scope    = 'app/provider'
        whyTwoLookups = 'GetSupportedPatterns is not a reliable negative: it is answered by the client-side proxy that happens to be bound. Every row therefore also records TryGetCurrentPattern per pattern, which is the call the adapter would actually make. Both lookups are recorded because they disagree.'
        comCounterpart = 'captures/08-uia3-com/census.json records the SAME notepad window through hand-declared UIA3 COM as one Document element carrying LegacyIAccessible, Scroll, Text, Text2 and Value. Any managed row below that reports Text absent is a client-stack divergence, not an absence of the provider.'
        p2o12Relevance = '2.0(3) text get/selection/caret/insert and P2-O12 both assume a Text-capable Edit. The WPF row shows what a full managed TextPattern surface supports; the notepad rows show what the real Win32 Edit gives the two client stacks.'
        rows     = @($textRows)
    }))

    # --- focus vs foreground ------------------------------------------------
    [void][AgentDesktopProbe.Native]::ShowWindow($wfDefault.MainWindowHandle, 6)
    Start-Sleep -Milliseconds 500
    [void][AgentDesktopProbe.Native]::ShowWindow($wfDefault.MainWindowHandle, 9)
    Start-Sleep -Milliseconds 900
    $fgPidBefore = [AgentDesktopProbe.Native]::GetForegroundProcessId()
    $fgTitleBefore = [AgentDesktopProbe.Interact]::ForegroundTitle()
    $focusTarget = Find-Scratch -Root $wpfRoot -Name 'txtValue'
    $focusError = ''
    $hadFocusBefore = $false
    try { $hadFocusBefore = [bool]$focusTarget.Current.HasKeyboardFocus } catch { }
    try { $focusTarget.SetFocus() } catch { $focusError = ($_.Exception.Message -replace '[\r\n]+', ' ') }
    Start-Sleep -Milliseconds 900
    $fgPidAfter = [AgentDesktopProbe.Native]::GetForegroundProcessId()
    $fgTitleAfter = [AgentDesktopProbe.Interact]::ForegroundTitle()
    $hasFocusAfter = $false
    try { $hasFocusAfter = [bool](Find-Scratch -Root $wpfRoot -Name 'txtValue').Current.HasKeyboardFocus } catch { }
    $focusedAutomationId = ''
    try { $focusedAutomationId = [string]$AE::FocusedElement.Current.AutomationId } catch { }

    [void](Write-ProbeJson -Probe $Probe -Name 'focus.json' -InputObject ([ordered]@{
        probe    = $Probe
        question = 'does AutomationElement.SetFocus on a background window move the desktop foreground, or only keyboard focus inside that window'
        why      = '2.7 wants headless interaction. If SetFocus steals foreground, no interaction that needs focus can be headless; if it does not, focus-dependent actions are headless-safe and the design can rely on it.'
        method   = 'a second probe-owned scratch window is brought forward with ShowWindow(SW_MINIMIZE) then ShowWindow(SW_RESTORE) - the sanctioned pattern, SetForegroundWindow is never called - then SetFocus is issued on an element of the OTHER probe-owned window and the foreground is re-read.'
        foregroundBefore    = [ordered]@{ windowTitle = $fgTitleBefore; processId = $fgPidBefore }
        foregroundAfter     = [ordered]@{ windowTitle = $fgTitleAfter; processId = $fgPidAfter }
        foregroundChanged   = ($fgPidBefore -ne $fgPidAfter)
        foregroundWasTheOtherScratchWindow = ($fgTitleBefore -like 'AgentDesktop Scratch WinForms*')
        setFocusTarget      = 'wpf txtValue'
        setFocusError       = $focusError
        hadKeyboardFocusBefore = $hadFocusBefore
        hasKeyboardFocusAfter  = $hasFocusAfter
        focusedElementAutomationIdAfter = $focusedAutomationId
        titleEvidenceNote   = 'the two foreground observations are nested objects with a key named exactly processId, so the KTD9 normalizer canonicalizes the run-varying pid to <pid>. A flat foregroundPidBefore/foregroundPidAfter pair was tried first and does NOT match the normalizer''s word-boundary rule: the raw pids survived into the twin and the capture failed to reproduce across runs. The probe-owned window titles are what carry the fact into the normalized twin.'
        verdict             = $(if ($fgPidBefore -ne $fgPidAfter) { 'SetFocus moved the desktop foreground' } else { 'SetFocus did not move the desktop foreground' })
    }))

    $ok = @($script:Rows | Where-Object { $_.verdict -eq 'ok' }).Count
    $unavailable = @($script:Rows | Where-Object { $_.verdict -eq 'pattern-unavailable' }).Count
    $resultData['interactionRows'] = @($script:Rows).Count
    $resultData['okRows'] = $ok
    $resultData['patternUnavailableRows'] = $unavailable
    $resultData['textRows'] = @($textRows).Count
    $resultData['foregroundChangedOnSetFocus'] = ($fgPidBefore -ne $fgPidAfter)
    $message = 'interactions: ' + $ok + ' ok / ' + $unavailable + ' pattern-unavailable of ' + @($script:Rows).Count + ' rows'
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
