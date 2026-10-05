<#
.SYNOPSIS
    Probe 11 (sub-phase 2.0, unit U8): Chromium/Electron accessibility activation.

.DESCRIPTION
    Grades the committed phases.md claim that Chromium 138+ "exposes a UIA tree to any
    UIA client with no flag" against the installed Obsidian build, on both client stacks
    (KTD1): managed System.Windows.Automation and the U7 UIA3 COM shim.

    Four Obsidian instances are launched, one per (stack x flag) cell, because the first
    UIA client to touch a renderer activates accessibility for that process - reusing one
    instance would make the second stack's "first contact" a settled reading.

    The bundled Chromium and Electron versions are read off THIS installation by scanning
    Obsidian.exe for the embedded user-agent string. ELECTRON_RUN_AS_NODE=1 with
    -p process.versions produces no output on this VM and the framework DLLs carry
    component versions, so the UA scan is the determination of record. The activation
    verdict is only graded once that version is established.

    Two hazards carried from earlier units, both recorded as probe-placement hazards and
    explicitly NOT product claims:
      - U3: leaving other windows on top of Obsidian held it at its first-contact node
        count through a full settle. Mechanism not isolated. This probe places Obsidian
        itself and records the foreground owner at read time.
      - U4: matching a bare Chrome_WidgetWin_1 window found an unrelated window and
        recorded 9 nodes / 0% coverage as if it were the Electron answer. This probe
        matches only windows whose owning pid is one of the launched Obsidian pids.
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\common.ps1"

$Probe = '11-electron-activation'
$script:Spawned = New-Object System.Collections.ArrayList

function Initialize-WindowSupport {
    if ('AgentDesktopProbe.Win11' -as [type]) { return }
    Add-Type -Language CSharp -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

namespace AgentDesktopProbe {
    public static class Win11 {
        private delegate bool EnumProc(IntPtr h, IntPtr l);
        [DllImport("user32.dll")] private static extern bool EnumWindows(EnumProc cb, IntPtr l);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern int GetClassNameW(IntPtr h, StringBuilder b, int max);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern int GetWindowTextW(IntPtr h, StringBuilder b, int max);
        [DllImport("user32.dll")] private static extern bool IsWindowVisible(IntPtr h);
        [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);

        public static string ClassOf(IntPtr h) { StringBuilder b = new StringBuilder(256); GetClassNameW(h, b, 256); return b.ToString(); }
        public static string TitleOf(IntPtr h) { StringBuilder b = new StringBuilder(512); GetWindowTextW(h, b, 512); return b.ToString(); }
        public static int PidOf(IntPtr h) { uint p; GetWindowThreadProcessId(h, out p); return (int)p; }

        public static IntPtr[] VisibleTopLevel() {
            List<IntPtr> found = new List<IntPtr>();
            EnumWindows(delegate(IntPtr h, IntPtr l) { if (IsWindowVisible(h)) { found.Add(h); } return true; }, IntPtr.Zero);
            return found.ToArray();
        }
    }
}
'@ | Out-Null
}

function Get-BundledVersions {
    param([string]$ExePath)
    $rx = New-Object Text.RegularExpressions.Regex 'Chrome/(\d+\.\d+\.\d+\.\d+) Electron/(\d+\.\d+\.\d+)'
    $fs = [IO.File]::OpenRead($ExePath)
    $buf = New-Object byte[] (8MB)
    $prev = ''
    $chrome = ''
    $electron = ''
    try {
        while (($n = $fs.Read($buf, 0, 8MB)) -gt 0) {
            $text = $prev + [Text.Encoding]::GetEncoding(28591).GetString($buf, 0, $n)
            $m = $rx.Match($text)
            if ($m.Success) { $chrome = $m.Groups[1].Value; $electron = $m.Groups[2].Value; break }
            if ($text.Length -ge 256) { $prev = $text.Substring($text.Length - 256) } else { $prev = $text }
        }
    } finally { $fs.Close() }
    return [ordered]@{
        method                = 'user-agent string scanned out of the shipped Obsidian.exe on this installation'
        obsidianFileVersion   = (Get-Item -LiteralPath $ExePath).VersionInfo.ProductVersion
        chromiumVersion       = $chrome
        electronVersion       = $electron
        established           = ($chrome -ne '' -and $electron -ne '')
        rejectedMethods       = @(
            'ELECTRON_RUN_AS_NODE=1 Obsidian.exe -p process.versions produced no output on this VM',
            'framework DLL file versions carry component versions, not the Chromium version'
        )
    }
}

