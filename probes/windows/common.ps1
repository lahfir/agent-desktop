$script:ProbeRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$script:CapturesRoot = Join-Path $script:ProbeRoot 'captures'
$script:BoundsBucket = 8
$script:NotMeasuredKey = 'not_measured'
$script:MandatoryExpected = New-Object System.Collections.ArrayList
$script:MandatoryProduced = New-Object System.Collections.ArrayList
$script:MandatoryDegraded = New-Object System.Collections.ArrayList
$script:UsersPathPattern = '[A-Za-z]:[\\/]+Users[\\/]+(?!Public[\\/\s"])(?!Default[\\/\s"])(?!All Users[\\/\s"])[^\\/:*?"<>|\s]+'

function Get-ProbeRoot {
    return $script:ProbeRoot
}

function Get-CaptureDir {
    param([Parameter(Mandatory = $true)][string]$Probe)
    $dir = Join-Path $script:CapturesRoot $Probe
    if (-not (Test-Path -LiteralPath $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }
    return (Resolve-Path -LiteralPath $dir).ProviderPath
}

function Get-ProbePidLedgerPath {
    if ($env:AGENT_DESKTOP_PROBE_PIDS) { return $env:AGENT_DESKTOP_PROBE_PIDS }
    return (Join-Path $env:TEMP ('agent-desktop-probe-pids-' + $PID + '.txt'))
}

function New-ProbeReplacement {
    param([string]$Pattern, [string]$Replacement)
    return [pscustomobject]@{ Pattern = $Pattern; Replacement = $Replacement }
}

function Initialize-ProbeRedaction {
    $rules = New-Object System.Collections.ArrayList
    $paths = @(
        @{ Value = $env:APPDATA;       Token = '<appdata>' },
        @{ Value = $env:LOCALAPPDATA;  Token = '<localappdata>' },
        @{ Value = $env:USERPROFILE;   Token = '<userprofile>' }
    )
    foreach ($p in $paths) {
        if (-not $p.Value) { continue }
        foreach ($form in @($p.Value, $p.Value.Replace('\', '/'), $p.Value.Replace('\', '\\'))) {
            [void]$rules.Add((New-ProbeReplacement ([regex]::Escape($form)) $p.Token))
        }
    }
    [void]$rules.Add((New-ProbeReplacement $script:UsersPathPattern '<userprofile>'))
    [void]$rules.Add((New-ProbeReplacement 'S-1-5-21-\d+-\d+-\d+-(\d+)' 'S-1-5-21-<redacted>-$1'))
    [void]$rules.Add((New-ProbeReplacement 'S-1-5-21-\d+-\d+-\d+' 'S-1-5-21-<redacted>'))
    $names = New-Object System.Collections.ArrayList
    if ($env:USERNAME) { [void]$names.Add(@{ Value = $env:USERNAME; Token = '<user>' }) }
    if ($env:COMPUTERNAME) { [void]$names.Add(@{ Value = $env:COMPUTERNAME; Token = '<machine>' }) }
    $dns = ''
    try { $dns = [System.Net.Dns]::GetHostName() } catch { $dns = '' }
    if ($dns) { [void]$names.Add(@{ Value = $dns; Token = '<machine>' }) }
    try {
        $fqdn = [System.Net.Dns]::GetHostEntry([string]$dns).HostName
        if ($fqdn) { [void]$names.Add(@{ Value = $fqdn; Token = '<machine>' }) }
    } catch { }
    $sorted = $names | Sort-Object -Property @{ Expression = { $_.Value.Length } } -Descending
    foreach ($n in $sorted) {
        [void]$rules.Add((New-ProbeReplacement ('(?<![A-Za-z0-9_-])' + [regex]::Escape($n.Value) + '(?![A-Za-z0-9_-])') $n.Token))
    }
    $script:RedactionRules = $rules
}

function Protect-ProbeText {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Text)
    if ([string]::IsNullOrEmpty($Text)) { return $Text }
    $out = $Text
    foreach ($rule in $script:RedactionRules) {
        $out = [regex]::Replace($out, $rule.Pattern, $rule.Replacement, [System.Text.RegularExpressions.RegexOptions]::IgnoreCase)
    }
    return $out
}

function Protect-ProbeName {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][AllowNull()][string]$Name)
    if ([string]::IsNullOrEmpty($Name)) { return $Name }
    return ('<redacted:' + $Name.Length + ' chars>')
}

