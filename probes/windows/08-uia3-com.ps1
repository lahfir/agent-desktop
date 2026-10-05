<#
.SYNOPSIS
    Probe 08 (sub-phase 2.0, unit U7): UIA3 COM shim - events/MTA threading,
    CacheRequest timing, tree-walker shape, and the authoritative pattern census.

.DESCRIPTION
    Compiles probes/windows/08-uia3-com.cs with the in-box pre-Roslyn csc.exe under
    /langversion:5 and drives it in five modes. The shim binds UIA3 COM through
    hand-declared [ComImport] interfaces and never references the GAC managed
    UIAutomationClient assembly, so every number it produces comes from the client
    stack the Rust `uiautomation` crate wraps (KTD1).

    A managed System.Windows.Automation sweep over the same three targets runs
    alongside as an explicitly non-authoritative cross-check, so the COM-vs-managed
    divergence is measured in one capture rather than asserted.

    Stable invocation contract for U4 (pattern census):

        <shim>.exe census --target LABEL=0xHWND [--target ...] [--max-nodes N] [--max-depth N]

    stdout is one compact JSON document:
        { stack, binding, coclass, mainThreadId, mainApartment,
          discoveredPatternAvailabilityProperties,
          result: { mode:"census", targets:[ { label, processId, controlViewNodes,
                    rawViewNodes, elementsWithAnyPattern,
                    elementsWithAnyPatternExcludingLegacy, patternTotals,
                    byControlType:[...], elements:[...] } ] } }

    Captures under captures/08-uia3-com/:
        ids.json          runtime-discovered UIA property/pattern identifier tables
        census.json       COM pattern census: WinForms (both fixture modes), WPF, Notepad
        comparison.json   COM vs managed side by side over the same three targets
        events.json       MTA handler add/remove, event ordering over 20 repetitions
        cache-timing.json IUIAutomationCacheRequest vs per-property reads
        walker.json       ControlView vs RawView vs ContentView node counts on Obsidian
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\common.ps1"

$Probe = '08-uia3-com'
$script:Spawned = New-Object System.Collections.ArrayList
$script:ExplorerWindows = New-Object System.Collections.ArrayList
$script:ObsidianLaunched = $false

