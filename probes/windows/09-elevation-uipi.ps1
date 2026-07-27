<#
.SYNOPSIS
    Probe 09 (sub-phase 2.0, unit U8): UIPI measured across a real integrity boundary.

.DESCRIPTION
    Engineering Invariant #6 is about a Medium-integrity automation process driving a
    High-integrity target. This box cannot produce that pair by ordinary means: the
    built-in Administrator (RID 500) runs a full token at S-1-16-12288 and
    FilterAdministratorToken is unset, so Admin Approval Mode is off and
    "Start-Process -Verb RunAs" yields High-vs-High. runas.exe /trustlevel:0x20000 was
    also ruled out - it restricts the token but leaves the mandatory label at High.
    The boundary is therefore manufactured with common.ps1's
    Start-MediumIntegrityProcess, which lowers a duplicated token to S-1-16-8192 and
    asserts the label on read-back.

    Two arms run the SAME worker code against the SAME probe-launched High Notepad;
    only the integrity level of the worker differs:

        arm "high"    control  - worker at S-1-16-12288, injection expected to land
        arm "medium"  test     - worker at S-1-16-8192,  injection expected to be dropped

    Effect is never read from SendInput's return value (which is documented to report
    success when UIPI drops the input). The parent re-reads the target's edit text over
    WM_GETTEXT before and after each arm, High-to-High, as independent observation.
#>
[CmdletBinding()]
param(
    [switch]$Worker,
    [string]$WorkerOut = '',
    [string]$HelperDir = '',
    [int]$TargetProcessId = 0,
    [string]$TargetHwnd = '0',
    [string]$EditHwnd = '0',
    [string]$Marker = 'ZZZ',
    [string]$Arm = 'unknown'
)

$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\common.ps1"

$Probe = '09-elevation-uipi'
$NativeShimSource = @'
using System;
using System.Runtime.InteropServices;

namespace AgentDesktopProbe {
    public static class Native {
        [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
        [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
        [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
        [DllImport("kernel32.dll", SetLastError = true)] public static extern IntPtr OpenProcess(uint access, bool inherit, int pid);
        [DllImport("advapi32.dll", SetLastError = true)] public static extern bool OpenProcessToken(IntPtr proc, uint access, out IntPtr tok);
        [DllImport("advapi32.dll", SetLastError = true)] public static extern bool GetTokenInformation(IntPtr tok, int cls, IntPtr buf, int len, out int ret);
        [DllImport("advapi32.dll", SetLastError = true, CharSet = CharSet.Unicode)] public static extern bool ConvertSidToStringSid(IntPtr sid, out IntPtr str);
        [DllImport("kernel32.dll", SetLastError = true)] public static extern bool CloseHandle(IntPtr h);
        [DllImport("kernel32.dll")] public static extern IntPtr LocalFree(IntPtr h);

        public static int GetForegroundProcessId() {
            IntPtr h = GetForegroundWindow();
            if (h == IntPtr.Zero) { return 0; }
            uint p = 0;
            GetWindowThreadProcessId(h, out p);
            return (int)p;
        }

        public static string GetIntegritySid(int processId) {
            IntPtr proc = OpenProcess(0x1000, false, processId);
            if (proc == IntPtr.Zero) { throw new InvalidOperationException("OpenProcess failed for pid " + processId + ", error " + Marshal.GetLastWin32Error()); }
            IntPtr tok = IntPtr.Zero;
            try {
                if (!OpenProcessToken(proc, 0x0008, out tok)) { throw new InvalidOperationException("OpenProcessToken failed, error " + Marshal.GetLastWin32Error()); }
                int len = 0;
                GetTokenInformation(tok, 25, IntPtr.Zero, 0, out len);
                if (len <= 0) { throw new InvalidOperationException("GetTokenInformation sizing failed, error " + Marshal.GetLastWin32Error()); }
                IntPtr buf = Marshal.AllocHGlobal(len);
                try {
                    if (!GetTokenInformation(tok, 25, buf, len, out len)) { throw new InvalidOperationException("GetTokenInformation failed, error " + Marshal.GetLastWin32Error()); }
                    IntPtr str = IntPtr.Zero;
                    if (!ConvertSidToStringSid(Marshal.ReadIntPtr(buf), out str)) { throw new InvalidOperationException("ConvertSidToStringSid failed, error " + Marshal.GetLastWin32Error()); }
                    string s = Marshal.PtrToStringUni(str);
                    LocalFree(str);
                    return s;
                } finally { Marshal.FreeHGlobal(buf); }
            } finally {
                if (tok != IntPtr.Zero) { CloseHandle(tok); }
                CloseHandle(proc);
            }
        }
    }
}
'@

$HelperSource = @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

namespace AgentDesktopProbe {
    [StructLayout(LayoutKind.Sequential)]
    public struct KeybdInput { public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }

    [StructLayout(LayoutKind.Sequential)]
    public struct Input { public uint type; public KeybdInput ki; public int padA; public int padB; }

    public static class U8 {
        private delegate bool EnumProc(IntPtr h, IntPtr l);

        [DllImport("user32.dll", SetLastError = true)] private static extern uint SendInput(uint n, Input[] inputs, int size);
        [DllImport("user32.dll")] private static extern bool EnumChildWindows(IntPtr parent, EnumProc cb, IntPtr l);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern int GetClassNameW(IntPtr h, StringBuilder b, int max);
        [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode, EntryPoint = "SendMessageTimeoutW")]
        private static extern IntPtr SendMessageTimeoutText(IntPtr h, uint msg, IntPtr w, StringBuilder l, uint flags, uint ms, out IntPtr res);
        [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode, EntryPoint = "SendMessageTimeoutW")]
        private static extern IntPtr SendMessageTimeoutStr(IntPtr h, uint msg, IntPtr w, string l, uint flags, uint ms, out IntPtr res);
        [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)] private static extern bool PostMessageW(IntPtr h, uint msg, IntPtr w, IntPtr l);
        [DllImport("kernel32.dll")] public static extern IntPtr GetConsoleWindow();
        [DllImport("kernel32.dll")] private static extern void SetLastError(uint code);