function Get-NormalizedCapture {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Text)
    if ([string]::IsNullOrEmpty($Text)) { return $Text }
    $opts = [System.Text.RegularExpressions.RegexOptions]::IgnoreCase
    $out = $Text
    $out = [regex]::Replace($out, '"RuntimeId"\s*:\s*\[[^\]]*\]', '"RuntimeId": "<runtimeid>"', $opts)
    $out = [regex]::Replace($out, '(RuntimeId\s*[:=]\s*)[\d,\s\.\-]+', '$1<runtimeid>', $opts)
    $out = [regex]::Replace($out, '[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}', '<guid>', $opts)
    $out = [regex]::Replace($out, '\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:?\d{2})?', '<timestamp>', $opts)
    $out = [regex]::Replace($out, '\d{1,2}/\d{1,2}/\d{4}( \d{1,2}:\d{2}(:\d{2})?( ?[ap]\.? ?m\.?)?)?', '<timestamp>', $opts)
    $out = [regex]::Replace($out, '(?<=[\\/])Temp([\\/]+)[^\s",;)\]}]+', 'Temp$1<tempfile>', $opts)
    $out = [regex]::Replace($out, '("?\b(?:threadid|tid|nativethreadid)\b"?\s*[:=]\s*)("?)-?\d+\2', '$1$2<tid>$2', $opts)
    $out = [regex]::Replace($out, '("?\b(?:pid|processid|parentprocessid|ownerprocessid|sessionprocessid)\b"?\s*[:=]\s*)("?)-?\d+\2', '$1$2<pid>$2', $opts)
    $out = [regex]::Replace($out, '("?\b(?:hwnd|windowhandle|mainwindowhandle|nativewindowhandle|handle|windowhandles)\b"?\s*[:=]\s*)("?)(?:0x[0-9a-fA-F]+|-?\d+)\2', '$1$2<hwnd>$2', $opts)
    $out = [regex]::Replace($out, '("?\b(?:elapsed\w*|duration\w*|latency\w*|ticks|totalticks|ms|milliseconds|microseconds|seconds|timems|timing\w*|perpropertyms|cachedms)\b"?\s*[:=]\s*)("?)-?[\d]+(?:\.\d+)?\2', '$1$2<duration>$2', $opts)
    $out = [regex]::Replace($out, '("?\b(?:x|y|left|top|right|bottom|width|height)\b"?\s*[:=]\s*)(-?\d+(?:\.\d+)?)', { param($m) $m.Groups[1].Value + (Get-BucketedNumber $m.Groups[2].Value) }, $opts)
    $out = [regex]::Replace($out, '("?Bounds"?\s*:\s*\[)([^\]]*)(\])', { param($m) $m.Groups[1].Value + (Get-BucketedNumberList $m.Groups[2].Value) + $m.Groups[3].Value }, $opts)
    return $out
}

function Get-BucketedNumber {
    param([string]$Value)
    $n = 0.0
    if (-not [double]::TryParse($Value, [ref]$n)) { return $Value }
    return ([string]([int]([Math]::Round($n / $script:BoundsBucket) * $script:BoundsBucket)))
}

function Get-BucketedNumberList {
    param([string]$Value)
    $parts = $Value -split ','
    $done = @()
    foreach ($p in $parts) { $done += (Get-BucketedNumber $p.Trim()) }
    return ($done -join ',')
}

function Write-ProbeCapture {
    param(
        [Parameter(Mandatory = $true)][string]$Probe,
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Content
    )
    $dir = Get-CaptureDir -Probe $Probe
    $redacted = Protect-ProbeText -Text $Content
    $path = Join-Path $dir $Name
    $utf8NoBom = New-Object System.Text.UTF8Encoding $false
    [IO.File]::WriteAllText($path, $redacted, $utf8NoBom)
    $normalized = Get-NormalizedCapture -Text $redacted
    [IO.File]::WriteAllText(($path + '.normalized'), $normalized, $utf8NoBom)
    return $path
}

function Write-ProbeJson {
    param(
        [Parameter(Mandatory = $true)][string]$Probe,
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][AllowNull()]$InputObject
    )
    $json = ConvertTo-Json -InputObject $InputObject -Depth 25
    return (Write-ProbeCapture -Probe $Probe -Name $Name -Content $json)
}