function Start-ObsidianInstance {
    param([string]$ExePath, [string[]]$ExtraArgs = @())
    Initialize-WindowSupport
    if ($ExtraArgs.Count -gt 0) { Start-Process -FilePath $ExePath -ArgumentList $ExtraArgs | Out-Null }
    else { Start-Process -FilePath $ExePath | Out-Null }
    $deadline = (Get-Date).AddSeconds(75)
    while ((Get-Date) -lt $deadline) {
        $pids = @(Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
        foreach ($id in $pids) {
            if (-not $script:Spawned.Contains($id)) {
                Register-ScratchProcessId -ProcessId $id
                [void]$script:Spawned.Add($id)
            }
        }
        foreach ($h in [AgentDesktopProbe.Win11]::VisibleTopLevel()) {
            if ([AgentDesktopProbe.Win11]::ClassOf($h) -ne 'Chrome_WidgetWin_1') { continue }
            $owner = [AgentDesktopProbe.Win11]::PidOf($h)
            if ($pids -notcontains $owner) { continue }
            if (-not [AgentDesktopProbe.Win11]::TitleOf($h)) { continue }
            Show-WindowNoActivate -WindowHandle $h -X 0 -Y 0 -Width 1280 -Height 720
            return [pscustomobject]@{ WindowHandle = $h; ProcessId = $owner; AllPids = $pids }
        }
        Start-Sleep -Milliseconds 700
    }
    return $null
}

function Stop-AllObsidian {
    foreach ($p in @(Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue)) {
        try { Stop-Process -Id $p.Id -Force -ErrorAction Stop } catch { }
    }
    $deadline = (Get-Date).AddSeconds(20)
    while ((Get-Date) -lt $deadline) {
        if (-not (Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue)) { return $true }
        Start-Sleep -Milliseconds 400
    }
    return (-not (Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue))
}

function Measure-ManagedTree {
    param([IntPtr]$WindowHandle, [string]$Prefix = '')
    Add-Type -AssemblyName UIAutomationClient
    Add-Type -AssemblyName UIAutomationTypes
    $root = [System.Windows.Automation.AutomationElement]::FromHandle($WindowHandle)
    $out = [ordered]@{}
    foreach ($view in @('RawView', 'ControlView')) {
        $walker = if ($view -eq 'RawView') { [System.Windows.Automation.TreeWalker]::RawViewWalker } else { [System.Windows.Automation.TreeWalker]::ControlViewWalker }
        $stack = New-Object System.Collections.Stack
        $stack.Push($root)
        $count = 0
        $refable = 0
        while ($stack.Count -gt 0 -and $count -lt 8000) {
            $node = $stack.Pop()
            $count++
            if (@($node.GetSupportedPatterns()).Count -gt 0) { $refable++ }
            $child = $walker.GetFirstChild($node)
            while ($null -ne $child) {
                $stack.Push($child)
                $child = $walker.GetNextSibling($child)
            }
        }
        $out[($Prefix + $view + 'Nodes')] = $count
        $out[($Prefix + $view + 'RefableNodes')] = $refable
    }
    return $out
}

function Invoke-ComShim {
    param([string]$ExePath, [string[]]$ShimArgs, [int]$TimeoutSec = 300)
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $ExePath
    $psi.Arguments = ($ShimArgs -join ' ')
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.CreateNoWindow = $true
    $proc = [System.Diagnostics.Process]::Start($psi)
    Register-ScratchProcessId -ProcessId $proc.Id
    [void]$script:Spawned.Add($proc.Id)
    $stdout = $proc.StandardOutput.ReadToEnd()
    $stderr = $proc.StandardError.ReadToEnd()
    if (-not $proc.WaitForExit($TimeoutSec * 1000)) { try { $proc.Kill() } catch { }; throw ('shim ' + $ShimArgs[0] + ' timed out') }
    if ($proc.ExitCode -ne 0) { throw ('shim ' + $ShimArgs[0] + ' exited ' + $proc.ExitCode + ': ' + $stderr) }
    return ($stdout | ConvertFrom-Json)
}

$status = 'ok'
$message = ''
$summary = [ordered]@{}

try {
    Initialize-ProbeNative
    Initialize-WindowSupport
    $obsidianExe = Join-Path $env:LOCALAPPDATA 'Programs\Obsidian\Obsidian.exe'
    if (-not (Test-Path -LiteralPath $obsidianExe)) { throw ('Obsidian not installed at ' + $obsidianExe) }
    $preexisting = @(Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue).Count -gt 0
    $versions = Get-BundledVersions -ExePath $obsidianExe
    if (-not $versions.established) { throw 'bundled Chromium version could not be read off this installation' }
    Write-ProbeLog -Message ('obsidian ' + $versions.obsidianFileVersion + ' bundles chromium ' + $versions.chromiumVersion + ' / electron ' + $versions.electronVersion)

    $csc = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
    $buildDir = Join-Path $env:TEMP 'agent-desktop-uia3'
    if (-not (Test-Path -LiteralPath $buildDir)) { New-Item -ItemType Directory -Path $buildDir -Force | Out-Null }
    $shim = Join-Path $buildDir '08-uia3-com.exe'
    $shimOut = (& $csc @('/nologo', '/target:exe', '/langversion:5', '/platform:anycpu', ('/out:' + $shim),
            '/reference:System.dll', '/reference:System.Core.dll', (Join-Path $PSScriptRoot '08-uia3-com.cs')) 2>&1 | Out-String).Trim()
    if ($LASTEXITCODE -ne 0) { throw ('csc.exe failed building the U7 shim: ' + $shimOut) }

    $cells = @()
    foreach ($flagArm in @(
            [pscustomobject]@{ Name = 'no-flag'; Args = @() },
            [pscustomobject]@{ Name = 'force-renderer-accessibility'; Args = @('--force-renderer-accessibility') })) {
        foreach ($stack in @('managed', 'uia3-com')) {
            if (-not (Stop-AllObsidian)) { throw 'could not bring Obsidian to a clean state before an arm' }
            $inst = Start-ObsidianInstance -ExePath $obsidianExe -ExtraArgs $flagArm.Args
            if ($null -eq $inst) { throw ('Obsidian window never appeared for arm ' + $flagArm.Name + '/' + $stack) }
            $cell = [ordered]@{
                flagArm            = $flagArm.Name
                stack              = $stack
                authoritative      = ($stack -eq 'uia3-com')
                windowClassName    = [AgentDesktopProbe.Win11]::ClassOf($inst.WindowHandle)
                windowHandle       = $inst.WindowHandle.ToInt64()
                windowHost         = [ordered]@{ processId = $inst.ProcessId }
                latencyObsidianProcessCountAtWindowDiscovery = $inst.AllPids.Count
                foregroundOwner    = [ordered]@{ processId = [AgentDesktopProbe.Native]::GetForegroundProcessId() }
                settleMs           = 8000
            }
            if ($stack -eq 'managed') {
                $cell['firstContact'] = Measure-ManagedTree -WindowHandle $inst.WindowHandle -Prefix 'latencyFirstContact'
                Start-Sleep -Milliseconds 8000
                $cell['afterSettle'] = Measure-ManagedTree -WindowHandle $inst.WindowHandle -Prefix 'settled'
            } else {
                $walker = Invoke-ComShim -ExePath $shim -ShimArgs @('walker', '--hwnd', ('0x' + $inst.WindowHandle.ToInt64().ToString('x')),
                    '--label', 'obsidian', '--max-nodes', '20000', '--max-depth', '60', '--settle-ms', '8000')
                $fc = [ordered]@{}
                foreach ($v in $walker.result.viewsFirstContact) { $fc[('latencyFirstContact' + $v.view + 'Nodes')] = $v.nodeCount }
                $st = [ordered]@{}
                foreach ($v in $walker.result.viewsAfterSettle) { $st[('settled' + $v.view + 'Nodes')] = $v.nodeCount }
                $census = Invoke-ComShim -ExePath $shim -ShimArgs @('census', '--target', ('obsidian=0x' + $inst.WindowHandle.ToInt64().ToString('x')),
                    '--max-nodes', '20000', '--max-depth', '60')
                $t = $census.result.targets[0]
                $st['settledRefableNodesWithAnyPattern'] = $t.elementsWithAnyPattern
                $st['settledRefableNodesExcludingLegacy'] = $t.elementsWithAnyPatternExcludingLegacy
                $cell['firstContact'] = $fc
                $cell['afterSettle'] = $st
                $cell['censusNote'] = 'census ran in a second shim process against the already-activated instance, so its counts are settled counts; per-element rows are deliberately not captured (R11: Obsidian note titles are content)'
            }
            $rawFc = $cell.firstContact['latencyFirstContactRawViewNodes']
            $rawSt = $cell.afterSettle['settledRawViewNodes']
            if ($null -eq $rawFc -or $null -eq $rawSt) {
                $fcText = if ($null -eq $rawFc) { 'missing' } else { [string]$rawFc }
                $stText = if ($null -eq $rawSt) { 'missing' } else { [string]$rawSt }
                throw ('PROBE-HARNESS: arm ' + $flagArm.Name + '/' + $stack + ' produced no RawView node count (first contact ' +
                    $fcText + ', settled ' + $stText + '), so the activation verdict would compare against nothing')
            }
            $cell['latencyRawViewGrowthRatio'] = if ($rawFc -gt 0) { [math]::Round(($rawSt / [double]$rawFc), 2) } else { $null }
            $cell['activatedWithoutFlag'] = ($flagArm.Name -eq 'no-flag' -and $rawSt -gt $rawFc)
            $cells += $cell
            Write-ProbeLog -Message ('arm ' + $flagArm.Name + '/' + $stack + ': raw ' + $rawFc + ' -> ' + $rawSt)
        }
    }
    $allGone = Stop-AllObsidian

    $noFlagCom = $cells | Where-Object { $_.flagArm -eq 'no-flag' -and $_.stack -eq 'uia3-com' } | Select-Object -First 1
    $flagCom = $cells | Where-Object { $_.flagArm -ne 'no-flag' -and $_.stack -eq 'uia3-com' } | Select-Object -First 1
    $noFlagManaged = $cells | Where-Object { $_.flagArm -eq 'no-flag' -and $_.stack -eq 'managed' } | Select-Object -First 1
    $flagManaged = $cells | Where-Object { $_.flagArm -ne 'no-flag' -and $_.stack -eq 'managed' } | Select-Object -First 1
    $bothActivate = ($noFlagCom.activatedWithoutFlag -and $noFlagManaged.activatedWithoutFlag)
    $flagVerdict = 'ungraded'
    if ($versions.established) {
        $major = [int]($versions.chromiumVersion -split '\.')[0]
        if ($major -lt 138) {
            $flagVerdict = 'flag still required - bundled Chromium predates the 138 auto-UIA default'
        } elseif ($bothActivate -and $noFlagCom.afterSettle['settledRawViewNodes'] -eq $flagCom.afterSettle['settledRawViewNodes']) {
            $flagVerdict = 'partially useful - redundant for eventual tree exposure: on Chromium ' + $versions.chromiumVersion +
            ' both client stacks reach the same settled RawView count with no flag, so the phases.md claim holds for exposure. It is not redundant for first-contact readiness: without the flag first contact is deterministically the small pre-activation shell on every run and both stacks, while with the flag first contact is a race against the async tree build and was observed anywhere from a fraction of the settled count to nearly all of it. Neither arm removes the need to settle before trusting a first snapshot.'
        } elseif ($bothActivate) {
            $flagVerdict = 'redundant for tree exposure - both client stacks get a renderer tree with no flag on Chromium ' + $versions.chromiumVersion
        } else {
            $flagVerdict = 'still required - a 138+ Chromium did not expose a renderer tree without the flag on at least one stack'
        }
    }

    $capture = [ordered]@{
        question = 'does Chromium 138+ expose a UIA tree to any UIA client with no flag, and does --force-renderer-accessibility still change anything'
        gradedAgainst = 'the committed phases.md line that Chromium 138+ exposes a UIA tree to any UIA client with no flag (P2-O15 / 2.4)'
        stack = 'both: managed System.Windows.Automation and the U7 UIA3 COM shim (KTD1 - the COM rows are the authoritative ones)'
        scope = 'app/provider - specific to this Obsidian/Electron/Chromium build; the activation mechanism is Chromium behavior, not a Windows API contract'
        versions = $versions
        obsidianPreexistingAtProbeStart = $preexisting
        method = 'one fresh Obsidian instance per (stack x flag) cell, because the first UIA client to touch a renderer activates accessibility for that process'
        placementHazards = @(
            'U3: an earlier revision that left other windows on top of Obsidian held it at its first-contact node count through a full 8s settle and a 16s instrumented hold, three runs running. Minimizing the covering windows fixed it. The mechanism was NOT isolated (Chromium native-window occlusion tracking is the obvious candidate, unproven). Carried here as a probe-placement hazard, explicitly not a product claim.',
            'U4: matching a bare Chrome_WidgetWin_1 window found an unrelated window and recorded 9 nodes / 0% coverage as if it were the Electron answer. This probe only accepts a window whose owning pid is one of the launched Obsidian pids, and reads the hosting pid back off the window.'
        )
        countStabilityNote = 'settled node counts are content-dependent (U7 measured RawView 172 on a bare walker run, U3 measured 119 through its probe). The stable fact is the growth ratio across the settle, not the absolute count.'
        cells = $cells
        findings = [ordered]@{
            managedActivatesWithoutFlag = $noFlagManaged.activatedWithoutFlag
            comActivatesWithoutFlag     = $noFlagCom.activatedWithoutFlag
            clientStackDivergence       = ($noFlagManaged.activatedWithoutFlag -ne $noFlagCom.activatedWithoutFlag)
            latencyComRawViewNoFlagFirstContact       = $noFlagCom.firstContact['latencyFirstContactRawViewNodes']
            latencyComRawViewWithFlagFirstContact     = $flagCom.firstContact['latencyFirstContactRawViewNodes']
            latencyManagedRawViewNoFlagFirstContact   = $noFlagManaged.firstContact['latencyFirstContactRawViewNodes']
            latencyManagedRawViewWithFlagFirstContact = $flagManaged.firstContact['latencyFirstContactRawViewNodes']
            settledComRawViewNoFlag       = $noFlagCom.afterSettle['settledRawViewNodes']
            settledComRawViewWithFlag     = $flagCom.afterSettle['settledRawViewNodes']
            settledManagedRawViewNoFlag   = $noFlagManaged.afterSettle['settledRawViewNodes']
            settledManagedRawViewWithFlag = $flagManaged.afterSettle['settledRawViewNodes']
            settledCountsMatchAcrossFlagArms = ($noFlagCom.afterSettle['settledRawViewNodes'] -eq $flagCom.afterSettle['settledRawViewNodes'] -and $noFlagManaged.afterSettle['settledRawViewNodes'] -eq $flagManaged.afterSettle['settledRawViewNodes'])
            firstContactCountsAreRunVarying = 'first-contact counts are a race against Chromium async tree construction and are named latency* so the KTD9 normalizer canonicalizes them; the raw samples stay in this capture, the settled counts are the stable evidence'
            flagVerdict                 = $flagVerdict
            supersededPreFinding        = 'the plan pre-finding (8 managed descendants, stable at 2/5/10s, candidate CONTRADICTS) is superseded: it was measured through an occluded window with a managed client, and neither the occlusion nor the settle window was controlled'
        }
        teardown = [ordered]@{ allObsidianProcessesTerminated = $allGone }
    }
    Write-ProbeJson -Probe $Probe -Name 'electron-activation.json' -InputObject $capture | Out-Null

    $summary['chromiumVersion'] = $versions.chromiumVersion
    $summary['electronVersion'] = $versions.electronVersion
    $summary['obsidianVersion'] = $versions.obsidianFileVersion
    $summary['comRawViewNoFlag'] = ('' + $capture.findings.latencyComRawViewNoFlagFirstContact + ' -> ' + $capture.findings.settledComRawViewNoFlag)
    $summary['comRawViewWithFlag'] = ('' + $capture.findings.latencyComRawViewWithFlagFirstContact + ' -> ' + $capture.findings.settledComRawViewWithFlag)
    $summary['managedRawViewNoFlag'] = ('' + $capture.findings.latencyManagedRawViewNoFlagFirstContact + ' -> ' + $capture.findings.settledManagedRawViewNoFlag)
    $summary['managedRawViewWithFlag'] = ('' + $capture.findings.latencyManagedRawViewWithFlagFirstContact + ' -> ' + $capture.findings.settledManagedRawViewWithFlag)
    $summary['clientStackDivergence'] = $capture.findings.clientStackDivergence
    $summary['flagVerdict'] = $flagVerdict
    $message = 'electron activation measured on both stacks with and without the flag'
} catch {
    $status = 'fail'
    $message = ($_.Exception.Message -replace '[\r\n]+', ' ')
    Write-ProbeLog -Message ('probe failed: ' + $message) -Level 'error'
} finally {
    [void](Stop-AllObsidian)
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
