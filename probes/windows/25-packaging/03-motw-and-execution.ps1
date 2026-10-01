#Requires -Version 5.1
<#
.SYNOPSIS
    Mark-of-the-Web and execution-behavior probe (area 25, sub-phase 2.13 U1).

.DESCRIPTION
    Measures TWO download/execution cases, because one of them cannot observe
    what KTD5 needs:

      Case a (row A25-5, scoped to the npm path): postinstall.js downloads
      through curl.exe, which writes with plain file I/O. A file fetched that
      way carries no Zone.Identifier alternate data stream, so no SmartScreen
      or Attachment Manager prompt is possible on the npm install path
      regardless of any other control. The transfer here is a real HTTP fetch
      from a probe-owned loopback listener, verified by byte count and SHA-256
      rather than by curl's own success output.

      Case b (row A25-8): a browser-equivalent download DOES carry the mark,
      so a stand-in executable built with the in-box compiler is written with
      a Zone.Identifier stream of ZoneId=3 - the same stream a browser writes -
      and its behavior is recorded under both launch modes KTD5's README claim
      distinguishes. The command-line arm invokes the executable directly,
      which is how npm's shim and every real use of this CLI starts the
      binary. The shell/GUI arm launches through ShellExecute - the Explorer
      double-click path - from a separate bounded launcher process whose
      progress is observed, never awaited unboundedly: if the shell path is
      gated by an interactive prompt, the launcher process blocks inside
      ShellExecute itself, and this probe records that gate as observed fact
      (a visible dialog window belonging to the launcher) rather than hanging
      on it or dismissing it.

      Host control surface, recorded alongside both rows: this box is Windows
      Server 2019, where Defender SmartScreen is off by default and Smart App
      Control does not exist as an OS feature. A row measured where the
      controls cannot fire bounds nothing about a Windows 11 client, and the
      capture says so in place.

    Captures: motw-execution-{devbox,ci}.json (+ .normalized twin). Corpus
    safety: the stand-in executable is probe-owned scratch code that writes
    one probe-named marker file, prints one probe-named sentinel and exits;
    no window title beyond the fixture's own literal text reaches a capture,
    and no URL host beyond the loopback literal, no path, no pid and no user
    or machine identity is recorded.
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'ci')][string]$Label = 'devbox'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

. (Join-Path (Split-Path -Parent $PSCommandPath) '..\common.ps1')
Initialize-ProbeRedaction
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$script:Probe = '25-packaging-03-motw-and-execution'
$script:ProbeDir = Split-Path -Parent $PSCommandPath
$script:CaptureDir = Join-Path $script:ProbeDir 'captures'
if (-not (Test-Path -LiteralPath $script:CaptureDir)) {
    New-Item -ItemType Directory -Path $script:CaptureDir -Force | Out-Null
}
$script:WorkDir = $null
$script:Spawned = New-Object System.Collections.ArrayList

<#
    Visible top-level dialog-class (#32770) windows owned by one pid. The
    Attachment Manager's launch prompt hosts itself in the calling process,
    so counting these against the launcher answers "gated on an interactive
    prompt" behaviorally; only a count is ever returned, no title text.
#>
Add-ProbeInlineCSharp -Source @'
using System;
using System.Runtime.InteropServices;
using System.Text;

namespace AgentDesktopProbe25 {
    public static class WinDialogs {
        private delegate bool EnumProc(IntPtr hWnd, IntPtr lParam);
        [DllImport("user32.dll")]
        private static extern bool EnumWindows(EnumProc callback, IntPtr lParam);
        [DllImport("user32.dll")]
        private static extern bool IsWindowVisible(IntPtr hWnd);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        private static extern int GetClassName(IntPtr hWnd, StringBuilder buffer, int maxCount);
        [DllImport("user32.dll")]
        private static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);

        public static int VisibleDialogWindowsForPid(int pid) {
            int found = 0;
            EnumProc callback = delegate(IntPtr hWnd, IntPtr lParam) {
                uint owner = 0;
                GetWindowThreadProcessId(hWnd, out owner);
                if ((int)owner == pid && IsWindowVisible(hWnd)) {
                    StringBuilder sb = new StringBuilder(256);
                    GetClassName(hWnd, sb, 256);
                    if (sb.ToString() == "#32770") { found++; }
                }
                return true;
            };
            EnumWindows(callback, IntPtr.Zero);
            return found;
        }
    }
}
'@ -AssemblyLeaf 'AgentDesktopProbe25WinDialogs'

