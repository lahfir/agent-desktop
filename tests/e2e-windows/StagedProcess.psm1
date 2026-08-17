#Requires -Version 5.1

<#
    StagedProcess.psm1 - `Start-StagedIntegrityProcess`, the orchestration
    NativeToken.psm1's token primitives exist to serve: build a restricted,
    integrity-labelled token; spawn FilePath under it with stdio piped back
    and the desktop lease handle inherited exactly the way
    `Invoke-GuardedAgent` inherits it (opt-in immediately before the spawn,
    cleared immediately after); bound the child's inherited handle set to
    the lease plus its own stdio, because `bInheritHandles = TRUE` is
    all-or-nothing and this path crosses a privilege boundary
    `Invoke-GuardedAgent`'s .NET path never does; and judge the result by
    reading the live process's own token, never by `CreateProcessAsUser`'s
    return value.
#>

Set-StrictMode -Version 2.0

Import-Module (Join-Path $PSScriptRoot 'NativeTypes.psm1') -Force -Global
Import-Module (Join-Path $PSScriptRoot 'Native.psm1') -Force -Global
Import-Module (Join-Path $PSScriptRoot 'NativeToken.psm1') -Force -Global

$script:LeaseHandleEnvName = 'AGENT_DESKTOP_INTERACTION_LEASE_HANDLE'
$script:HandleReportExePath = $null
$script:TokenQuery = 0x0008
$script:CreateNoWindow = 0x08000000
$script:NormalPriorityClass = 0x00000020
$script:CreateUnicodeEnvironment = 0x400
$script:WaitTimeout = 0x102
$script:IntegritySidByLevel = @{ Medium = 'S-1-16-8192'; Low = 'S-1-16-4096' }

function Get-HandleReportExePath {
    <#
    .SYNOPSIS
        Compiles, once per host process, a small pre-built console app that
        a staged child runs instead of the CLI under test: it enumerates its
        OWN inherited handle set the same way `Get-NativeInheritableHandleValues`
        does and prints it as JSON. This is what step 3b-i's "the staged
        child enumerates its inherited set and reports it" needs - a
        product binary has no such introspection, so the leg stages this
        exe through the identical restricted-token, bounded-handle-set path
        every other leg uses. Pre-compiled rather than `Add-Type`-compiled
        inside the staged child itself: measured live, `CreateRestrictedToken
        (DISABLE_MAX_PRIVILEGE)` strips a privilege the in-process C#
        compiler needs, so a restricted child's own `Add-Type` call fails
        "A required privilege is not held by the client" - compiling once
        from this (unrestricted) process and running the resulting binary
        sidesteps that entirely.
    #>
    [CmdletBinding()]
    param()
    if ($script:HandleReportExePath -and (Test-Path -LiteralPath $script:HandleReportExePath)) {
        return $script:HandleReportExePath
    }
    $work = Join-Path $env:TEMP ('agent-desktop-e2e-handle-report-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $work -Force | Out-Null
    $csPath = Join-Path $work 'HandleReport.cs'
    $exePath = Join-Path $work 'HandleReport.exe'
    $source = @'
using System;
using System.Runtime.InteropServices;
namespace AgentDesktopE2EHandleReport {
    public static class Program {
        [DllImport("kernel32.dll")]
        public static extern IntPtr GetCurrentProcess();
        [DllImport("ntdll.dll")]
        public static extern int NtQueryInformationProcess(IntPtr h, int cls, IntPtr buf, int len, out int ret);

        public static int Main() {
            int size = 1 << 16;
            IntPtr buffer = IntPtr.Zero;
            try {
                for (int attempt = 0; attempt < 10; attempt++) {
                    buffer = Marshal.AllocHGlobal(size);
                    int returnLength;
                    int status = NtQueryInformationProcess(GetCurrentProcess(), 0x33, buffer, size, out returnLength);
                    if (status == 0) { break; }
                    Marshal.FreeHGlobal(buffer);
                    buffer = IntPtr.Zero;
                    if (status == unchecked((int)0xC0000004)) { size *= 2; continue; }
                    Console.Error.WriteLine("NtQueryInformationProcess failed: 0x" + status.ToString("X8"));
                    return 1;
                }
                long count = Marshal.ReadInt64(buffer, 0);
                var parts = new System.Collections.Generic.List<string>();
                for (long i = 0; i < count; i++) {
                    IntPtr entry = IntPtr.Add(buffer, 16 + (int)(i * 40));
                    IntPtr handleValue = Marshal.ReadIntPtr(entry, 0);
                    int attrs = Marshal.ReadInt32(entry, 32);
                    if ((attrs & 0x2) != 0) { parts.Add(handleValue.ToInt64().ToString()); }
                }
                Console.WriteLine("[" + string.Join(",", parts.ToArray()) + "]");
                return 0;
            } finally {
                if (buffer != IntPtr.Zero) { Marshal.FreeHGlobal(buffer); }
            }
        }
    }
}
'@
    $utf8NoBom = New-Object System.Text.UTF8Encoding $false
    [System.IO.File]::WriteAllText($csPath, $source, $utf8NoBom)
    $csc = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
    if (-not (Test-Path -LiteralPath $csc)) {
        throw "Get-HandleReportExePath: csc.exe not found at $csc"
    }
    $cscArgs = @('/nologo', '/target:exe', '/langversion:5', '/platform:x64', ('/out:' + $exePath), $csPath)
    $buildOutput = & $csc $cscArgs 2>&1 | ForEach-Object { "$_" }
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $exePath)) {
        throw "Get-HandleReportExePath: csc build failed: $($buildOutput -join '; ')"
    }
    $script:HandleReportExePath = $exePath
    return $exePath
}

