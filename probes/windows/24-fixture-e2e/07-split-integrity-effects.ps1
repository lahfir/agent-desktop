#Requires -Version 5.1
<#
.SYNOPSIS
    Split-integrity live-effects probe (area 24, sub-phase 2.12, U11).

.DESCRIPTION
    A24-4 (02-split-integrity-staging.ps1) measured that this box can stage a
    genuine Medium-integrity process (CreateRestrictedToken + SetTokenInformation,
    confirmed by token read-back). This probe reconfirms that staging and then
    takes the two measurements U11's plan text assigns to a probe row rather
    than to a CI-gated leg:

      - the mid-walk owning-process identity read rate at Medium integrity,
        over a stated iteration count (`§2.12`'s scope bullet: the restricted-
        token observer this rig is the first environment able to stage);
      - the never-identifiable-window population, bounded at Medium, over a
        stated iteration count (`A23-9`'s 0 was a floor taken at High
        integrity only).

    Both are read directly with raw Win32 calls (EnumWindows,
    GetWindowThreadProcessId, OpenProcess, OpenProcessToken,
    GetTokenInformation(TokenUser)) from INSIDE a Medium-integrity process
    this probe stages - never through the product - because the question is
    what a Medium-integrity *caller* can resolve on this desktop, not what
    the product's own adapter chooses to report.

    Staging and the live-effect legs (read/write/activation/capture) route
    through `tests/e2e-windows/StagedProcess.psm1`'s already-proven
    `Start-StagedIntegrityProcess` rather than a self-contained P/Invoke
    block: that module implements the one launcher shape that actually works
    on PowerShell 5.1 (CreateProcessAsUser wrapped inside a compiled C#
    method - PowerShell's own direct P/Invoke of a `ref STARTUPINFOW`
    reliably fails with ERROR_PATH_NOT_FOUND, measured live while building
    it) and the lease-inheriting spawn discipline U11's harness scenario
    needs. Reusing the tested module here is a deliberate exception to this
    corpus's usual self-containment: re-deriving that launcher a second time
    under this unit's time budget would risk shipping the same class of bug
    with no time left to invert-verify it.

    Captures under captures\ as split-integrity-effects-{devbox,ci}.json
    (+ .normalized twin). Corpus safety: shapes and counts only - route
    names, booleans, integrity SIDs/RIDs, small integers (iteration/window
    counts) and bounded/redacted error strings. No window titles, file
    paths, pids, machine names, user names or message text ever reach
    ConvertTo-Json.
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'ci')][string]$Label = 'devbox',
    [int]$RaceIterations = 30,
    [int]$NeverIdentifiableIterations = 15
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

. (Join-Path (Split-Path -Parent $PSCommandPath) '..\common.ps1')
Initialize-ProbeRedaction
Initialize-ProbeNative

$repoRoot = Split-Path -Parent (Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $PSCommandPath)))
$e2eWindowsDir = Join-Path $repoRoot 'tests\e2e-windows'
Import-Module (Join-Path $e2eWindowsDir 'NativeTypes.psm1') -Force
Import-Module (Join-Path $e2eWindowsDir 'Native.psm1') -Force
Import-Module (Join-Path $e2eWindowsDir 'NativeToken.psm1') -Force
Import-Module (Join-Path $e2eWindowsDir 'StagedProcess.psm1') -Force

$script:ProbeDir = Split-Path -Parent $PSCommandPath
$script:CaptureDir = Join-Path $script:ProbeDir 'captures'
if (-not (Test-Path -LiteralPath $script:CaptureDir)) {
    New-Item -ItemType Directory -Path $script:CaptureDir -Force | Out-Null
}

Register-MandatoryCapture -Name @("split-integrity-effects-$Label.json")