Register-MandatoryCapture -Name @("motw-execution-$Label.json")

function Write-A25Capture {
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
    if ($ProcessId -gt 0 -and -not $script:Spawned.Contains($ProcessId)) { [void]$script:Spawned.Add($ProcessId) }
}

function Stop-AllSpawned {
    foreach ($id in @($script:Spawned)) {
        try { Stop-ScratchProcess -ProcessId $id } catch { }
    }
    $script:Spawned.Clear()
}

<#
    Structured CS#### extraction instead of verbatim compiler output - the
    same rule area 24's toolchain probe applies: a per-file diagnostic embeds
    the full source path, which carries the operator profile directory.
#>
function Get-DiagnosticCodes {
    param([string[]]$Lines)
    $codes = New-Object System.Collections.ArrayList
    foreach ($line in $Lines) {
        foreach ($m in [regex]::Matches([string]$line, 'CS\d{4}')) {
            if (-not $codes.Contains($m.Value)) { [void]$codes.Add($m.Value) }
        }
    }
    return @($codes)
}

function Get-ByteSha256 {
    param([Parameter(Mandatory = $true)][byte[]]$Bytes)
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        return ([BitConverter]::ToString($sha.ComputeHash($Bytes))).Replace('-', '').ToLowerInvariant()
    } finally {
        $sha.Dispose()
    }
}

function Get-FileSha256 {
    param([Parameter(Mandatory = $true)][string]$Path)
    return (Get-ByteSha256 -Bytes ([IO.File]::ReadAllBytes($Path)))
}

<#
    Serves exactly one HTTP GET response over a raw loopback TCP connection:
    a deterministic payload with an honest Content-Length, so the curl leg is
    a genuine HTTP fetch through curl.exe's own network write path while
    staying offline-deterministic and free of any external host name.
#>
function Invoke-LoopbackFetchByCurl {
    param(
        [Parameter(Mandatory = $true)][string]$CurlExe,
        [Parameter(Mandatory = $true)][byte[]]$Payload,
        [Parameter(Mandatory = $true)][string]$OutPath
    )
    $listener = New-Object System.Net.Sockets.TcpListener ([System.Net.IPAddress]::Loopback, 0)
    $listener.Start()
    $port = ([System.Net.IPEndPoint]$listener.LocalEndpoint).Port
    $curlProc = $null
    try {
        $curlProc = Start-Process -FilePath $CurlExe -ArgumentList @('-sS', '-o', $OutPath, ('http://127.0.0.1:' + $port + '/probe25.bin')) -PassThru -WindowStyle Hidden
        Register-SpawnedPid -ProcessId $curlProc.Id
        $client = $listener.AcceptTcpClient()
        try {
            $stream = $client.GetStream()
            $buffer = New-Object byte[] 4096
            $requestText = ''
            $deadline = (Get-Date).AddSeconds(10)
            while ((Get-Date) -lt $deadline -and -not $requestText.Contains("`r`n`r`n")) {
                if ($stream.DataAvailable) {
                    $read = $stream.Read($buffer, 0, $buffer.Length)
                    if ($read -le 0) { break }
                    $requestText += [System.Text.Encoding]::ASCII.GetString($buffer, 0, $read)
                } else {
                    Start-Sleep -Milliseconds 20
                }
            }
            $header = 'HTTP/1.1 200 OK' + "`r`n" + ('Content-Length: ' + $payload.Length) + "`r`n" + 'Connection: close' + "`r`n`r`n"
            $headerBytes = [System.Text.Encoding]::ASCII.GetBytes($header)
            $stream.Write($headerBytes, 0, $headerBytes.Length)
            $stream.Write($payload, 0, $payload.Length)
            $stream.Flush()
            Start-Sleep -Milliseconds 150
        } finally {
            $client.Close()
        }
        $null = $curlProc.WaitForExit(20000)
        $exitCode = $null
        if ($curlProc.HasExited) { $exitCode = $curlProc.ExitCode }
        return $exitCode
    } finally {
        $listener.Stop()
        if ($null -ne $curlProc -and -not $curlProc.HasExited) {
            try { Stop-ScratchProcess -ProcessId $curlProc.Id } catch { }
        }
    }
}