function New-StagedEnvironmentBlock {
    <#
    .SYNOPSIS
        A double-null-terminated Unicode environment block (the shape
        `CreateProcessAsUser` + `CREATE_UNICODE_ENVIRONMENT` requires):
        every variable this process currently has, so the staged child
        resolves the same isolated HOME/TEMP the harness set up, plus
        `AGENT_DESKTOP_INTERACTION_LEASE_HANDLE` when LeaseHandleValue is
        given - the one variable `crate::system::interaction_lease::
        adopt_inherited` reads to choose adoption over self-acquire.
    .OUTPUTS
        IntPtr: caller must `Marshal.FreeHGlobal` it.
    #>
    [CmdletBinding()]
    param([Nullable[long]]$LeaseHandleValue)
    $table = @{}
    foreach ($entry in [System.Environment]::GetEnvironmentVariables().GetEnumerator()) {
        $table[[string]$entry.Key] = [string]$entry.Value
    }
    if ($null -ne $LeaseHandleValue) {
        $table[$script:LeaseHandleEnvName] = [string]$LeaseHandleValue
    } else {
        $table.Remove($script:LeaseHandleEnvName)
    }
    $lines = New-Object System.Collections.Generic.List[string]
    foreach ($key in $table.Keys) { $lines.Add("$key=$($table[$key])") }
    $blockText = ($lines -join "`0") + "`0`0"
    return [System.Runtime.InteropServices.Marshal]::StringToHGlobalUni($blockText)
}