        public static int InputStructSize() { return Marshal.SizeOf(typeof(Input)); }

        public static IntPtr FindChildByClass(IntPtr parent, string wanted) {
            IntPtr hit = IntPtr.Zero;
            EnumChildWindows(parent, delegate(IntPtr h, IntPtr l) {
                StringBuilder b = new StringBuilder(256);
                GetClassNameW(h, b, 256);
                if (String.Equals(b.ToString(), wanted, StringComparison.OrdinalIgnoreCase)) { hit = h; return false; }
                return true;
            }, IntPtr.Zero);
            return hit;
        }

        public static string ReadText(IntPtr h, out long sendResult, out int lastError) {
            StringBuilder b = new StringBuilder(8192);
            IntPtr res;
            IntPtr ok = SendMessageTimeoutText(h, 0x000D, (IntPtr)8192, b, 0x0002, 4000, out res);
            lastError = Marshal.GetLastWin32Error();
            sendResult = res.ToInt64();
            if (ok == IntPtr.Zero) { return null; }
            return b.ToString();
        }

        public static bool WriteText(IntPtr h, string text, out int lastError) {
            IntPtr res;
            IntPtr ok = SendMessageTimeoutStr(h, 0x000C, IntPtr.Zero, text, 0x0002, 4000, out res);
            lastError = Marshal.GetLastWin32Error();
            return ok != IntPtr.Zero;
        }

        public static bool PostChar(IntPtr h, char c, out int lastError) {
            bool ok = PostMessageW(h, 0x0102, (IntPtr)c, IntPtr.Zero);
            lastError = Marshal.GetLastWin32Error();
            return ok;
        }

        public static uint SendUnicode(string text, out int lastError) {
            List<Input> batch = new List<Input>();
            for (int i = 0; i < text.Length; i++) {
                Input down = new Input();
                down.type = 1;
                down.ki.wScan = (ushort)text[i];
                down.ki.dwFlags = 0x0004;
                batch.Add(down);
                Input up = new Input();
                up.type = 1;
                up.ki.wScan = (ushort)text[i];
                up.ki.dwFlags = 0x0004 | 0x0002;
                batch.Add(up);
            }
            Input[] arr = batch.ToArray();
            SetLastError(0);
            uint sent = SendInput((uint)arr.Length, arr, Marshal.SizeOf(typeof(Input)));
            lastError = Marshal.GetLastWin32Error();
            return sent;
        }
    }
}
'@

