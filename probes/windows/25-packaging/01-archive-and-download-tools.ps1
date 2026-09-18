#Requires -Version 5.1
<#
.SYNOPSIS
    In-box archive and download tools probe (area 25, sub-phase 2.13 U1).

.DESCRIPTION
    KTD1 rests the Windows CLI asset format (.tar.gz, no .zip, no new
    extraction code in postinstall.js) on three measured facts: tar.exe and
    curl.exe exist in System32 and are therefore always on PATH, their
    versions speak the gzip-tar dialect postinstall already drives, and a
    create/list/extract round trip of such a tarball succeeds end to end.
    This probe measures all three rather than leaving them as plan prose,
    because the entire archive-format decision rests on them and a decision
    resting on an unrecorded observation is indistinguishable from one
    resting on an assumption.

    Measured legs:
      - absolute System32 presence and version banner for tar.exe/curl.exe
      - PATH-order resolution for both tools: which copy a bare invocation
        resolves to, how many copies sit on PATH at all, and where a
        Git-for-Windows tool directory sits relative to System32 (Git ships
        its own GNU tar in usr\bin, so PATH order decides whether the in-box
        bsdtar or an msys tar answers a bare `tar`)
      - a gzip-tarball create/list/extract round trip whose single entry is a
        stand-in agent-desktop.exe of deterministic pseudo-random bytes,
        verified by byte count and SHA-256, never by the tool's own success
        output alone

    Captures: archive-tools-{devbox,ci}.json (+ .normalized twin). Corpus
    safety: no PATH directory, no install location and no user or machine
    identity reaches a capture - PATH facts are recorded as booleans and
    positions only, banners are the tools' own version lines which carry no
    environment detail, and everything passes through
    Protect-ProbeText/Test-CaptureRedaction as a backstop.
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'ci')][string]$Label = 'devbox'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

. (Join-Path (Split-Path -Parent $PSCommandPath) '..\common.ps1')
Initialize-ProbeRedaction

$script:Probe = '25-packaging-01-archive-and-download-tools'
$script:ProbeDir = Split-Path -Parent $PSCommandPath
$script:CaptureDir = Join-Path $script:ProbeDir 'captures'
if (-not (Test-Path -LiteralPath $script:CaptureDir)) {
    New-Item -ItemType Directory -Path $script:CaptureDir -Force | Out-Null
}
$script:WorkDir = $null

Register-MandatoryCapture -Name @("archive-tools-$Label.json")

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

function Get-FirstOutputLine {
    param([string[]]$Lines)
    foreach ($line in $Lines) {
        $text = ([string]$line).Trim()
        if ($text) { return $text }
    }
    return ''
}

<#
    The banner is recorded verbatim because it IS the version evidence
    bsdtar/curl print, and neither banner carries environment detail - no
    path, host or user token appears in either. The structured family and
    version beside it are what a row quotes, parsed from the same line.
