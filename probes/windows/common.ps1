$script:ProbeRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$script:CapturesRoot = Join-Path $script:ProbeRoot 'captures'
$script:BoundsBucket = 8
$script:NotMeasuredKey = 'not_measured'
$script:MandatoryExpected = New-Object System.Collections.ArrayList
$script:MandatoryProduced = New-Object System.Collections.ArrayList
$script:MandatoryDegraded = New-Object System.Collections.ArrayList
$script:MandatoryGateSelfTestFailures = New-Object System.Collections.ArrayList
$script:UsersPathPattern = '[A-Za-z]:[\\/]+Users[\\/]+(?!Public[\\/\s"])(?!Default[\\/\s"])(?!All Users[\\/\s"])[^\\/:*?"<>|\s]+'
$script:NodeRecordNameField = ' name='
$script:NodeRecordPatternField = ' pats='
$script:ReducedNamePattern = '^<redacted:(\d+) chars>$'
$script:EchoedNameMinLength = 4

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

<#
    A node record's fields are JSON-escaped by the time they reach a committed
    capture - `<` and `>` become \u003c and \u003e, an apostrophe becomes
    \u0027 - so the reduction marker cannot be recognised, and a Name's length
    cannot be counted, without undoing that first. Done in one left-to-right
    pass rather than a chain of Replace calls: a Name carrying a literal
    backslash arrives as `C:\\new`, and any chain that looks for `\n` before
    it has consumed the escaped backslash turns that into a newline.
#>
function ConvertFrom-ProbeJsonEscape {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Text)
    if ([string]::IsNullOrEmpty($Text)) { return $Text }
    return [regex]::Replace($Text, '\\(u[0-9a-fA-F]{4}|.)', {
            param($m)
            $token = $m.Groups[1].Value
            if ($token.Length -eq 5) { return ([string][char][Convert]::ToInt32($token.Substring(1), 16)) }
            if ($token -eq 'n') { return "`n" }
            if ($token -eq 'r') { return "`r" }
            if ($token -eq 't') { return "`t" }
            if ($token -eq 'b') { return "`b" }
            if ($token -eq 'f') { return "`f" }
            return $token
        })
}

<#
    Every node record in a capture, whatever probe wrote it, keyed back to the
    pass it belongs to. A pass restarts its index at 0, which is what separates
    one dump from the next inside a single capture file.

    The Name is read from the first ' name=' at or after ' pats=' rather than
    from the last one in the line: the field order is fixed and pats/children
    carry no spaces, so that lands on the real field even when a Name itself
    contains the text ' name='. A line whose header has no digits - the
    NodeRecordFormat prose describing the shape - never matches.
#>
function Read-ProbeNodeRecords {
    param([Parameter(Mandatory = $true)][string]$Path)
    $records = New-Object System.Collections.ArrayList
    $pass = -1
    foreach ($line in [IO.File]::ReadAllLines($Path)) {
        $header = [regex]::Match($line, 'i=(\d+) d=\d+ parent=(-?\d+) ')
        if (-not $header.Success) { continue }
        $body = ([string]$line).Substring($header.Index).TrimEnd()
        if ($body.EndsWith(',')) { $body = $body.Substring(0, $body.Length - 1) }
        if ($body.EndsWith('"')) { $body = $body.Substring(0, $body.Length - 1) }
        $patternAt = $body.IndexOf($script:NodeRecordPatternField)
        if ($patternAt -lt 0) { continue }
        $nameAt = $body.IndexOf($script:NodeRecordNameField, $patternAt)
        if ($nameAt -lt 0) { continue }
        $index = [int]$header.Groups[1].Value
        if ($index -eq 0) { $pass++ }
        $name = ConvertFrom-ProbeJsonEscape -Text $body.Substring($nameAt + $script:NodeRecordNameField.Length)
        $reduced = [regex]::Match($name, $script:ReducedNamePattern)
        $length = 0
        if ($reduced.Success) {
            $length = [int]$reduced.Groups[1].Value
        } elseif ($name -ne '-') {
            $length = $name.Length
        }
        [void]$records.Add([pscustomobject]@{
                Pass    = $pass
                Index   = $index
                Parent  = [int]$header.Groups[2].Value
                Reduced = $reduced.Success
                Length  = $length
                Named   = ($name -ne '-' -and $name.Length -gt 0)
            })
    }
    return , $records
}