function Get-UiaSnapshot {
    param([IntPtr]$WindowHandle)
    $row = [ordered]@{}
    try {
        Add-Type -AssemblyName UIAutomationClient -ErrorAction Stop
        Add-Type -AssemblyName UIAutomationTypes -ErrorAction Stop
        $root = [System.Windows.Automation.AutomationElement]::FromHandle($WindowHandle)
        $row['fromHandle'] = 'ok'
        $row['name'] = $root.Current.Name
        $row['className'] = $root.Current.ClassName
        $row['controlType'] = ($root.Current.ControlType.ProgrammaticName -replace '^ControlType\.', '')
        $row['ownerProcessId'] = $root.Current.ProcessId
        $r = $root.Current.BoundingRectangle
        if ($r.IsEmpty -or [double]::IsInfinity($r.Left)) {
            $row['boundingRectangle'] = 'empty (window minimized or off-screen)'
        } else {
            $row['boundingRectangle'] = [ordered]@{ Left = [int]$r.Left; Top = [int]$r.Top; Width = [int]$r.Width; Height = [int]$r.Height }
        }
        $walker = [System.Windows.Automation.TreeWalker]::RawViewWalker
        $stack = New-Object System.Collections.Stack
        $stack.Push($root)
        $count = 0
        $editValue = $null
        $editPatterns = @()
        while ($stack.Count -gt 0 -and $count -lt 200) {
            $node = $stack.Pop()
            $count++
            if ($node.Current.ClassName -eq 'Edit' -and $null -eq $editValue) {
                $editPatterns = @($node.GetSupportedPatterns() | ForEach-Object { $_.ProgrammaticName -replace '^(\w+)PatternIdentifiers\.Pattern$', '$1' })
                try {
                    $vp = $node.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
                    $editValue = $vp.Current.Value
                } catch {
                    try {
                        $tp = $node.GetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern)
                        $editValue = $tp.DocumentRange.GetText(-1)
                    } catch { $editValue = '<no value/text pattern: ' + $_.Exception.Message + '>' }
                }
            }
            $child = $walker.GetFirstChild($node)
            while ($null -ne $child) {
                $stack.Push($child)
                $child = $walker.GetNextSibling($child)
            }
        }
        $row['rawViewNodes'] = $count
        $row['editPatterns'] = $editPatterns
        $row['editValueSeenByThisProcess'] = $editValue
    } catch {
        $row['fromHandle'] = 'failed'
        $row['error'] = $_.Exception.Message
        $row['hresult'] = ('0x' + ('{0:x8}' -f $_.Exception.HResult))
    }
    return $row
}