#>
function Get-ToolRecord {
    param(
        [Parameter(Mandatory = $true)][string]$ToolName,
        [Parameter(Mandatory = $true)][string]$System32Path
    )
    $record = [ordered]@{
        tool                  = $ToolName
        system32_present      = (Test-Path -LiteralPath $System32Path)
        family                = $null
        version               = $null
        banner_first_line     = $null
        path_copies_found     = 0
        resolves_to_system32  = $false
    }
    if (-not $record.system32_present) { return $record }
    $banner = Get-FirstOutputLine -Lines @(& $System32Path '--version' 2>&1 | ForEach-Object { "$_" })
    $record.banner_first_line = $banner
    if ($banner -match '^([A-Za-z][A-Za-z0-9_+\-]*)\s+(\d+\.\d+(?:\.\d+)?)') {
        $record.family = $Matches[1]
        $record.version = $Matches[2]
    }
    $seenDirs = New-Object System.Collections.ArrayList
    foreach ($dir in ($env:PATH -split ';')) {
        $clean = ([string]$dir).Trim()
        if (-not $clean) { continue }
        $key = $clean.TrimEnd('\').ToLowerInvariant()
        if ($seenDirs.Contains($key)) { continue }
        [void]$seenDirs.Add($key)
        if (Test-Path -LiteralPath (Join-Path $clean ($ToolName + '.exe')) -PathType Leaf) {
            $record.path_copies_found++
        }
    }
    $resolved = (Get-Command ($ToolName + '.exe') -ErrorAction SilentlyContinue)
    if ($null -ne $resolved -and $resolved.Source) {
        $record.resolves_to_system32 = ($resolved.Source -ieq $System32Path)
    }
    return $record
}

<#
    PATH-order facts as positions and counts only. A Git-for-Windows checkout
    puts its own GNU tar ahead of or behind System32 depending on install
    choices, and WHICH of the two answers a bare `tar` decides whether the
    postinstall extraction path drives bsdtar or msys tar on a developer
    machine - so the ordering relation is the fact, not either directory.
#>
function Get-PathOrderRecord {
    param([Parameter(Mandatory = $true)][string]$System32Dir)
    $normalized = New-Object System.Collections.ArrayList
    foreach ($dir in ($env:PATH -split ';')) {
        $clean = ([string]$dir).Trim()
        if (-not $clean) { continue }
        $key = $clean.TrimEnd('\').ToLowerInvariant()
        if ($normalized.Contains($key)) { continue }
        [void]$normalized.Add(@{ Key = $key; Raw = $clean.TrimEnd('\') })
    }
    $sysKey = $System32Dir.TrimEnd('\').ToLowerInvariant()
    $gitPattern = '\\git\\(?:cmd|bin|mingw64\\bin|usr\\bin)$'
    $system32Position = 0
    $firstGitPosition = 0
    $gitCount = 0
    for ($i = 0; $i -lt $normalized.Count; $i++) {
        $entry = $normalized[$i]
        if ($system32Position -eq 0 -and $entry.Key -ieq $sysKey) { $system32Position = $i + 1 }
        if ($entry.Raw -imatch $gitPattern) {
            $gitCount++
            if ($firstGitPosition -eq 0) { $firstGitPosition = $i + 1 }
        }
    }
    return [ordered]@{
        system32_dir_on_path      = ($system32Position -gt 0)
        system32_position_1based  = $system32Position
        git_family_dirs_on_path   = $gitCount
        first_git_position_1based = $firstGitPosition
        git_precedes_system32     = (($firstGitPosition -gt 0) -and ($system32Position -gt 0) -and ($firstGitPosition -lt $system32Position))
    }
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

$result = $null

try {
    $system32Dir = Join-Path $env:WINDIR 'System32'
    $sysTar = Join-Path $system32Dir 'tar.exe'
    $sysCurl = Join-Path $system32Dir 'curl.exe'

    $tarRecord = Get-ToolRecord -ToolName 'tar' -System32Path $sysTar
    $curlRecord = Get-ToolRecord -ToolName 'curl' -System32Path $sysCurl
    $pathOrder = Get-PathOrderRecord -System32Dir $system32Dir

    $roundtrip = [ordered]@{
        attempted                        = $false
        create_exit_code                 = $null
        listing_entry_count              = 0
        single_entry_is_stand_in_exe     = $false
        extract_exit_code                = $null
        extracted_file_count             = 0
        extracted_bytes_match_source     = $false
        extracted_sha256_matches_source  = $false
        roundtrip_ok                     = $false
    }

    if ($tarRecord.system32_present) {
        $script:WorkDir = Join-Path $env:TEMP ('agent-desktop-probe25-archive-' + [guid]::NewGuid().ToString('N'))
        New-Item -ItemType Directory -Path $script:WorkDir -Force | Out-Null
        $stage = Join-Path $script:WorkDir 'stage'
        $out = Join-Path $script:WorkDir 'extract'
        New-Item -ItemType Directory -Path $stage -Force | Out-Null
        New-Item -ItemType Directory -Path $out -Force | Out-Null

        $standIn = New-Object byte[] 65536
        $rng = New-Object System.Random 20260824
        $rng.NextBytes($standIn)
        $sourceExe = Join-Path $stage 'agent-desktop.exe'
        [IO.File]::WriteAllBytes($sourceExe, $standIn)
        $sourceSha = Get-ByteSha256 -Bytes $standIn

        $tgz = Join-Path $script:WorkDir 'agent-desktop-standin.tar.gz'
        & $sysTar -czf $tgz -C $stage 'agent-desktop.exe' 2>&1 | Out-Null
        $roundtrip.create_exit_code = $LASTEXITCODE

        $listing = @(& $sysTar -tzf $tgz 2>&1 | ForEach-Object { ([string]$_).Trim() } | Where-Object { $_ })
        $roundtrip.attempted = $true
        $roundtrip.listing_entry_count = $listing.Count
        $roundtrip.single_entry_is_stand_in_exe = (($listing.Count -eq 1) -and ($listing[0] -eq 'agent-desktop.exe'))

        & $sysTar -xzf $tgz -C $out 2>&1 | Out-Null
        $roundtrip.extract_exit_code = $LASTEXITCODE
        $extracted = @(Get-ChildItem -LiteralPath $out -Recurse -File)
        $roundtrip.extracted_file_count = $extracted.Count
        if ($extracted.Count -eq 1) {
            $extractedExe = Join-Path $out 'agent-desktop.exe'
            $roundtrip.extracted_bytes_match_source = ((Get-Item -LiteralPath $extractedExe).Length -eq $standIn.Length)
            $roundtrip.extracted_sha256_matches_source = ((Get-FileSha256 -Path $extractedExe) -eq $sourceSha)
        }
        $roundtrip.roundtrip_ok = (
            ($roundtrip.create_exit_code -eq 0) -and
            $roundtrip.single_entry_is_stand_in_exe -and
            ($roundtrip.extract_exit_code -eq 0) -and
            ($roundtrip.extracted_file_count -eq 1) -and
            $roundtrip.extracted_bytes_match_source -and
            $roundtrip.extracted_sha256_matches_source
        )
    }

    $result = [ordered]@{
        probe                    = $script:Probe
        question                 = 'are tar.exe and curl.exe present in System32 and first in practice on PATH relative to a Git-for-Windows install, do their versions speak the gzip-tar dialect postinstall drives, and does a create-list-extract round trip of a gzip tarball holding one stand-in agent-desktop.exe succeed through the in-box tool'
        measurable               = $true
        branch                   = 'archive_tools_exercised'
        label                    = $Label
        system32_tar_present     = [bool]$tarRecord.system32_present
        system32_curl_present    = [bool]$curlRecord.system32_present
        tools                    = [ordered]@{
            tar  = $tarRecord
            curl = $curlRecord
        }
        path_order               = $pathOrder
        roundtrip                = $roundtrip
        stand_in_bytes           = 65536
        summary                  = [ordered]@{
            both_system32_tools_present = ([bool]$tarRecord.system32_present -and [bool]$curlRecord.system32_present)
            gzip_roundtrip_succeeded    = [bool]$roundtrip.roundtrip_ok
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
    if ($script:WorkDir -and (Test-Path -LiteralPath $script:WorkDir)) {
        Remove-Item -LiteralPath $script:WorkDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

$capturePath = Write-A25Capture -Name "archive-tools-$Label.json" -Content (ConvertTo-Json -InputObject $result -Depth 12)
Register-MandatoryPass -Capture $capturePath -Result $result

Assert-MandatoryMeasurement -Probe $script:Probe -Label $Label

Write-ProbeResult -Probe $script:Probe -Status 'ok' -Message 'archive and download tools probe captured' -Data @{
    capture = Split-Path -Leaf $capturePath
}
exit 0