function Initialize-Uia3Support {
    if ('AgentDesktopProbe.Uia3Support' -as [type]) { return }
    $src = @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

namespace AgentDesktopProbe {
    public static class Uia3Support {
        private delegate bool EnumProc(IntPtr hWnd, IntPtr lParam);

        [DllImport("user32.dll")] private static extern bool EnumWindows(EnumProc cb, IntPtr lParam);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern int GetClassNameW(IntPtr hWnd, StringBuilder buf, int max);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern int GetWindowTextW(IntPtr hWnd, StringBuilder buf, int max);
        [DllImport("user32.dll")] private static extern bool IsWindowVisible(IntPtr hWnd);
        [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern bool PostMessageW(IntPtr hWnd, uint msg, IntPtr w, IntPtr l);

        public static string ClassOf(IntPtr h) { StringBuilder b = new StringBuilder(256); GetClassNameW(h, b, 256); return b.ToString(); }
        public static string TitleOf(IntPtr h) { StringBuilder b = new StringBuilder(512); GetWindowTextW(h, b, 512); return b.ToString(); }
        public static int PidOf(IntPtr h) { uint p; GetWindowThreadProcessId(h, out p); return (int)p; }
        public static void Close(IntPtr h) { PostMessageW(h, 0x0010, IntPtr.Zero, IntPtr.Zero); }

        public static IntPtr[] TopLevelWindows() {
            List<IntPtr> found = new List<IntPtr>();
            EnumWindows(delegate(IntPtr h, IntPtr l) {
                if (IsWindowVisible(h)) { found.Add(h); }
                return true;
            }, IntPtr.Zero);
            return found.ToArray();
        }
    }
}
'@
    Add-Type -TypeDefinition $src -Language CSharp | Out-Null
}

function Find-ProbeWindow {
    param([int]$ProcessId = 0, [string]$ClassPattern = '', [string]$TitlePattern = '', [int]$TimeoutSec = 20)
    Initialize-Uia3Support
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ((Get-Date) -lt $deadline) {
        foreach ($h in [AgentDesktopProbe.Uia3Support]::TopLevelWindows()) {
            if ($ProcessId -gt 0 -and [AgentDesktopProbe.Uia3Support]::PidOf($h) -ne $ProcessId) { continue }
            if ($ClassPattern -and ([AgentDesktopProbe.Uia3Support]::ClassOf($h) -notmatch $ClassPattern)) { continue }
            $title = [AgentDesktopProbe.Uia3Support]::TitleOf($h)
            if ($TitlePattern -and ($title -notmatch $TitlePattern)) { continue }
            if (-not $TitlePattern -and -not $title) { continue }
            return $h
        }
        Start-Sleep -Milliseconds 300
    }
    return [IntPtr]::Zero
}

function Get-ShellFolderWindows {
    Initialize-Uia3Support
    $found = New-Object System.Collections.ArrayList
    foreach ($h in [AgentDesktopProbe.Uia3Support]::TopLevelWindows()) {
        $cls = [AgentDesktopProbe.Uia3Support]::ClassOf($h)
        if ($cls -eq 'CabinetWClass' -or $cls -eq 'ExploreWClass') { [void]$found.Add($h) }
    }
    return $found.ToArray()
}

function Start-TrackedScratch {
    param([string]$FilePath, [string[]]$ArgumentList = @(), [int]$TimeoutSec = 20)
    $p = Start-ScratchProcess -FilePath $FilePath -ArgumentList $ArgumentList -NoActivate -TimeoutSec $TimeoutSec
    [void]$script:Spawned.Add($p.ProcessId)
    return [pscustomobject]@{ ProcessId = $p.ProcessId; MainWindowHandle = $p.MainWindowHandle }
}

function ConvertTo-HandleHex {
    param([IntPtr]$Handle)
    return ('0x' + $Handle.ToInt64().ToString('x'))
}

function Invoke-Shim {
    param([string]$ExePath, [string[]]$ShimArgs, [int]$TimeoutSec = 620)
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $ExePath
    $psi.Arguments = ($ShimArgs | ForEach-Object { if ($_ -match '\s') { '"' + $_ + '"' } else { $_ } }) -join ' '
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.CreateNoWindow = $true
    $proc = [System.Diagnostics.Process]::Start($psi)
    [void]$script:Spawned.Add($proc.Id)
    $stdout = $proc.StandardOutput.ReadToEnd()
    $stderr = $proc.StandardError.ReadToEnd()
    if (-not $proc.WaitForExit($TimeoutSec * 1000)) {
        try { $proc.Kill() } catch { }
        throw ('shim mode ' + $ShimArgs[0] + ' exceeded ' + $TimeoutSec + 's')
    }
    if ($proc.ExitCode -ne 0) {
        throw ('shim mode ' + $ShimArgs[0] + ' exited ' + $proc.ExitCode + ': ' + $stderr)
    }
    if ($stderr) { Write-ProbeLog -Message ('shim ' + $ShimArgs[0] + ' stderr: ' + $stderr.Trim()) -Level 'warn' }
    return ($stdout | ConvertFrom-Json)
}

function Get-ManagedCensus {
    param([string]$Label, [IntPtr]$Handle)
    Add-Type -AssemblyName UIAutomationClient -ErrorAction SilentlyContinue
    Add-Type -AssemblyName UIAutomationTypes -ErrorAction SilentlyContinue
    $row = [ordered]@{ label = $Label; stack = 'managed-System.Windows.Automation'; authoritative = $false }
    try {
        $root = [System.Windows.Automation.AutomationElement]::FromHandle($Handle)
        $walker = [System.Windows.Automation.TreeWalker]::RawViewWalker
        $stack = New-Object System.Collections.Stack
        $stack.Push($root)
        $count = 0
        $withPattern = 0
        $types = @{}
        while ($stack.Count -gt 0 -and $count -lt 400) {
            $node = $stack.Pop()
            $count++
            $patterns = @($node.GetSupportedPatterns())
            if ($patterns.Count -gt 0) { $withPattern++ }
            $tn = $node.Current.ControlType.ProgrammaticName -replace '^ControlType\.', ''
            if ($types.ContainsKey($tn)) { $types[$tn] = $types[$tn] + 1 } else { $types[$tn] = 1 }
            $child = $walker.GetFirstChild($node)
            while ($child -ne $null) {
                $stack.Push($child)
                $child = $walker.GetNextSibling($child)
            }
        }
        $row['rawViewNodes'] = $count
        $row['elementsWithAnyPattern'] = $withPattern
        $row['controlTypes'] = $types
    } catch {
        $row['error'] = $_.Exception.Message
    }
    return $row
}

$status = 'ok'
$message = ''
$summary = [ordered]@{}

try {
    $captureDir = Get-CaptureDir -Probe $Probe
    Write-ProbeLog -Message ('capture dir ' + $captureDir)

    $csc = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
    if (-not (Test-Path -LiteralPath $csc)) { throw ('csc.exe not found at ' + $csc) }
    $cscVersion = (Get-Item -LiteralPath $csc).VersionInfo.FileVersion
    $buildDir = Join-Path $env:TEMP 'agent-desktop-uia3'
    if (-not (Test-Path -LiteralPath $buildDir)) { New-Item -ItemType Directory -Path $buildDir -Force | Out-Null }
    $shim = Join-Path $buildDir '08-uia3-com.exe'
    $source = Join-Path $PSScriptRoot '08-uia3-com.cs'
    $cscArgs = @('/nologo', '/target:exe', '/langversion:5', '/platform:anycpu', ('/out:' + $shim),
        '/reference:System.dll', '/reference:System.Core.dll', $source)
    $compilerOutput = (& $csc $cscArgs 2>&1 | Out-String).Trim()
    if ($LASTEXITCODE -ne 0) { throw ('csc.exe failed (' + $LASTEXITCODE + '): ' + $compilerOutput) }
    Write-ProbeLog -Message ('compiled shim with csc ' + $cscVersion + ' under /langversion:5')

    $scratchExe = Join-Path $PSScriptRoot 'scratch\bin\ScratchForms.exe'
    if (-not (Test-Path -LiteralPath $scratchExe)) {
        Write-ProbeLog -Message 'building scratch WinForms fixture'
        & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'scratch\build-scratch.ps1') | Out-Null
    }
    if (-not (Test-Path -LiteralPath $scratchExe)) { throw ('scratch fixture missing at ' + $scratchExe) }

    # --- identifier tables -------------------------------------------------
    $ids = Invoke-Shim -ExePath $shim -ShimArgs @('ids')
    Write-ProbeJson -Probe $Probe -Name 'ids.json' -InputObject $ids | Out-Null
    $summary['coclass'] = $ids.coclass
    $summary['patternAvailabilityPropertiesDiscovered'] = $ids.discoveredPatternAvailabilityProperties

    # --- pattern census (U4 consumes this mode) ----------------------------
    $winDefault = Start-TrackedScratch -FilePath $scratchExe -ArgumentList @('--tag', 'u7-default', '--pos', '40,40')
    $winHosted = Start-TrackedScratch -FilePath $scratchExe -ArgumentList @('--tag', 'u7-hosted', '--pos', '840,40', '--host-providers')
    $wpfArgs = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', (Join-Path $PSScriptRoot 'scratch\ScratchWpf.ps1'),
        '-Tag', 'u7', '-Left', '40', '-Top', '600', '-TimeoutSeconds', '300')
    $wpf = Start-TrackedScratch -FilePath 'powershell.exe' -ArgumentList $wpfArgs
    $wpfWindow = Find-ProbeWindow -ProcessId $wpf.ProcessId -TitlePattern 'AgentDesktop Scratch WPF'
    $notepad = Start-TrackedScratch -FilePath (Join-Path $env:WINDIR 'System32\notepad.exe')
    if ($wpfWindow -eq [IntPtr]::Zero) { throw 'WPF scratch window never appeared' }
    if ($winDefault.MainWindowHandle -eq [IntPtr]::Zero) { throw 'WinForms default-mode window never appeared' }
    if ($winHosted.MainWindowHandle -eq [IntPtr]::Zero) { throw 'WinForms host-providers window never appeared' }
    if ($notepad.MainWindowHandle -eq [IntPtr]::Zero) { throw 'notepad window never appeared' }

    $censusArgs = @('census',
        '--target', ('winforms-default=' + (ConvertTo-HandleHex $winDefault.MainWindowHandle)),
        '--target', ('winforms-host-providers=' + (ConvertTo-HandleHex $winHosted.MainWindowHandle)),
        '--target', ('wpf=' + (ConvertTo-HandleHex $wpfWindow)),
        '--target', ('notepad=' + (ConvertTo-HandleHex $notepad.MainWindowHandle)),
        '--max-nodes', '300', '--max-depth', '30')
    $census = Invoke-Shim -ExePath $shim -ShimArgs $censusArgs
    Write-ProbeJson -Probe $Probe -Name 'census.json' -InputObject $census | Out-Null

    $comRows = @()
    foreach ($t in $census.result.targets) {
        $comRows += [ordered]@{
            label = $t.label
            stack = 'uia3-com'
            authoritative = $true
            rawViewNodes = $t.rawViewNodes
            controlViewNodes = $t.controlViewNodes
            elementsWithAnyPattern = $t.elementsWithAnyPattern
            elementsWithAnyPatternExcludingLegacy = $t.elementsWithAnyPatternExcludingLegacy
        }
    }
    $managedRows = @(
        (Get-ManagedCensus -Label 'winforms-default' -Handle $winDefault.MainWindowHandle),
        (Get-ManagedCensus -Label 'winforms-host-providers' -Handle $winHosted.MainWindowHandle),
        (Get-ManagedCensus -Label 'wpf' -Handle $wpfWindow),
        (Get-ManagedCensus -Label 'notepad' -Handle $notepad.MainWindowHandle)
    )
    $comparison = [ordered]@{
        question = 'do UIA3 COM and managed System.Windows.Automation report the same pattern availability on this VM'
        note = 'COM rows are authoritative (KTD1); managed rows are a labeled non-authoritative cross-check'
        com = $comRows
        managed = $managedRows
    }
    $diverged = $false
    foreach ($c in $comRows) {
        $m = $managedRows | Where-Object { $_.label -eq $c.label } | Select-Object -First 1
        if ($m -and $m.elementsWithAnyPattern -ne $c.elementsWithAnyPattern) { $diverged = $true }
    }
    $comparison['stacksDiverge'] = $diverged
    Write-ProbeJson -Probe $Probe -Name 'comparison.json' -InputObject $comparison | Out-Null
    $summary['stacksDiverge'] = $diverged

    Stop-ScratchProcess -ProcessId $winDefault.ProcessId
    Stop-ScratchProcess -ProcessId $wpf.ProcessId
    Stop-ScratchProcess -ProcessId $notepad.ProcessId

    # --- events / MTA threading -------------------------------------------
    $events = Invoke-Shim -ExePath $shim -ShimArgs @('events', '--hwnd', (ConvertTo-HandleHex $winHosted.MainWindowHandle), '--reps', '20')
    Write-ProbeJson -Probe $Probe -Name 'events.json' -InputObject $events | Out-Null
    $noWindowChurn = Invoke-Shim -ExePath $shim -ShimArgs @('events', '--hwnd', (ConvertTo-HandleHex $winHosted.MainWindowHandle), '--reps', '20', '--drive', 'vcf')
    $windowChurnOnly = Invoke-Shim -ExePath $shim -ShimArgs @('events', '--hwnd', (ConvertTo-HandleHex $winHosted.MainWindowHandle), '--reps', '20', '--drive', 'w', '--cheap-callbacks')
    $isolation = [ordered]@{
        question = 'what makes RemoveXEventHandler on the MTA worker block for tens of seconds'
        method = 'same 20-repetition drive loop with one action class removed at a time; teardown timed per handler on the registering thread'
        arms = @(
            [ordered]@{
                arm = 'full drive loop (value + click + focus + window hide/show), property reads inside callbacks'
                driveActions = $events.result.driveActions
                cheapCallbacks = $events.result.cheapCallbacks
                totalEvents = $events.result.totalEvents
                quiescentTeardownElapsedMs = $events.result.quiescentTeardownElapsedMs
                inflightTeardownElapsedMs = $events.result.inflightTeardownElapsedMs
                quiescentTeardownPerHandler = $events.result.quiescentTeardownPerHandler
            },
            [ordered]@{
                arm = 'no window hide/show (value + click + focus only)'
                driveActions = $noWindowChurn.result.driveActions
                cheapCallbacks = $noWindowChurn.result.cheapCallbacks
                totalEvents = $noWindowChurn.result.totalEvents
                quiescentTeardownElapsedMs = $noWindowChurn.result.quiescentTeardownElapsedMs
                inflightTeardownElapsedMs = $noWindowChurn.result.inflightTeardownElapsedMs
                quiescentTeardownPerHandler = $noWindowChurn.result.quiescentTeardownPerHandler
            },
            [ordered]@{
                arm = 'window hide/show only, with callbacks that do no property reads'
                driveActions = $windowChurnOnly.result.driveActions
                cheapCallbacks = $windowChurnOnly.result.cheapCallbacks
                totalEvents = $windowChurnOnly.result.totalEvents
                quiescentTeardownElapsedMs = $windowChurnOnly.result.quiescentTeardownElapsedMs
                inflightTeardownElapsedMs = $windowChurnOnly.result.inflightTeardownElapsedMs
                quiescentTeardownPerHandler = $windowChurnOnly.result.quiescentTeardownPerHandler
            }
        )
    }
    Write-ProbeJson -Probe $Probe -Name 'events-teardown-isolation.json' -InputObject $isolation | Out-Null
    $summary['quiescentTeardownElapsedMsNoWindowChurn'] = $noWindowChurn.result.quiescentTeardownElapsedMs
    $summary['quiescentTeardownElapsedMsWindowChurnOnly'] = $windowChurnOnly.result.quiescentTeardownElapsedMs
    $summary['orderingStableAcrossReps'] = $events.result.orderingStableAcrossReps
    $summary['orderingStableExcludingWindowEvents'] = $events.result.orderingStableExcludingWindowEvents
    $summary['quiescentTeardownCompleted'] = $events.result.quiescentTeardownCompleted
    $summary['quiescentTeardownElapsedMs'] = $events.result.quiescentTeardownElapsedMs
    $summary['inflightTeardownCompletedUnderLoad'] = $events.result.inflightTeardownCompletedUnderLoad
    $summary['eventThreadsDistinctFromMain'] = $events.result.eventThreadsDistinctFromMain
    Stop-ScratchProcess -ProcessId $winHosted.ProcessId

    # --- CacheRequest timing on Explorer ----------------------------------
    $explorerHandle = [IntPtr]::Zero
    $explorerKind = ''
    Initialize-Uia3Support
    $preExisting = @(Get-ShellFolderWindows)
    try {
        Start-Process -FilePath (Join-Path $env:WINDIR 'explorer.exe') -ArgumentList (Join-Path $env:WINDIR 'System32') | Out-Null
        $deadline = (Get-Date).AddSeconds(30)
        while ((Get-Date) -lt $deadline) {
            foreach ($w in (Get-ShellFolderWindows)) {
                if ($preExisting -notcontains $w) { [void]$script:ExplorerWindows.Add($w) }
            }
            if ($script:ExplorerWindows.Count -gt 0) { break }
            Start-Sleep -Milliseconds 500
        }
        if ($script:ExplorerWindows.Count -gt 0) {
            Start-Sleep -Seconds 2
            $explorerHandle = $script:ExplorerWindows[0]
            $explorerKind = 'probe-opened folder window (CabinetWClass) over %WINDIR%\System32, closed in teardown'
        }
    } catch {
        Write-ProbeLog -Message ('could not open an Explorer folder window: ' + $_.Exception.Message) -Level 'warn'
    }
    if ($explorerHandle -eq [IntPtr]::Zero) {
        $explorerHandle = Find-ProbeWindow -ClassPattern '^Shell_TrayWnd$' -TitlePattern '.*' -TimeoutSec 5
        $explorerKind = 'existing shell taskbar (Shell_TrayWnd), read-only'
    }
    if ($explorerHandle -eq [IntPtr]::Zero) { throw 'no Explorer-owned window available for the cache-timing measurement' }
    $cache = Invoke-Shim -ExePath $shim -ShimArgs @('cache', '--hwnd', (ConvertTo-HandleHex $explorerHandle), '--label', 'explorer', '--max-nodes', '1500', '--reps', '5')
    $cacheCapture = [ordered]@{
        target = 'explorer.exe'
        targetKind = $explorerKind
        shim = $cache
    }
    Write-ProbeJson -Probe $Probe -Name 'cache-timing.json' -InputObject $cacheCapture | Out-Null
    $summary['cacheTimingMultiplier'] = $cache.result.timingMultiplier
    $summary['cacheClaimVerdict'] = $cache.result.claimVerdict

    # --- walker shape on Obsidian -----------------------------------------
    $obsidianExe = Join-Path $env:LOCALAPPDATA 'Programs\Obsidian\Obsidian.exe'
    $obsidianPreexisting = $false
    $obsidianWindow = [IntPtr]::Zero
    $obsidianProcessId = 0
    $existing = @(Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero })
    if ($existing.Count -gt 0) {
        $obsidianPreexisting = $true
        $obsidianWindow = $existing[0].MainWindowHandle
        $obsidianProcessId = $existing[0].Id
        Write-ProbeLog -Message 'Obsidian already running - walking the existing process read-only'
    } elseif (Test-Path -LiteralPath $obsidianExe) {
        $obs = Start-TrackedScratch -FilePath $obsidianExe -TimeoutSec 40
        $obsidianProcessId = $obs.ProcessId
        $script:ObsidianLaunched = $true
        $obsidianWindow = Find-ProbeWindow -ClassPattern '^Chrome_WidgetWin_1$' -TitlePattern 'Obsidian' -TimeoutSec 60
        Start-Sleep -Seconds 5
    }
    if ($obsidianWindow -eq [IntPtr]::Zero) {
        Write-ProbeLog -Message 'Obsidian window unavailable - walker mode falls back to the WinForms fixture' -Level 'warn'
        $fallback = Start-TrackedScratch -FilePath $scratchExe -ArgumentList @('--tag', 'u7-walker', '--pos', '40,40')
        $obsidianWindow = $fallback.MainWindowHandle
        $walkerTarget = 'winforms-scratch-fallback'
    } else {
        $walkerTarget = 'obsidian'
    }
    $walker = Invoke-Shim -ExePath $shim -ShimArgs @('walker', '--hwnd', (ConvertTo-HandleHex $obsidianWindow), '--label', $walkerTarget, '--max-nodes', '20000', '--max-depth', '60')
    $walkerCapture = [ordered]@{
        target = $walkerTarget
        obsidianPreexisting = $obsidianPreexisting
        obsidianVersion = ''
        shim = $walker
    }
    if (Test-Path -LiteralPath $obsidianExe) {
        $walkerCapture['obsidianVersion'] = (Get-Item -LiteralPath $obsidianExe).VersionInfo.ProductVersion
    }
    Write-ProbeJson -Probe $Probe -Name 'walker.json' -InputObject $walkerCapture | Out-Null
    foreach ($v in $walker.result.viewsAfterSettle) {
        $summary[('walker_' + $v.view)] = $v.nodeCount
    }

    $summary['compiler'] = $cscVersion
    $message = 'uia3 com shim: census+comparison+events+cache+walker captured'
} catch {
    $status = 'fail'
    $message = ($_.Exception.Message -replace '[\r\n]+', ' ')
    Write-ProbeLog -Message ('probe failed: ' + $message) -Level 'error'
} finally {
    foreach ($w in @($script:ExplorerWindows)) {
        try {
            Initialize-Uia3Support
            [AgentDesktopProbe.Uia3Support]::Close($w)
        } catch { }
    }
    if ($script:ObsidianLaunched) {
        foreach ($p in @(Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue)) {
            Register-ScratchProcessId -ProcessId $p.Id
            if (-not ($script:Spawned -contains $p.Id)) { [void]$script:Spawned.Add($p.Id) }
        }
    }
    foreach ($id in @($script:Spawned)) {
        try { Stop-ScratchProcess -ProcessId $id } catch { Write-ProbeLog -Message ('teardown: ' + $_.Exception.Message) -Level 'warn' }
    }
    foreach ($f in @(Get-ChildItem -LiteralPath (Get-CaptureDir -Probe $Probe) -File -ErrorAction SilentlyContinue)) {
        if ($f.Name -like '*.normalized') { continue }
        if (-not (Test-CaptureRedaction -Path $f.FullName)) { $status = 'fail'; $message = ('redaction residue in ' + $f.Name) }
    }
}

Write-ProbeResult -Probe $Probe -Status $status -Message $message -Data $summary
if ($status -eq 'fail') { exit 1 }
exit 0
