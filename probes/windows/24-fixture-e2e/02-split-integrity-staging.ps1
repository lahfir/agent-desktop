#Requires -Version 5.1
<#
.SYNOPSIS
    Split-integrity boundary staging probe (area 24, sub-phase 2.12).

.DESCRIPTION
    Rows A18-4, A19-4, A20-2 and A21-7 all deferred split-integrity
    verification onto 2.12 on the strength of a single route
    (Start-MediumIntegrityProcess-shaped CreateProcessAsUser) not always
    being usable. Nobody had tried the alternatives. This probe measures
    four independent routes to a below-High-integrity process on THIS box:

      route A: runas.exe /trustlevel:0x20000 (basic-user trust level)
      route B: copy an exe, apply a Low mandatory label with icacls, run it
      route C: a Task Scheduler task registered /RL LIMITED
      route D: CreateRestrictedToken + SetTokenInformation(TokenIntegrityLevel)
               + CreateProcessAsUser via a self-contained P/Invoke helper

    Every route launches a harmless renamed copy of cmd.exe running
    `/c ping -n 6 127.0.0.1` (a few seconds of no-op) so there is a live PID
    to read GetTokenInformation(TokenIntegrityLevel) from before it exits.
    The RID read back from that token is the proof; a route that reports
    launch success but yields a High-integrity process is recorded as a
    failed staging, not a success, per the calling instructions.

    Captures under captures\ as split-integrity-staging-{devbox,ci}.json
    (+ .normalized twin). Corpus safety: only route names, booleans, exit
    codes, integrity SIDs/RIDs and bounded/redacted Win32 error strings are
    ever handed to ConvertTo-Json - no window titles, file paths, pids,
    machine names, user names or message text. Every renamed scratch binary
    and scheduled task this probe creates is removed on every exit path.
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'ci')][string]$Label = 'devbox'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

. (Join-Path (Split-Path -Parent $PSCommandPath) '..\common.ps1')
Initialize-ProbeRedaction
Initialize-ProbeNative

$script:ProbeDir = Split-Path -Parent $PSCommandPath
$script:CaptureDir = Join-Path $script:ProbeDir 'captures'
if (-not (Test-Path -LiteralPath $script:CaptureDir)) {
    New-Item -ItemType Directory -Path $script:CaptureDir -Force | Out-Null
}
$script:Spawned = New-Object System.Collections.ArrayList
$script:ScratchDir = $null
$script:ScratchFiles = New-Object System.Collections.ArrayList
$script:ScratchTasks = New-Object System.Collections.ArrayList

Register-MandatoryCapture -Name @("split-integrity-staging-$Label.json")

function Write-A24Capture {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Content
    )
    $redacted = Protect-ProbeText -Text $Content
    $path = Join-Path $script:CaptureDir $Name
    $utf8NoBom = New-Object System.Text.UTF8Encoding $false
    [IO.File]::WriteAllText($path, $redacted, $utf8NoBom)
    $normalized = Get-NormalizedCapture -Text $redacted
    [IO.File]::WriteAllText(($path + '.normalized'), $normalized, $utf8NoBom)
    if (-not (Test-CaptureRedaction -Path $path)) {
        throw "redaction residue in $path"
    }
    return $path
}

function Register-SpawnedPid {
    param([int]$ProcessId)
    if ($ProcessId -gt 0) { [void]$script:Spawned.Add($ProcessId) }
}

function Stop-AllSpawned {
    foreach ($id in @($script:Spawned)) {
        try { Stop-ScratchProcess -ProcessId $id } catch { }
    }
    $script:Spawned.Clear()
}

function Get-BoundedErrorText {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Text)
    if ([string]::IsNullOrEmpty($Text)) { return '' }
    $flat = ($Text -replace '[\r\n]+', ' ').Trim()
    if ($flat.Length -gt 300) { $flat = $flat.Substring(0, 300) + '...' }
    return $flat
}

<#
    The RID is the last dash-delimited component of a mandatory-label SID
    (S-1-16-<rid>). The label table mirrors 00-environment.ps1's so a
    reader cross-checking this capture against that one sees the same
    names for the same RIDs.