function Invoke-WorkerArm {
    $result = [ordered]@{ arm = $Arm; processId = $PID }
    Add-Type -Path (Join-Path $HelperDir 'u8native.dll')
    Add-Type -Path (Join-Path $HelperDir 'u8helper.dll')
    $console = [AgentDesktopProbe.U8]::GetConsoleWindow()
    [void][AgentDesktopProbe.Native]::ShowWindow($console, 6)
    [void][AgentDesktopProbe.Native]::ShowWindow($console, 0)
    $result['addTypeCompilationAvailable'] = $null
    try {
        Add-Type -TypeDefinition 'public static class U8Compile { public static int V() { return 1; } }' -Language CSharp -ErrorAction Stop
        $result['addTypeCompilationAvailable'] = $true
    } catch {
        $result['addTypeCompilationAvailable'] = $false
        $result['addTypeCompilationError'] = $_.Exception.Message
    }
    $target = [IntPtr][int64]$TargetHwnd
    $edit = [IntPtr][int64]$EditHwnd
    $result['selfIntegritySid'] = [AgentDesktopProbe.Native]::GetIntegritySid($PID)
    try { $result['targetIntegritySid'] = [AgentDesktopProbe.Native]::GetIntegritySid($TargetProcessId) }
    catch { $result['targetIntegritySid'] = 'ERROR ' + $_.Exception.Message }
    $result['uiaRead'] = Get-UiaSnapshot -WindowHandle $target

    $err = 0
    $raw = [int64]0
    $textBefore = [AgentDesktopProbe.U8]::ReadText($edit, [ref]$raw, [ref]$err)
    $result['wmGetTextFromWorker'] = [ordered]@{ returned = $textBefore; sendMessageResult = $raw; lastError = $err }

    [IO.File]::WriteAllText(($WorkerOut + '.ready'), 'ready')
    $deadline = (Get-Date).AddSeconds(120)
    while ((Get-Date) -lt $deadline -and -not (Test-Path -LiteralPath ($WorkerOut + '.go'))) { Start-Sleep -Milliseconds 200 }
    $deadline = (Get-Date).AddSeconds(20)
    while ((Get-Date) -lt $deadline -and [AgentDesktopProbe.Native]::GetForegroundProcessId() -ne $TargetProcessId) {
        Start-Sleep -Milliseconds 200
    }
    $result['foregroundOwnerAtAssert'] = [ordered]@{ processId = [AgentDesktopProbe.Native]::GetForegroundProcessId() }
    try {
        Assert-Foreground -ExpectedProcessId $TargetProcessId -Stage ('sendinput-pre-' + $Arm)
        $result['foregroundAssertPre'] = 'ok'
        $sent = [AgentDesktopProbe.U8]::SendUnicode($Marker, [ref]$err)
        $result['sendInputEventsAccepted'] = [int]$sent
        $result['sendInputLastError'] = $err
        $result['sendInputStructSize'] = [AgentDesktopProbe.U8]::InputStructSize()
        Start-Sleep -Milliseconds 700
        Assert-Foreground -ExpectedProcessId $TargetProcessId -Stage ('sendinput-post-' + $Arm)
        $result['foregroundAssertPost'] = 'ok'
    } catch {
        $result['foregroundAssertPre'] = 'interference'
        $result['interference'] = $_.Exception.Message
    }

    $postOk = [AgentDesktopProbe.U8]::PostChar($edit, 'P', [ref]$err)
    $result['postMessageWmChar'] = [ordered]@{ returned = $postOk; lastError = $err }
    Start-Sleep -Milliseconds 400
    $textAfter = [AgentDesktopProbe.U8]::ReadText($edit, [ref]$raw, [ref]$err)
    $result['wmGetTextAfterFromWorker'] = [ordered]@{ returned = $textAfter; sendMessageResult = $raw; lastError = $err }
    $result['uiaReadAfter'] = Get-UiaSnapshot -WindowHandle $target
    [IO.File]::WriteAllText($WorkerOut, (ConvertTo-Json -InputObject $result -Depth 12), (New-Object System.Text.UTF8Encoding $false))
}

if ($Worker) {
    try { Invoke-WorkerArm } catch {
        [IO.File]::WriteAllText($WorkerOut, (ConvertTo-Json -InputObject ([ordered]@{ arm = $Arm; fatal = $_.Exception.Message }) -Depth 6), (New-Object System.Text.UTF8Encoding $false))
    }
    exit 0
}

$status = 'ok'
$message = ''
$summary = [ordered]@{}
$script:Spawned = New-Object System.Collections.ArrayList

