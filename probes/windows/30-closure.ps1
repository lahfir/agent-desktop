#Requires -Version 5.1
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

<#
    30-closure.ps1 - the two measurements the last sub-phase owes.

    A28-2 asked for a single-process observation that resolves the tray
    toolbar and enumerates the same parent in the same breath, because two
    earlier readings disagreed and neither could be told which generation of
    windows it was describing. A26-13, carried by A28-3, asked for the
    positive-area versus zero-extent split of nameless content leaves on a
    Chromium tree, which A28-3 established is reachable on this host.

    The leaf classification runs through the shipped release binary rather
    than through a managed UI Automation client. KTD1 gives the COM stack
    authority where the two disagree, and A28-5 measured them disagreeing on
    exactly this kind of question - so the reading that matters is the one
    the product itself takes.
#>

. (Join-Path (Split-Path -Parent $MyInvocation.MyCommand.Path) 'common.ps1')

$probe = '30-closure'
[void](Get-CaptureDir -Probe $probe)

$traySource = @'
using System;
using System.Runtime.InteropServices;
using System.Text;

namespace AgentDesktopClosureProbe {
    public static class Tray {
        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        public static extern IntPtr FindWindowW(string cls, string name);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        public static extern IntPtr FindWindowExW(IntPtr parent, IntPtr after, string cls, string name);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        public static extern int GetClassNameW(IntPtr h, StringBuilder s, int n);
        [DllImport("user32.dll")]
        public static extern IntPtr GetParent(IntPtr h);
        [DllImport("user32.dll")]
        public static extern bool IsWindowVisible(IntPtr h);
        [DllImport("user32.dll")]
        public static extern bool IsWindow(IntPtr h);

        public static string ClassOf(IntPtr handle) {
            StringBuilder buffer = new StringBuilder(256);
            GetClassNameW(handle, buffer, buffer.Capacity);
            return buffer.ToString();
        }

        /// Resolves the shipped four-hop chain and enumerates the same parent
        /// in one pass, so the two readings describe one generation of windows
        /// rather than two observations taken minutes apart.
        public static string ResolveAndEnumerate() {
            IntPtr tray = FindWindowW("Shell_TrayWnd", null);
            if (tray == IntPtr.Zero) { return "no-tray"; }
            IntPtr notify = FindWindowExW(tray, IntPtr.Zero, "TrayNotifyWnd", null);
            if (notify == IntPtr.Zero) { return "no-notify"; }

            IntPtr directChild = FindWindowExW(notify, IntPtr.Zero, "ToolbarWindow32", null);
            IntPtr pager = FindWindowExW(notify, IntPtr.Zero, "SysPager", null);
            IntPtr pagerChild = pager == IntPtr.Zero
                ? IntPtr.Zero
                : FindWindowExW(pager, IntPtr.Zero, "ToolbarWindow32", null);

            int enumerated = 0;
            IntPtr cursor = IntPtr.Zero;
            while (true) {
                cursor = FindWindowExW(notify, cursor, null, null);
                if (cursor == IntPtr.Zero) { break; }
                enumerated++;
                if (enumerated > 64) { break; }
            }

            return string.Format(
                "direct={0};directValid={1};pager={2};pagerChild={3};pagerChildValid={4};pagerChildVisible={5};enumeratedChildren={6}",
                directChild == IntPtr.Zero ? "0" : "1",
                directChild != IntPtr.Zero && IsWindow(directChild) ? "1" : "0",
                pager == IntPtr.Zero ? "0" : "1",
                pagerChild == IntPtr.Zero ? "0" : "1",
                pagerChild != IntPtr.Zero && IsWindow(pagerChild) ? "1" : "0",
                pagerChild != IntPtr.Zero && IsWindowVisible(pagerChild) ? "1" : "0",
                enumerated);
        }
    }
}
'@

Add-ProbeInlineCSharp -Source $traySource -AssemblyLeaf 'AgentDesktopClosureProbe'

function Get-ReleaseBinary {
    $candidate = Join-Path (Split-Path -Parent (Split-Path -Parent (Get-ProbeRoot))) 'target/release/agent-desktop.exe'
    if (-not (Test-Path -LiteralPath $candidate)) {
        throw ('PROBE-HARNESS: the release binary is required for the leaf classification and is absent at ' + $candidate)
    }
    return (Resolve-Path -LiteralPath $candidate).ProviderPath
}