#>
function Get-IntegrityLabelForSid {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Sid)
    switch ($Sid) {
        'S-1-16-0' { return 'Untrusted' }
        'S-1-16-4096' { return 'Low' }
        'S-1-16-8192' { return 'Medium' }
        'S-1-16-8448' { return 'Medium Plus' }
        'S-1-16-12288' { return 'High' }
        'S-1-16-16384' { return 'System' }
        default { return 'Unknown' }
    }
}

function Get-IntegrityRidForSid {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Sid)
    if ([string]::IsNullOrEmpty($Sid)) { return $null }
    $parts = $Sid.Split('-')
    $rid = 0
    if ([int]::TryParse($parts[$parts.Length - 1], [ref]$rid)) { return $rid }
    return $null
}

<#
    A self-contained P/Invoke surface for route D, deliberately not reusing
    common.ps1's AgentDesktopProbe.Native.StartMediumIntegrityProcess: that
    helper lowers the mandatory label directly on a duplicated primary
    token. Route D is a materially different mechanism the calling
    instructions name explicitly - CreateRestrictedToken first (stripping
    privileges via DISABLE_MAX_PRIVILEGE), then lowering the label on the
    restricted token, then CreateProcessAsUser - so it is implemented here
    independently rather than aliased to the existing helper.
