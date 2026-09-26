#Requires -Version 5.1
<#
.SYNOPSIS
    FFI cdylib release-profile artifact-set probe (area 25, sub-phase 2.13 U1).

.DESCRIPTION
    R14's archive change needs to know exactly what the pinned release-ffi
    profile produces for the cdylib on Windows: the DLL itself is staged by
    release.yml today, but cargo also emits an MSVC import library beside it,
    and a linking consumer (C++ or Rust against the .lib) gets nothing to
    link against if the shipped archive omits it. This probe runs the same
    package-scoped build release.yml's build-ffi job runs - bare workspace
    cargo invocations fail on this box because default-members pulls the
    macOS crate in, so the invocation is deliberately package-scoped - and
    records every agent_desktop_ffi artifact that appears in the profile
    directory with its byte length. Row A25-6 exists so U6's archive change
    cites a measured artifact set rather than a remembered one.

    Captures: cdylib-artifacts-{devbox,ci}.json (+ .normalized twin). Corpus
    safety: only artifact leaf names under target\release-ffi, their byte
    lengths and the build exit code are recorded; no absolute path and no
    toolchain output text reaches a capture.
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'ci')][string]$Label = 'devbox'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

. (Join-Path (Split-Path -Parent $PSCommandPath) '..\common.ps1')
Initialize-ProbeRedaction

$script:Probe = '25-packaging-04-cdylib-artifacts'
$script:ProbeDir = Split-Path -Parent $PSCommandPath
$script:CaptureDir = Join-Path $script:ProbeDir 'captures'
if (-not (Test-Path -LiteralPath $script:CaptureDir)) {
    New-Item -ItemType Directory -Path $script:CaptureDir -Force | Out-Null
}
$script:RepoRoot = (Resolve-Path -LiteralPath (Join-Path $script:ProbeDir '..\..\..')).ProviderPath

Register-MandatoryCapture -Name @("cdylib-artifacts-$Label.json")

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

function Get-ArtifactKind {
    param([Parameter(Mandatory = $true)][string]$Leaf)
    if ($Leaf -ieq 'agent_desktop_ffi.dll') { return 'cdylib' }
    if ($Leaf -ieq 'agent_desktop_ffi.dll.lib') { return 'import_library' }
    if ($Leaf -ieq 'agent_desktop_ffi.dll.exp') { return 'export_file' }
    if ($Leaf -ieq 'agent_desktop_ffi.pdb') { return 'debug_database' }
    if ($Leaf -ieq 'agent_desktop_ffi.d') { return 'dep_info' }
    return 'other'
}

$result = $null

try {
    $cargo = (Get-Command 'cargo.exe' -ErrorAction SilentlyContinue).Source
    if (-not $cargo) { $cargo = 'cargo' }

    <#
        cargo writes progress to stderr, and a native command's redirected
        stderr becomes ErrorRecords that a Stop preference turns into a
        thrown RemoteException - so the build runs detached with its streams
        going nowhere, and only the exit code and the artifact directory are
        read back. Bounded at fifteen minutes: a cold profile build of the
        ffi crate plus its dependency tree fits comfortably, and an overrun
        is a named branch rather than a hang.
    #>
    $buildExit = $null
    $buildTimedOut = $false
    $cargoArgs = @('build', '--locked', '--profile', 'release-ffi', '-p', 'agent-desktop-ffi')
    $cargoProc = Start-Process -FilePath $cargo -ArgumentList $cargoArgs -WorkingDirectory $script:RepoRoot -PassThru -WindowStyle Hidden
    $finished = $cargoProc.WaitForExit(900000)
    if ($finished) {
        $buildExit = $cargoProc.ExitCode
    } else {
        $buildTimedOut = $true
        try { Stop-Process -Id $cargoProc.Id -Force -ErrorAction SilentlyContinue } catch { }
    }

    $profileDir = Join-Path $script:RepoRoot 'target\release-ffi'
    $artifacts = New-Object System.Collections.ArrayList
    $hasDll = $false
    $hasImportLib = $false
    $hasExp = $false
    $hasPdb = $false
    $importLibBytes = 0

    if ($buildExit -eq 0 -and -not $buildTimedOut -and (Test-Path -LiteralPath $profileDir -PathType Container)) {
        foreach ($file in @(Get-ChildItem -LiteralPath $profileDir -Filter 'agent_desktop_ffi.*' -File)) {
            $kind = Get-ArtifactKind -Leaf $file.Name
            [void]$artifacts.Add([ordered]@{
                    artifact  = $file.Name
                    kind      = $kind
                    byte_len  = [int64]$file.Length
                })
            if ($kind -eq 'cdylib') { $hasDll = $true }
            if ($kind -eq 'import_library') { $hasImportLib = $true; $importLibBytes = [int64]$file.Length }
            if ($kind -eq 'export_file') { $hasExp = $true }
            if ($kind -eq 'debug_database') { $hasPdb = $true }
        }
    }

    $result = [ordered]@{
        probe                       = $script:Probe
        question                    = 'what does the pinned release-ffi profile actually produce for the Windows cdylib: which of the DLL, its MSVC import library, its export file and its debug database appear in the profile directory, and how large is the import library a linking consumer needs beside the DLL release.yml stages today'
        measurable                  = $true
        branch                      = 'artifact_set_recorded'
        label                       = $Label
        build_invocation            = 'cargo build --locked --profile release-ffi -p agent-desktop-ffi'
        build_exit_code             = $buildExit
        build_timed_out             = $buildTimedOut
        artifact_count              = $artifacts.Count
        artifacts                   = @($artifacts)
        dll_present                 = $hasDll
        import_lib_present          = $hasImportLib
        exp_present                 = $hasExp
        pdb_present                 = $hasPdb
        import_lib_bytes            = $importLibBytes
        all_expected_kinds_present  = ($hasDll -and $hasImportLib -and $hasExp -and $hasPdb)
        summary                     = [ordered]@{
            build_ok                  = ($buildExit -eq 0)
            all_expected_kinds_present= ($hasDll -and $hasImportLib -and $hasExp -and $hasPdb)
            import_lib_overhead_kb_rounded = [int][math]::Round($importLibBytes / 1024.0)
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
}

$capturePath = Write-A25Capture -Name "cdylib-artifacts-$Label.json" -Content (ConvertTo-Json -InputObject $result -Depth 8)
Register-MandatoryPass -Capture $capturePath -Result $result

Assert-MandatoryMeasurement -Probe $script:Probe -Label $Label

Write-ProbeResult -Probe $script:Probe -Status 'ok' -Message 'cdylib artifact-set probe captured' -Data @{
    capture = Split-Path -Leaf $capturePath
}
exit 0