function Find-ReducedDescendant {
    param($ByIndex, $Children, [int]$Index, [int]$MaxLength)
    $pending = New-Object System.Collections.Stack
    if ($Children.ContainsKey($Index)) { foreach ($c in $Children[$Index]) { $pending.Push($c) } }
    while ($pending.Count -gt 0) {
        $current = $pending.Pop()
        $node = $null
        if ($ByIndex.ContainsKey($current)) { $node = $ByIndex[$current] }
        if ($null -ne $node -and $node.Reduced -and $node.Length -ge $script:EchoedNameMinLength -and $node.Length -le $MaxLength) {
            return $current
        }
        if (-not $Children.ContainsKey($current)) { continue }
        foreach ($c in $Children[$current]) {
            if ($c -gt $current) { $pending.Push($c) }
        }
    }
    return -1
}

<#
    A container with no label of its own takes its accessible name from the
    content beneath it, so a document title reaches a Group or a Button that no
    content-control-type test can see. The reducer that writes a capture
    catches that at the point of record by comparing the real strings. This is
    the same rule re-asserted over a committed capture, where the raw strings
    are gone and only lengths survive: a kept Name is residue when its own
    subtree holds a reduced Name short enough to fit inside it.

    Length containment is deliberately weaker than the string containment the
    reducer applies, so this gate flags a superset and can never miss what the
    reducer would have caught.

    Both are scoped to a pass whose root Name is itself reduced, which is the
    capture's own record that the caller declared this target to carry user
    content. Measured over the corpus, that scope is what separates the rule
    from evidence destruction rather than making it merely quieter: on the
    Settings frame the window title and the home-page header are both the word
    Settings, so an unscoped rule reduces the title of a system app that holds
    no user content at all - and reduces it on the frame window and the core
    window while leaving the title-bar window verbatim, dismantling the
    frame/host split those three nodes exist to demonstrate.