#>
function Initialize-A24RestrictedTokenNative {
    if ('AgentDesktopProbe.A24.RestrictedTokenLauncher' -as [type]) { return }
    $src = @'
using System;
using System.Runtime.InteropServices;
using System.Text;

namespace AgentDesktopProbe.A24 {
    [StructLayout(LayoutKind.Sequential)]
    public struct ProcInfo { public IntPtr hProcess; public IntPtr hThread; public int dwProcessId; public int dwThreadId; }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct StartInfo {
        public int cb; public string lpReserved; public string lpDesktop; public string lpTitle;
        public int dwX; public int dwY; public int dwXSize; public int dwYSize;
        public int dwXCountChars; public int dwYCountChars; public int dwFillAttribute;
        public int dwFlags; public short wShowWindow; public short cbReserved2;
        public IntPtr lpReserved2; public IntPtr hStdInput; public IntPtr hStdOutput; public IntPtr hStdError;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct SidAndAttr { public IntPtr Sid; public uint Attributes; }

    [StructLayout(LayoutKind.Sequential)]
    public struct TokenMandatoryLabelD { public SidAndAttr Label; }

    public static class RestrictedTokenLauncher {
        [DllImport("kernel32.dll")]
        public static extern IntPtr GetCurrentProcess();
        [DllImport("advapi32.dll", SetLastError = true)]
        public static extern bool OpenProcessToken(IntPtr h, uint access, out IntPtr token);
        [DllImport("advapi32.dll", SetLastError = true)]
        public static extern bool CreateRestrictedToken(IntPtr existingToken, uint flags,
            int disableSidCount, IntPtr sidsToDisable,
            int deletePrivCount, IntPtr privilegesToDelete,
            int restrictSidCount, IntPtr sidsToRestrict,
            out IntPtr newToken);
        [DllImport("advapi32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        public static extern bool ConvertStringSidToSid(string sid, out IntPtr pSid);
        [DllImport("advapi32.dll", SetLastError = true)]
        public static extern bool SetTokenInformation(IntPtr token, int cls, IntPtr info, int len);
        [DllImport("advapi32.dll", SetLastError = true)]
        public static extern int GetLengthSid(IntPtr pSid);
        [DllImport("kernel32.dll")]
        public static extern IntPtr LocalFree(IntPtr h);
        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern bool CloseHandle(IntPtr h);
        [DllImport("advapi32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        public static extern bool CreateProcessAsUser(IntPtr token, string appName, StringBuilder cmdLine,
            IntPtr procAttr, IntPtr threadAttr, bool inherit, uint flags, IntPtr env, string curDir,
            ref StartInfo si, out ProcInfo pi);

        private const uint DISABLE_MAX_PRIVILEGE = 0x1;
        private const uint TOKEN_ALL_ACCESS = 0xF01FF;
        private const int TokenIntegrityLevel = 25;
        private const uint CREATE_NO_WINDOW = 0x08000000;
        private const uint NORMAL_PRIORITY_CLASS = 0x00000020;

        public static int LaunchWithRestrictedLowToken(string commandLine, string workingDirectory, out string errorDetail) {
            errorDetail = "";
            IntPtr hSelf = IntPtr.Zero, hRestricted = IntPtr.Zero, pSid = IntPtr.Zero, pLabel = IntPtr.Zero;
            try {
                if (!OpenProcessToken(GetCurrentProcess(), TOKEN_ALL_ACCESS, out hSelf)) {
                    errorDetail = "OpenProcessToken failed, error " + Marshal.GetLastWin32Error();
                    return -1;
                }
                if (!CreateRestrictedToken(hSelf, DISABLE_MAX_PRIVILEGE, 0, IntPtr.Zero, 0, IntPtr.Zero, 0, IntPtr.Zero, out hRestricted)) {
                    errorDetail = "CreateRestrictedToken failed, error " + Marshal.GetLastWin32Error();
                    return -1;
                }
                if (!ConvertStringSidToSid("S-1-16-4096", out pSid)) {
                    errorDetail = "ConvertStringSidToSid failed, error " + Marshal.GetLastWin32Error();
                    return -1;
                }
                TokenMandatoryLabelD tml = new TokenMandatoryLabelD();
                tml.Label.Sid = pSid;
                tml.Label.Attributes = 0x20;
                int size = Marshal.SizeOf(typeof(TokenMandatoryLabelD));
                pLabel = Marshal.AllocHGlobal(size);
                Marshal.StructureToPtr(tml, pLabel, false);
                if (!SetTokenInformation(hRestricted, TokenIntegrityLevel, pLabel, size + GetLengthSid(pSid))) {
                    errorDetail = "SetTokenInformation(TokenIntegrityLevel) failed, error " + Marshal.GetLastWin32Error();
                    return -1;
                }
                StartInfo si = new StartInfo();
                si.cb = Marshal.SizeOf(typeof(StartInfo));
                si.lpDesktop = "winsta0\\default";
                ProcInfo pi;
                StringBuilder cmd = new StringBuilder(commandLine);
                if (!CreateProcessAsUser(hRestricted, null, cmd, IntPtr.Zero, IntPtr.Zero, false,
                        CREATE_NO_WINDOW | NORMAL_PRIORITY_CLASS, IntPtr.Zero, workingDirectory, ref si, out pi)) {
                    errorDetail = "CreateProcessAsUser failed, error " + Marshal.GetLastWin32Error();
                    return -1;
                }
                CloseHandle(pi.hThread);
                CloseHandle(pi.hProcess);
                return pi.dwProcessId;
            } finally {
                if (pLabel != IntPtr.Zero) { Marshal.FreeHGlobal(pLabel); }
                if (pSid != IntPtr.Zero) { LocalFree(pSid); }
                if (hRestricted != IntPtr.Zero) { CloseHandle(hRestricted); }
                if (hSelf != IntPtr.Zero) { CloseHandle(hSelf); }
            }
        }
    }
}
'@
    Add-ProbeInlineCSharp -Source $src -AssemblyLeaf 'AgentDesktopProbeA24RestrictedToken'
}

function New-A24ScratchCopy {
    param([Parameter(Mandatory = $true)][string]$Leaf)
    if (-not $script:ScratchDir) {
        $script:ScratchDir = Join-Path $env:TEMP ('agent-desktop-a24-' + [guid]::NewGuid().ToString('N'))
        New-Item -ItemType Directory -Path $script:ScratchDir -Force | Out-Null
    }
    $dest = Join-Path $script:ScratchDir ($Leaf + '.exe')
    Copy-Item -LiteralPath (Join-Path $env:WINDIR 'System32\cmd.exe') -Destination $dest -Force
    [void]$script:ScratchFiles.Add($dest)
    return $dest
}

<#
    Every route targets the same harmless command: a renamed cmd.exe copy
    running a short-lived, no-op ping so there is a live PID window to read
    a token from before the process exits on its own. The renamed leaf name
    is what later routes poll Get-Process by, rather than matching on any
    command line or window text that would need redaction.
#>
function Get-A24TargetArgs {
    return @('/c', 'ping', '-n', '6', '127.0.0.1')
}

function Wait-A24ProcessByName {
    param([Parameter(Mandatory = $true)][string]$Name, [int]$TimeoutMs = 6000)
    $deadline = [Diagnostics.Stopwatch]::StartNew()
    while ($deadline.ElapsedMilliseconds -lt $TimeoutMs) {
        $proc = Get-Process -Name $Name -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($proc) { return $proc }
        Start-Sleep -Milliseconds 200
    }
    return $null
}

<#
    Common tail every route shares once it believes it has a live PID:
    read the token integrity SID (never trusting the launcher's own success
    report), classify it, and kill the process. Isolated so all four routes
    apply the identical proof standard the calling instructions require.
#>
function Get-A24IntegrityVerdict {
    param([Parameter(Mandatory = $true)][int]$ProcessId)
    try {
        $sid = [AgentDesktopProbe.Native]::GetIntegritySid($ProcessId)
    } catch {
        return [ordered]@{
            process_observed  = $true
            integrity_read_ok = $false
            integrity_error   = Get-BoundedErrorText -Text $_.Exception.Message
            integrity_sid     = $null
            integrity_rid     = $null
            integrity_label   = $null
            verified_non_high = $false
        }
    }
    $rid = Get-IntegrityRidForSid -Sid $sid
    $label = Get-IntegrityLabelForSid -Sid $sid
    $nonHigh = ($null -ne $rid -and $rid -lt 12288)
    return [ordered]@{
        process_observed  = $true
        integrity_read_ok = $true
        integrity_error   = $null
        integrity_sid     = $sid
        integrity_rid     = $rid
        integrity_label   = $label
        verified_non_high = $nonHigh
    }
}

function Get-A24AbsentProcessResult {
    param([Parameter(Mandatory = $true)][string]$Branch)
    return [ordered]@{
        measurable        = $true
        branch            = $Branch
        process_observed  = $false
        integrity_read_ok = $false
        integrity_sid     = $null
        integrity_rid     = $null
        integrity_label   = $null
        verified_non_high = $false
        staged            = $false
    }
}

# ---------------------------------------------------------------- route A: runas /trustlevel:0x20000

function Measure-RouteA-RunasTrustlevel {
    $runas = Get-Command 'runas.exe' -ErrorAction SilentlyContinue
    if (-not $runas) {
        return [ordered]@{
            measurable = $true
            branch     = 'route_prerequisite_missing'
            detail     = 'runas.exe not found on PATH'
            staged     = $false
        }
    }
    $leaf = 'adprobe-route-a'
    $exe = New-A24ScratchCopy -Leaf $leaf
    $outFile = Join-Path $env:TEMP ('a24-runas-stdout-' + [guid]::NewGuid().ToString('N') + '.txt')
    $errFile = Join-Path $env:TEMP ('a24-runas-stderr-' + [guid]::NewGuid().ToString('N') + '.txt')
    $launcherPid = 0
    try {
        $commandAndArgs = '"' + $exe + '" ' + (Get-A24TargetArgs -join ' ')
        $proc = Start-Process -FilePath 'runas.exe' `
            -ArgumentList @('/trustlevel:0x20000', $commandAndArgs) `
            -WindowStyle Hidden -PassThru `
            -RedirectStandardOutput $outFile -RedirectStandardError $errFile
        $launcherPid = $proc.Id
        Register-SpawnedPid -ProcessId $launcherPid

        $target = Wait-A24ProcessByName -Name $leaf -TimeoutMs 6000
        if (-not $target) {
            $stderrText = ''
            if (Test-Path -LiteralPath $errFile) { $stderrText = (Get-Content -LiteralPath $errFile -Raw -ErrorAction SilentlyContinue) }
            $result = Get-A24AbsentProcessResult -Branch 'process_not_observed_within_timeout'
            $result.launcher_exit_code = $proc.ExitCode
            $result.launch_error = Get-BoundedErrorText -Text $stderrText
            return $result
        }
        Register-SpawnedPid -ProcessId $target.Id
        $verdict = Get-A24IntegrityVerdict -ProcessId $target.Id
        try { Stop-ScratchProcess -ProcessId $target.Id } catch { }
        $branch = if ($verdict.verified_non_high) { 'staged_non_high_integrity_confirmed' }
        elseif (-not $verdict.integrity_read_ok) { 'integrity_read_failed' }
        else { 'staged_but_still_high_integrity' }
        $result = [ordered]@{ measurable = $true; branch = $branch }
        foreach ($k in $verdict.Keys) { $result[$k] = $verdict[$k] }
        $result.staged = [bool]$verdict.verified_non_high
        return $result
    } catch {
        return [ordered]@{
            measurable  = $true
            branch      = 'launch_command_failed'
            launch_error = Get-BoundedErrorText -Text $_.Exception.Message
            staged      = $false
        }
    } finally {
        if ($launcherPid -gt 0) { try { Stop-ScratchProcess -ProcessId $launcherPid } catch { } }
        Remove-Item -LiteralPath $outFile -Force -ErrorAction SilentlyContinue
        Remove-Item -LiteralPath $errFile -Force -ErrorAction SilentlyContinue
    }
}

# ---------------------------------------------------------------- route B: icacls mandatory label

function Measure-RouteB-IcaclsLabel {
    $leaf = 'adprobe-route-b'
    $exe = New-A24ScratchCopy -Leaf $leaf
    try {
        $icaclsOut = & icacls.exe $exe '/setintegritylevel' '(OI)(CI)' 'Low' 2>&1 | ForEach-Object { "$_" }
        $icaclsExit = $LASTEXITCODE
        if ($icaclsExit -ne 0) {
            return [ordered]@{
                measurable   = $true
                branch       = 'icacls_setintegritylevel_failed'
                icacls_exit_code = $icaclsExit
                launch_error = Get-BoundedErrorText -Text ($icaclsOut -join ' ')
                staged       = $false
            }
        }

        $proc = Start-Process -FilePath $exe -ArgumentList (Get-A24TargetArgs) -WindowStyle Hidden -PassThru
        Register-SpawnedPid -ProcessId $proc.Id
        Start-Sleep -Milliseconds 400
        $proc.Refresh()
        if ($proc.HasExited) {
            return Get-A24AbsentProcessResult -Branch 'process_exited_before_read'
        }
        $verdict = Get-A24IntegrityVerdict -ProcessId $proc.Id
        try { Stop-ScratchProcess -ProcessId $proc.Id } catch { }
        $branch = if ($verdict.verified_non_high) { 'staged_non_high_integrity_confirmed' }
        elseif (-not $verdict.integrity_read_ok) { 'integrity_read_failed' }
        else { 'staged_but_still_high_integrity' }
        $result = [ordered]@{ measurable = $true; branch = $branch; icacls_exit_code = $icaclsExit }
        foreach ($k in $verdict.Keys) { $result[$k] = $verdict[$k] }
        $result.staged = [bool]$verdict.verified_non_high
        $result.note = 'a file mandatory label restricts who may write/execute the file; it does not itself set the integrity level of a process launched from an already-High caller, which is what this leg either confirms or refutes empirically'
        return $result
    } catch {
        return [ordered]@{
            measurable  = $true
            branch      = 'launch_command_failed'
            launch_error = Get-BoundedErrorText -Text $_.Exception.Message
            staged      = $false
        }
    }
}

# ---------------------------------------------------------------- route C: scheduled task /RL LIMITED

function Measure-RouteC-ScheduledTaskLimited {
    $schtasks = Get-Command 'schtasks.exe' -ErrorAction SilentlyContinue
    if (-not $schtasks) {
        return [ordered]@{
            measurable = $true
            branch     = 'route_prerequisite_missing'
            detail     = 'schtasks.exe not found on PATH'
            staged     = $false
        }
    }
    $leaf = 'adprobe-route-c'
    $exe = New-A24ScratchCopy -Leaf $leaf
    $taskName = 'AgentDesktopProbe-SplitIntegrity-' + [guid]::NewGuid().ToString('N').Substring(0, 8)
    [void]$script:ScratchTasks.Add($taskName)
    try {
        $trValue = '"' + $exe + '" ' + (Get-A24TargetArgs -join ' ')
        $createOut = & schtasks.exe '/Create' '/TN' $taskName '/TR' $trValue '/SC' 'ONCE' '/ST' '00:00' '/RL' 'LIMITED' '/F' 2>&1 | ForEach-Object { "$_" }
        $createExit = $LASTEXITCODE
        if ($createExit -ne 0) {
            return [ordered]@{
                measurable       = $true
                branch           = 'schtasks_create_failed'
                create_exit_code = $createExit
                launch_error     = Get-BoundedErrorText -Text ($createOut -join ' ')
                staged           = $false
            }
        }

        $runOut = & schtasks.exe '/Run' '/TN' $taskName 2>&1 | ForEach-Object { "$_" }
        $runExit = $LASTEXITCODE
        if ($runExit -ne 0) {
            return [ordered]@{
                measurable       = $true
                branch           = 'schtasks_run_failed'
                create_exit_code = $createExit
                run_exit_code    = $runExit
                launch_error     = Get-BoundedErrorText -Text ($runOut -join ' ')
                staged           = $false
            }
        }

        $target = Wait-A24ProcessByName -Name $leaf -TimeoutMs 8000
        if (-not $target) {
            $result = Get-A24AbsentProcessResult -Branch 'process_not_observed_within_timeout'
            $result.create_exit_code = $createExit
            $result.run_exit_code = $runExit
            return $result
        }
        Register-SpawnedPid -ProcessId $target.Id
        $verdict = Get-A24IntegrityVerdict -ProcessId $target.Id
        try { Stop-ScratchProcess -ProcessId $target.Id } catch { }
        $branch = if ($verdict.verified_non_high) { 'staged_non_high_integrity_confirmed' }
        elseif (-not $verdict.integrity_read_ok) { 'integrity_read_failed' }
        else { 'staged_but_still_high_integrity' }
        $result = [ordered]@{
            measurable       = $true
            branch           = $branch
            create_exit_code = $createExit
            run_exit_code    = $runExit
        }
        foreach ($k in $verdict.Keys) { $result[$k] = $verdict[$k] }
        $result.staged = [bool]$verdict.verified_non_high
        return $result
    } catch {
        return [ordered]@{
            measurable  = $true
            branch      = 'launch_command_failed'
            launch_error = Get-BoundedErrorText -Text $_.Exception.Message
            staged      = $false
        }
    } finally {
        try { & schtasks.exe '/Delete' '/TN' $taskName '/F' 2>&1 | Out-Null } catch { }
    }
}

# ---------------------------------------------------------------- route D: CreateRestrictedToken + CreateProcessAsUser

function Measure-RouteD-CreateRestrictedToken {
    Initialize-A24RestrictedTokenNative
    $leaf = 'adprobe-route-d'
    $exe = New-A24ScratchCopy -Leaf $leaf
    try {
        $commandLine = '"' + $exe + '" ' + (Get-A24TargetArgs -join ' ')
        $errorDetail = ''
        $foundPid = [AgentDesktopProbe.A24.RestrictedTokenLauncher]::LaunchWithRestrictedLowToken(
            $commandLine, $script:ScratchDir, [ref]$errorDetail)
        if ($foundPid -le 0) {
            return [ordered]@{
                measurable   = $true
                branch       = 'launch_command_failed'
                launch_error = Get-BoundedErrorText -Text $errorDetail
                staged       = $false
            }
        }
        Register-SpawnedPid -ProcessId $foundPid
        $verdict = Get-A24IntegrityVerdict -ProcessId $foundPid
        try { Stop-ScratchProcess -ProcessId $foundPid } catch { }
        $branch = if ($verdict.verified_non_high) { 'staged_non_high_integrity_confirmed' }
        elseif (-not $verdict.integrity_read_ok) { 'integrity_read_failed' }
        else { 'staged_but_still_high_integrity' }
        $result = [ordered]@{ measurable = $true; branch = $branch }
        foreach ($k in $verdict.Keys) { $result[$k] = $verdict[$k] }
        $result.staged = [bool]$verdict.verified_non_high
        return $result
    } catch {
        return [ordered]@{
            measurable  = $true
            branch      = 'launch_command_failed'
            launch_error = Get-BoundedErrorText -Text $_.Exception.Message
            staged      = $false
        }
    }
}

# ---------------------------------------------------------------- orchestration

function Get-A24Environment {
    $selfSid = [AgentDesktopProbe.Native]::GetIntegritySid($PID)
    $seclogon = $null
    $schedule = $null
    try { $seclogon = (Get-Service -Name 'Seclogon' -ErrorAction Stop).Status.ToString() } catch { $seclogon = 'unknown' }
    try { $schedule = (Get-Service -Name 'Schedule' -ErrorAction Stop).Status.ToString() } catch { $schedule = 'unknown' }
    return [ordered]@{
        self_integrity_sid   = $selfSid
        self_integrity_rid   = Get-IntegrityRidForSid -Sid $selfSid
        self_integrity_label = Get-IntegrityLabelForSid -Sid $selfSid
        seclogon_service     = $seclogon
        schedule_service     = $schedule
    }
}

function Measure-SplitIntegrityStaging {
    $environment = Get-A24Environment
    $routes = [ordered]@{
        route_a_runas_trustlevel        = Measure-RouteA-RunasTrustlevel
        route_b_icacls_mandatory_label  = Measure-RouteB-IcaclsLabel
        route_c_scheduled_task_limited  = Measure-RouteC-ScheduledTaskLimited
        route_d_createrestrictedtoken   = Measure-RouteD-CreateRestrictedToken
    }
    $stagedBy = New-Object System.Collections.ArrayList
    $attempted = 0
    $succeeded = 0
    foreach ($name in $routes.Keys) {
        $r = $routes[$name]
        if ($r.branch -ne 'route_prerequisite_missing') { $attempted++ }
        if ($r.staged) { $succeeded++; [void]$stagedBy.Add($name) }
    }
    $staged = ($succeeded -gt 0)
    $reason = if ($staged) {
        'at least one route produced a live process independently confirmed below High integrity by GetTokenInformation(TokenIntegrityLevel); split-integrity verification is deliverable on this box without a dedicated rig'
    } else {
        'no route produced a live process independently confirmed below High integrity; split-integrity verification stays genuinely blocked on this box'
    }
    return [ordered]@{
        probe       = '24-fixture-e2e/02-split-integrity-staging'
        question    = 'can a genuinely split-integrity boundary (a live process independently confirmed below High integrity) be staged on this box, and by which of four independent routes'
        methodology = 'every route targets a renamed copy of cmd.exe running a harmless multi-second no-op (ping -n 6 127.0.0.1); success is never taken from the launcher''s own reported exit code - the process, once observed, has its token integrity level read back via GetTokenInformation(TokenIntegrityLevel) and classified by RID before being counted as staged'
        environment = $environment
        routes      = $routes
        verdict     = [ordered]@{
            staged           = $staged
            staged_by        = @($stagedBy)
            routes_attempted = $attempted
            routes_total     = $routes.Count
            routes_succeeded = $succeeded
            reason           = $reason
        }
    }
}

try {
    $measurement = Measure-SplitIntegrityStaging
    $script:CapturePath = Write-A24Capture -Name "split-integrity-staging-$Label.json" -Content (ConvertTo-Json -InputObject $measurement -Depth 12)
    Register-MandatoryPass -Capture $script:CapturePath -Result $measurement
} finally {
    Stop-AllSpawned
    foreach ($taskName in @($script:ScratchTasks)) {
        try { & schtasks.exe '/Delete' '/TN' $taskName '/F' 2>&1 | Out-Null } catch { }
    }
    foreach ($file in @($script:ScratchFiles)) {
        try { Remove-Item -LiteralPath $file -Force -ErrorAction SilentlyContinue } catch { }
    }
    if ($script:ScratchDir -and (Test-Path -LiteralPath $script:ScratchDir)) {
        try { Remove-Item -LiteralPath $script:ScratchDir -Recurse -Force -ErrorAction SilentlyContinue } catch { }
    }
}

Assert-MandatoryMeasurement -Probe '24-fixture-e2e/02-split-integrity-staging' -Label $Label

Write-ProbeResult -Probe '24-fixture-e2e/02-split-integrity-staging' -Status 'ok' -Message 'split-integrity staging routes measured' -Data @{
    capture = if ($script:CapturePath) { Split-Path -Leaf $script:CapturePath } else { '<none>' }
    staged  = [bool]$measurement.verdict.staged
}
exit 0