function Test-CaptureRedaction {
    param([Parameter(Mandatory = $true)][string]$Path)
    $text = [IO.File]::ReadAllText($Path)
    $residue = New-Object System.Collections.ArrayList
    $checks = @(
        @{ Pattern = 'S-1-5-21-\d+-\d+-\d+'; Reason = 'unredacted domain SID sub-authorities' }
    )
    if ($env:USERNAME) { $checks += @{ Pattern = '(?<![A-Za-z0-9_-])' + [regex]::Escape($env:USERNAME) + '(?![A-Za-z0-9_-])'; Reason = 'operator user name' } }
    if ($env:COMPUTERNAME) { $checks += @{ Pattern = '(?<![A-Za-z0-9_-])' + [regex]::Escape($env:COMPUTERNAME) + '(?![A-Za-z0-9_-])'; Reason = 'machine name' } }
    foreach ($v in @($env:USERPROFILE, $env:LOCALAPPDATA, $env:APPDATA)) {
        if (-not $v) { continue }
        $checks += @{ Pattern = [regex]::Escape($v); Reason = 'user-profile path' }
        $checks += @{ Pattern = [regex]::Escape($v.Replace('\', '/')); Reason = 'user-profile path (forward slash)' }
        $checks += @{ Pattern = [regex]::Escape($v.Replace('\', '\\')); Reason = 'user-profile path (json-escaped)' }
    }
    $checks += @{ Pattern = $script:UsersPathPattern; Reason = 'C:\Users\<name> path' }
    foreach ($c in $checks) {
        $m = [regex]::Match($text, $c.Pattern, [System.Text.RegularExpressions.RegexOptions]::IgnoreCase)
        if ($m.Success) {
            [void]$residue.Add(($c.Reason + " (match: '" + $m.Value + "')"))
        }
    }
    if ($residue.Count -gt 0) {
        foreach ($r in $residue) {
            Write-ProbeLog -Message ('redaction residue in ' + $Path + ': ' + $r) -Level 'error'
        }
        return $false
    }
    return $true
}

function Initialize-ProbeNative {
    if ('AgentDesktopProbe.Native' -as [type]) { return }
    $src = @'
using System;
using System.Runtime.InteropServices;
using System.Text;

namespace AgentDesktopProbe {
    [StructLayout(LayoutKind.Sequential)]
    public struct SidAndAttributes { public IntPtr Sid; public uint Attributes; }

    [StructLayout(LayoutKind.Sequential)]
    public struct TokenMandatoryLabel { public SidAndAttributes Label; }

    [StructLayout(LayoutKind.Sequential)]
    public struct Luid { public uint LowPart; public int HighPart; }

    [StructLayout(LayoutKind.Sequential)]
    public struct LuidAndAttributes { public Luid Luid; public uint Attributes; }

    [StructLayout(LayoutKind.Sequential)]
    public struct TokenPrivileges { public uint PrivilegeCount; public LuidAndAttributes Privilege; }

    [StructLayout(LayoutKind.Sequential)]
    public struct ProcessInformation { public IntPtr hProcess; public IntPtr hThread; public int dwProcessId; public int dwThreadId; }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct StartupInfo {
        public int cb; public string lpReserved; public string lpDesktop; public string lpTitle;
        public int dwX; public int dwY; public int dwXSize; public int dwYSize;
        public int dwXCountChars; public int dwYCountChars; public int dwFillAttribute;
        public int dwFlags; public short wShowWindow; public short cbReserved2;
        public IntPtr lpReserved2; public IntPtr hStdInput; public IntPtr hStdOutput; public IntPtr hStdError;
    }

    public static class Native {
        [DllImport("user32.dll")]
        public static extern IntPtr GetForegroundWindow();
        [DllImport("user32.dll", SetLastError = true)]
        public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint lpdwProcessId);
        [DllImport("user32.dll")]
        public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
        [DllImport("user32.dll", SetLastError = true)]
        public static extern bool SetWindowPos(IntPtr hWnd, IntPtr hWndInsertAfter, int X, int Y, int cx, int cy, uint uFlags);
        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern IntPtr OpenProcess(uint dwDesiredAccess, bool bInheritHandle, int dwProcessId);
        [DllImport("advapi32.dll", SetLastError = true)]
        public static extern bool OpenProcessToken(IntPtr ProcessHandle, uint DesiredAccess, out IntPtr TokenHandle);
        [DllImport("advapi32.dll", SetLastError = true)]
        public static extern bool GetTokenInformation(IntPtr TokenHandle, int TokenInformationClass, IntPtr TokenInformation, int TokenInformationLength, out int ReturnLength);
        [DllImport("advapi32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        public static extern bool ConvertSidToStringSid(IntPtr Sid, out IntPtr StringSid);
        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern bool CloseHandle(IntPtr hObject);
        [DllImport("kernel32.dll")]
        public static extern IntPtr LocalFree(IntPtr hMem);
        [DllImport("kernel32.dll")]
        public static extern IntPtr GetCurrentProcess();
        [DllImport("advapi32.dll", SetLastError = true)]
        public static extern bool DuplicateTokenEx(IntPtr hExistingToken, uint dwDesiredAccess, IntPtr lpTokenAttributes, int ImpersonationLevel, int TokenType, out IntPtr phNewToken);
        [DllImport("advapi32.dll", SetLastError = true)]
        public static extern bool SetTokenInformation(IntPtr TokenHandle, int TokenInformationClass, IntPtr TokenInformation, int TokenInformationLength);
        [DllImport("advapi32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        public static extern bool ConvertStringSidToSid(string StringSid, out IntPtr Sid);
        [DllImport("advapi32.dll", SetLastError = true)]
        public static extern int GetLengthSid(IntPtr pSid);
        [DllImport("advapi32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        public static extern bool LookupPrivilegeValue(string lpSystemName, string lpName, out Luid lpLuid);
        [DllImport("advapi32.dll", SetLastError = true)]
        public static extern bool AdjustTokenPrivileges(IntPtr TokenHandle, bool DisableAllPrivileges, ref TokenPrivileges NewState, int BufferLength, IntPtr PreviousState, IntPtr ReturnLength);
        [DllImport("advapi32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        public static extern bool CreateProcessAsUser(IntPtr hToken, string lpApplicationName, StringBuilder lpCommandLine, IntPtr lpProcessAttributes, IntPtr lpThreadAttributes, bool bInheritHandles, uint dwCreationFlags, IntPtr lpEnvironment, string lpCurrentDirectory, ref StartupInfo lpStartupInfo, out ProcessInformation lpProcessInformation);

        private static void EnablePrivilege(string name) {
            IntPtr hTok = IntPtr.Zero;
            if (!OpenProcessToken(GetCurrentProcess(), 0x0020 | 0x0008, out hTok)) { return; }
            try {
                Luid luid;
                if (!LookupPrivilegeValue(null, name, out luid)) { return; }
                TokenPrivileges tp = new TokenPrivileges();
                tp.PrivilegeCount = 1;
                tp.Privilege.Luid = luid;
                tp.Privilege.Attributes = 0x00000002;
                AdjustTokenPrivileges(hTok, false, ref tp, 0, IntPtr.Zero, IntPtr.Zero);
            } finally {
                CloseHandle(hTok);
            }
        }

        public static int StartMediumIntegrityProcess(string commandLine, string workingDirectory) {
            EnablePrivilege("SeIncreaseQuotaPrivilege");
            EnablePrivilege("SeAssignPrimaryTokenPrivilege");
            IntPtr hSelf = IntPtr.Zero;
            IntPtr hDup = IntPtr.Zero;
            IntPtr pSid = IntPtr.Zero;
            IntPtr pLabel = IntPtr.Zero;
            try {
                if (!OpenProcessToken(GetCurrentProcess(), 0x0002 | 0x0008 | 0x0001 | 0x0080, out hSelf)) {
                    throw new InvalidOperationException("OpenProcessToken failed, error " + Marshal.GetLastWin32Error());
                }
                if (!DuplicateTokenEx(hSelf, 0x02000000, IntPtr.Zero, 2, 1, out hDup)) {
                    throw new InvalidOperationException("DuplicateTokenEx failed, error " + Marshal.GetLastWin32Error());
                }
                if (!ConvertStringSidToSid("S-1-16-8192", out pSid)) {
                    throw new InvalidOperationException("ConvertStringSidToSid failed, error " + Marshal.GetLastWin32Error());
                }
                TokenMandatoryLabel tml = new TokenMandatoryLabel();
                tml.Label.Sid = pSid;
                tml.Label.Attributes = 0x00000020;
                int size = Marshal.SizeOf(typeof(TokenMandatoryLabel));
                pLabel = Marshal.AllocHGlobal(size);
                Marshal.StructureToPtr(tml, pLabel, false);
                if (!SetTokenInformation(hDup, 25, pLabel, size + GetLengthSid(pSid))) {
                    throw new InvalidOperationException("SetTokenInformation(TokenIntegrityLevel) failed, error " + Marshal.GetLastWin32Error());
                }
                StartupInfo si = new StartupInfo();
                si.cb = Marshal.SizeOf(typeof(StartupInfo));
                si.lpDesktop = "winsta0\\default";
                ProcessInformation pi;
                StringBuilder cmd = new StringBuilder(commandLine);
                if (!CreateProcessAsUser(hDup, null, cmd, IntPtr.Zero, IntPtr.Zero, false, 0x00000010, IntPtr.Zero, workingDirectory, ref si, out pi)) {
                    throw new InvalidOperationException("CreateProcessAsUser failed, error " + Marshal.GetLastWin32Error());
                }
                CloseHandle(pi.hThread);
                CloseHandle(pi.hProcess);
                return pi.dwProcessId;
            } finally {
                if (pLabel != IntPtr.Zero) { Marshal.FreeHGlobal(pLabel); }
                if (pSid != IntPtr.Zero) { LocalFree(pSid); }
                if (hDup != IntPtr.Zero) { CloseHandle(hDup); }
                if (hSelf != IntPtr.Zero) { CloseHandle(hSelf); }
            }
        }

        public static int GetForegroundProcessId() {
            IntPtr hWnd = GetForegroundWindow();
            if (hWnd == IntPtr.Zero) { return 0; }
            uint pid = 0;
            GetWindowThreadProcessId(hWnd, out pid);
            return (int)pid;
        }

        public static string GetIntegritySid(int processId) {
            IntPtr hProc = OpenProcess(0x1000, false, processId);
            if (hProc == IntPtr.Zero) {
                throw new InvalidOperationException("OpenProcess failed for pid " + processId + ", error " + Marshal.GetLastWin32Error());
            }
            IntPtr hTok = IntPtr.Zero;
            try {
                if (!OpenProcessToken(hProc, 0x0008, out hTok)) {
                    throw new InvalidOperationException("OpenProcessToken failed, error " + Marshal.GetLastWin32Error());
                }
                int len = 0;
                GetTokenInformation(hTok, 25, IntPtr.Zero, 0, out len);
                if (len <= 0) {
                    throw new InvalidOperationException("GetTokenInformation sizing failed, error " + Marshal.GetLastWin32Error());
                }
                IntPtr buf = Marshal.AllocHGlobal(len);
                try {
                    if (!GetTokenInformation(hTok, 25, buf, len, out len)) {
                        throw new InvalidOperationException("GetTokenInformation failed, error " + Marshal.GetLastWin32Error());
                    }
                    IntPtr pSid = Marshal.ReadIntPtr(buf);
                    IntPtr pStr = IntPtr.Zero;
                    if (!ConvertSidToStringSid(pSid, out pStr)) {
                        throw new InvalidOperationException("ConvertSidToStringSid failed, error " + Marshal.GetLastWin32Error());
                    }
                    string s = Marshal.PtrToStringUni(pStr);
                    LocalFree(pStr);
                    return s;
                } finally {
                    Marshal.FreeHGlobal(buf);
                }
            } finally {
                if (hTok != IntPtr.Zero) { CloseHandle(hTok); }
                CloseHandle(hProc);
            }
        }
    }
}
'@
    Add-Type -TypeDefinition $src -Language CSharp | Out-Null
}

function Register-ScratchProcessId {
    param([Parameter(Mandatory = $true)][int]$ProcessId)
    $ledger = Get-ProbePidLedgerPath
    Add-Content -LiteralPath $ledger -Value ([string]$ProcessId) -Encoding ASCII
}

function Get-ScratchProcessIds {
    $ledger = Get-ProbePidLedgerPath
    if (-not (Test-Path -LiteralPath $ledger)) { return @() }
    $alive = New-Object System.Collections.ArrayList
    foreach ($line in (Get-Content -LiteralPath $ledger)) {
        $trimmed = ([string]$line).Trim()
        if (-not $trimmed) { continue }
        $id = 0
        if (-not [int]::TryParse($trimmed, [ref]$id)) { continue }
        $proc = Get-Process -Id $id -ErrorAction SilentlyContinue
        if ($proc) { [void]$alive.Add($id) }
    }
    return ($alive | Select-Object -Unique)
}

function Show-WindowNoActivate {
    param(
        [Parameter(Mandatory = $true)][IntPtr]$WindowHandle,
        [int]$X = -1, [int]$Y = -1, [int]$Width = -1, [int]$Height = -1
    )
    Initialize-ProbeNative
    [void][AgentDesktopProbe.Native]::ShowWindow($WindowHandle, 4)
    if ($X -ge 0 -and $Y -ge 0 -and $Width -gt 0 -and $Height -gt 0) {
        [void][AgentDesktopProbe.Native]::SetWindowPos($WindowHandle, [IntPtr]::Zero, $X, $Y, $Width, $Height, 0x0014)
    }
}

function Start-ScratchProcess {
    param(
        [Parameter(Mandatory = $true)][string]$FilePath,
        [string[]]$ArgumentList = @(),
        [switch]$NoActivate,
        [int]$TimeoutSec = 15
    )
    Initialize-ProbeNative
    if ($ArgumentList -and $ArgumentList.Count -gt 0) {
        $proc = Start-Process -FilePath $FilePath -ArgumentList $ArgumentList -PassThru
    } else {
        $proc = Start-Process -FilePath $FilePath -PassThru
    }
    Register-ScratchProcessId -ProcessId $proc.Id
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    $handle = [IntPtr]::Zero
    while ((Get-Date) -lt $deadline) {
        $proc.Refresh()
        if ($proc.HasExited) { break }
        if ($proc.MainWindowHandle -ne [IntPtr]::Zero) { $handle = $proc.MainWindowHandle; break }
        Start-Sleep -Milliseconds 150
    }
    if ($NoActivate -and $handle -ne [IntPtr]::Zero) {
        Show-WindowNoActivate -WindowHandle $handle
    }
    return [pscustomobject]@{
        ProcessId        = $proc.Id
        MainWindowHandle = $handle
        Process          = $proc
    }
}

function Stop-ScratchProcess {
    param([Parameter(Mandatory = $true)][int]$ProcessId)
    $proc = Get-Process -Id $ProcessId -ErrorAction SilentlyContinue
    if ($proc) {
        try { Stop-Process -Id $ProcessId -Force -ErrorAction Stop } catch { }
    }
    $deadline = (Get-Date).AddSeconds(10)
    while ((Get-Date) -lt $deadline) {
        if (-not (Get-Process -Id $ProcessId -ErrorAction SilentlyContinue)) { return }
        Start-Sleep -Milliseconds 150
    }
    if (Get-Process -Id $ProcessId -ErrorAction SilentlyContinue) {
        throw ('PROBE-HARNESS: scratch process ' + $ProcessId + ' survived termination')
    }
}

function Assert-Foreground {
    param(
        [Parameter(Mandatory = $true)][int]$ExpectedProcessId,
        [Parameter(Mandatory = $true)][string]$Stage
    )
    Initialize-ProbeNative
    $actual = [AgentDesktopProbe.Native]::GetForegroundProcessId()
    if ($actual -ne $ExpectedProcessId) {
        throw ('PROBE-INTERFERENCE: stage ' + $Stage + ' expected foreground pid ' + $ExpectedProcessId + ' but observed ' + $actual)
    }
}

function Start-MediumIntegrityProcess {
    param(
        [Parameter(Mandatory = $true)][string]$FilePath,
        [string[]]$ArgumentList = @()
    )
    Initialize-ProbeNative
    $commandLine = '"' + $FilePath + '"'
    if ($ArgumentList -and $ArgumentList.Count -gt 0) { $commandLine = $commandLine + ' ' + ($ArgumentList -join ' ') }
    $workingDir = Split-Path -Parent $FilePath
    $found = [AgentDesktopProbe.Native]::StartMediumIntegrityProcess($commandLine, $workingDir)
    if ($found -le 0) {
        throw 'PROBE-HARNESS: CreateProcessAsUser returned no process id'
    }
    Register-ScratchProcessId -ProcessId $found
    $sid = ''
    $deadline = (Get-Date).AddSeconds(5)
    while ((Get-Date) -lt $deadline) {
        try { $sid = [AgentDesktopProbe.Native]::GetIntegritySid($found); break } catch { Start-Sleep -Milliseconds 100 }
    }
    if ($sid -ne 'S-1-16-8192') {
        try { Stop-ScratchProcess -ProcessId $found } catch { }
        throw ('PROBE-HARNESS: expected Medium integrity S-1-16-8192 for pid ' + $found + ' but token label is ' + $sid)
    }
    $proc = Get-Process -Id $found -ErrorAction SilentlyContinue
    $handle = [IntPtr]::Zero
    if ($proc) {
        $waitUntil = (Get-Date).AddSeconds(10)
        while ((Get-Date) -lt $waitUntil) {
            $proc.Refresh()
            if ($proc.HasExited -or $proc.MainWindowHandle -ne [IntPtr]::Zero) { break }
            Start-Sleep -Milliseconds 150
        }
        if (-not $proc.HasExited) { $handle = $proc.MainWindowHandle }
    }
    return [pscustomobject]@{
        ProcessId        = $found
        MainWindowHandle = $handle
        IntegritySid     = $sid
        Process          = $proc
    }
}

<#
    A pass that runs and answers "no" is data. A pass that never answered -
    because the probe binary exited non-zero, or because the pass never ran at
    all and wrote no capture - is not data, and on CI it is indistinguishable
    from success unless the harness says so: the artifact upload is CI's only
    other signal, and it is satisfied by a placeholder or by the surviving
    captures of the passes that did run. These four helpers let a probe declare
    which captures a CI run must contain, mark each one as it is produced, and
    ask at the end whether anything is missing or degraded.
#>
function New-NotMeasuredResult {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Reason)
    return [ordered]@{ not_measured = $true; skipped = $Reason }
}

function Test-PassNotMeasured {
    param([Parameter(Mandatory = $true)][AllowNull()]$Result)
    if ($null -eq $Result) { return $true }
    if ($Result -is [System.Collections.IDictionary]) {
        if ($Result.Contains($script:NotMeasuredKey)) { return [bool]$Result[$script:NotMeasuredKey] }
        return $Result.Contains('skipped')
    }
    $properties = $Result.PSObject.Properties
    $marker = $properties[$script:NotMeasuredKey]
    if ($null -ne $marker) { return [bool]$marker.Value }
    return ($null -ne $properties['skipped'])
}

function Register-MandatoryCapture {
    param([Parameter(Mandatory = $true)][string[]]$Name)
    foreach ($entry in $Name) {
        if (-not $script:MandatoryExpected.Contains($entry)) { [void]$script:MandatoryExpected.Add($entry) }
    }
}

function Register-MandatoryPass {
    param(
        [Parameter(Mandatory = $true)][string]$Capture,
        [Parameter(Mandatory = $true)][AllowNull()]$Result
    )
    $leaf = Split-Path -Leaf $Capture
    if (-not $script:MandatoryProduced.Contains($leaf)) { [void]$script:MandatoryProduced.Add($leaf) }
    if (Test-PassNotMeasured -Result $Result) {
        if (-not $script:MandatoryDegraded.Contains($leaf)) { [void]$script:MandatoryDegraded.Add($leaf) }
    }
}

function Get-MandatoryMeasurementGap {
    $missing = New-Object System.Collections.ArrayList
    foreach ($entry in $script:MandatoryExpected) {
        if (-not $script:MandatoryProduced.Contains($entry)) { [void]$missing.Add($entry) }
    }
    if ($missing.Count -eq 0 -and $script:MandatoryDegraded.Count -eq 0) { return $null }
    return [ordered]@{
        missing_captures  = @($missing)
        missing_count     = $missing.Count
        degraded_captures = @($script:MandatoryDegraded)
        degraded_count    = $script:MandatoryDegraded.Count
    }
}

function Write-ProbeLog {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Message,
        [string]$Level = 'info'
    )
    Write-Host ('PROBE-LOG [' + $Level + '] ' + $Message)
}

function Write-ProbeResult {
    param(
        [Parameter(Mandatory = $true)][string]$Probe,
        [Parameter(Mandatory = $true)][ValidateSet('ok', 'fail', 'skip')][string]$Status,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Message,
        [AllowNull()]$Data = $null
    )
    $payload = ''
    if ($null -ne $Data) {
        $payload = (ConvertTo-Json -InputObject $Data -Depth 25 -Compress)
    }
    $flat = (Protect-ProbeText -Text ($Message + '')) -replace '[\r\n|]', ' '
    $flatData = (Protect-ProbeText -Text $payload) -replace '[\r\n|]', ' '
    Write-Host ('PROBE-RESULT|' + $Probe + '|' + $Status + '|' + $flat + '|' + $flatData)
}

Initialize-ProbeRedaction