try {
    Initialize-ProbeNative
    $work = Join-Path $env:TEMP 'agent-desktop-u8'
    if (-not (Test-Path -LiteralPath $work)) { New-Item -ItemType Directory -Path $work -Force | Out-Null }
    Add-Type -TypeDefinition $NativeShimSource -Language CSharp -OutputAssembly (Join-Path $work 'u8native.dll') -OutputType Library
    Add-Type -TypeDefinition $HelperSource -Language CSharp -OutputAssembly (Join-Path $work 'u8helper.dll') -OutputType Library
    Add-Type -Path (Join-Path $work 'u8helper.dll')
    Write-ProbeLog -Message 'compiled u8native.dll + u8helper.dll at High integrity for the Medium worker to load'

    $policy = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System'
    $uac = [ordered]@{}
    foreach ($n in @('EnableLUA', 'FilterAdministratorToken', 'ConsentPromptBehaviorAdmin', 'EnableInstallerDetection')) {
        $v = (Get-ItemProperty -Path $policy -Name $n -ErrorAction SilentlyContinue).$n
        if ($null -eq $v) { $uac[$n] = '<value not present>' } else { $uac[$n] = [int]$v }
    }
    $uac['sessionIntegritySid'] = [AgentDesktopProbe.Native]::GetIntegritySid($PID)
    $uac['adminApprovalModeActive'] = ($uac['EnableLUA'] -eq 1 -and $uac['FilterAdministratorToken'] -eq 1)

    $notepad = Start-ScratchProcess -FilePath (Join-Path $env:WINDIR 'System32\notepad.exe') -TimeoutSec 20
    [void]$script:Spawned.Add($notepad.ProcessId)
    if ($notepad.MainWindowHandle -eq [IntPtr]::Zero) { throw 'target notepad window never appeared' }
    $targetSid = [AgentDesktopProbe.Native]::GetIntegritySid($notepad.ProcessId)
    if ($targetSid -ne 'S-1-16-12288') { throw ('target notepad is not High integrity: ' + $targetSid) }
    $editHandle = [AgentDesktopProbe.U8]::FindChildByClass($notepad.MainWindowHandle, 'Edit')
    if ($editHandle -eq [IntPtr]::Zero) { throw 'notepad Edit child window not found' }

    $arms = @()
    $lastErr = 0
    $rawRes = [int64]0
    foreach ($armSpec in @(
            [pscustomobject]@{ Name = 'high'; Marker = 'HHH'; Medium = $false },
            [pscustomobject]@{ Name = 'medium'; Marker = 'MMM'; Medium = $true })) {
        [void][AgentDesktopProbe.U8]::WriteText($editHandle, 'U8-BASE', [ref]$lastErr)
        Start-Sleep -Milliseconds 300
        $before = [AgentDesktopProbe.U8]::ReadText($editHandle, [ref]$rawRes, [ref]$lastErr)
        $outFile = Join-Path $work ('arm-' + $armSpec.Name + '.json')
        foreach ($stale in @($outFile, ($outFile + '.ready'), ($outFile + '.go'))) {
            if (Test-Path -LiteralPath $stale) { Remove-Item -LiteralPath $stale -Force }
        }
        $argv = @('-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass',
            '-File', $PSCommandPath, '-Worker', '-WorkerOut', $outFile, '-HelperDir', $work,
            '-TargetProcessId', ([string]$notepad.ProcessId),
            '-TargetHwnd', ([string]$notepad.MainWindowHandle.ToInt64()),
            '-EditHwnd', ([string]$editHandle.ToInt64()),
            '-Marker', $armSpec.Marker, '-Arm', $armSpec.Name)
        $shell = Join-Path $env:WINDIR 'System32\WindowsPowerShell\v1.0\powershell.exe'
        $launchError = ''
        $workerPid = 0
        $workerSid = ''
        try {
            if ($armSpec.Medium) {
                $w = Start-MediumIntegrityProcess -FilePath $shell -ArgumentList $argv
                $workerPid = $w.ProcessId
                $workerSid = $w.IntegritySid
            } else {
                $w = Start-Process -FilePath $shell -ArgumentList $argv -PassThru
                $workerPid = $w.Id
                Register-ScratchProcessId -ProcessId $workerPid
                $workerSid = [AgentDesktopProbe.Native]::GetIntegritySid($workerPid)
            }
            [void]$script:Spawned.Add($workerPid)
        } catch {
            $launchError = $_.Exception.Message
        }
        if ($armSpec.Medium -and $workerSid -ne 'S-1-16-8192') {
            throw ('PROBE-HARNESS: refusing to record a UIPI verdict without a real boundary - medium arm token label is ' + $workerSid + ' ' + $launchError)
        }
        $handoff = 'worker never signalled ready'
        $deadline = (Get-Date).AddSeconds(120)
        while ((Get-Date) -lt $deadline -and -not (Test-Path -LiteralPath ($outFile + '.ready'))) { Start-Sleep -Milliseconds 300 }
        if (Test-Path -LiteralPath ($outFile + '.ready')) {
            [void][AgentDesktopProbe.Native]::ShowWindow($notepad.MainWindowHandle, 6)
            Start-Sleep -Milliseconds 500
            [void][AgentDesktopProbe.Native]::ShowWindow($notepad.MainWindowHandle, 9)
            $fgDeadline = (Get-Date).AddSeconds(15)
            while ((Get-Date) -lt $fgDeadline -and [AgentDesktopProbe.Native]::GetForegroundProcessId() -ne $notepad.ProcessId) { Start-Sleep -Milliseconds 250 }
            $observedFg = [AgentDesktopProbe.Native]::GetForegroundProcessId()
            $handoff = if ($observedFg -eq $notepad.ProcessId) { 'target foreground restored by parent ShowWindow(SW_MINIMIZE->SW_RESTORE) on the probe-launched window; SetForegroundWindow never called' }
            else { 'FAILED - foreground pid ' + $observedFg + ' is not the probe-launched target' }
            [IO.File]::WriteAllText(($outFile + '.go'), 'go')
        }
        $deadline = (Get-Date).AddSeconds(180)
        while ((Get-Date) -lt $deadline -and -not (Test-Path -LiteralPath $outFile)) { Start-Sleep -Milliseconds 500 }
        $workerData = $null
        if (Test-Path -LiteralPath $outFile) { $workerData = (Get-Content -LiteralPath $outFile -Raw | ConvertFrom-Json) }
        Start-Sleep -Milliseconds 500
        $after = [AgentDesktopProbe.U8]::ReadText($editHandle, [ref]$rawRes, [ref]$lastErr)
        $arms += [ordered]@{
            arm                            = $armSpec.Name
            workerIntegritySid             = $workerSid
            workerLaunchError              = $launchError
            marker                         = $armSpec.Marker
            parentForegroundHandoff        = $handoff
            targetStateBeforeInjection     = $before
            targetStateAfterInjection      = $after
            targetStateChanged             = ($before -ne $after)
            markerPresentInTargetAfterwards = ($null -ne $after -and $after.Contains($armSpec.Marker))
            observedBy                     = 'parent High-integrity WM_GETTEXT on the notepad Edit child, independent of SendInput return'
            worker                         = $workerData
        }
        try { Stop-ScratchProcess -ProcessId $workerPid } catch { Write-ProbeLog -Message ('worker teardown: ' + $_.Exception.Message) -Level 'warn' }
    }

    $high = $arms | Where-Object { $_.arm -eq 'high' } | Select-Object -First 1
    $medium = $arms | Where-Object { $_.arm -eq 'medium' } | Select-Object -First 1
    $capture = [ordered]@{
        question         = 'does UIPI block SendInput and window messages from a Medium-integrity process into a High-integrity target, and are UIA reads still allowed'
        invariant        = 'Engineering Invariant #6 (UIPI)'
        stack            = 'managed (UIA reads) + raw Win32 SendInput/SendMessage/PostMessage'
        scope            = 'api-contract'
        boundaryOrigin   = 'manufactured by common.ps1 Start-MediumIntegrityProcess (DuplicateTokenEx + SetTokenInformation(TokenIntegrityLevel, S-1-16-8192) + CreateProcessAsUser), label asserted on read-back'
        boundaryRuledOut = @(
            'Start-Process -Verb RunAs: AAM off on this box (FilterAdministratorToken unset, RID 500 full token) so it yields High-vs-High',
            'runas.exe /trustlevel:0x20000: restricts the token but leaves the mandatory label at S-1-16-12288'
        )
        uacPolicy        = $uac
        target           = [ordered]@{
            process             = 'notepad.exe'
            processId           = $notepad.ProcessId
            integritySid        = $targetSid
            windowHandle        = $notepad.MainWindowHandle.ToInt64()
            editChild           = [ordered]@{ className = 'Edit'; windowHandle = $editHandle.ToInt64() }
            launchedByThisProbe = $true
        }
        arms             = $arms
        findings         = [ordered]@{
            uiaReadFromMediumAgainstHigh = if ($medium -and $medium.worker) { $medium.worker.uiaRead.fromHandle } else { 'unavailable' }
            sendInputFromHighLanded      = if ($high) { $high.targetStateChanged } else { $null }
            sendInputFromMediumLanded    = if ($medium) { $medium.targetStateChanged } else { $null }
            sendInputReturnIsNotEvidence = 'SendInput reports the event count it accepted in both arms; only the re-read of the target distinguishes them'
            wmGetTextFromMediumBlocked   = if ($medium -and $medium.worker) { ($null -eq $medium.worker.wmGetTextFromWorker.returned -or $medium.worker.wmGetTextFromWorker.returned -eq '') } else { $null }
        }
    }
    Write-ProbeJson -Probe $Probe -Name 'uipi.json' -InputObject $capture | Out-Null

    $summary['sessionIntegritySid'] = $uac['sessionIntegritySid']
    $summary['targetIntegritySid'] = $targetSid
    $summary['mediumArmIntegritySid'] = if ($medium) { $medium.workerIntegritySid } else { '' }
    $summary['highArmInjectionLanded'] = $capture.findings.sendInputFromHighLanded
    $summary['mediumArmInjectionLanded'] = $capture.findings.sendInputFromMediumLanded
    $summary['mediumArmUiaRead'] = $capture.findings.uiaReadFromMediumAgainstHigh
    $message = 'uipi measured across a manufactured Medium-vs-High boundary'
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

Write-ProbeResult -Probe $Probe -Status $status -Message $message -Data $summary
if ($status -eq 'fail') { exit 1 }
exit 0