function Test-ZoneIdentifierPresent {
    param([Parameter(Mandatory = $true)][string]$Path)
    $streams = @(Get-Item -LiteralPath $Path -Stream * -ErrorAction SilentlyContinue)
    $zone = @($streams | Where-Object { $_.Stream -eq 'Zone.Identifier' })
    return @{
        present       = ($zone.Count -gt 0)
        streams_count = $streams.Count
    }
}

function Get-HostControlSurface {
    $currentVersion = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
    $ubr = 0
    if ($currentVersion.PSObject.Properties['UBR']) { $ubr = [int]$currentVersion.UBR }
    $smartScreenValuePresent = $false
    $smartScreenStateClass = 'unknown'
    $explorerKey = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer' -ErrorAction SilentlyContinue
    if ($null -ne $explorerKey -and $explorerKey.PSObject.Properties['SmartScreenEnabled']) {
        $smartScreenValuePresent = $true
        $raw = [string]$explorerKey.SmartScreenEnabled
        if ($raw -ieq 'off') {
            $smartScreenStateClass = 'off_by_value'
        } elseif ($raw) {
            $smartScreenStateClass = 'set_enforcing_variant'
        } else {
            $smartScreenStateClass = 'value_present_empty'
        }
    } else {
        $smartScreenStateClass = 'value_absent_server_default_off'
    }
    $sacValuePresent = $false
    $ciPolicy = Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\CI\Policy' -ErrorAction SilentlyContinue
    if ($null -ne $ciPolicy -and $ciPolicy.PSObject.Properties['VerifiedAndReputablePolicyState']) {
        $sacValuePresent = $true
    }
    return [ordered]@{
        product_name                           = [string]$currentVersion.ProductName
        build                                  = [int]$currentVersion.CurrentBuildNumber
        ubr                                    = $ubr
        smart_screen_value_present             = $smartScreenValuePresent
        smart_screen_state_class               = $smartScreenStateClass
        smart_app_control_policy_value_present = $sacValuePresent
        smart_app_control_available_on_host    = $sacValuePresent
        bound_note                             = 'Server 2019 host: SmartScreen off by default and Smart App Control does not exist as an OS feature here; rows measured where the controls cannot fire bound nothing about a Windows 11 client'
    }
}

<#
    Root-level children of one pid through the managed UIA client every other
    probe in this corpus uses - here it answers whether the blocked launcher
    is hosting a visible dialog rather than merely being slow, without
    recording any title text.
#>
function Get-LauncherChildWindowFacts {
    param([Parameter(Mandatory = $true)][int]$ProcessId)
    $root = [System.Windows.Automation.AutomationElement]::RootElement
    $cond = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ProcessIdProperty, $ProcessId)
    $children = $null
    try { $children = $root.FindAll([System.Windows.Automation.TreeScope]::Children, $cond) } catch { }
    $count = 0
    $dialogPresent = $false
    if ($null -ne $children) {
        $count = $children.Count
        foreach ($c in $children) {
            try {
                if ($c.Current.ControlType -eq [System.Windows.Automation.ControlType]::Dialog) { $dialogPresent = $true }
            } catch { }
        }
    }
    return @{ child_windows = $count; dialog_present = $dialogPresent }
}

<#
    One bounded ShellExecute launch arm. The launch happens in a separate
    launcher process so this probe is never the process blocked inside
    ShellExecuteEx: while the launcher has not yet recorded a launched pid,
    its visible dialog-class window count answers whether it is hosting an
    interactive prompt. SEE_MASK_NOZONECHECKS in the launcher's own
    environment is Microsoft's documented Attachment Manager bypass and is
    what attributes a gate to the zone check rather than to anything else.
