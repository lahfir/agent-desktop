<#
.SYNOPSIS
    Probe 07 (sub-phase 2.0, unit U6): ElementFromPoint against occluded, zero-size and
    minimized targets - the primitives 2.6's occlusion gate is built on.

.DESCRIPTION
    Stack: managed System.Windows.Automation for ElementFromPoint, Win32 for the
    independent cross-checks (WindowFromPoint, GetWindowRect, GetDlgItem). Scope: system.

    Three questions, all measured against probe-owned scratch windows:

    1. Occlusion. Two scratch windows are launched at deterministic origins so their
       rects overlap by construction, the second is raised with ShowWindow(SW_MINIMIZE)
       then ShowWindow(SW_RESTORE) on its own window - SetForegroundWindow is never
       called - and ElementFromPoint is asked about a point inside the overlap. The
       occlusion gate is only sound if the answer is the occluder.

    2. Zero size. btnZeroSize is a real 0x0 Win32 control with control id 1007. U7's COM
       census recorded it as absent from both RawView and ControlView; this probe
       confirms that from the managed side, shows it is still reachable by GetDlgItem,
       and asks what ElementFromPoint returns at its origin.

    3. Minimize. U3 measured that minimizing degenerates geometry in two different
       shapes: the top-level window reports an empty rect while its descendants report
       REAL dimensions anchored at -32000, and every node still reports IsOffscreen
       false. An occlusion gate that tests emptiness or IsOffscreen alone accepts both
       as visible. This probe re-confirms both shapes and adds the consequence the
       earlier unit could not measure: what ElementFromPoint returns at the coordinates
       the window used to occupy.

    Rectangles are recorded as comma-joined strings, not as numeric x/y/width/height
    keys, because the KTD9 normalizer buckets those to 8 px - which would round -32000
    into a neighbouring bucket and quietly destroy the evidence.

    Captures under captures/07-hittest/:
        hittest.json   occlusion, zero-size and minimized rows
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\common.ps1"

Add-Type -AssemblyName UIAutomationClient | Out-Null
Add-Type -AssemblyName UIAutomationTypes | Out-Null

$Probe = '07-hittest'
$AE = [System.Windows.Automation.AutomationElement]
$Walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker
$RawWalker = [System.Windows.Automation.TreeWalker]::RawViewWalker
$Descendants = [System.Windows.Automation.TreeScope]::Descendants
$UnderOrigin = @{ X = 100; Y = 100 }
$OverOrigin = @{ X = 260; Y = 180 }
$ZeroSizeControlId = 1007
$script:Spawned = New-Object System.Collections.ArrayList

