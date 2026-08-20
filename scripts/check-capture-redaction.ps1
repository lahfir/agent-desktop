#Requires -Version 5.1
<#
.SYNOPSIS
    R21's redaction gate: Test-CaptureRedaction (probes/windows/common.ps1)
    plus Test-CaptureCliRedaction (scripts/lib/capture-redaction-cli.psm1)
    over every committed capture and the dogfood report body.

.DESCRIPTION
    Two self-tests run before the real scan, and the real scan never
    executes if either fails - a redaction gate that has not been shown
    able to both catch and pass is not trustworthy over content nobody
    will re-read before it reaches a public repository:

      1. The CLI-envelope fixtures under
         scripts/fixtures/capture-redaction/{must-catch,must-pass}/ prove
         Test-CaptureCliRedaction's per-field rules (name, value,
         description, title, path[], occluder.name, pid) independently of
         this machine's identity.
      2. Test-CaptureRedaction's own machine-identity rules (user name,
         profile path) are proven against fixtures generated at run time
         from this process's live USERNAME/USERPROFILE and deleted
         afterward - committing a fixture that actually contains a real
         username would defeat the gate it exists to prove, and the value
         is not portable to another box in any case (mirrors
         check-e2e-windows-contract.ps1's own generated-not-committed
         rule13 fixtures).

    The real scan (default mode, no -Path) derives its file set from
    tracked files rather than a hardcoded list of roots (finding #8):
    `git ls-files --cached --others --exclude-standard`, filtered to any
    path with a segment literally named "captures" or ending in
    "-captures" - the same shape contract-gate rule 17 already uses to
    prove a walk and a git listing agree, applied here to prove the scan
    set is not two roots someone remembered to update. Before this fix the
    gate scanned exactly two hardcoded roots; broadening it surfaced 21
    pre-existing capture files elsewhere in the tree that had never been
    scanned by anything and do carry unredacted content - named exactly,
    by path, in $script:PreexistingUnredactedAllowlist below. That
    allowlist is not an acceptance of those 21 files; it exists so this PR
    can close the blind spot without also being the PR that silently
    redacts 21 files nobody asked this group to touch. Every other capture
    directory in the tree, including one added anywhere after this script
    was written, is scanned and gated for real. The dogfood report body is
    scanned separately, when present.

    Coverage limitation (finding #9): every check here is a JSON-field-
    shaped or machine-identity-shaped pattern match. A dogfood report's
    prose can still name a real target (an application, a document, a
    person) outside any of those shapes, and no rule below can see that -
    a prose-aware rule built on the same regex approach would be exactly as
    unfalsifiable as a "read carefully" comment, so this gate states the
    gap instead of claiming coverage it does not have. Report authors
    remain responsible for prose review; nothing here substitutes for it.

.PARAMETER Path
    Override the default capture roots (used by windows-e2e.yml to point
    at the live run's own capture directory instead). The allowlist below
    applies only to the default (derived) scan; an explicit -Path is
    scanned in full.
#>
[CmdletBinding()]
param(
    [string[]]$Path
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$scriptsDir = $PSScriptRoot
$repoRoot = Split-Path -Parent $scriptsDir

. (Join-Path $repoRoot 'probes\windows\common.ps1')
Import-Module (Join-Path $scriptsDir 'lib\capture-redaction-cli.psm1') -Force

$failed = 0
function Add-RedactionGateFailure {
    param([string]$Message)
    Write-Host "FAIL $Message"
    $script:failed = 1
}

<#
    Exact tracked paths only - never a directory or a pattern, so nothing
    new can hide behind an entry. Discovered by broadening the default scan
    from two hardcoded roots to every tracked "captures"/"*-captures"
    directory (finding #8); pending redaction by whichever group owns each
    corpus. Not accepted, not this group's file set to fix, and not to be
    added to for any newly-written capture - a fresh violation belongs in
    the code that wrote it, not in this list.
#>
$script:PreexistingUnredactedAllowlist = @(
    'docs/dogfood-reports/2026-08-03-001-captures/live-dogfood-run.json',
    'docs/dogfood-reports/2026-08-06-001-captures/actionability-dogfood-run.json',
    'docs/dogfood-reports/2026-08-07-001-captures/semantic-dogfood-run.json',
    'docs/dogfood-reports/2026-08-15-001-captures/signals-wait-parity-dogfood-run.json',
    'probes/windows/21-system-lifecycle/captures/lifecycle-manifest-ci.json',
    'probes/windows/21-system-lifecycle/captures/lifecycle-manifest-devbox.json',
    'probes/windows/22-capture-clipboard/captures/capture-manifest-devbox.json',
    'probes/windows/22-capture-clipboard/captures/capture-manifest-devbox.json.normalized',
    'probes/windows/captures/00-environment/environment.json',
    'probes/windows/captures/01-tree-dump/explorer.json',
    'probes/windows/captures/01-tree-dump/notepad.json',
    'probes/windows/captures/01-tree-dump/obsidian.json',
    'probes/windows/captures/01-tree-dump/settings.json',
    'probes/windows/captures/01-tree-dump/summary.json',
    'probes/windows/captures/05-interactions/focus.json',
    'probes/windows/captures/08-uia3-com/census.json',
    'probes/windows/captures/08-uia3-com/walker.json',
    'probes/windows/captures/09-elevation-uipi/uipi.json',
    'probes/windows/captures/10-session-dpi/session-dpi.json',
    'probes/windows/captures/11-electron-activation/electron-activation.json',
    'probes/windows/captures/12-private-file-io/ownership-under-elevation.json'
)

function Get-CaptureRedactionScanRelativePaths {
    <#
    .SYNOPSIS
        Finding #8's derived scan set: every tracked-or-untracked-and-not-
        ignored file under a path segment literally named "captures" or
        ending in "-captures", repo-root-relative with forward slashes.
        Mirrors check-e2e-windows-contract.ps1's rule 17 in method (git
        ls-files as the ground truth, not a hand-maintained root list) - the
        predicate excludes this gate's own fixtures under scripts/fixtures/
        (they carry deliberately-unredacted MUST-CATCH content and are
        proven separately above) and a filename like
        "capture-clipboard-surface" or "Capture22.cs" (no path segment of
        theirs is exactly "captures" or ends in "-captures").
    #>
    param([Parameter(Mandatory = $true)][string]$RepoRoot)
    Push-Location $RepoRoot
    try {
        $raw = git ls-files --cached --others --exclude-standard 2>$null
    } finally {
        Pop-Location
    }
    $captureFiles = @($raw | Where-Object {
            $segments = $_ -split '/'
            @($segments | Where-Object { $_ -ieq 'captures' -or $_ -imatch '-captures$' }).Count -gt 0
        })
    return , @($captureFiles | Sort-Object)
}

# --- self-test 1: the committed CLI-envelope fixtures ---
$fixturesRoot = Join-Path $scriptsDir 'fixtures\capture-redaction'
$mustCatchDir = Join-Path $fixturesRoot 'must-catch'
$mustPassDir = Join-Path $fixturesRoot 'must-pass'
$mustCatchFiles = @(Get-ChildItem -LiteralPath $mustCatchDir -File -ErrorAction SilentlyContinue)
$mustPassFiles = @(Get-ChildItem -LiteralPath $mustPassDir -File -ErrorAction SilentlyContinue)
if ($mustCatchFiles.Count -eq 0) { Add-RedactionGateFailure "no MUST-CATCH fixtures found under $mustCatchDir" }
if ($mustPassFiles.Count -eq 0) { Add-RedactionGateFailure "no MUST-PASS fixtures found under $mustPassDir" }
foreach ($file in $mustCatchFiles) {
    $violations = Get-CaptureCliRedactionViolations -Path $file.FullName
    if ($violations.Count -eq 0) {
        Add-RedactionGateFailure "self-test: MUST-CATCH fixture $($file.Name) produced no Test-CaptureCliRedaction violation"
    }
}
foreach ($file in $mustPassFiles) {
    $violations = Get-CaptureCliRedactionViolations -Path $file.FullName
    if ($violations.Count -gt 0) {
        Add-RedactionGateFailure "self-test: MUST-PASS fixture $($file.Name) produced $($violations.Count) Test-CaptureCliRedaction violation(s): $($violations -join '; ')"
    }
    if (-not (Test-CaptureRedaction -Path $file.FullName)) {
        Add-RedactionGateFailure "self-test: MUST-PASS fixture $($file.Name) failed Test-CaptureRedaction"
    }
}

# --- self-test 2: Test-CaptureRedaction's machine-identity rules, against
#     fixtures generated here and never committed. ---
$identityScratch = Join-Path ([IO.Path]::GetTempPath()) ('agent-desktop-redaction-selftest-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
New-Item -ItemType Directory -Path $identityScratch -Force | Out-Null
try {
    $utf8NoBom = New-Object System.Text.UTF8Encoding $false
    $usernameCase = Join-Path $identityScratch 'must-catch-username.json'
    [IO.File]::WriteAllText($usernameCase, ('{"probe":"self-test","note":"logged in as ' + $env:USERNAME + '"}'), $utf8NoBom)
    if (Test-CaptureRedaction -Path $usernameCase) {
        Add-RedactionGateFailure 'self-test: a generated fixture carrying the live USERNAME was not caught by Test-CaptureRedaction'
    }
    if ($env:USERPROFILE) {
        $pathCase = Join-Path $identityScratch 'must-catch-userprofile-path.json'
        [IO.File]::WriteAllText($pathCase, ('{"probe":"self-test","note":"staged under ' + $env:USERPROFILE + '\\Documents"}'), $utf8NoBom)
        if (Test-CaptureRedaction -Path $pathCase) {
            Add-RedactionGateFailure 'self-test: a generated fixture carrying the live USERPROFILE path was not caught by Test-CaptureRedaction'
        }
    }
    $cleanCase = Join-Path $identityScratch 'must-pass-clean.json'
    [IO.File]::WriteAllText($cleanCase, '{"probe":"self-test","measurable":true,"count":3}', $utf8NoBom)
    if (-not (Test-CaptureRedaction -Path $cleanCase)) {
        Add-RedactionGateFailure 'self-test: a clean generated fixture with no identity residue was rejected by Test-CaptureRedaction'
    }
} finally {
    Remove-Item -LiteralPath $identityScratch -Recurse -Force -ErrorAction SilentlyContinue
}

<#
    self-test 3 (finding #8): the tracked-file scan-set derivation actually
    reaches a capture directory planted anywhere in the tree, not only the
    two roots this gate used to hardcode. Planted under a real
    "*-captures" directory name and removed again before the real scan runs
    below - `git ls-files --others --exclude-standard` picks up an
    untracked file the same way it would pick up a contributor's own
    unstaged addition, so this is the same invert-verification a committed
    MUST-CATCH fixture would give, generated instead of committed because
    committing it would mean shipping a real USERNAME in the repository.
#>
$scanSetSelfTestDir = Join-Path $repoRoot 'probes\windows\zz-redaction-gate-selftest-captures'
try {
    New-Item -ItemType Directory -Path $scanSetSelfTestDir -Force | Out-Null
    $plantedFile = Join-Path $scanSetSelfTestDir 'plant.json'
    [IO.File]::WriteAllText($plantedFile, ('{"probe":"self-test","note":"logged in as ' + $env:USERNAME + '"}'), (New-Object System.Text.UTF8Encoding $false))
    $derivedForSelfTest = Get-CaptureRedactionScanRelativePaths -RepoRoot $repoRoot
    $plantedRel = 'probes/windows/zz-redaction-gate-selftest-captures/plant.json'
    if ($derivedForSelfTest -notcontains $plantedRel) {
        Add-RedactionGateFailure "self-test: the tracked-file scan-set derivation did not reach a freshly planted capture directory ($plantedRel) - finding #8 has regressed"
    }
} finally {
    if (Test-Path -LiteralPath $scanSetSelfTestDir) { Remove-Item -LiteralPath $scanSetSelfTestDir -Recurse -Force -ErrorAction SilentlyContinue }
}

if ($failed -ne 0) {
    Write-Host 'The redaction gate failed its own self-test; refusing to scan real captures on an untrusted gate.'
    exit 1
}
Write-Host 'OK: capture-redaction gate self-test passed (CLI-envelope fixtures, machine-identity fixtures, derived-scan-set reachability).'

# --- the real scan ---
$reportPath = Join-Path $repoRoot 'docs\dogfood-reports\2026-08-16-001-feat-windows-2-12-fixture-e2e-harness-dogfood.md'

function Test-FileRedaction {
    param([Parameter(Mandatory = $true)][string]$FullPath)
    $ok = $true
    if (-not (Test-CaptureRedaction -Path $FullPath)) {
        Add-RedactionGateFailure "$FullPath`: Test-CaptureRedaction residue (see warning above)"
        $ok = $false
    }
    foreach ($violation in (Get-CaptureCliRedactionViolations -Path $FullPath)) {
        Add-RedactionGateFailure $violation
        $ok = $false
    }
    return $ok
}

$scanned = 0
$allowlistedSkipped = 0
if ($Path) {
    foreach ($root in $Path) {
        if (-not (Test-Path -LiteralPath $root)) {
            Write-Host "SKIP: capture root does not exist yet: $root"
            continue
        }
        foreach ($file in (Get-ChildItem -LiteralPath $root -File -Recurse)) {
            $scanned++
            [void](Test-FileRedaction -FullPath $file.FullName)
        }
    }
} else {
    $captureRelPaths = Get-CaptureRedactionScanRelativePaths -RepoRoot $repoRoot
    $allowlistSet = New-Object System.Collections.Generic.HashSet[string]
    foreach ($entry in $script:PreexistingUnredactedAllowlist) { [void]$allowlistSet.Add($entry) }
    foreach ($rel in $captureRelPaths) {
        $full = Join-Path $repoRoot ($rel -replace '/', '\')
        if (-not (Test-Path -LiteralPath $full)) { continue }
        if ($allowlistSet.Contains($rel)) {
            $allowlistedSkipped++
            continue
        }
        $scanned++
        [void](Test-FileRedaction -FullPath $full)
    }
    <#
        A stale allowlist entry - one that no longer names a tracked
        capture file - is allowlist rot: it hides nothing today but invites
        someone to add a fresh violation under a path that reads as
        "already reviewed". Fail it the same way rule 17 fails a tracked
        file a walk cannot reach.
    #>
    foreach ($stale in $script:PreexistingUnredactedAllowlist) {
        if ($captureRelPaths -notcontains $stale) {
            Add-RedactionGateFailure "allowlist entry no longer names a tracked capture file - remove it from `$script:PreexistingUnredactedAllowlist: $stale"
        }
    }
    if ($allowlistedSkipped -gt 0) {
        Write-Host "WARN: $allowlistedSkipped pre-existing capture file(s) skipped via the named allowlist in this script - not accepted, pending redaction by the owning corpus (finding #8)."
    }
}
if (Test-Path -LiteralPath $reportPath) {
    $scanned++
    [void](Test-FileRedaction -FullPath $reportPath)
} elseif (-not $Path) {
    Write-Host "SKIP: dogfood report not written yet: $reportPath"
}

<#
    Zero scanned is only suspicious against the default, always-populated
    corpus roots - it is how a capture added later without ever being
    wired into this gate would go undetected. An explicit -Path override
    (windows-e2e.yml pointing at a live run's own capture directory) may
    legitimately not exist yet on a run that produced no captures, and
    upload-artifact's own if-no-files-found already handles that case; this
    gate must not invent a failure a caller who asked for a specific,
    possibly-empty directory did not.
#>
if ($scanned -eq 0 -and -not $Path) {
    Add-RedactionGateFailure 'zero files scanned over the default capture roots - a redaction gate handed nothing to check is not a check'
}

if ($failed -ne 0) {
    Write-Host 'FAIL: the capture redaction gate found residue.'
    exit 1
}
Write-Host "OK: $scanned file(s) carry no unredacted machine-identity or CLI-envelope content ($allowlistedSkipped pre-existing file(s) allowlisted, not scanned - see `$script:PreexistingUnredactedAllowlist)."
Write-Host 'NOTE: this gate matches JSON-field and machine-identity shapes only - it does not read prose for an embedded real name (finding #9); report authors remain responsible for that review.'
exit 0