function Invoke-AgentDesktopJson {
    param([Parameter(Mandatory = $true)][string[]]$Arguments)
    $binary = Get-ReleaseBinary
    $output = & $binary @Arguments 2>&1 | Out-String
    if ([string]::IsNullOrWhiteSpace($output)) {
        throw 'PROBE-HARNESS: the binary produced no output'
    }
    return (ConvertFrom-Json -InputObject $output)
}

function Measure-LeafExtents {
    param([Parameter(Mandatory = $true)]$Node, [Parameter(Mandatory = $true)]$Tally)
    $children = @()
    if ($Node.PSObject.Properties.Name -contains 'children' -and $Node.children) {
        $children = @($Node.children)
    }
    if ($children.Count -gt 0) {
        foreach ($child in $children) { Measure-LeafExtents -Node $child -Tally $Tally }
        return
    }

    $Tally.leaves++
    $named = $Node.PSObject.Properties.Name -contains 'name' -and -not [string]::IsNullOrWhiteSpace([string]$Node.name)
    if ($named) {
        $Tally.namedLeaves++
        return
    }
    $Tally.namelessLeaves++
    if ($Node.PSObject.Properties.Name -notcontains 'bounds' -or -not $Node.bounds) {
        $Tally.namelessWithoutBounds++
        return
    }
    if ([double]$Node.bounds.width -gt 0 -and [double]$Node.bounds.height -gt 0) {
        $Tally.namelessPositiveArea++
    } else {
        $Tally.namelessZeroExtent++
    }
}

$results = [ordered]@{}

# --- A28-2: resolve and enumerate in one breath ----------------------------
$reading = [AgentDesktopClosureProbe.Tray]::ResolveAndEnumerate()
$trayFields = [ordered]@{}
foreach ($pair in $reading.Split(';')) {
    $parts = $pair.Split('=')
    if ($parts.Count -eq 2) { $trayFields[$parts[0]] = $parts[1] }
}
$results['tray_resolve_and_enumerate'] = [ordered]@{
    raw    = $reading
    fields = $trayFields
}

# --- A26-13 via A28-3: the nameless-leaf split on a Chromium tree ----------
$leafSplit = [ordered]@{}
$apps = Invoke-AgentDesktopJson -Arguments @('list-apps')
$chromiumApp = $null
foreach ($app in $apps.data.apps) {
    $snapshot = $null
    try {
        $snapshot = Invoke-AgentDesktopJson -Arguments @(
            'snapshot', '--app', $app.name, '--include-bounds', '--timeout-ms', '20000')
    } catch {
        continue
    }
    if (-not $snapshot.ok) { continue }
    $tally = [pscustomobject]@{
        leaves                 = 0
        namedLeaves            = 0
        namelessLeaves         = 0
        namelessPositiveArea   = 0
        namelessZeroExtent     = 0
        namelessWithoutBounds  = 0
    }
    Measure-LeafExtents -Node $snapshot.data.tree -Tally $tally
    $leafSplit[('app_' + $app.pid)] = [ordered]@{
        ref_count             = $snapshot.data.ref_count
        complete              = $snapshot.data.complete
        leaves                = $tally.leaves
        named_leaves          = $tally.namedLeaves
        nameless_leaves       = $tally.namelessLeaves
        nameless_positive_area = $tally.namelessPositiveArea
        nameless_zero_extent  = $tally.namelessZeroExtent
        nameless_without_bounds = $tally.namelessWithoutBounds
    }
    if ($tally.namelessLeaves -gt 0 -and $null -eq $chromiumApp) { $chromiumApp = $app.pid }
}
$results['nameless_leaf_split'] = $leafSplit
$results['classifiable_population_found'] = ($chromiumApp -ne $null)

[void](Write-ProbeJson -Probe $probe -Name 'closure.json' -InputObject $results)
Write-ProbeResult -Probe $probe -Status 'ok' -Message 'tray generation and nameless-leaf extents measured' -Data $results