#>
function Invoke-ShellLaunchArm {
    param(
        [Parameter(Mandatory = $true)][string]$InnerScript,
        [Parameter(Mandatory = $true)][string]$TargetPath,
        [Parameter(Mandatory = $true)][string]$PidFile,
        [Parameter(Mandatory = $true)][string]$MarkerPath,
        [switch]$BypassZoneCheck
    )
    Remove-Item -LiteralPath $PidFile -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $MarkerPath -Force -ErrorAction SilentlyContinue

    $launcherArgs = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $InnerScript, $TargetPath, $PidFile)
    if ($BypassZoneCheck) { $launcherArgs += '-Bypass' }
    $launcher = Start-Process -FilePath 'powershell.exe' -ArgumentList $launcherArgs -PassThru -WindowStyle Hidden
    Register-SpawnedPid -ProcessId $launcher.Id

    $record = [ordered]@{
        observation                = 'observation_timeout'
        class_32770_dialog_windows = 0
        launcher_child_windows     = 0
        ran_to_completion          = $false
        exit_code                  = $null
        started_marker_emitted     = $false
        exited_without_manual_dismiss = $false
    }

    $launchedPid = 0
    $deadline = (Get-Date).AddSeconds(25)
    while ((Get-Date) -lt $deadline) {
        $launcher.Refresh()
        if (Test-Path -LiteralPath $PidFile -PathType Leaf) {
            $capturedId = 0
            if ([int]::TryParse(([IO.File]::ReadAllText($PidFile)).Trim(), [ref]$capturedId)) { $launchedPid = $capturedId }
            break
        }
        if (-not $launcher.HasExited) {
            $dialogCount = [AgentDesktopProbe25.WinDialogs]::VisibleDialogWindowsForPid($launcher.Id)
            $record.class_32770_dialog_windows = [int]$dialogCount
            if ($dialogCount -gt 0) {
                $record.observation = 'blocked_on_interactive_prompt'
                break
            }
            $facts = Get-LauncherChildWindowFacts -ProcessId $launcher.Id
            $record.launcher_child_windows = $facts.child_windows
            if ($facts.dialog_present) {
                $record.observation = 'blocked_on_interactive_prompt'
                break
            }
        } else {
            break
        }
        Start-Sleep -Milliseconds 400
    }

    if ($launchedPid -gt 0) {
        $record.observation = 'unblocked_shellexecute_completed'
        $child = Get-Process -Id $launchedPid -ErrorAction SilentlyContinue
        if ($null -ne $child) {
            Register-SpawnedPid -ProcessId $launchedPid
            $exited = $child.WaitForExit(10000)
            if ($exited) {
                $child.Refresh()
                $record.exit_code = $child.ExitCode
                $record.exited_without_manual_dismiss = $true
            } else {
                $record.observation = 'unblocked_but_wait_timed_out'
                try { Stop-ScratchProcess -ProcessId $launchedPid } catch { }
            }
        } else {
            $record.exited_without_manual_dismiss = $true
        }
        $record.started_marker_emitted = (Test-Path -LiteralPath $MarkerPath -PathType Leaf)
        $record.ran_to_completion = ([bool]$record.started_marker_emitted)
    } elseif ($record.observation -ne 'blocked_on_interactive_prompt') {
        $launcher.Refresh()
        if ($launcher.HasExited) {
            $record.observation = 'launcher_exited_without_pid_record'
        } else {
            $record.observation = 'blocked_without_visible_dialog'
        }
    }
    if ($record.observation -eq 'blocked_on_interactive_prompt') {
        try { Stop-ScratchProcess -ProcessId $launcher.Id } catch { }
    }
    Get-Process | Where-Object { $_.ProcessName -ieq 'Probe25Marked' } | Stop-Process -Force -ErrorAction SilentlyContinue
    return $record
}

$standInSource = @'
using System;
using System.IO;
using System.Threading;

namespace AgentDesktopProbe25 {
    public static class MarkedApp {
        public static int Main() {
            try {
                string dir = AppDomain.CurrentDomain.BaseDirectory;
                File.WriteAllText(Path.Combine(dir, "probe25-started.marker"), DateTime.UtcNow.Ticks.ToString());
            } catch (IOException) {
            } catch (UnauthorizedAccessException) {
            }
            Console.WriteLine("probe25-started");
            Thread.Sleep(250);
            return 0;
        }
    }
}
'@

$launcherSource = @'
param([string]$TargetPath, [string]$OutPath, [switch]$Bypass)
if ($Bypass) { $env:SEE_MASK_NOZONECHECKS = '1' }
$proc = Start-Process -FilePath $TargetPath -PassThru
Set-Content -LiteralPath $OutPath -Value ([string]$proc.Id) -Encoding ASCII
'@

$result = $null