function Write-A24Capture {
    param([Parameter(Mandatory = $true)][string]$Name, [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Content)
    $redacted = Protect-ProbeText -Text $Content
    $path = Join-Path $script:CaptureDir $Name
    $utf8NoBom = New-Object System.Text.UTF8Encoding $false
    [IO.File]::WriteAllText($path, $redacted, $utf8NoBom)
    $normalized = Get-NormalizedCapture -Text $redacted
    [IO.File]::WriteAllText(($path + '.normalized'), $normalized, $utf8NoBom)
    if (-not (Test-CaptureRedaction -Path $path)) { throw "redaction residue in $path" }
    return $path
}

function Get-RaceProbeExePath {
    <# Compiles once: a Medium-staged introspection exe that walks
       EnumWindows for N iterations, reading each window's owning-process
       identity the same way window_ops.rs's own listing does
       (GetWindowThreadProcessId, then OpenProcess + OpenProcessToken +
       GetTokenInformation(TokenUser)) - counting per-iteration failures as
       race/unidentifiable hits, and separately tracking which window
       handles NEVER resolve across every iteration for the
       never-identifiable-population reading. One exe, two readings, since
       both are the same underlying walk at different aggregations. #>
    $work = Join-Path $env:TEMP ('agent-desktop-a24-race-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $work -Force | Out-Null
    $csPath = Join-Path $work 'RaceProbe.cs'
    $exePath = Join-Path $work 'RaceProbe.exe'
    $source = @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

namespace AgentDesktopE2ERaceProbe {
    public static class Program {
        public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
        [DllImport("user32.dll")]
        public static extern bool EnumWindows(EnumWindowsProc proc, IntPtr lParam);
        [DllImport("user32.dll")]
        public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
        [DllImport("kernel32.dll")]
        public static extern IntPtr OpenProcess(uint access, bool inherit, uint pid);
        [DllImport("kernel32.dll")]
        public static extern bool CloseHandle(IntPtr h);
        [DllImport("advapi32.dll", SetLastError = true)]
        public static extern bool OpenProcessToken(IntPtr h, uint access, out IntPtr token);
        [DllImport("advapi32.dll", SetLastError = true)]
        public static extern bool GetTokenInformation(IntPtr token, int cls, IntPtr buf, int len, out int retLen);

        public static bool TryReadIdentity(IntPtr hWnd) {
            uint pid;
            GetWindowThreadProcessId(hWnd, out pid);
            if (pid == 0) { return false; }
            IntPtr hProc = OpenProcess(0x1000, false, pid);
            if (hProc == IntPtr.Zero) { return false; }
            try {
                IntPtr hTok;
                if (!OpenProcessToken(hProc, 0x0008, out hTok)) { return false; }
                try {
                    int len;
                    GetTokenInformation(hTok, 1, IntPtr.Zero, 0, out len);
                    return len > 0;
                } finally { CloseHandle(hTok); }
            } finally { CloseHandle(hProc); }
        }

        public static int Main(string[] args) {
            int raceIterations = int.Parse(args[0]);
            int neverIdIterations = int.Parse(args[1]);

            int windowsExaminedTotal = 0;
            int raceHitsTotal = 0;
            int iterationsWithRace = 0;
            for (int i = 0; i < raceIterations; i++) {
                int examined = 0, hits = 0;
                EnumWindows(delegate (IntPtr hWnd, IntPtr lp) {
                    examined++;
                    if (!TryReadIdentity(hWnd)) { hits++; }
                    return true;
                }, IntPtr.Zero);
                windowsExaminedTotal += examined;
                raceHitsTotal += hits;
                if (hits > 0) { iterationsWithRace++; }
            }

            var everFailed = new Dictionary<long, int>();
            var everSeen = new HashSet<long>();
            int distinctWindowsSeen = 0;
            for (int i = 0; i < neverIdIterations; i++) {
                EnumWindows(delegate (IntPtr hWnd, IntPtr lp) {
                    long key = hWnd.ToInt64();
                    everSeen.Add(key);
                    if (!TryReadIdentity(hWnd)) {
                        int count;
                        everFailed.TryGetValue(key, out count);
                        everFailed[key] = count + 1;
                    }
                    return true;
                }, IntPtr.Zero);
            }
            distinctWindowsSeen = everSeen.Count;
            int persistentFailures = 0;
            int intermittentFailures = 0;
            foreach (var kv in everFailed) {
                if (kv.Value >= neverIdIterations) { persistentFailures++; }
                else { intermittentFailures++; }
            }

            Console.WriteLine("{" +
                "\"race_iterations\":" + raceIterations + "," +
                "\"windows_examined_total\":" + windowsExaminedTotal + "," +
                "\"race_hits_total\":" + raceHitsTotal + "," +
                "\"iterations_with_race\":" + iterationsWithRace + "," +
                "\"never_identifiable_iterations\":" + neverIdIterations + "," +
                "\"distinct_windows_seen\":" + distinctWindowsSeen + "," +
                "\"persistent_failures\":" + persistentFailures + "," +
                "\"intermittent_failures\":" + intermittentFailures +
                "}");
            return 0;
        }
    }
}
'@
    $utf8NoBom = New-Object System.Text.UTF8Encoding $false
    [System.IO.File]::WriteAllText($csPath, $source, $utf8NoBom)
    $csc = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
    if (-not (Test-Path -LiteralPath $csc)) { throw "Get-RaceProbeExePath: csc.exe not found at $csc" }
    $cscArgs = @('/nologo', '/target:exe', '/langversion:5', '/platform:x64', ('/out:' + $exePath), $csPath)
    $buildOutput = & $csc $cscArgs 2>&1 | ForEach-Object { "$_" }
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $exePath)) {
        throw "Get-RaceProbeExePath: csc build failed: $($buildOutput -join '; ')"
    }
    return $exePath
}

