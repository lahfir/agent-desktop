#Requires -Version 5.1
<#
.SYNOPSIS
    npm global-install mechanics and pack-install cost probe (area 25, sub-phase 2.13 U1).

.DESCRIPTION
    R6's Windows install path reuses postinstall.js's download, checksum and
    atomic-install code unchanged, so the environment facts it depends on are
    measured here rather than assumed:

      - what `npm prefix -g` reports, as a shape only: whether the reported
        prefix sits under the user profile and what its leaf segment is,
        never the raw path
      - the shim triad a global install of this package generates for its one
        bin entry - the extensionless sh script, the .cmd and the .ps1 - by
        installing the packed tarball into a throwaway --prefix, never into
        the machine's real global prefix
      - whether a postinstall-shaped write into an installed package directory
        succeeds reliably or only intermittently: the same small file write,
        readback-verified and removed, repeated enough times to separate a
        dependable write from an antivirus-timing flake; a failed write is
        retried once immediately so a transient hold that clears on retry is
        distinguishable from a persistent refusal
      - the pack-plus-install cycle cost (row A25-7): a full `npm pack` plus
        install-from-local-tarball into a fresh scratch prefix, timed with
        the corpus methodology - one discarded warm-up, then min of seven
        with median and max beside it

    Captures: npm-global-{devbox,ci}.json and
    npm-pack-install-cost-{devbox,ci}.json (+ .normalized twins). Corpus
    safety: scratch prefixes live under TEMP\opencode and are removed in
    finally; no raw prefix path, no npm output text and no identity reaches a
    capture - outcomes are booleans, counts, iteration numbers and timings.
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'ci')][string]$Label = 'devbox'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

. (Join-Path (Split-Path -Parent $PSCommandPath) '..\common.ps1')
Initialize-ProbeRedaction

$script:Probe = '25-packaging-02-npm-global-install'
$script:ProbeDir = Split-Path -Parent $PSCommandPath
$script:CaptureDir = Join-Path $script:ProbeDir 'captures'
if (-not (Test-Path -LiteralPath $script:CaptureDir)) {
    New-Item -ItemType Directory -Path $script:CaptureDir -Force | Out-Null
}
$script:RepoRoot = (Resolve-Path -LiteralPath (Join-Path $script:ProbeDir '..\..\..')).ProviderPath
$script:NpmDir = Join-Path $script:RepoRoot 'npm'
$script:ScratchRoot = Join-Path $env:TEMP 'opencode'
$script:NpmCmd = $null
$script:Prefixes = New-Object System.Collections.ArrayList
$script:PackedLeaves = New-Object System.Collections.ArrayList

Register-MandatoryCapture -Name @("npm-global-$Label.json", "npm-pack-install-cost-$Label.json")

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

