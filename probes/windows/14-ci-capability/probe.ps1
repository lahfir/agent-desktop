#Requires -Version 5.1
<#
.SYNOPSIS
    Sub-phase 2.2 CI capability probe.

.DESCRIPTION
    Measures the runner-environment facts the 2.2 plan refuses to infer - the
    session, window station and interactivity a UI Automation client needs -
    and drives probe.rs, which measures the real uiautomation crate's
    end-of-list discriminator, cross-process walkability and secure-field
    behaviour.

    Captures are written beside this script under captures\, BOM-less UTF-8,
    through the corpus redaction gate in ..\common.ps1.
#>
[CmdletBinding()]
param(
    [switch]$SkipRustProbe,
    [ValidateSet('devbox', 'ci')][string]$Label = 'devbox',
    [string]$UiAutomationVersion = '0.25.0'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

. (Join-Path (Split-Path -Parent $PSCommandPath) '..\common.ps1')
Initialize-ProbeRedaction

$script:ProbeDir = Split-Path -Parent $PSCommandPath
$script:CaptureDir = Join-Path $script:ProbeDir 'captures'
if (-not (Test-Path -LiteralPath $script:CaptureDir)) {
    New-Item -ItemType Directory -Path $script:CaptureDir -Force | Out-Null
}

function Write-CapabilityCapture {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Content
    )
    $redacted = Protect-ProbeText -Text $Content
    $path = Join-Path $script:CaptureDir $Name
    $utf8NoBom = New-Object System.Text.UTF8Encoding $false
    [IO.File]::WriteAllText($path, $redacted, $utf8NoBom)
    if (-not (Test-CaptureRedaction -Path $path)) {
        throw "redaction residue in $path"
    }
    return $path
}

function Initialize-CapabilityNative {
    if ('AgentDesktopCapability.Native' -as [type]) { return }
    $source = @'
using System;
using System.Runtime.InteropServices;
using System.Text;

namespace AgentDesktopCapability {
    public static class Native {
        [DllImport("user32.dll", SetLastError = true)]
        public static extern IntPtr GetProcessWindowStation();
        [DllImport("user32.dll", SetLastError = true)]
        public static extern IntPtr GetThreadDesktop(uint dwThreadId);
        [DllImport("kernel32.dll")]
        public static extern uint GetCurrentThreadId();
        [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        public static extern bool GetUserObjectInformationW(IntPtr hObj, int nIndex, StringBuilder pvInfo, int nLength, out int lpnLengthNeeded);

        public static string ObjectName(IntPtr handle) {
            if (handle == IntPtr.Zero) { return "<null>"; }
            StringBuilder buffer = new StringBuilder(256);
            int needed = 0;
            if (!GetUserObjectInformationW(handle, 2, buffer, buffer.Capacity, out needed)) {
                return "<error " + Marshal.GetLastWin32Error() + ">";
            }
            return buffer.ToString();
        }

        public static string WindowStationName() { return ObjectName(GetProcessWindowStation()); }
        public static string DesktopName() { return ObjectName(GetThreadDesktop(GetCurrentThreadId())); }
    }
}
'@
    Add-Type -TypeDefinition $source -Language CSharp | Out-Null
}

function Get-QuerySessionOutput {
    try {
        $output = & qwinsta.exe 2>&1 | Out-String
        return $output.Trim()
    } catch {
        return "<qwinsta unavailable: $($_.Exception.Message)>"
    }
}

function Get-SessionEvidence {
    Initialize-CapabilityNative
    $os = Get-CimInstance -ClassName Win32_OperatingSystem
    return [ordered]@{
        probe             = '14-ci-capability'
        label             = $Label
        scope             = 'app/provider'
        stack             = 'n/a'
        runnerImageOs      = if ($env:ImageOS) { $env:ImageOS } else { '<not a hosted runner>' }
        runnerImageVersion = if ($env:ImageVersion) { $env:ImageVersion } else { '<not a hosted runner>' }
        osCaption         = $os.Caption
        osVersion         = $os.Version
        osBuildNumber     = $os.BuildNumber
        osInstallationType = (Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion').InstallationType
        sessionName       = $env:SESSIONNAME
        sessionId         = (Get-Process -Id $PID).SessionId
        userInteractive   = [Environment]::UserInteractive
        windowStation     = [AgentDesktopCapability.Native]::WindowStationName()
        desktop           = [AgentDesktopCapability.Native]::DesktopName()
        queryUserSessions = Get-QuerySessionOutput
    }
}

function Invoke-RustProbe {
    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
    if (-not $cargo) {
        return [ordered]@{ skipped = 'cargo is not installed on this machine' }
    }
    $work = Join-Path ([IO.Path]::GetTempPath()) ('agent-desktop-capability-' + [guid]::NewGuid())
    New-Item -ItemType Directory -Path (Join-Path $work 'src') -Force | Out-Null
    try {
        $manifest = @"
[package]
name = "agent-desktop-capability-probe"
version = "0.0.0"
edition = "2021"

[dependencies]
serde_json = "1"
uiautomation = "=$UiAutomationVersion"

[target.'cfg(target_os = "windows")'.dependencies]
windows-sys = { version = "0.61", features = [
  "Win32_Foundation",
  "Win32_Graphics_Gdi",
  "Win32_System_Com",
  "Win32_System_LibraryLoader",
  "Win32_UI_WindowsAndMessaging",
] }

[workspace]
"@
        $utf8NoBom = New-Object System.Text.UTF8Encoding $false
        [IO.File]::WriteAllText((Join-Path $work 'Cargo.toml'), $manifest, $utf8NoBom)
        Copy-Item -LiteralPath (Join-Path $script:ProbeDir 'probe.rs') -Destination (Join-Path $work 'src\main.rs') -Force
        $env:PROBE_UIAUTOMATION_VERSION = $UiAutomationVersion
        Push-Location $work
        try {
            & cargo build --quiet 2>&1 | Write-Verbose
            if ($LASTEXITCODE -ne 0) {
                return [ordered]@{ skipped = "cargo build failed with exit code $LASTEXITCODE" }
            }
            $raw = & cargo run --quiet 2>$null | Out-String
            if ($LASTEXITCODE -ne 0) {
                return [ordered]@{ skipped = "the probe exited with code $LASTEXITCODE" }
            }
            return ($raw | ConvertFrom-Json)
        } finally {
            Pop-Location
        }
    } finally {
        Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
    }
}

$session = Get-SessionEvidence
$sessionPath = Write-CapabilityCapture -Name "session-$Label.json" -Content (ConvertTo-Json -InputObject $session -Depth 10)
Write-Host "wrote $sessionPath"

if ($SkipRustProbe) {
    Write-Host 'skipping the Rust probe by request'
    exit 0
}

$capability = Invoke-RustProbe
$capabilityPath = Write-CapabilityCapture -Name "uia-capability-$Label.json" -Content (ConvertTo-Json -InputObject $capability -Depth 20)
Write-Host "wrote $capabilityPath"
exit 0