function ConvertTo-Win32Arg {
    <#
    .SYNOPSIS
        The standard Win32 command-line quoting algorithm, duplicated here
        rather than imported: `BoundedProcess.psm1`'s own copy is
        module-private (not on its `Export-ModuleMember` list), the same
        reason `probes/windows/24-fixture-e2e/02-split-integrity-staging.ps1`
        carries an independent copy rather than reaching across the
        probes/tests boundary.
    #>
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Value)
    if ($Value.Length -gt 0 -and $Value -notmatch '[\s"]') { return $Value }
    $builder = New-Object System.Text.StringBuilder
    [void]$builder.Append('"')
    $backslashes = 0
    foreach ($char in $Value.ToCharArray()) {
        if ($char -eq '\') { $backslashes++; continue }
        if ($char -eq '"') {
            [void]$builder.Append('\', ($backslashes * 2 + 1))
            [void]$builder.Append('"')
            $backslashes = 0
            continue
        }
        if ($backslashes -gt 0) { [void]$builder.Append('\', $backslashes); $backslashes = 0 }
        [void]$builder.Append($char)
    }
    if ($backslashes -gt 0) { [void]$builder.Append('\', ($backslashes * 2)) }
    [void]$builder.Append('"')
    return $builder.ToString()
}

function ConvertTo-Win32CommandLine {
    param([string[]]$Tokens)
    if (-not $Tokens -or $Tokens.Count -eq 0) { return '' }
    return (($Tokens | ForEach-Object { ConvertTo-Win32Arg -Value $_ }) -join ' ')
}

function Start-StagedIntegrityProcess {
    <#
    .SYNOPSIS
        Spawns FilePath under a `New-RestrictedIntegrityToken` at
        IntegrityLevel, with stdio piped back through inheritable anonymous
        pipes and, when InheritLeaseHandle is given, the desktop lease
        handle marked inheritable for exactly this one spawn - mirroring
        `Invoke-GuardedAgent`'s discipline (KTD4). Every OTHER currently-
        inheritable handle in this process is turned non-inheritable for
        the duration of the spawn and restored immediately after, because
        `bInheritHandles = TRUE` is all-or-nothing and this path crosses a
        privilege boundary the .NET path never does (step 3b-i). Pass
        `-ReportOwnHandles` instead of `-FilePath` to stage
        `Get-HandleReportExePath`'s introspection binary rather than a
        real command - the counterpart the inherited-handle-set leg needs.

        Staging is judged by reading the live spawned process's own primary
        token (`OpenProcessToken` against the handle `CreateProcessAsUser`
        itself returned), immediately, before anything else runs - never by
        `CreateProcessAsUser`'s own boolean return (A24-4 route A: a
        launcher that reports success while the launched process stays
        High).
    .OUTPUTS
        PSCustomObject: ProcessId, ExitCode, StdOut, StdErr, TimedOut,
        RestrictedTokenIntegritySid, LiveProcessIntegritySid,
        LiveProcessIntegrityReadOk, IntegrityConfirmedNonHigh,
        HarnessInheritableHandlesBeforeSpawn, ExpectedInheritableHandles,
        BoundedExtraHandleCount.
    #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][ValidateSet('Medium', 'Low')][string]$IntegrityLevel,
        [string]$FilePath,
        [string[]]$ArgumentList = @(),
        [switch]$ReportOwnHandles,
        [Nullable[long]]$InheritLeaseHandle,
        [int]$TimeoutSeconds = 20,
        [string]$WorkingDirectory
    )
    Initialize-E2ENative
    if (-not $ReportOwnHandles -and -not $FilePath) {
        throw 'Start-StagedIntegrityProcess: pass -FilePath or -ReportOwnHandles'
    }

    $effectiveFilePath = $FilePath
    $effectiveArgs = $ArgumentList
    if ($ReportOwnHandles) {
        $effectiveFilePath = Get-HandleReportExePath
        $effectiveArgs = @()
    }
    if (-not $WorkingDirectory) { $WorkingDirectory = Split-Path -Parent $effectiveFilePath }

    $stdoutPipe = New-Object System.IO.Pipes.AnonymousPipeServerStream('In', [System.IO.HandleInheritability]::Inheritable)
    $stderrPipe = New-Object System.IO.Pipes.AnonymousPipeServerStream('In', [System.IO.HandleInheritability]::Inheritable)
    $stdoutReader = New-Object System.IO.StreamReader($stdoutPipe)
    $stderrReader = New-Object System.IO.StreamReader($stderrPipe)
    $genericRead = [uint32]2147483648
    $stdin = Open-NativeFile -Path 'NUL' -DesiredAccess $genericRead -ShareMode 7 -CreationDisposition 3 -Inheritable
    if (-not $stdin.Success) {
        throw "Start-StagedIntegrityProcess: could not open NUL for stdin: Win32 error $($stdin.Win32Error)"
    }

    $expected = New-Object System.Collections.Generic.List[long]
    $expected.Add([long]$stdoutPipe.ClientSafePipeHandle.DangerousGetHandle())
    $expected.Add([long]$stderrPipe.ClientSafePipeHandle.DangerousGetHandle())
    $expected.Add([long]$stdin.Handle)
    $hasLease = ($PSBoundParameters.ContainsKey('InheritLeaseHandle') -and $null -ne $InheritLeaseHandle)
    if ($hasLease) {
        Set-NativeHandleInheritable -Handle ([IntPtr]$InheritLeaseHandle) -Enabled $true
        $expected.Add([long]$InheritLeaseHandle)
    }

    $before = @(Get-NativeInheritableHandleValues)
    $extra = @($before | Where-Object { -not ($expected -contains $_) })
    foreach ($handle in $extra) {
        try { Set-NativeHandleInheritable -Handle ([IntPtr]$handle) -Enabled $false } catch { }
    }

    $envPtr = New-StagedEnvironmentBlock -LeaseHandleValue $InheritLeaseHandle
    $restrictedToken = [IntPtr]::Zero
    $processHandle = [IntPtr]::Zero
    $threadHandle = [IntPtr]::Zero
    try {
        $restrictedToken = New-RestrictedIntegrityToken -IntegrityLevel $IntegrityLevel
        $restrictedTokenIntegritySid = Get-NativeTokenIntegritySid -TokenHandle $restrictedToken

        <# Routed through SpawnUnderToken (NativeTypes.psm1), never through
           CreateProcessAsUser directly: measured live, PowerShell's own
           P/Invoke marshaling of a `ref STARTUPINFOW` - non-blittable, since
           it embeds string fields - reliably fails with ERROR_PATH_NOT_FOUND
           even with every field set correctly, while the identical call from
           inside a compiled method succeeds. #>
        $commandLine = ConvertTo-Win32CommandLine -Tokens (@($effectiveFilePath) + $effectiveArgs)
        $flags = $script:CreateNoWindow -bor $script:NormalPriorityClass -bor $script:CreateUnicodeEnvironment
        $spawnErrorDetail = ''
        $spawnedProcessId = [AgentDesktopE2E.Native]::SpawnUnderToken(
            $restrictedToken, $commandLine, $WorkingDirectory,
            $stdin.Handle, $stdoutPipe.ClientSafePipeHandle.DangerousGetHandle(), $stderrPipe.ClientSafePipeHandle.DangerousGetHandle(),
            $envPtr, $true, $flags, [ref]$processHandle, [ref]$threadHandle, [ref]$spawnErrorDetail)
        if ($spawnedProcessId -le 0) {
            throw "Start-StagedIntegrityProcess: $spawnErrorDetail"
        }

        $liveTokenHandle = [IntPtr]::Zero
        $liveOk = [AgentDesktopE2E.Native]::OpenProcessToken($processHandle, $script:TokenQuery, [ref]$liveTokenHandle)
        $liveIntegritySid = $null
        if ($liveOk) {
            try { $liveIntegritySid = Get-NativeTokenIntegritySid -TokenHandle $liveTokenHandle } finally { Close-NativeHandle -Handle $liveTokenHandle }
        }

        foreach ($handle in $extra) {
            try { Set-NativeHandleInheritable -Handle ([IntPtr]$handle) -Enabled $true } catch { }
        }
        if ($hasLease) {
            Set-NativeHandleInheritable -Handle ([IntPtr]$InheritLeaseHandle) -Enabled $false
        }

        $stdoutPipe.DisposeLocalCopyOfClientHandle()
        $stderrPipe.DisposeLocalCopyOfClientHandle()
        Close-NativeHandle -Handle $stdin.Handle

        $stdoutTask = $stdoutReader.ReadToEndAsync()
        $stderrTask = $stderrReader.ReadToEndAsync()

        $waitResult = [AgentDesktopE2E.Native]::WaitForSingleObject($processHandle, [uint32]($TimeoutSeconds * 1000))
        $timedOut = ($waitResult -eq $script:WaitTimeout)
        if ($timedOut) {
            [void][AgentDesktopE2E.Native]::TerminateProcess($processHandle, 1)
            [void][AgentDesktopE2E.Native]::WaitForSingleObject($processHandle, 5000)
        }
        [void][System.Threading.Tasks.Task]::WaitAll(@($stdoutTask, $stderrTask), 5000)

        $exitCode = 0
        $exitOk = [AgentDesktopE2E.Native]::GetExitCodeProcess($processHandle, [ref]$exitCode)

        $childReportedHandles = $null
        if ($ReportOwnHandles) {
            <# ConvertFrom-Json returns its whole result as ONE pipeline
               object; wrapping that call in @(...) then collects it as a
               single-element array whose one element is the real array -
               measured live, this silently defeated every leak comparison
               downstream (a real leaked handle would never be found
               "-contains" a 1-element array of arrays). The parsed value is
               used as-is; a scalar JSON result (an empty array or one bare
               number) is still normalized to an array with ,@(). #>
            try {
                $parsed = ConvertFrom-Json -InputObject $stdoutTask.Result
                if ($null -eq $parsed) { $childReportedHandles = @() }
                elseif ($parsed -is [array]) { $childReportedHandles = $parsed }
                else { $childReportedHandles = , $parsed }
            } catch { $childReportedHandles = @() }
        }

        return [pscustomobject]@{
            ProcessId                            = $spawnedProcessId
            ExitCode                             = if ($exitOk) { [int]$exitCode } else { $null }
            StdOut                                = $stdoutTask.Result
            StdErr                                = $stderrTask.Result
            TimedOut                              = [bool]$timedOut
            RestrictedTokenIntegritySid           = $restrictedTokenIntegritySid
            LiveProcessIntegritySid               = $liveIntegritySid
            LiveProcessIntegrityReadOk            = [bool]$liveOk
            IntegrityConfirmedNonHigh             = [bool]($liveIntegritySid -and $liveIntegritySid -eq $script:IntegritySidByLevel[$IntegrityLevel])
            HarnessInheritableHandlesBeforeSpawn  = @($before)
            ExpectedInheritableHandles            = @($expected)
            ChildReportedInheritableHandles       = $childReportedHandles
            BoundedExtraHandleCount               = @($extra).Count
        }
    } finally {
        foreach ($handle in $extra) {
            try { Set-NativeHandleInheritable -Handle ([IntPtr]$handle) -Enabled $true } catch { }
        }
        if ($hasLease) {
            try { Set-NativeHandleInheritable -Handle ([IntPtr]$InheritLeaseHandle) -Enabled $false } catch { }
        }
        if ($threadHandle -ne [IntPtr]::Zero) { Close-NativeHandle -Handle $threadHandle }
        if ($processHandle -ne [IntPtr]::Zero) { Close-NativeHandle -Handle $processHandle }
        if ($restrictedToken -ne [IntPtr]::Zero) { Close-NativeHandle -Handle $restrictedToken }
        [System.Runtime.InteropServices.Marshal]::FreeHGlobal($envPtr)
        $stdoutReader.Dispose()
        $stderrReader.Dispose()
    }
}

Export-ModuleMember -Function @('Get-HandleReportExePath', 'New-StagedEnvironmentBlock', 'Start-StagedIntegrityProcess')