function New-ScratchPrefix {
    if (-not (Test-Path -LiteralPath $script:ScratchRoot)) {
        New-Item -ItemType Directory -Path $script:ScratchRoot -Force | Out-Null
    }
    $prefix = Join-Path $script:ScratchRoot ('agent-desktop-probe25-npm-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $prefix -Force | Out-Null
    [void]$script:Prefixes.Add($prefix)
    return $prefix
}

function Remove-AllScratchPrefixes {
    foreach ($p in @($script:Prefixes)) {
        if ($p -and (Test-Path -LiteralPath $p)) {
            Remove-Item -LiteralPath $p -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
    $script:Prefixes.Clear()
}

<#
    npm pack always writes into the package directory itself; the tarball it
    leaves there is this probe's artifact, not the repository's, so every
    leaf this run packed is removed again on cleanup.
#>
function Remove-PackedTarballs {
    foreach ($leaf in @($script:PackedLeaves)) {
        $path = Join-Path $script:NpmDir $leaf
        if ($leaf -and (Test-Path -LiteralPath $path)) {
            Remove-Item -LiteralPath $path -Force -ErrorAction SilentlyContinue
        }
    }
    $script:PackedLeaves.Clear()
}

function Invoke-NpmPack {
    <#
        Returns the packed tarball leaf name and byte size from npm pack's
        own --json payload; the filename is product-and-version only.
    #>
    Push-Location $script:NpmDir
    try {
        $prevEap = $ErrorActionPreference
        $ErrorActionPreference = 'Continue'
        try {
            $out = & $script:NpmCmd pack --json 2>&1
            $exit = $LASTEXITCODE
        } finally {
            $ErrorActionPreference = $prevEap
        }
    } finally {
        Pop-Location
    }
    $record = @{ exit_code = $exit; tgz_leaf = $null; size_bytes = 0 }
    if ($exit -eq 0) {
        $text = (@($out) | ForEach-Object { "$_" }) -join "`n"
        try {
            $parsed = ConvertFrom-Json -InputObject $text
            $entry = $null
            if ($parsed -is [array]) {
                $entry = $parsed[0]
            } else {
                $firstProp = @($parsed.PSObject.Properties)[0]
                $entry = $firstProp.Value
            }
            $record.tgz_leaf = [string]$entry.filename
            $record.size_bytes = [int]$entry.size
        } catch {
            $leafMatch = [regex]::Match($text, '[A-Za-z0-9._\-]+\.tgz')
            if ($leafMatch.Success) { $record.tgz_leaf = $leafMatch.Value }
        }
        if ($record.tgz_leaf -and -not ($script:PackedLeaves.Contains($record.tgz_leaf))) {
            [void]$script:PackedLeaves.Add($record.tgz_leaf)
        }
    }
    return $record
}

function Invoke-NpmGlobalInstall {
    param(
        [Parameter(Mandatory = $true)][string]$TgzPath,
        [Parameter(Mandatory = $true)][string]$Prefix
    )
    $prevEap = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        & $script:NpmCmd install -g $TgzPath --prefix $Prefix --no-audit --no-fund --loglevel=error 2>&1 | Out-Null
        return $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $prevEap
    }
}

function Get-ShimRecord {
    param(
        [Parameter(Mandatory = $true)][string]$Prefix,
        [Parameter(Mandatory = $true)][string]$BinName
    )
    $cmdShim = Join-Path $Prefix ($BinName + '.cmd')
    $ps1Shim = Join-Path $Prefix ($BinName + '.ps1')
    $bareShim = Join-Path $Prefix $BinName
    $allTopLevel = @(Get-ChildItem -LiteralPath $Prefix -File -ErrorAction SilentlyContinue)
    $binPrefixed = @($allTopLevel | Where-Object { $_.Name -like ($BinName + '*') })
    $cmdPresent = (Test-Path -LiteralPath $cmdShim -PathType Leaf)
    $ps1Present = (Test-Path -LiteralPath $ps1Shim -PathType Leaf)
    $barePresent = (Test-Path -LiteralPath $bareShim -PathType Leaf)
    return [ordered]@{
        checked                    = $true
        cmd_shim_present           = $cmdPresent
        ps1_shim_present           = $ps1Present
        extensionless_shim_present = $barePresent
        bin_named_files_count      = $binPrefixed.Count
        top_level_file_count       = $allTopLevel.Count
        shim_triad_complete        = ($cmdPresent -and $ps1Present -and $barePresent)
    }
}

function Test-PackageDirWritable {
    <#
        One postinstall-shaped write: create a small file inside the installed
        package directory, read it back and compare, delete it. A failure is
        retried once immediately, so a transient antivirus hold that clears on
        retry is distinguishable from a persistent refusal.
    #>
    param([Parameter(Mandatory = $true)][string]$PackageDir)
    $target = Join-Path $PackageDir '.probe25-write-test'
    $payload = 'probe25-write-' + [guid]::NewGuid().ToString('N')
    $ok = $false
    $retryRecovered = $false
    try {
        try {
            [IO.File]::WriteAllText($target, $payload, (New-Object System.Text.UTF8Encoding $false))
            $ok = (([IO.File]::ReadAllText($target)) -eq $payload)
        } catch {
            try {
                Start-Sleep -Milliseconds 250
                [IO.File]::WriteAllText($target, $payload, (New-Object System.Text.UTF8Encoding $false))
                $ok = (([IO.File]::ReadAllText($target)) -eq $payload)
                $retryRecovered = $ok
            } catch {
                $ok = $false
            }
        }
    } finally {
        Remove-Item -LiteralPath $target -Force -ErrorAction SilentlyContinue
    }
    return @{ ok = $ok; retry_recovered = $retryRecovered }
}

$result = $null
$costResult = $null

try {
    $script:NpmCmd = (Get-Command 'npm.cmd' -ErrorAction SilentlyContinue).Source
    if (-not $script:NpmCmd) { $script:NpmCmd = 'npm' }

    $nodeVersionOut = ((& node --version 2>&1 | ForEach-Object { "$_" }) -join '').Trim()
    $npmVersionOut = ((& $script:NpmCmd --version 2>&1 | ForEach-Object { "$_" }) -join '').Trim()
    $nodeArch = ((& node -p 'process.arch' 2>&1 | ForEach-Object { "$_" }) -join '').Trim()
    $nodeMajor = 0
    if ($nodeVersionOut -match '^v(\d+)') { $nodeMajor = [int]$Matches[1] }
    $npmMajor = 0
    if ($npmVersionOut -match '^(\d+)') { $npmMajor = [int]$Matches[1] }

    $globalPrefixLeaf = ''
    $globalPrefixUnderProfile = $false
    $globalPrefixSegments = 0
    $prefixLine = @((& $script:NpmCmd prefix -g 2>&1 | ForEach-Object { "$_" }) | Where-Object { $_ } | Select-Object -First 1)
    if ($prefixLine.Count -gt 0) {
        $clean = ([string]$prefixLine[0]).Trim()
        $globalPrefixLeaf = Split-Path -Leaf $clean
        $globalPrefixUnderProfile = (($env:USERPROFILE) -and $clean.StartsWith($env:USERPROFILE, [StringComparison]::OrdinalIgnoreCase))
        $globalPrefixSegments = (@(($clean -split '[\\/]') | Where-Object { $_ })).Count
    }

    $pack = Invoke-NpmPack

    $shims = [ordered]@{
        checked                    = $false
        cmd_shim_present           = $false
        ps1_shim_present           = $false
        extensionless_shim_present = $false
        bin_named_files_count      = 0
        top_level_file_count       = 0
        shim_triad_complete        = $false
    }
    $installExit = $null
    $packageDirPresent = $false
    $writeIterations = 12
    $writesOk = 0
    $retriesRecovered = 0
    $failedIterations = New-Object System.Collections.ArrayList

    if ($pack.exit_code -eq 0 -and $pack.tgz_leaf) {
        $tgzPath = Join-Path $script:NpmDir $pack.tgz_leaf
        $prefix = New-ScratchPrefix
        $installExit = Invoke-NpmGlobalInstall -TgzPath $tgzPath -Prefix $prefix
        if ($installExit -eq 0) {
            $shims = Get-ShimRecord -Prefix $prefix -BinName 'agent-desktop'
            $packageDir = Join-Path (Join-Path $prefix 'node_modules') 'agent-desktop'
            $packageDirPresent = (Test-Path -LiteralPath $packageDir -PathType Container)
            if ($packageDirPresent) {
                for ($i = 1; $i -le $writeIterations; $i++) {
                    $outcome = Test-PackageDirWritable -PackageDir $packageDir
                    if ($outcome.ok) { $writesOk++ } else { [void]$failedIterations.Add($i) }
                    if ($outcome.retry_recovered) { $retriesRecovered++ }
                }
            }
        }
    }

    $result = [ordered]@{
        probe                         = $script:Probe
        question                      = 'does the npm global-install surface this package relies on behave as the unchanged macOS install path needs on Windows: where does the global prefix sit, does a global install generate the full three-shim bin triad for the one bin entry, and does a postinstall-shaped write into the installed package directory succeed reliably rather than intermittently'
        measurable                    = $true
        branch                        = 'npm_surface_exercised'
        label                         = $Label
        node_major                    = $nodeMajor
        npm_major                     = $npmMajor
        platform_key                  = ('win32-' + $nodeArch)
        global_prefix_under_profile   = [bool]$globalPrefixUnderProfile
        global_prefix_leaf            = $globalPrefixLeaf
        global_prefix_segments        = $globalPrefixSegments
        pack_exit_code                = $pack.exit_code
        packed_tgz_leaf               = $pack.tgz_leaf
        packed_size_bytes             = $pack.size_bytes
        install_exit_code             = $installExit
        shim_triad                    = $shims
        installed_package_dir_present = $packageDirPresent
        write_iterations              = $writeIterations
        writes_ok                     = $writesOk
        retries_recovered             = $retriesRecovered
        failed_write_iterations       = @($failedIterations)
        summary                       = [ordered]@{
            install_succeeded   = ($installExit -eq 0)
            shim_triad_complete = [bool]$shims.shim_triad_complete
            all_writes_ok       = ($writesOk -eq $writeIterations)
        }
    }

    $warmupMs = $null
    $samples = New-Object System.Collections.ArrayList
    $allRunsZero = $true
    for ($run = 0; $run -lt 8; $run++) {
        $runPrefix = New-ScratchPrefix
        $sw = [System.Diagnostics.Stopwatch]::StartNew()
        $cyclePack = Invoke-NpmPack
        $exitThisRun = $cyclePack.exit_code
        if ($exitThisRun -eq 0 -and $cyclePack.tgz_leaf) {
            $exitThisRun = Invoke-NpmGlobalInstall -TgzPath (Join-Path $script:NpmDir $cyclePack.tgz_leaf) -Prefix $runPrefix
        }
        $sw.Stop()
        if ($exitThisRun -ne 0) { $allRunsZero = $false }
        if ($run -eq 0) {
            $warmupMs = [math]::Round($sw.Elapsed.TotalMilliseconds, 1)
        } else {
            [void]$samples.Add([math]::Round($sw.Elapsed.TotalMilliseconds, 1))
        }
        Remove-Item -LiteralPath $runPrefix -Recurse -Force -ErrorAction SilentlyContinue
        [void]$script:Prefixes.Remove($runPrefix)
    }
    $sorted = @($samples | Sort-Object)
    $costResult = [ordered]@{
        probe              = $script:Probe
        question           = 'what does one full agent-desktop distribution cycle cost on this box: npm pack followed by an npm global install of the packed tarball into a fresh scratch prefix'
        methodology        = 'min-of-seven, warm-up discarded (A15-13)'
        warmup_discarded   = $true
        runs               = $sorted.Count
        warmup_ms          = $warmupMs
        min_ms             = $sorted[0]
        median_ms          = $sorted[[int][math]::Floor($sorted.Count / 2)]
        max_ms             = $sorted[$sorted.Count - 1]
        all_runs_exit_zero = $allRunsZero
    }
} catch {
    if ($null -eq $result) {
        $result = [ordered]@{
            probe       = $script:Probe
            measurable  = $false
            branch      = 'probe_threw'
            error_class = $_.Exception.GetType().Name
            error_line  = [int]$_.InvocationInfo.ScriptLineNumber
        }
    }
    if ($null -eq $costResult) {
        $costResult = [ordered]@{
            measurable = $false
            skipped    = ('cost leg did not run or threw: ' + $_.Exception.GetType().Name)
        }
    }
} finally {
    Remove-AllScratchPrefixes
    Remove-PackedTarballs
}

$capture1 = Write-A25Capture -Name "npm-global-$Label.json" -Content (ConvertTo-Json -InputObject $result -Depth 12)
Register-MandatoryPass -Capture $capture1 -Result $result
$capture2 = Write-A25Capture -Name "npm-pack-install-cost-$Label.json" -Content (ConvertTo-Json -InputObject $costResult -Depth 6)
Register-MandatoryPass -Capture $capture2 -Result $costResult

Assert-MandatoryMeasurement -Probe $script:Probe -Label $Label

Write-ProbeResult -Probe $script:Probe -Status 'ok' -Message 'npm global-install mechanics and pack-install cost captured' -Data @{
    captures = @((Split-Path -Leaf $capture1), (Split-Path -Leaf $capture2))
}
exit 0