function Measure-SplitIntegrityStagingReconfirmed {
    $result = Start-StagedIntegrityProcess -IntegrityLevel Medium -ReportOwnHandles -TimeoutSeconds 15
    return [ordered]@{
        measurable                 = $true
        live_process_integrity_sid = $result.LiveProcessIntegritySid
        confirmed_non_high         = [bool]$result.IntegrityConfirmedNonHigh
        judged_by                  = 'live_token_readback_never_launcher_exit_code'
    }
}

function Measure-RaceAndNeverIdentifiable {
    param([Parameter(Mandatory = $true)][int]$RaceIterations, [Parameter(Mandatory = $true)][int]$NeverIdIterations)
    $exePath = Get-RaceProbeExePath
    $result = Start-StagedIntegrityProcess -IntegrityLevel Medium -FilePath $exePath `
        -ArgumentList @([string]$RaceIterations, [string]$NeverIdIterations) -TimeoutSeconds 60
    if (-not $result.IntegrityConfirmedNonHigh) {
        throw "Measure-RaceAndNeverIdentifiable: staged process was not confirmed Medium (sid=$($result.LiveProcessIntegritySid))"
    }
    if ($result.TimedOut -or [string]::IsNullOrWhiteSpace($result.StdOut)) {
        throw "Measure-RaceAndNeverIdentifiable: staged race-probe produced no result (TimedOut=$($result.TimedOut), ExitCode=$($result.ExitCode))"
    }
    $parsed = ConvertFrom-Json -InputObject $result.StdOut
    return [ordered]@{
        measurable                    = $true
        integrity_confirmed           = [bool]$result.IntegrityConfirmedNonHigh
        race_iterations                = $parsed.race_iterations
        windows_examined_total         = $parsed.windows_examined_total
        race_hits_total                = $parsed.race_hits_total
        iterations_with_race           = $parsed.iterations_with_race
        never_identifiable_iterations  = $parsed.never_identifiable_iterations
        distinct_windows_seen          = $parsed.distinct_windows_seen
        persistent_failures            = $parsed.persistent_failures
        intermittent_failures          = $parsed.intermittent_failures
        note                           = 'read via raw EnumWindows/OpenProcess/OpenProcessToken(TokenUser) from inside the staged Medium process itself, never through the product; the restricted-token/standard-user-logon gap is accepted per docs/phases.md Deferred to Follow-Up Work'
    }
}

function Measure-SplitIntegrityEffects {
    $staging = Measure-SplitIntegrityStagingReconfirmed
    if (-not $staging.confirmed_non_high) {
        return [ordered]@{
            probe   = '24-fixture-e2e/07-split-integrity-effects'
            staging = $staging
            verdict = [ordered]@{ staged = $false; reason = 'staging reconfirmation did not read back Medium; effects not attempted' }
        }
    }
    $raceAndNeverId = Measure-RaceAndNeverIdentifiable -RaceIterations $RaceIterations -NeverIdIterations $NeverIdentifiableIterations
    return [ordered]@{
        probe                       = '24-fixture-e2e/07-split-integrity-effects'
        question                    = 'the Medium-integrity mid-walk owning-process identity read rate and the never-identifiable-window population, both bounded at Medium rather than assumed from a High-integrity floor'
        staging                     = $staging
        race_rate_and_never_identifiable = $raceAndNeverId
        verdict                     = [ordered]@{ staged = $true }
    }
}

try {
    $measurement = Measure-SplitIntegrityEffects
    $script:CapturePath = Write-A24Capture -Name "split-integrity-effects-$Label.json" -Content (ConvertTo-Json -InputObject $measurement -Depth 12)
    Register-MandatoryPass -Capture $script:CapturePath -Result $measurement
} catch {
    Write-ProbeLog -Message "split-integrity-effects probe failed: $($_.Exception.Message)"
    throw
}

Assert-MandatoryMeasurement -Probe '24-fixture-e2e/07-split-integrity-effects' -Label $Label

Write-ProbeResult -Probe '24-fixture-e2e/07-split-integrity-effects' -Status 'ok' -Message 'split-integrity live effects measured' -Data @{
    capture = if ($script:CapturePath) { Split-Path -Leaf $script:CapturePath } else { '<none>' }
    staged  = [bool]$measurement.verdict.staged
}
exit 0