try {
    $hostSurface = Get-HostControlSurface

    $script:WorkDir = Join-Path $env:TEMP ('agent-desktop-probe25-motw-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $script:WorkDir -Force | Out-Null

    $sysCurl = Join-Path $env:WINDIR 'System32\curl.exe'

    $downloadCase = [ordered]@{
        attempted                      = $false
        transport                      = 'http_loopback_probe_owned'
        curl_exit_code                 = $null
        payload_bytes                  = 131072
        received_bytes_match_source    = $false
        received_sha256_matches_source = $false
        alternate_streams_count        = 0
        zone_identifier_present        = $null
    }

    if (Test-Path -LiteralPath $sysCurl -PathType Leaf) {
        $payload = New-Object byte[] 131072
        $rng = New-Object System.Random 20260824
        $rng.NextBytes($payload)
        $downloaded = Join-Path $script:WorkDir 'probe25-download.bin'
        $downloadCase.attempted = $true
        $downloadCase.curl_exit_code = (Invoke-LoopbackFetchByCurl -CurlExe $sysCurl -Payload $payload -OutPath $downloaded)
        if ((Test-Path -LiteralPath $downloaded -PathType Leaf) -and $downloadCase.curl_exit_code -eq 0) {
            $downloadCase.received_bytes_match_source = ((Get-Item -LiteralPath $downloaded).Length -eq $payload.Length)
            $downloadCase.received_sha256_matches_source = ((Get-FileSha256 -Path $downloaded) -eq (Get-ByteSha256 -Bytes $payload))
            $streamCheck = Test-ZoneIdentifierPresent -Path $downloaded
            $downloadCase.alternate_streams_count = $streamCheck.streams_count
            $downloadCase.zone_identifier_present = $streamCheck.present
        }
    }

    $compile = [ordered]@{
        attempted   = $false
        target      = 'console_exe'
        exit_code   = $null
        compiled_ok = $false
        error_codes = @()
    }
    $markCase = [ordered]@{
        attempted                           = $false
        zone_id_written                     = 3
        zone_identifier_present_after_write = $false
        zone_id_readback                    = $null
        command_line_launch                 = [ordered]@{
            ran_to_completion      = $false
            exit_code              = $null
            started_marker_emitted = $false
            stdout_sentinel_seen   = $false
        }
        shell_gui_launch                    = [ordered]@{
            unmarked_control            = [ordered]@{ observation = 'not_attempted' }
            marked                      = [ordered]@{ observation = 'not_attempted' }
            marked_zonecheck_bypassed   = [ordered]@{ observation = 'not_attempted' }
        }
    }

    $csc = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
    if (Test-Path -LiteralPath $csc -PathType Leaf) {
        $compile.attempted = $true
        $srcPath = Join-Path $script:WorkDir 'MarkedApp.cs'
        [IO.File]::WriteAllText($srcPath, $standInSource, (New-Object System.Text.UTF8Encoding $false))
        $builtExe = Join-Path $script:WorkDir 'Probe25StandIn.exe'
        $cscArgs = @(
            '/nologo', '/target:exe', '/langversion:5', '/platform:anycpu', ('/out:' + $builtExe),
            '/reference:System.dll', $srcPath
        )
        $buildOutput = @(& $csc $cscArgs 2>&1 | ForEach-Object { "$_" })
        $compile.exit_code = $LASTEXITCODE
        $compile.error_codes = @(Get-DiagnosticCodes -Lines $buildOutput)
        $compile.compiled_ok = (($LASTEXITCODE -eq 0) -and (Test-Path -LiteralPath $builtExe -PathType Leaf))

        if ($compile.compiled_ok) {
            $markCase.attempted = $true
            $markedExe = Join-Path $script:WorkDir 'Probe25Marked.exe'
            Copy-Item -LiteralPath $builtExe -Destination $markedExe -Force
            $zoneText = '[ZoneTransfer]' + "`r`n" + 'ZoneId=3' + "`r`n"
            Set-Content -LiteralPath $markedExe -Stream 'Zone.Identifier' -Value $zoneText -Encoding ASCII
            $afterWrite = Test-ZoneIdentifierPresent -Path $markedExe
            $markCase.zone_identifier_present_after_write = $afterWrite.present
            $zoneContent = (Get-Content -LiteralPath $markedExe -Stream 'Zone.Identifier' | Out-String)
            if ($zoneContent -match 'ZoneId\s*=\s*(\d+)') {
                $markCase.zone_id_readback = [int]$Matches[1]
            }

            $markerPath = Join-Path $script:WorkDir 'probe25-started.marker'

            Remove-Item -LiteralPath $markerPath -Force -ErrorAction SilentlyContinue
            $cmdStdout = @(& $markedExe | ForEach-Object { "$_" })
            $markCase.command_line_launch.exit_code = $LASTEXITCODE
            $markCase.command_line_launch.ran_to_completion = ($LASTEXITCODE -eq 0)
            $markCase.command_line_launch.started_marker_emitted = (Test-Path -LiteralPath $markerPath -PathType Leaf)
            $markCase.command_line_launch.stdout_sentinel_seen = (@($cmdStdout) -contains 'probe25-started')

            Remove-Item -LiteralPath $markerPath -Force -ErrorAction SilentlyContinue
            $innerScript = Join-Path $script:WorkDir 'launcher.ps1'
            [IO.File]::WriteAllText($innerScript, $launcherSource, (New-Object System.Text.UTF8Encoding $false))
            $pidFile = Join-Path $script:WorkDir 'launched.pid.txt'
            $unmarkedExe = Join-Path $script:WorkDir 'Probe25Unmarked.exe'
            Copy-Item -LiteralPath $builtExe -Destination $unmarkedExe -Force

            $markCase.shell_gui_launch.unmarked_control = Invoke-ShellLaunchArm `
                -InnerScript $innerScript -TargetPath $unmarkedExe -PidFile $pidFile -MarkerPath $markerPath
            $markCase.shell_gui_launch.marked = Invoke-ShellLaunchArm `
                -InnerScript $innerScript -TargetPath $markedExe -PidFile $pidFile -MarkerPath $markerPath
            $markCase.shell_gui_launch.marked_zonecheck_bypassed = Invoke-ShellLaunchArm `
                -InnerScript $innerScript -TargetPath $markedExe -PidFile $pidFile -MarkerPath $markerPath -BypassZoneCheck
        }
    }

    $result = [ordered]@{
        probe                 = $script:Probe
        question              = 'does a curl-downloaded file carry a Mark-of-the-Web stream at all, and against a file that genuinely carries a browser-equivalent ZoneId=3 mark, what do direct command-line invocation and a ShellExecute GUI launch actually do on this host, bounded by which download-protection controls this OS even has'
        measurable            = $true
        branch                = 'both_cases_measured'
        label                 = $Label
        host_control_surface  = $hostSurface
        curl_download_case    = $downloadCase
        compile               = $compile
        marked_execution_case = $markCase
        summary               = [ordered]@{
            npm_path_carries_no_motw      = ($downloadCase.zone_identifier_present -eq $false)
            marked_file_ran_from_shell    = [bool]$markCase.command_line_launch.ran_to_completion
            unmarked_shell_control_ran    = [bool]$markCase.shell_gui_launch.unmarked_control.ran_to_completion
            marked_shell_launch_gated     = ([string]$markCase.shell_gui_launch.marked.observation).StartsWith('blocked')
            zonecheck_bypass_unblocks     = [bool]$markCase.shell_gui_launch.marked_zonecheck_bypassed.ran_to_completion
            host_can_fire_either_control  = $false
        }
    }
} catch {
    $result = [ordered]@{
        probe        = $script:Probe
        measurable   = $false
        branch       = 'probe_threw'
        error_class  = $_.Exception.GetType().Name
        error_line   = [int]$_.InvocationInfo.ScriptLineNumber
        error_source = (Protect-ProbeText -Text ([string]$_.InvocationInfo.Line).Trim())
    }
} finally {
    Stop-AllSpawned
    Get-Process | Where-Object { $_.ProcessName -ieq 'Probe25Marked' } | Stop-Process -Force -ErrorAction SilentlyContinue
    if ($script:WorkDir -and (Test-Path -LiteralPath $script:WorkDir)) {
        Remove-Item -LiteralPath $script:WorkDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

$capturePath = Write-A25Capture -Name "motw-execution-$Label.json" -Content (ConvertTo-Json -InputObject $result -Depth 12)
Register-MandatoryPass -Capture $capturePath -Result $result

Assert-MandatoryMeasurement -Probe $script:Probe -Label $Label

Write-ProbeResult -Probe $script:Probe -Status 'ok' -Message 'MOTW and execution probe captured' -Data @{
    capture = Split-Path -Leaf $capturePath
}
exit 0