#>
function Test-CaptureNameEcho {
    param([Parameter(Mandatory = $true)][string]$Path)
    $records = Read-ProbeNodeRecords -Path $Path
    if ($records.Count -eq 0) { return $true }
    $residue = New-Object System.Collections.ArrayList
    foreach ($pass in ($records | Group-Object -Property Pass)) {
        $byIndex = @{}
        $children = @{}
        foreach ($record in $pass.Group) {
            $byIndex[$record.Index] = $record
            if ($record.Parent -lt 0) { continue }
            if (-not $children.ContainsKey($record.Parent)) { $children[$record.Parent] = New-Object System.Collections.ArrayList }
            [void]$children[$record.Parent].Add($record.Index)
        }
        if (-not $byIndex.ContainsKey(0)) { continue }
        if (-not $byIndex[0].Reduced) { continue }
        foreach ($record in $pass.Group) {
            if ($record.Reduced) { continue }
            if (-not $record.Named) { continue }
            $echo = Find-ReducedDescendant -ByIndex $byIndex -Children $children -Index $record.Index -MaxLength $record.Length
            if ($echo -ge 0) {
                [void]$residue.Add('pass ' + $pass.Name + ' node i=' + $record.Index + ' keeps a ' + $record.Length +
                    '-character Name verbatim while its descendant i=' + $echo + ' carries a reduced Name that fits inside it')
            }
        }
    }
    if ($residue.Count -gt 0) {
        foreach ($r in $residue) {
            Write-ProbeLog -Message ('name-echo residue in ' + $Path + ': ' + $r) -Level 'error'
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
    captures of the passes that did run. These helpers let a probe declare which
    captures a CI run must contain, mark each one as it is produced, and ask at
    the end whether anything is missing or degraded.

    An empty declaration is itself a gap, and Get-MandatoryMeasurementGap
    reports it as one. The declaration is what gives the gate its teeth, so a
    probe that declares nothing - a new leg written without one, a refactor that
    drops the call - would otherwise sail through the same check having asserted
    nothing at all. Answering it here rather than at each call site is what makes
    it unforgettable: every probe already asks this function whether the run
    measured what it promised, so every probe, including one written next year,
    inherits the answer without adding a line. Dev-box runs are unaffected for
    the same reason the rest of the gate leaves them alone - the -Label ci
    condition sits at the call site (Assert-MandatoryMeasurement), so a dev box
    reads the gap and keeps going.
#>
function New-NotMeasuredResult {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Reason)
    return [ordered]@{ not_measured = $true; skipped = $Reason }
}

<#
    A result that carries no not-measured marker is a measurement, whatever
    shape it has: a bare JSON array, a raw string and a number are all legitimate
    captures, and only New-NotMeasuredResult - which returns a dictionary -
    produces a placeholder, so demanding a property bag would flag real data. A
    result with no content is the exception. An empty array, an empty dictionary
    and a blank string assert exactly as much as the $null a pass returns when it
    produced nothing at all, and are treated the same way.
#>
function Test-PassNotMeasured {
    param([Parameter(Mandatory = $true)][AllowNull()]$Result)
    if ($null -eq $Result) { return $true }
    if ($Result -is [string]) { return [string]::IsNullOrWhiteSpace($Result) }
    if ($Result -is [System.Collections.IDictionary]) {
        if ($Result.Contains($script:NotMeasuredKey)) { return [bool]$Result[$script:NotMeasuredKey] }
        if ($Result.Contains('skipped')) { return $true }
        return ($Result.Count -eq 0)
    }
    if ($Result -is [System.Collections.ICollection]) { return ($Result.Count -eq 0) }
    $properties = $Result.PSObject.Properties
    $marker = $properties[$script:NotMeasuredKey]
    if ($null -ne $marker) { return [bool]$marker.Value }
    if ($null -ne $properties['skipped']) { return $true }
    # ConvertFrom-Json '{}' returns a PSCustomObject with no properties, which
    # is neither IDictionary nor ICollection, so neither branch above sees it.
    # That is the exact shape every probe pass produces when its Rust probe
    # prints an empty document and exits 0, and it was reaching the gate as a
    # measurement carrying nothing. The dictionary branch already treats a
    # property-less bag as no content; this applies the same rule to the type
    # ConvertFrom-Json actually returns.
    #
    # The test is on PSCustomObject specifically, not on "has no properties":
    # a bare 0 and a bare $false also expose no properties here, and both are
    # legitimate measurements. Widening this to every type made the gate's own
    # must-pass fixtures fail, which is what those fixtures are for.
    if ($Result -is [System.Management.Automation.PSCustomObject]) {
        return (@($properties).Count -eq 0)
    }
    return $false
}

<#
    The ledgers are addressable rather than only appendable so the self-test
    below can drive whole runs - a declaration, its passes and the verdict -
    through the shipped functions and then hand the next probe the same empty
    state it would have had. A saved-and-restored reference would work too, but
    a fresh ledger is exactly what dot-sourcing produces, so there is nothing to
    get subtly wrong.
#>
function Reset-MandatoryMeasurementState {
    $script:MandatoryExpected = New-Object System.Collections.ArrayList
    $script:MandatoryProduced = New-Object System.Collections.ArrayList
    $script:MandatoryDegraded = New-Object System.Collections.ArrayList
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
    if ($script:MandatoryExpected.Count -eq 0) {
        return [ordered]@{
            reason            = 'the probe declared no mandatory captures, so the run asserted nothing'
            empty_declaration = $true
            missing_captures  = @()
            missing_count     = 0
            degraded_captures = @($script:MandatoryDegraded)
            degraded_count    = $script:MandatoryDegraded.Count
        }
    }
    $missing = New-Object System.Collections.ArrayList
    foreach ($entry in $script:MandatoryExpected) {
        if (-not $script:MandatoryProduced.Contains($entry)) { [void]$missing.Add($entry) }
    }
    if ($missing.Count -eq 0 -and $script:MandatoryDegraded.Count -eq 0) { return $null }
    return [ordered]@{
        reason            = 'a mandatory pass produced no capture or recorded a placeholder instead of a measurement'
        empty_declaration = $false
        missing_captures  = @($missing)
        missing_count     = $missing.Count
        degraded_captures = @($script:MandatoryDegraded)
        degraded_count    = $script:MandatoryDegraded.Count
    }
}

<#
    Whether this run fails, and why. Every rule that decides it lives here and
    nothing here exits, which is what lets the self-test below drive the
    decision under both labels instead of only being able to watch a healthy
    run go by. Extracting it was not tidiness: with the decision inlined in the
    exit path, replacing "fail when there is a gap" with "return" left the gate
    reporting success on a run that measured nothing, and nothing anywhere
    noticed - the gate's own defect, one level up, sitting in the gate.

    The label is validated rather than free text, so a run label this gate has
    no policy for fails the parameter bind instead of quietly skipping the
    check. The gap is read on every label, not only ci, so a dev-box run
    exercises the same code path CI depends on.

    A gate whose own rules have stopped detecting would report success having
    asserted nothing, exactly as a probe that measured nothing would. So the
    self-test verdict is consulted here too, and before the gap, because a gap
    of $null means nothing once the rule that computes it is known to be
    broken.
#>
function Get-MandatoryGateSelfTestVerdict {
    if ($script:MandatoryGateSelfTestFailures.Count -eq 0) { return $null }
    return [ordered]@{
        reason                  = 'the mandatory-measurement gate failed its own self-test, so its verdict on this run means nothing'
        self_test_failed        = $true
        self_test_failures      = @($script:MandatoryGateSelfTestFailures)
        self_test_failure_count = $script:MandatoryGateSelfTestFailures.Count
    }
}

function Get-MandatoryMeasurementVerdict {
    param([Parameter(Mandatory = $true)][ValidateSet('devbox', 'ci')][string]$Label)
    $gap = Get-MandatoryMeasurementGap
    if ($Label -ne 'ci') { return $null }
    $selfTest = Get-MandatoryGateSelfTestVerdict
    if ($null -ne $selfTest) { return $selfTest }
    return $gap
}

<#
    The exit, so that no probe hand-rolls it, and the one part of the gate no
    in-process test can drive: a test that provoked the exit would end the run
    it is trying to report on. Everything it could get wrong other than the
    exit has been moved into Get-MandatoryMeasurementVerdict, which the
    self-test drives under both labels.

    The second read of the self-test verdict is not a duplicated rule, it is a
    deliberately independent wire. The first read reaches this function through
    the label policy and the verdict, and those are two of the things the
    self-test exists to catch breaking: with the alarm routed through them, a
    label policy inverted to skip ci silenced every self-test failure the run
    had just logged and the build went green. An alarm wired through the
    circuit it monitors is not an alarm. The failures are logged as errors on
    every label so a dev-box operator sees them, and -Label ci is what turns
    them into an exit code, exactly as for a gap.
#>
function Assert-MandatoryMeasurement {
    param(
        [Parameter(Mandatory = $true)][string]$Probe,
        [Parameter(Mandatory = $true)][ValidateSet('devbox', 'ci')][string]$Label
    )
    $verdict = Get-MandatoryMeasurementVerdict -Label $Label
    if ($null -eq $verdict -and $Label -eq 'ci') { $verdict = Get-MandatoryGateSelfTestVerdict }
    if ($null -eq $verdict) { return }
    Write-ProbeResult -Probe $Probe -Status 'fail' -Message ([string]$verdict['reason']) -Data $verdict
    exit 1
}

<#
    The gate's own MUST-CATCH / MUST-PASS fixture.

    Every case below is driven through the shipped functions - the placeholder
    comes from New-NotMeasuredResult, the marker key from $script:NotMeasuredKey,
    the ledgers from Register-MandatoryCapture and Register-MandatoryPass, the
    gap from Get-MandatoryMeasurementGap and the pass/fail decision from
    Get-MandatoryMeasurementVerdict - so the fixture cannot drift away from the
    rule by testing a second copy of it that agrees with itself. What is
    written out here is only the inputs and the expected answers.

    Both halves are driven, because a gate that reports a gap for everything is
    as useless as one that never reports one, and only the second announces
    itself. The must-pass half is the one that has to survive: a pass that ran
    and answered "no" - an empty candidate list inside a result that says so, a
    false flag, a zero - is a measurement, and a gate that called it a gap would
    teach every probe author to stop declaring captures.

    It runs at dot-source time, so every probe that uses the gate runs it,
    including one written next year that never hears about it, and it costs a
    few milliseconds of in-memory work. The ledger state it leaves behind is a
    fresh one, identical to what dot-sourcing alone produces.

    What it cannot reach, so that nobody assumes otherwise: the last three
    lines of Assert-MandatoryMeasurement, where a verdict becomes an exit code,
    and the alarm in Get-MandatoryGateSelfTestVerdict that carries these
    failures there. A test that provoked the exit would end the run it is
    reporting on, and an alarm cannot raise itself once silenced. Breaking
    either is still visible - every failure below is written to the log on
    every label - but it is visible as log text on a green run, which is the
    weakest signal this gate has. Keep both minimal for that reason.
#>
function Test-MandatoryMeasurementGate {
    # Nested so it cannot be mistaken for probe API. It reads whatever ledger
    # state the scenario around it has just built, through the same verdict
    # function Assert-MandatoryMeasurement calls, and asserts both halves of
    # the label policy: ci fails on a gap, and a dev box never fails at all.
    function Test-VerdictLabelPolicy {
        param(
            [Parameter(Mandatory = $true)][string]$Case,
            [Parameter(Mandatory = $true)][bool]$ExpectCiFailure
        )
        $found = New-Object System.Collections.ArrayList
        $ci = Get-MandatoryMeasurementVerdict -Label 'ci'
        $devbox = Get-MandatoryMeasurementVerdict -Label 'devbox'
        if ($ExpectCiFailure -and $null -eq $ci) {
            [void]$found.Add('MUST CATCH, not failed: ' + $Case + ' did not fail a ci run')
        }
        if ($ExpectCiFailure -and $null -ne $ci -and [string]::IsNullOrWhiteSpace([string]$ci['reason'])) {
            [void]$found.Add('MUST CATCH, unexplained: ' + $Case + ' failed a ci run with no reason for the operator to read')
        }
        if ((-not $ExpectCiFailure) -and $null -ne $ci) {
            [void]$found.Add('MUST PASS, false positive: ' + $Case + ' failed a ci run')
        }
        if ($null -ne $devbox) {
            [void]$found.Add('MUST PASS, false positive: ' + $Case + ' failed a dev-box run, which this gate never does')
        }
        return , $found
    }

    $failures = New-Object System.Collections.ArrayList
    $alpha = 'gate-self-test-alpha.json'
    $beta = 'gate-self-test-beta.json'
    $alphaPath = Join-Path 'C:\gate-self-test' $alpha
    try {
        $placeholder = New-NotMeasuredResult -Reason 'the probe exited with code 1'
        $markerOnly = @{}
        $markerOnly[$script:NotMeasuredKey] = $true
        $markerFalse = @{}
        $markerFalse[$script:NotMeasuredKey] = $false
        $markerFalse['rows'] = 3
        $notMeasured = @(
            @{ Case = 'a pass that returned nothing at all'; Value = $null },
            @{ Case = 'the placeholder New-NotMeasuredResult returns'; Value = $placeholder },
            @{ Case = 'a placeholder read back from its own capture'; Value = (ConvertFrom-Json (ConvertTo-Json -InputObject $placeholder -Depth 5)) },
            @{ Case = 'a not-measured marker carrying no skip reason'; Value = $markerOnly },
            @{ Case = 'a not-measured marker carrying no skip reason, read back from JSON'; Value = (ConvertFrom-Json (ConvertTo-Json -InputObject $markerOnly -Depth 5)) },
            @{ Case = 'a skip reason carrying no marker'; Value = @{ skipped = 'cargo is not installed on this machine' } },
            @{ Case = 'a skip reason carrying no marker, read back from JSON'; Value = (ConvertFrom-Json '{"skipped":"cargo is not installed on this machine"}') },
            @{ Case = 'an empty list'; Value = @() },
            @{ Case = 'an empty dictionary'; Value = @{} },
            @{ Case = 'an empty ordered dictionary'; Value = ([ordered]@{}) },
            @{ Case = 'an empty string'; Value = '' },
            @{ Case = 'a blank string'; Value = '   ' },
            @{ Case = 'an empty JSON object read back from its own capture'; Value = (ConvertFrom-Json '{}') }
        )
        $measured = @(
            @{ Case = 'the number zero'; Value = 0 },
            @{ Case = 'the boolean false'; Value = $false },
            @{ Case = 'a bare string'; Value = 'RawView' },
            @{ Case = 'a bare array'; Value = @(1, 2) },
            @{ Case = 'a measured negative that found nothing and says so'; Value = ([ordered]@{ labeled_by_resolved = $false; candidates = @() }) },
            @{ Case = 'a measured negative read back from its own capture'; Value = (ConvertFrom-Json '{"labeled_by_resolved":false,"candidates":[]}') },
            @{ Case = 'a measurement that states the marker is false'; Value = $markerFalse },
            @{ Case = 'a JSON object whose only property is itself empty'; Value = (ConvertFrom-Json '{"measurements":{}}') }
        )
        foreach ($case in $notMeasured) {
            if (-not (Test-PassNotMeasured -Result $case.Value)) {
                [void]$failures.Add('MUST CATCH, missed: ' + $case.Case + ' was accepted as a measurement')
            }
        }
        foreach ($case in $measured) {
            if (Test-PassNotMeasured -Result $case.Value) {
                [void]$failures.Add('MUST PASS, false positive: ' + $case.Case + ' was rejected as not measured')
            }
        }

        Reset-MandatoryMeasurementState
        Register-MandatoryCapture -Name @($alpha)
        Register-MandatoryPass -Capture $alphaPath -Result $placeholder
        $degradedGap = Get-MandatoryMeasurementGap
        if ($null -eq $degradedGap) {
            [void]$failures.Add('MUST CATCH, missed: a mandatory pass that recorded a placeholder was reported as a complete run')
        } else {
            if (-not (@($degradedGap['degraded_captures']) -contains $alpha)) {
                [void]$failures.Add('MUST CATCH, unnamed: the degraded capture ' + $alpha + ' was not named in the gap')
            }
            if ($degradedGap['degraded_count'] -ne 1) {
                [void]$failures.Add('MUST CATCH, miscounted: one degraded capture was reported as ' + $degradedGap['degraded_count'])
            }
            if ($degradedGap['empty_declaration']) {
                [void]$failures.Add('MUST CATCH, misattributed: a degraded pass was reported as an empty declaration')
            }
            if ([string]::IsNullOrWhiteSpace([string]$degradedGap['reason'])) {
                [void]$failures.Add('MUST CATCH, unexplained: the degraded-pass gap carries no reason for the operator to read')
            }
        }
        foreach ($f in (Test-VerdictLabelPolicy -Case 'a mandatory pass that recorded a placeholder' -ExpectCiFailure $true)) {
            [void]$failures.Add($f)
        }

        Reset-MandatoryMeasurementState
        Register-MandatoryCapture -Name @($alpha, $beta)
        Register-MandatoryPass -Capture $alphaPath -Result ([ordered]@{ rows = 1 })
        $missingGap = Get-MandatoryMeasurementGap
        if ($null -eq $missingGap) {
            [void]$failures.Add('MUST CATCH, missed: a mandatory pass that never ran was reported as a complete run')
        } else {
            if (-not (@($missingGap['missing_captures']) -contains $beta)) {
                [void]$failures.Add('MUST CATCH, unnamed: the absent capture ' + $beta + ' was not named in the gap')
            }
            if ($missingGap['missing_count'] -ne 1) {
                [void]$failures.Add('MUST CATCH, miscounted: one absent capture was reported as ' + $missingGap['missing_count'])
            }
            if ([string]::IsNullOrWhiteSpace([string]$missingGap['reason'])) {
                [void]$failures.Add('MUST CATCH, unexplained: the absent-capture gap carries no reason for the operator to read')
            }
        }
        foreach ($f in (Test-VerdictLabelPolicy -Case 'a mandatory pass that never ran' -ExpectCiFailure $true)) {
            [void]$failures.Add($f)
        }

        Reset-MandatoryMeasurementState
        $emptyGap = Get-MandatoryMeasurementGap
        if ($null -eq $emptyGap) {
            [void]$failures.Add('MUST CATCH, missed: a run that declared no mandatory captures was reported as a complete run')
        } else {
            if (-not $emptyGap['empty_declaration']) {
                [void]$failures.Add('MUST CATCH, misattributed: an empty declaration was not reported as one')
            }
            if ([string]::IsNullOrWhiteSpace([string]$emptyGap['reason'])) {
                [void]$failures.Add('MUST CATCH, unexplained: the empty-declaration gap carries no reason for the operator to read')
            }
        }
        foreach ($f in (Test-VerdictLabelPolicy -Case 'a run that declared no mandatory captures' -ExpectCiFailure $true)) {
            [void]$failures.Add($f)
        }

        Reset-MandatoryMeasurementState
        Register-MandatoryCapture -Name @($alpha, $beta)
        Register-MandatoryPass -Capture $alphaPath -Result ([ordered]@{ labeled_by_resolved = $false; candidates = @() })
        Register-MandatoryPass -Capture (Join-Path 'C:\gate-self-test' $beta) -Result (ConvertFrom-Json '{"found":false,"rows":[]}')
        $cleanGap = Get-MandatoryMeasurementGap
        if ($null -ne $cleanGap) {
            [void]$failures.Add('MUST PASS, false positive: a run whose passes all measured a negative was reported as a gap - ' + [string]$cleanGap['reason'])
        }
        foreach ($f in (Test-VerdictLabelPolicy -Case 'a run whose passes all measured a negative' -ExpectCiFailure $false)) {
            [void]$failures.Add($f)
        }

        # The branch that carries this whole self-test into a CI exit code,
        # driven with an injected failure over an otherwise complete run: no
        # gap, so a ci failure here can only come from the self-test verdict
        # being honoured. Without this, a change that ignored the self-test
        # would leave every case above still passing and reporting nothing.
        $quietAlarm = Get-MandatoryGateSelfTestVerdict
        $realFailures = $script:MandatoryGateSelfTestFailures
        $script:MandatoryGateSelfTestFailures = New-Object System.Collections.ArrayList
        [void]$script:MandatoryGateSelfTestFailures.Add('injected by the gate self-test')
        $injectedAlarm = Get-MandatoryGateSelfTestVerdict
        $injectedCi = Get-MandatoryMeasurementVerdict -Label 'ci'
        $injectedDevbox = Get-MandatoryMeasurementVerdict -Label 'devbox'
        $script:MandatoryGateSelfTestFailures = $realFailures
        if ($null -ne $quietAlarm) {
            [void]$failures.Add('MUST PASS, false positive: the self-test alarm fired with no self-test failure recorded')
        }
        if ($null -eq $injectedAlarm) {
            [void]$failures.Add('MUST CATCH, not failed: the self-test alarm stayed quiet with a self-test failure recorded')
        } elseif (-not $injectedAlarm['self_test_failed']) {
            [void]$failures.Add('MUST CATCH, misattributed: a self-test failure was not reported as one')
        }
        if ($null -eq $injectedCi) {
            [void]$failures.Add('MUST CATCH, not failed: a gate whose own self-test failed did not fail a ci run')
        }
        if ($null -ne $injectedDevbox) {
            [void]$failures.Add('MUST PASS, false positive: a gate whose own self-test failed failed a dev-box run')
        }
    } catch {
        [void]$failures.Add('the gate threw while being tested: ' + $_.Exception.Message)
    } finally {
        Reset-MandatoryMeasurementState
    }
    return , $failures
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

$script:MandatoryGateSelfTestFailures = Test-MandatoryMeasurementGate
foreach ($selfTestFailure in $script:MandatoryGateSelfTestFailures) {
    Write-ProbeLog -Message ('mandatory-measurement gate self-test: ' + $selfTestFailure) -Level 'error'
}