function Initialize-HitTestNative {
    if ('AgentDesktopProbe.HitTest' -as [type]) { return }
    $src = @'
using System;
using System.Runtime.InteropServices;
using System.Text;

namespace AgentDesktopProbe {
    [StructLayout(LayoutKind.Sequential)]
    public struct HitRect { public int Left; public int Top; public int Right; public int Bottom; }

    [StructLayout(LayoutKind.Sequential)]
    public struct HitPoint { public int X; public int Y; }

    public static class HitTest {
        [DllImport("user32.dll", SetLastError = true)]
        public static extern bool GetWindowRect(IntPtr hWnd, out HitRect lpRect);
        [DllImport("user32.dll")]
        public static extern IntPtr WindowFromPoint(HitPoint p);
        [DllImport("user32.dll", SetLastError = true)]
        public static extern IntPtr GetDlgItem(IntPtr hDlg, int nIDDlgItem);
        [DllImport("user32.dll")]
        public static extern IntPtr GetAncestor(IntPtr hWnd, uint flags);
        [DllImport("user32.dll")]
        public static extern bool IsWindowVisible(IntPtr hWnd);
        [DllImport("user32.dll")]
        public static extern bool IsIconic(IntPtr hWnd);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        private static extern int GetWindowTextW(IntPtr hWnd, StringBuilder buffer, int maxCount);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        private static extern int GetClassNameW(IntPtr hWnd, StringBuilder buffer, int maxCount);

        public static string TitleOf(IntPtr h) {
            if (h == IntPtr.Zero) { return "<null-hwnd>"; }
            StringBuilder sb = new StringBuilder(512);
            GetWindowTextW(h, sb, sb.Capacity);
            return sb.ToString();
        }

        public static string ClassOf(IntPtr h) {
            if (h == IntPtr.Zero) { return "<null-hwnd>"; }
            StringBuilder sb = new StringBuilder(256);
            GetClassNameW(h, sb, sb.Capacity);
            return sb.ToString();
        }

        public static string RootTitleAt(int x, int y) {
            HitPoint p = new HitPoint();
            p.X = x;
            p.Y = y;
            IntPtr h = WindowFromPoint(p);
            if (h == IntPtr.Zero) { return "<no-window>"; }
            IntPtr root = GetAncestor(h, 2);
            return TitleOf(root);
        }

        public static string RectOf(IntPtr h) {
            HitRect r;
            if (!GetWindowRect(h, out r)) { return "<getwindowrect-failed>"; }
            return r.Left.ToString() + "," + r.Top.ToString() + "," +
                (r.Right - r.Left).ToString() + "," + (r.Bottom - r.Top).ToString();
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

function Get-WindowRectString {
    param([IntPtr]$Handle)
    return [AgentDesktopProbe.HitTest]::RectOf($Handle)
}

function Get-ElementRectString {
    param($El)
    try {
        $r = $El.Current.BoundingRectangle
        if ($r.IsEmpty) { return '<empty>' }
        return ([string][int]$r.Left + ',' + [int]$r.Top + ',' + [int]$r.Width + ',' + [int]$r.Height)
    } catch { return '<read-failed>' }
}

function Get-SafeTopLevelName {
    param([AllowEmptyString()][string]$Name)
    if ([string]::IsNullOrEmpty($Name)) { return '' }
    if ($Name -like 'AgentDesktop Scratch*') { return $Name }
    if (@('Program Manager', 'Desktop', 'Windows Input Experience') -contains $Name) { return $Name }
    return (Protect-ProbeName -Name $Name)
}

function Get-TopLevelName {
    param($El)
    $cur = $El
    for ($i = 0; $i -lt 40; $i++) {
        if ($null -eq $cur) { break }
        $parent = $null
        try { $parent = $Walker.GetParent($cur) } catch { }
        if ($null -eq $parent) { break }
        try { if ([System.Windows.Automation.Automation]::Compare($parent, $AE::RootElement)) { return (Get-SafeTopLevelName -Name ([string]$cur.Current.Name)) } } catch { }
        $cur = $parent
    }
    return '<no-top-level-reached>'
}

function Get-ElementAtPoint {
    param([int]$X, [int]$Y)
    $point = New-Object System.Windows.Point($X, $Y)
    $el = $null
    try { $el = $AE::FromPoint($point) } catch { }
    if ($null -eq $el) {
        return [ordered]@{ resolved = $false }
    }
    $controlType = ''
    $automationId = ''
    $className = ''
    try { $controlType = $el.Current.ControlType.ProgrammaticName -replace '^ControlType\.', '' } catch { }
    try { $automationId = [string]$el.Current.AutomationId } catch { }
    try { $className = [string]$el.Current.ClassName } catch { }
    return [ordered]@{
        resolved       = $true
        controlType    = $controlType
        automationId   = $automationId
        className      = $className
        topLevelWindow = (Get-TopLevelName -El $el)
        rect           = (Get-ElementRectString -El $el)
    }
}

function Test-PointInRect {
    param([string]$Rect, [int]$X, [int]$Y)
    $parts = $Rect -split ','
    if ($parts.Count -ne 4) { return $false }
    $l = [int]$parts[0]
    $t = [int]$parts[1]
    $r = $l + [int]$parts[2]
    $b = $t + [int]$parts[3]
    return ($X -ge $l -and $X -lt $r -and $Y -ge $t -and $Y -lt $b)
}

# A null return is this probe's evidence that btnZeroSize is in no tree walk, so a
# lookup that failed must not be able to produce one: FindFirst already answers a
# genuine absence with null, and anything that throws is a broken measurement that
# has to stop the run rather than be filed as a negative result.
function Find-ByAutomationId {
    param($Root, [string]$AutomationId, $Scope)
    $c = New-Object System.Windows.Automation.PropertyCondition($AE::AutomationIdProperty, $AutomationId)
    return $Root.FindFirst($Scope, $c)
}

function Measure-Descendants {
    param($Root, $TreeWalkerToUse, [int]$Budget = 400)
    $n = 0
    $stack = New-Object System.Collections.Stack
    $stack.Push($Root)
    while ($stack.Count -gt 0 -and $n -lt $Budget) {
        $el = $stack.Pop()
        $n++
        try {
            $k = $TreeWalkerToUse.GetFirstChild($el)
            while ($null -ne $k) { $stack.Push($k); $k = $TreeWalkerToUse.GetNextSibling($k) }
        } catch { }
    }
    return $n
}

$status = 'ok'
$message = ''
$resultData = [ordered]@{}

try {
    Initialize-ProbeNative
    Initialize-HitTestNative
    $scratchExe = Join-Path (Get-ProbeRoot) 'scratch\bin\ScratchForms.exe'
    if (-not (Test-Path -LiteralPath $scratchExe)) {
        & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path (Get-ProbeRoot) 'scratch\build-scratch.ps1') | Out-Null
    }
    if (-not (Test-Path -LiteralPath $scratchExe)) { throw ('scratch fixture missing at ' + $scratchExe) }

    $under = Start-Tracked -FilePath $scratchExe -ArgumentList @('--tag', 'u6-under', '--pos', ($UnderOrigin.X.ToString() + ',' + $UnderOrigin.Y))
    if ($under.MainWindowHandle -eq [IntPtr]::Zero) { throw 'the under window never appeared' }
    $over = Start-Tracked -FilePath $scratchExe -ArgumentList @('--tag', 'u6-over', '--pos', ($OverOrigin.X.ToString() + ',' + $OverOrigin.Y))
    if ($over.MainWindowHandle -eq [IntPtr]::Zero) { throw 'the over window never appeared' }
    Start-Sleep -Milliseconds 800

    $underTitle = [AgentDesktopProbe.HitTest]::TitleOf($under.MainWindowHandle)
    $overTitle = [AgentDesktopProbe.HitTest]::TitleOf($over.MainWindowHandle)

    [void][AgentDesktopProbe.Native]::ShowWindow($over.MainWindowHandle, 6)
    Start-Sleep -Milliseconds 500
    [void][AgentDesktopProbe.Native]::ShowWindow($over.MainWindowHandle, 9)
    Start-Sleep -Milliseconds 900

    $underRect = Get-WindowRectString -Handle $under.MainWindowHandle
    $overRect = Get-WindowRectString -Handle $over.MainWindowHandle
    $u = $underRect -split ','
    $o = $overRect -split ','
    $overlapLeft = [Math]::Max([int]$u[0], [int]$o[0])
    $overlapTop = [Math]::Max([int]$u[1], [int]$o[1])
    $overlapRight = [Math]::Min(([int]$u[0] + [int]$u[2]), ([int]$o[0] + [int]$o[2]))
    $overlapBottom = [Math]::Min(([int]$u[1] + [int]$u[3]), ([int]$o[1] + [int]$o[3]))
    $probeX = [int](($overlapLeft + $overlapRight) / 2)
    $probeY = [int](($overlapTop + $overlapBottom) / 2)

    $occludedElement = Get-ElementAtPoint -X $probeX -Y $probeY
    $win32RootTitle = Get-SafeTopLevelName -Name ([AgentDesktopProbe.HitTest]::RootTitleAt($probeX, $probeY))

    $occlusion = [ordered]@{
        question             = 'at a point covered by a second window, does ElementFromPoint return the occluder or the covered target'
        why                  = "2.6's occlusion gate decides whether an element is actually clickable. If ElementFromPoint answered with the covered element the gate would have no primitive to build on and every actionability check would be a guess."
        underWindowTitle     = $underTitle
        overWindowTitle      = $overTitle
        underWindowRect      = $underRect
        overWindowRect       = $overRect
        raiseMethod          = 'ShowWindow(SW_MINIMIZE) then ShowWindow(SW_RESTORE) on the probe-owned over window. SetForegroundWindow is never called (KTD5).'
        overlapRect          = ([string]$overlapLeft + ',' + $overlapTop + ',' + ($overlapRight - $overlapLeft) + ',' + ($overlapBottom - $overlapTop))
        probePoint           = ([string]$probeX + ',' + $probeY)
        probePointInsideUnder = (Test-PointInRect -Rect $underRect -X $probeX -Y $probeY)
        probePointInsideOver  = (Test-PointInRect -Rect $overRect -X $probeX -Y $probeY)
        elementFromPoint     = $occludedElement
        win32WindowFromPointRootTitle = $win32RootTitle
        resolvedToOccluder   = ($occludedElement.topLevelWindow -eq $overTitle)
        resolvedToOccluded   = ($occludedElement.topLevelWindow -eq $underTitle)
        win32AgreesWithUia   = ($win32RootTitle -eq $occludedElement.topLevelWindow)
        titleEvidenceNote    = 'the two windows are distinguished by the --tag switch, so the identity that proves which window answered survives KTD9 normalization - process ids and window handles do not.'
        verdict              = $(if ($occludedElement.topLevelWindow -eq $overTitle) { 'ok - ElementFromPoint returned the occluder' } elseif ($occludedElement.topLevelWindow -eq $underTitle) { 'FAIL - ElementFromPoint returned the covered target' } else { 'unexpected - ElementFromPoint returned neither scratch window' })
    }

    # --- zero-size element --------------------------------------------------
    # the remaining two blocks ask what is at a coordinate INSIDE the under window, so
    # the under window is raised first with the same probe-owned minimize/restore. On
    # the first run of this probe it was not, and a hit test at btnZeroSize's origin
    # resolved to an unrelated console window that happened to sit above it - a foreign
    # element in the capture and a run-varying one.
    [void][AgentDesktopProbe.Native]::ShowWindow($under.MainWindowHandle, 6)
    Start-Sleep -Milliseconds 500
    [void][AgentDesktopProbe.Native]::ShowWindow($under.MainWindowHandle, 9)
    Start-Sleep -Milliseconds 900

    $underRoot = $AE::FromHandle($under.MainWindowHandle)
    $zeroHandle = [AgentDesktopProbe.HitTest]::GetDlgItem($under.MainWindowHandle, $ZeroSizeControlId)
    $zeroRect = '<no-handle>'
    $zeroClass = '<no-handle>'
    $zeroVisible = $false
    if ($zeroHandle -ne [IntPtr]::Zero) {
        $zeroRect = Get-WindowRectString -Handle $zeroHandle
        $zeroClass = [AgentDesktopProbe.HitTest]::ClassOf($zeroHandle)
        $zeroVisible = [AgentDesktopProbe.HitTest]::IsWindowVisible($zeroHandle)
    }
    $zeroInControlView = Find-ByAutomationId -Root $underRoot -AutomationId 'btnZeroSize' -Scope $Descendants
    $zeroFromHandle = $null
    if ($zeroHandle -ne [IntPtr]::Zero) {
        try { $zeroFromHandle = $AE::FromHandle($zeroHandle) } catch { }
    }
    $zeroOrigin = $zeroRect -split ','
    $zeroPointElement = [ordered]@{ resolved = $false }
    if ($zeroOrigin.Count -eq 4) {
        $zeroPointElement = Get-ElementAtPoint -X ([int]$zeroOrigin[0]) -Y ([int]$zeroOrigin[1])
    }

    $zeroSize = [ordered]@{
        question           = 'what does a 0x0 control look like to a hit test, and is it reachable at all'
        controlId          = $ZeroSizeControlId
        automationId       = 'btnZeroSize'
        reachableByGetDlgItem = ($zeroHandle -ne [IntPtr]::Zero)
        win32Rect          = $zeroRect
        win32ClassName     = $zeroClass
        win32IsWindowVisible = $zeroVisible
        foundByFindFirstAutomationId = ($null -ne $zeroInControlView)
        controlViewNodeCount = (Measure-Descendants -Root $underRoot -TreeWalkerToUse $Walker)
        rawViewNodeCount   = (Measure-Descendants -Root $underRoot -TreeWalkerToUse $RawWalker)
        fromHandleResolved = ($null -ne $zeroFromHandle)
        fromHandleControlType = $(if ($null -ne $zeroFromHandle) { ($zeroFromHandle.Current.ControlType.ProgrammaticName -replace '^ControlType\.', '') } else { '<not-resolved>' })
        fromHandleRect     = $(if ($null -ne $zeroFromHandle) { (Get-ElementRectString -El $zeroFromHandle) } else { '<not-resolved>' })
        fromHandleIsOffscreen = $(if ($null -ne $zeroFromHandle) { [bool]$zeroFromHandle.Current.IsOffscreen } else { $false })
        elementFromPointAtItsOrigin = $zeroPointElement
        priorEvidence      = 'U7 recorded this control as absent from BOTH the COM RawView and ControlView walks of the same fixture. The rows above are the managed-side confirmation plus the two things the COM census could not answer: whether the control is still reachable by handle, and what a hit test at its coordinates returns.'
        consequence        = 'a zero-size control is addressable through Win32 and through AutomationElement.FromHandle, but it can be enumerated by no tree walk and hit by no point. Any Windows actionability check that treats "not hit-testable" as "does not exist" will be right about the click and wrong about the element.'
    }

    # --- minimized window ---------------------------------------------------
    $underOnlyX = [int]$u[0] + 30
    $underOnlyY = [int]$u[1] + [int]([int]$u[3] / 2)
    $visiblePointBefore = Get-ElementAtPoint -X $underOnlyX -Y $underOnlyY
    $descendantBefore = Find-ByAutomationId -Root $underRoot -AutomationId 'btnAction' -Scope $Descendants
    $topRectBefore = Get-ElementRectString -El $underRoot
    $topOffscreenBefore = [bool]$underRoot.Current.IsOffscreen
    $descRectBefore = '<not-found>'
    $descOffscreenBefore = $false
    if ($null -ne $descendantBefore) {
        $descRectBefore = Get-ElementRectString -El $descendantBefore
        $descOffscreenBefore = [bool]$descendantBefore.Current.IsOffscreen
    }

    [void][AgentDesktopProbe.Native]::ShowWindow($under.MainWindowHandle, 6)
    Start-Sleep -Milliseconds 1200

    $underRootAfter = $AE::FromHandle($under.MainWindowHandle)
    $descendantAfter = Find-ByAutomationId -Root $underRootAfter -AutomationId 'btnAction' -Scope $Descendants
    $topRectAfter = Get-ElementRectString -El $underRootAfter
    $topOffscreenAfter = [bool]$underRootAfter.Current.IsOffscreen
    $descRectAfter = '<not-found>'
    $descOffscreenAfter = $false
    if ($null -ne $descendantAfter) {
        $descRectAfter = Get-ElementRectString -El $descendantAfter
        $descOffscreenAfter = [bool]$descendantAfter.Current.IsOffscreen
    }
    $visiblePointAfter = Get-ElementAtPoint -X $underOnlyX -Y $underOnlyY
    $minimizedDescendantPoint = [ordered]@{ resolved = $false }
    $descParts = $descRectAfter -split ','
    if ($descParts.Count -eq 4 -and [int]$descParts[0] -gt -32768) {
        $minimizedDescendantPoint = Get-ElementAtPoint -X ([int]$descParts[0] + 2) -Y ([int]$descParts[1] + 2)
    }
    $win32IsIconic = [AgentDesktopProbe.HitTest]::IsIconic($under.MainWindowHandle)

    [void][AgentDesktopProbe.Native]::ShowWindow($under.MainWindowHandle, 9)
    Start-Sleep -Milliseconds 900
    $topRectRestored = Get-ElementRectString -El ($AE::FromHandle($under.MainWindowHandle))

    $minimized = [ordered]@{
        question             = 'what does a minimized window report, and what does a hit test at its former coordinates return'
        priorEvidence        = 'U3 measured that minimizing degenerates geometry in two different shapes: only the TOP-LEVEL window reports an empty rect, while descendants report REAL dimensions anchored at -32000, and every node still reports IsOffscreen false. An occlusion gate testing emptiness or IsOffscreen alone accepts both as visible.'
        pointProbed          = ([string]$underOnlyX + ',' + $underOnlyY)
        pointChoice          = 'a point inside the under window but outside the over window, so the only thing that can change the answer is the minimize itself'
        topLevelRectBefore   = $topRectBefore
        topLevelRectAfterMinimize = $topRectAfter
        topLevelRectAfterRestore  = $topRectRestored
        topLevelIsOffscreenBefore = $topOffscreenBefore
        topLevelIsOffscreenAfterMinimize = $topOffscreenAfter
        descendantAutomationId = 'btnAction'
        descendantFoundAfterMinimize = ($null -ne $descendantAfter)
        descendantRectBefore = $descRectBefore
        descendantRectAfterMinimize = $descRectAfter
        descendantIsOffscreenBefore = $descOffscreenBefore
        descendantIsOffscreenAfterMinimize = $descOffscreenAfter
        win32IsIconic        = $win32IsIconic
        elementFromPointBefore = $visiblePointBefore
        elementFromPointAfterMinimize = $visiblePointAfter
        elementFromPointAtDescendantAnchor = $minimizedDescendantPoint
        hitTestFreedTheCoordinates = ($visiblePointBefore.topLevelWindow -ne $visiblePointAfter.topLevelWindow)
        gateConsequence      = 'ElementFromPoint is the one primitive here that degrades cleanly: after the minimize the coordinates resolve to whatever is genuinely on top instead of to the minimized window. Emptiness and IsOffscreen do not - the top-level rect empties while its descendants keep real dimensions at -32000, and IsOffscreen stays false on both. 2.6 should gate on a hit test, and where it must reason about geometry it has to test the -32000 anchor explicitly, because a -32000 rect is neither empty nor offscreen by either property.'
        rectRecordingNote    = 'every rect here is a comma-joined string. As numeric x/y/width/height keys the KTD9 normalizer would bucket them to 8 px, which rounds -32000 into a neighbouring bucket and erases exactly the anchor this row exists to record.'
    }

    [void](Write-ProbeJson -Probe $Probe -Name 'hittest.json' -InputObject ([ordered]@{
        probe    = $Probe
        question = 'do the hit-test primitives 2.6 needs behave correctly against occluded, zero-size and minimized targets'
        stack    = 'managed-System.Windows.Automation AutomationElement.FromPoint, cross-checked with Win32 WindowFromPoint'
        scope    = 'system'
        safety   = 'both windows are launched by this probe at deterministic --pos origins; nothing else on the desktop is touched, moved or raised.'
        occlusion = $occlusion
        zeroSize  = $zeroSize
        minimized = $minimized
    }))

    $resultData['occlusionVerdict'] = $occlusion.verdict
    $resultData['zeroSizeInTree'] = $zeroSize.foundByFindFirstAutomationId
    $resultData['zeroSizeReachableByHandle'] = $zeroSize.reachableByGetDlgItem
    $resultData['minimizedTopRect'] = $minimized.topLevelRectAfterMinimize
    $resultData['minimizedDescendantRect'] = $minimized.descendantRectAfterMinimize
    $resultData['minimizedDescendantIsOffscreen'] = $minimized.descendantIsOffscreenAfterMinimize
    $message = 'hittest: ' + $occlusion.verdict + '; zero-size in tree=' + $zeroSize.foundByFindFirstAutomationId + '; minimized descendant rect=' + $minimized.descendantRectAfterMinimize
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
