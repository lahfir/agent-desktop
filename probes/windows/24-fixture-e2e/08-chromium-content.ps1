#Requires -Version 5.1
<#
.SYNOPSIS
    Content-staged Chromium/Electron STALE_REF rate, semantic-action proof,
    and a bounded menu attempt (area 24, sub-phase 2.12, U12).

.DESCRIPTION
    A24-5 measured `content_staged: false` for a restored-but-not-activated
    Electron target with no document open (30 nodes at max depth 10 - a
    shell reading, not a content reading), so the aggregate `STALE_REF` rate
    A17-8 wanted stayed unmeasurable. This probe stages real content first:
    a scratch Obsidian vault, registered as the sole vault in the user's
    `obsidian.json` (backed up and restored around the run) so the launch
    deterministically opens THIS vault rather than whatever vault was open
    before, carrying one markdown note with headings, links and task-list
    checkboxes across 30 sections. The note is opened with a real keystroke
    sequence (Ctrl+O, type, Enter via `SendKeys`/`AppActivate`, both of which
    inject through `keybd_event`, never a posted message) rather than through
    the product.

    Five legs, run against the shipped `agent-desktop.exe`, exercising the
    same graded resolver A17-8 could not reach:

      1. Negative control (runs FIRST): the vault is registered and launched
         with NO note opened. `snapshot --max-depth 40` is taken and must
         NOT reach R18's threshold (150 nodes at depth >= 12 below the
         RootWebArea document root) - proving the threshold discriminates a
         content tree from a shell tree, which A24-5 lacked.
      2. Content precondition: the note is opened; the same measurement MUST
         reach the threshold, or the leg fails rather than reporting a rate
         over an empty tree.
      3. Rate: N `is --property checked` resolve attempts against refs
         allocated in the staged content, interleaved with unrelated
         `list-windows` observations, classified ok / STALE_REF / other.
      4. Semantic: `click` on a positive-area checkbox inside the content
         (Toggle/Click available, not offscreen), independently re-observed
         via `is --property checked`.
      5. Menu: a bounded attempt (Alt-tap and a content-area right-click via
         raw `SendInput`) evaluated against the two sources
         `crates/windows/src/system/menu_state.rs`'s `menu_is_open` composes
         - `GetGUIThreadInfo` per-thread flags and a UIA Menu/MenuBar/
         MenuItem search - read here directly rather than through the
         product, plus a host-wide search for another Chromium/Electron
         shell (VS Code, Slack, Teams, Edge).

    Captures under captures\ as chromium-content-{devbox,ci}.json (+
    .normalized twin). Corpus safety: node counts, depths, booleans and
    symbolic branch/role strings only - no window titles, file paths, pids,
    machine names, user names or message text ever reach the capture. Every
    Obsidian process this probe spawns is removed on every exit path, and
    `obsidian.json` is restored from its own on-disk backup (never held only
    in memory) before the probe returns.
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'ci')][string]$Label = 'devbox',
    [string]$AgentDesktopPath,
    [int]$RateSampleCount = 8
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

. (Join-Path (Split-Path -Parent $PSCommandPath) '..\common.ps1')
Initialize-ProbeRedaction
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName Microsoft.VisualBasic
Add-Type -AssemblyName System.Web.Extensions

$script:Probe = '24-fixture-e2e-08-chromium-content'
$script:ProbeDir = Split-Path -Parent $PSCommandPath
$script:CaptureDir = Join-Path $script:ProbeDir 'captures'
if (-not (Test-Path -LiteralPath $script:CaptureDir)) {
    New-Item -ItemType Directory -Path $script:CaptureDir -Force | Out-Null
}
$script:Spawned = New-Object System.Collections.Generic.List[int]
$script:ObsidianJsonPath = Join-Path $env:APPDATA 'obsidian\obsidian.json'
$script:ObsidianJsonBackupPath = Join-Path ([IO.Path]::GetTempPath()) ('a24-08-obsidian-json-backup-' + [guid]::NewGuid().ToString('N') + '.json')
$script:ObsidianJsonBackedUp = $false
$script:MinDepthNodes = 150
$script:MinDepth = 12
$script:MaxSnapshotDepth = 40
$script:SnapshotTimeoutMs = 15000

Register-MandatoryCapture -Name @("chromium-content-$Label.json")

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

function ConvertFrom-A24ChromiumJson {
    <# The same RecursionLimit/MaxJsonLength-raised JavaScriptSerializer
       Harness.psm1's ConvertFrom-AgentJson uses (R12): a content-staged
       snapshot nests two levels per UI level on top of the envelope and a
       ControlView walk into a rendered document reaches well past
       ConvertFrom-Json's own measured 101-level ceiling. #>
    param([Parameter(Mandatory = $true)][string]$Json)
    $serializer = New-Object System.Web.Script.Serialization.JavaScriptSerializer
    $serializer.RecursionLimit = 4096
    $serializer.MaxJsonLength = 67108864
    return $serializer.DeserializeObject($Json)
}

function Get-A24AgentDesktopPath {
    if ($AgentDesktopPath) { return $AgentDesktopPath }
    $repoRoot = Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $script:ProbeDir))
    return Join-Path $repoRoot 'target\release\agent-desktop.exe'
}

function Invoke-A24AgentDesktop {
    param([Parameter(Mandatory = $true)][string[]]$Arguments)
    $exe = Get-A24AgentDesktopPath
    $lines = & $exe @Arguments
    $joined = ($lines -join "`n")
    return ConvertFrom-A24ChromiumJson -Json $joined
}

function Register-A24SpawnedPid {
    param([int]$ProcessId)
    if ($ProcessId -gt 0 -and -not $script:Spawned.Contains($ProcessId)) { [void]$script:Spawned.Add($ProcessId) }
}

function Stop-A24AllSpawned {
    Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue | ForEach-Object {
        try { Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue } catch { }
    }
    $script:Spawned.Clear()
}

# ------------------------------------------------------------ vault + registry staging

function New-A24ScratchVault {
    <# `.obsidian/app.json` alone (no vault-registry entry, no user-data-dir
       isolation) is what A24-5's own methodology used; content richness is
       new here. 30 sections of heading + paragraph + two task-list
       checkboxes each is well past the node count a single short note
       produces (measured: this shape reaches 245+ nodes at depth >= 12,
       comfortably past the 150 threshold with margin). #>
    $vault = Join-Path ([IO.Path]::GetTempPath()) ('a24-08-vault-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $vault -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $vault '.obsidian') -Force | Out-Null
    $utf8NoBom = New-Object System.Text.UTF8Encoding $false
    [IO.File]::WriteAllText((Join-Path $vault '.obsidian\app.json'), '{"newFileLocation":"root","alwaysUpdateLinks":false}', $utf8NoBom)

    $lines = New-Object System.Collections.Generic.List[string]
    $lines.Add('# Probe24 Content Note')
    $lines.Add('')
    for ($i = 1; $i -le 30; $i++) {
        $lines.Add("## Section $i")
        $lines.Add('')
        $lines.Add("Paragraph $i with **bold text**, *italic text*, and a [link](https://example.invalid/$i).")
        $lines.Add('')
        $lines.Add("- [ ] task item $i alpha")
        $lines.Add("- [ ] task item $i bravo")
        $lines.Add('')
    }
    [IO.File]::WriteAllText((Join-Path $vault 'content-note.md'), ($lines -join "`r`n"), $utf8NoBom)
    return $vault
}

function Set-A24SoleVaultRegistry {
    <# Overwrites the user's real `obsidian.json` so the vault Obsidian
       auto-opens is deterministically this scratch vault rather than
       whatever vault was previously registered `"open": true` - measured
       live: without this, a command-line vault argument is not honoured
       once any vault is already registered open, and Obsidian instead
       restores its previously-open vault. Backed up to a FILE (not a
       variable) before being overwritten, because a variable does not
       survive this probe crashing before its `finally` runs. #>
    param([Parameter(Mandatory = $true)][string]$VaultPath)
    $parent = Split-Path -Parent $script:ObsidianJsonPath
    if (-not (Test-Path -LiteralPath $parent)) { New-Item -ItemType Directory -Path $parent -Force | Out-Null }
    if (Test-Path -LiteralPath $script:ObsidianJsonPath) {
        Copy-Item -LiteralPath $script:ObsidianJsonPath -Destination $script:ObsidianJsonBackupPath -Force
        $script:ObsidianJsonBackedUp = $true
    }
    $utf8NoBom = New-Object System.Text.UTF8Encoding $false
    $vaultKey = [guid]::NewGuid().ToString('N').Substring(0, 16)
    $vaultEscaped = $VaultPath -replace '\\', '\\\\'
    $registry = '{"vaults":{"' + $vaultKey + '":{"path":"' + $vaultEscaped + '","ts":' + [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() + ',"open":true}}}'
    [IO.File]::WriteAllText($script:ObsidianJsonPath, $registry, $utf8NoBom)
}

function Restore-A24VaultRegistry {
    if ($script:ObsidianJsonBackedUp -and (Test-Path -LiteralPath $script:ObsidianJsonBackupPath)) {
        Copy-Item -LiteralPath $script:ObsidianJsonBackupPath -Destination $script:ObsidianJsonPath -Force
        Remove-Item -LiteralPath $script:ObsidianJsonBackupPath -Force -ErrorAction SilentlyContinue
    }
}

function Start-A24ObsidianOnVault {
    <# Kills any running Obsidian first (a running instance forwards argv to
       itself and ignores the registry this function just wrote), then
       launches against the freshly-registered sole vault and waits for a
       titled top-level window naming the vault's own folder leaf - the
       title carries the vault folder name, never a path, so this check
       stays inside the corpus's own safety envelope. #>
    param([Parameter(Mandatory = $true)][string]$VaultPath, [int]$TimeoutSeconds = 20)
    Stop-A24AllSpawned
    Start-Sleep -Seconds 2
    $obsidianExe = Join-Path $env:LOCALAPPDATA 'Programs\Obsidian\Obsidian.exe'
    if (-not (Test-Path -LiteralPath $obsidianExe)) { return $null }
    $launched = Start-Process -FilePath $obsidianExe -PassThru
    Register-A24SpawnedPid -ProcessId $launched.Id
    $vaultLeaf = Split-Path -Leaf $VaultPath
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    $proc = $null
    while ((Get-Date) -lt $deadline) {
        $proc = Get-Process -Name 'Obsidian' -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowTitle -and $_.MainWindowTitle.Contains($vaultLeaf) } | Select-Object -First 1
        if ($proc) { break }
        Start-Sleep -Milliseconds 500
    }
    return $proc
}

function Open-A24NoteByQuickSwitch {
    <# Ctrl+O -> type -> Enter through SendKeys/AppActivate, both of which
       inject via `keybd_event` - real synthesized input the OS delivers
       through normal input processing, never a message posted directly
       into a window's queue. Verified by title change, not assumed. #>
    param([Parameter(Mandatory = $true)]$Process, [Parameter(Mandatory = $true)][string]$NoteStem, [int]$TimeoutSeconds = 10)
    [Microsoft.VisualBasic.Interaction]::AppActivate($Process.Id) | Out-Null
    Start-Sleep -Milliseconds 500
    [System.Windows.Forms.SendKeys]::SendWait('^o')
    Start-Sleep -Milliseconds 800
    [System.Windows.Forms.SendKeys]::SendWait($NoteStem)
    Start-Sleep -Milliseconds 800
    [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        $p = Get-Process -Id $Process.Id -ErrorAction SilentlyContinue
        if ($p -and $p.MainWindowTitle -and $p.MainWindowTitle.StartsWith($NoteStem)) { return $true }
        Start-Sleep -Milliseconds 300
    }
    return $false
}

# ------------------------------------------------------------ tree measurement

function Get-A24DocumentRootAndDepthCount {
    <# Walks the compact `snapshot` tree looking for the Chromium
       `RootWebArea` (native_id value) and counts DESCENDANTS OF THAT NODE
       whose own ABSOLUTE depth (from the window root the snapshot walk
       started at) is >= $script:MinDepth (12) - "at least 150 nodes below
       the document root at depth >= 12" (R18) read as two independent
       predicates over the same node (a descendant of the document root,
       and at or past the fixed absolute depth), not as "12 levels below
       the document root itself" - the document root already sits at depth
       9 in this fixture's own chrome, so the latter reading would demand a
       cutoff the CLI's `--max-depth 40` ceiling can comfortably reach but
       real content need not. #>
    param([Parameter(Mandatory = $true)]$Tree)
    $script:a24DocDepth = $null
    $script:a24MaxDepth = 0
    $script:a24Count = 0
    function Walk-DepthCount($n, $d, $underDocRoot) {
        $nid = $null
        if ($n.ContainsKey('native_id') -and $n['native_id']) { $nid = $n['native_id']['value'] }
        $isDocRoot = ($nid -eq 'RootWebArea')
        if ($isDocRoot -and $script:a24DocDepth -eq $null) { $script:a24DocDepth = $d }
        $inDocSubtree = ($underDocRoot -or $isDocRoot)
        if ($inDocSubtree -and -not $isDocRoot -and $d -ge $script:MinDepth) { $script:a24Count++ }
        if ($d -gt $script:a24MaxDepth) { $script:a24MaxDepth = $d }
        if ($n.ContainsKey('children')) { foreach ($ch in $n['children']) { Walk-DepthCount $ch ($d + 1) $inDocSubtree } }
    }
    Walk-DepthCount $Tree 0 $false
    return [ordered]@{
        document_root_found              = ($script:a24DocDepth -ne $null)
        document_root_depth              = $script:a24DocDepth
        threshold_depth_absolute          = $script:MinDepth
        nodes_at_or_past_threshold_depth  = $script:a24Count
        max_depth                        = $script:a24MaxDepth
    }
}

function Find-A24PositiveAreaCheckboxRef {
    param([Parameter(Mandatory = $true)]$Tree)
    $script:a24Ref = $null
    function Walk($n) {
        if ($script:a24Ref -eq $null -and $n['role'] -eq 'checkbox') {
            $states = @(); if ($n.ContainsKey('states')) { $states = $n['states'] }
            $acts = @(); if ($n.ContainsKey('available_actions')) { $acts = $n['available_actions'] }
            if (($states -notcontains 'offscreen') -and ($acts -contains 'Click')) { $script:a24Ref = $n['ref_id'] }
        }
        if ($n.ContainsKey('children')) { foreach ($ch in $n['children']) { Walk $ch } }
    }
    Walk $Tree
    return $script:a24Ref
}

function Get-A24AllRefs {
    param([Parameter(Mandatory = $true)]$Tree)
    $refs = New-Object System.Collections.Generic.List[string]
    function Walk($n) {
        if ($n.ContainsKey('ref_id') -and $n['ref_id']) { [void]$refs.Add($n['ref_id']) }
        if ($n.ContainsKey('children')) { foreach ($ch in $n['children']) { Walk $ch } }
    }
    Walk $Tree
    return $refs
}

# ------------------------------------------------------------ leg a/b: precondition + negative control

function Measure-A24ContentPrecondition {
    param([Parameter(Mandatory = $true)][string]$Label)
    $snap = Invoke-A24AgentDesktop -Arguments @('snapshot', '--app', 'Obsidian.exe', '--max-depth', [string]$script:MaxSnapshotDepth, '--timeout-ms', [string]$script:SnapshotTimeoutMs)
    if (-not $snap -or $snap['ok'] -ne $true) {
        $errorCode = $null
        if ($snap -and $snap['error']) { $errorCode = $snap['error']['code'] }
        return [ordered]@{ measurable = $false; branch = 'snapshot_failed'; label = $Label; error_code = $errorCode; meets_threshold = $false }
    }
    $shape = Get-A24DocumentRootAndDepthCount -Tree $snap['data']['tree']
    $meetsThreshold = ($shape.document_root_found -and $shape.nodes_at_or_past_threshold_depth -ge $script:MinDepthNodes)
    $preconditionBranch = 'below_threshold'
    if ($meetsThreshold) { $preconditionBranch = 'content_tree_meets_threshold' }
    return [ordered]@{
        measurable          = $true
        label                = $Label
        snapshot_id          = $snap['data']['snapshot_id']
        ref_count            = $snap['data']['ref_count']
        complete              = $snap['data']['complete']
        shape                 = $shape
        threshold_nodes       = $script:MinDepthNodes
        threshold_depth       = $script:MinDepth
        meets_threshold       = $meetsThreshold
        branch                = $preconditionBranch
    }
}

# ------------------------------------------------------------ leg c: rate

function Measure-A24StaleRefRate {
    <# Returns via an explicitly-built `OrderedDictionary` rather than an
       `[ordered]@{...}` literal: measured live, a literal whose `attempts`
       value is an array of OTHER `[ordered]@{...}` literals fails at the
       `return` statement itself with "Argument types do not match" -
       PowerShell 5.1 misresolves the nested-literal construction. Every
       other single-level `[ordered]@{...}` return in this file is
       unaffected; only this doubly-nested shape needs the explicit form. #>
    param($Refs, $SnapshotId, $N)
    $attempts = New-Object System.Collections.Generic.List[object]
    $okCount = 0
    $staleCount = 0
    $otherCount = 0
    for ($i = 0; $i -lt $N; $i++) {
        $ref = $Refs[$i % $Refs.Count]
        $result = Invoke-A24AgentDesktop -Arguments @('is', $ref, '--snapshot', $SnapshotId, '--property', 'checked')
        $status = 'other'
        if ($result -and $result['ok'] -eq $true) { $status = 'ok'; $okCount++ }
        elseif ($result -and $result['error'] -and $result['error']['code'] -eq 'STALE_REF') { $status = 'stale_ref'; $staleCount++ }
        else { $otherCount++ }
        $attemptErrorCode = $null
        if ($result -and $result['error']) { $attemptErrorCode = $result['error']['code'] }
        $attemptRecord = New-Object System.Collections.Specialized.OrderedDictionary
        $attemptRecord['index'] = $i
        $attemptRecord['status'] = $status
        $attemptRecord['error_code'] = $attemptErrorCode
        [void]$attempts.Add($attemptRecord)
        Invoke-A24AgentDesktop -Arguments @('list-windows') | Out-Null
    }
    $staleRateVal = 0.0
    if ([int]$N -gt 0) { $staleRateVal = [double]$staleCount / [double]$N }
    $out = New-Object System.Collections.Specialized.OrderedDictionary
    $out['measurable'] = $true
    $out['n'] = $N
    $out['ok_count'] = $okCount
    $out['stale_count'] = $staleCount
    $out['other_count'] = $otherCount
    $out['stale_rate'] = $staleRateVal
    $out['attempts'] = $attempts.ToArray()
    return $out
}

# ------------------------------------------------------------ leg d: semantic

function Measure-A24SemanticClick {
    param([Parameter(Mandatory = $true)][string]$Ref, [Parameter(Mandatory = $true)][string]$SnapshotId)
    $clickResult = Invoke-A24AgentDesktop -Arguments @('click', $Ref, '--snapshot', $SnapshotId)
    $clickOk = ($clickResult -and $clickResult['ok'] -eq $true)
    $clickErrorCode = $null
    if (-not $clickOk -and $clickResult -and $clickResult['error']) { $clickErrorCode = $clickResult['error']['code'] }
    $reobserve = $null
    $reobserveChecked = $null
    if ($clickOk) {
        $reobserve = Invoke-A24AgentDesktop -Arguments @('is', $Ref, '--snapshot', $SnapshotId, '--property', 'checked')
        if ($reobserve -and $reobserve['ok'] -eq $true) { $reobserveChecked = [bool]$reobserve['data']['result'] }
    }
    $branch = if ($clickOk -and $reobserveChecked -eq $true) { 'click_verified_by_independent_reobservation' }
    elseif ($clickOk) { 'click_ok_but_reobservation_did_not_confirm' }
    else { 'click_did_not_deliver' }
    return [ordered]@{
        measurable          = $true
        target_ref_role      = 'checkbox'
        click_ok              = $clickOk
        click_error_code       = $clickErrorCode
        reobserved_checked     = $reobserveChecked
        branch                 = $branch
    }
}

# ------------------------------------------------------------ leg e: menu (raw two-source oracle + host search)

function Get-A24ClassicMenuMode {
    <# Mirrors `crates/windows/src/system/menu_state.rs`'s `classic_menu_mode_active`:
       every thread of the target pid read through `GetGUIThreadInfo`,
       looking for GUI_INMENUMODE|GUI_POPUPMENUMODE|GUI_SYSTEMMENUMODE. #>
    param([Parameter(Mandatory = $true)][int]$ProcessId)
    if (-not ('A24Chromium.MenuNative' -as [type])) {
        Add-Type -Namespace A24Chromium -Name MenuNative -MemberDefinition @'
[System.Runtime.InteropServices.StructLayout(System.Runtime.InteropServices.LayoutKind.Sequential)]
public struct GUITHREADINFO {
    public int cbSize; public uint flags;
    public System.IntPtr hwndActive, hwndFocus, hwndCapture, hwndMenuOwner, hwndMoveSize, hwndCaret;
    public int rcCaretLeft, rcCaretTop, rcCaretRight, rcCaretBottom;
}
[System.Runtime.InteropServices.StructLayout(System.Runtime.InteropServices.LayoutKind.Sequential)]
public struct THREADENTRY32 { public uint dwSize, cntUsage, th32ThreadID, th32OwnerProcessID; public int tpBasePri, tpDeltaPri; public uint dwFlags; }
[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern bool GetGUIThreadInfo(uint idThread, ref GUITHREADINFO lpgui);
[System.Runtime.InteropServices.DllImport("kernel32.dll", SetLastError = true)] public static extern System.IntPtr CreateToolhelp32Snapshot(uint dwFlags, uint th32ProcessID);
[System.Runtime.InteropServices.DllImport("kernel32.dll")] public static extern bool Thread32First(System.IntPtr hSnapshot, ref THREADENTRY32 lpte);
[System.Runtime.InteropServices.DllImport("kernel32.dll")] public static extern bool Thread32Next(System.IntPtr hSnapshot, ref THREADENTRY32 lpte);
[System.Runtime.InteropServices.DllImport("kernel32.dll")] public static extern bool CloseHandle(System.IntPtr h);
'@
    }
    $TH32CS_SNAPTHREAD = 0x00000004
    $GUI_INMENUMODE = 0x0004; $GUI_POPUPMENUMODE = 0x0010; $GUI_SYSTEMMENUMODE = 0x0008
    $invalid = [IntPtr]::new(-1)
    $snap = [A24Chromium.MenuNative]::CreateToolhelp32Snapshot($TH32CS_SNAPTHREAD, 0)
    if ($snap -eq [IntPtr]::Zero -or $snap -eq $invalid) { return $false }
    try {
        $te = New-Object A24Chromium.MenuNative+THREADENTRY32
        $te.dwSize = [System.Runtime.InteropServices.Marshal]::SizeOf($te)
        $any = $false
        if ([A24Chromium.MenuNative]::Thread32First($snap, [ref]$te)) {
            do {
                if ($te.th32OwnerProcessID -eq [uint32]$ProcessId) {
                    $gti = New-Object A24Chromium.MenuNative+GUITHREADINFO
                    $gti.cbSize = [System.Runtime.InteropServices.Marshal]::SizeOf($gti)
                    if ([A24Chromium.MenuNative]::GetGUIThreadInfo($te.th32ThreadID, [ref]$gti)) {
                        if (($gti.flags -band ($GUI_INMENUMODE -bor $GUI_POPUPMENUMODE -bor $GUI_SYSTEMMENUMODE)) -ne 0) { $any = $true }
                    }
                }
                $te.dwSize = [System.Runtime.InteropServices.Marshal]::SizeOf($te)
            } while ([A24Chromium.MenuNative]::Thread32Next($snap, [ref]$te))
        }
        return $any
    } finally {
        [void][A24Chromium.MenuNative]::CloseHandle($snap)
    }
}

function Get-A24UiaMenuFamilyReachable {
    <# Mirrors `uia_menu_reachable`: any root-level UIA child of the pid
       carrying a reachable Menu/MenuBar/MenuItem descendant. #>
    param([Parameter(Mandatory = $true)][int]$ProcessId)
    try {
        $root = [System.Windows.Automation.AutomationElement]::RootElement
        $cond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ProcessIdProperty, $ProcessId)
        $children = $root.FindAll([System.Windows.Automation.TreeScope]::Children, $cond)
        $menuFamily = New-Object System.Windows.Automation.OrCondition(@(
                (New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Menu)),
                (New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::MenuBar)),
                (New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::MenuItem))
            ))
        foreach ($c in $children) {
            try {
                if ($null -ne $c.FindFirst([System.Windows.Automation.TreeScope]::Subtree, $menuFamily)) { return $true }
            } catch { }
        }
    } catch { }
    return $false
}

function Get-A24MenuSample {
    param([Parameter(Mandatory = $true)][int]$ProcessId)
    return [ordered]@{
        classic_any_thread_menu_mode = (Get-A24ClassicMenuMode -ProcessId $ProcessId)
        uia_menu_family_reachable    = (Get-A24UiaMenuFamilyReachable -ProcessId $ProcessId)
    }
}

function Invoke-A24SendInputAltTap {
    if (-not ('A24Chromium.InputNative' -as [type])) {
        Add-Type -Namespace A24Chromium -Name InputNative -MemberDefinition @'
[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern void keybd_event(byte bVk, byte bScan, uint dwFlags, System.UIntPtr dwExtraInfo);
[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern void mouse_event(uint dwFlags, uint dx, uint dy, uint dwData, System.UIntPtr dwExtraInfo);
'@
    }
    $VK_MENU = 0x12; $KEYEVENTF_KEYUP = 0x0002
    [A24Chromium.InputNative]::keybd_event($VK_MENU, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [A24Chromium.InputNative]::keybd_event($VK_MENU, 0, $KEYEVENTF_KEYUP, [UIntPtr]::Zero)
}

function Invoke-A24SendInputRightClick {
    param([Parameter(Mandatory = $true)][int]$X, [Parameter(Mandatory = $true)][int]$Y)
    $MOUSEEVENTF_RIGHTDOWN = 0x0008; $MOUSEEVENTF_RIGHTUP = 0x0010
    [A24Chromium.InputNative]::SetCursorPos($X, $Y) | Out-Null
    Start-Sleep -Milliseconds 60
    [A24Chromium.InputNative]::mouse_event($MOUSEEVENTF_RIGHTDOWN, 0, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [A24Chromium.InputNative]::mouse_event($MOUSEEVENTF_RIGHTUP, 0, 0, 0, [UIntPtr]::Zero)
}

function Invoke-A24EscapeClose {
    [A24Chromium.InputNative]::keybd_event(0x1B, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 40
    [A24Chromium.InputNative]::keybd_event(0x1B, 0, 0x0002, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 300
}

function Find-A24OtherChromiumTargets {
    <# Deferral item 4's host-wide search, trimmed to the standard install
       locations 03-chromium-target.ps1's fuller scan already covers - a
       fresh discovery pass here is what keeps the deferral resting on a
       search taken THIS run rather than only on 03's earlier one. #>
    $found = New-Object System.Collections.Generic.List[string]
    $pf = $env:ProgramFiles
    $pfx86 = ${env:ProgramFiles(x86)}
    $lad = $env:LOCALAPPDATA
    $candidates = @(
        @{ Path = (Join-Path $pf 'Microsoft\Edge\Application\msedge.exe'); Name = 'edge' }
        @{ Path = (Join-Path $pf 'Google\Chrome\Application\chrome.exe'); Name = 'chrome' }
        @{ Path = (Join-Path $pfx86 'Google\Chrome\Application\chrome.exe'); Name = 'chrome_x86' }
        @{ Path = (Join-Path $pf 'BraveSoftware\Brave-Browser\Application\brave.exe'); Name = 'brave' }
        @{ Path = (Join-Path $lad 'Microsoft\Teams\current\Teams.exe'); Name = 'teams' }
        @{ Path = (Join-Path $lad 'Programs\Microsoft VS Code\Code.exe'); Name = 'vscode' }
        @{ Path = (Join-Path $lad 'slack\slack.exe'); Name = 'slack' }
    )
    foreach ($c in $candidates) {
        if ($c.Path -and (Test-Path -LiteralPath $c.Path -PathType Leaf)) { [void]$found.Add($c.Name) }
    }
    return [ordered]@{ searched = @($candidates | ForEach-Object { $_.Name }); found = @($found) }
}

function Measure-A24MenuAttempt {
    param([Parameter(Mandatory = $true)]$Process, [Parameter(Mandatory = $true)]$WindowRect)
    $idle = Get-A24MenuSample -ProcessId $Process.Id
    $idleAllFalse = (-not $idle.classic_any_thread_menu_mode) -and (-not $idle.uia_menu_family_reachable)

    [Microsoft.VisualBasic.Interaction]::AppActivate($Process.Id) | Out-Null
    Start-Sleep -Milliseconds 400
    Invoke-A24SendInputAltTap
    Start-Sleep -Milliseconds 700
    $afterAlt = Get-A24MenuSample -ProcessId $Process.Id
    Invoke-A24EscapeClose

    $cx = [int](($WindowRect.Left + $WindowRect.Right) / 2)
    $cy = [int](($WindowRect.Top + $WindowRect.Bottom) / 2)
    Invoke-A24SendInputRightClick -X $cx -Y $cy
    Start-Sleep -Milliseconds 700
    $afterRightClick = Get-A24MenuSample -ProcessId $Process.Id
    Invoke-A24EscapeClose

    $anyStaged = ($afterAlt.classic_any_thread_menu_mode -or $afterAlt.uia_menu_family_reachable -or $afterRightClick.classic_any_thread_menu_mode -or $afterRightClick.uia_menu_family_reachable)
    $hostSearch = Find-A24OtherChromiumTargets
    $menuBranch = 'no_menu_surface_staged_by_either_method'
    if ($anyStaged) { $menuBranch = 'at_least_one_menu_surface_staged' }

    return [ordered]@{
        measurable          = $true
        idle_all_false        = $idleAllFalse
        idle                   = $idle
        after_alt_tap          = $afterAlt
        after_right_click       = $afterRightClick
        branch                  = $menuBranch
        cites                   = @('A23-3', 'A24-6')
        host_wide_search        = $hostSearch
    }
}

# ------------------------------------------------------------ main

$question = 'against a Chromium/Electron target with real content actually staged: (a) what is the aggregate STALE_REF rate for refs allocated in that content, with a threshold-negative-control proving the threshold discriminates content from a shell; (b) can a positive-area content leaf be clicked through the semantic tier, verified by independent re-observation; (c) with the vault-configured session this run stages, is either shipped menu-detector source staged by an Alt-tap or a content-area right-click, and does a host-wide search find another Chromium/Electron shell'

$result = $null
$vault = $null
try {
    $obsidianExe = Join-Path $env:LOCALAPPDATA 'Programs\Obsidian\Obsidian.exe'
    if (-not (Test-Path -LiteralPath $obsidianExe -PathType Leaf)) {
        $result = [ordered]@{ probe = $script:Probe; question = $question; measurable = $false; branch = 'chromium_target_absent' }
    } else {
        $vault = New-A24ScratchVault
        Set-A24SoleVaultRegistry -VaultPath $vault

        $procNoDoc = Start-A24ObsidianOnVault -VaultPath $vault
        if (-not $procNoDoc) {
            $result = [ordered]@{ probe = $script:Probe; question = $question; measurable = $false; branch = 'obsidian_never_presented_a_titled_window' }
        } else {
            $negativeControl = Measure-A24ContentPrecondition -Label 'no_document_open'
            $negativeControlDiscriminates = (-not $negativeControl.meets_threshold)

            $opened = Open-A24NoteByQuickSwitch -Process $procNoDoc -NoteStem 'content-note'
            $precondition = Measure-A24ContentPrecondition -Label 'document_open'

            $rate = $null
            $semantic = $null
            $menu = $null
            if ($precondition.measurable -and $precondition.meets_threshold) {
                $snap = Invoke-A24AgentDesktop -Arguments @('snapshot', '--app', 'Obsidian.exe', '--max-depth', [string]$script:MaxSnapshotDepth, '--timeout-ms', [string]$script:SnapshotTimeoutMs)
                $allRefs = @(Get-A24AllRefs -Tree $snap['data']['tree'])
                $sampleRefs = if ($allRefs.Count -ge $RateSampleCount) { $allRefs[0..($RateSampleCount - 1)] } else { $allRefs }
                if ($sampleRefs.Count -gt 0) {
                    [string[]]$sampleRefsStr = @($sampleRefs | ForEach-Object { [string]$_ })
                    [string]$snapshotIdStr = [string]$snap['data']['snapshot_id']
                    [int]$rateN = [int]$RateSampleCount
                    $rate = Measure-A24StaleRefRate -Refs $sampleRefsStr -SnapshotId $snapshotIdStr -N $rateN
                } else {
                    $rate = [ordered]@{ measurable = $false; branch = 'no_refs_in_staged_content' }
                }

                $checkboxSnap = Invoke-A24AgentDesktop -Arguments @('snapshot', '--app', 'Obsidian.exe', '--max-depth', [string]$script:MaxSnapshotDepth, '--timeout-ms', [string]$script:SnapshotTimeoutMs)
                $checkboxRef = Find-A24PositiveAreaCheckboxRef -Tree $checkboxSnap['data']['tree']
                if ($checkboxRef) {
                    $semantic = Measure-A24SemanticClick -Ref $checkboxRef -SnapshotId $checkboxSnap['data']['snapshot_id']
                } else {
                    $semantic = [ordered]@{ measurable = $false; branch = 'no_positive_area_content_leaf_found' }
                }

                $proc2 = Get-Process -Id $procNoDoc.Id -ErrorAction SilentlyContinue
                if ($proc2) {
                    $rect = $proc2.MainWindowHandle
                    $winRect = $null
                    try {
                        $ae = [System.Windows.Automation.AutomationElement]::FromHandle($rect)
                        $r = $ae.Current.BoundingRectangle
                        $winRect = [ordered]@{ Left = [int]$r.Left; Top = [int]$r.Top; Right = [int]$r.Right; Bottom = [int]$r.Bottom }
                    } catch { $winRect = [ordered]@{ Left = 0; Top = 0; Right = 800; Bottom = 600 } }
                    $menu = Measure-A24MenuAttempt -Process $proc2 -WindowRect $winRect
                } else {
                    $menu = [ordered]@{ measurable = $false; branch = 'obsidian_process_gone_before_menu_leg' }
                }
            }

            $result = [ordered]@{
                probe                          = $script:Probe
                question                       = $question
                measurable                     = $true
                branch                         = 'chromium_target_measured'
                negative_control               = $negativeControl
                negative_control_discriminates = $negativeControlDiscriminates
                note_opened_confirmed          = $opened
                content_precondition           = $precondition
                stale_ref_rate                 = $rate
                semantic_action                = $semantic
                menu_attempt                   = $menu
            }
        }
    }
} catch {
    Write-ProbeLog -Message "probe threw: $($_.Exception.GetType().Name) at $($_.InvocationInfo.PositionMessage)" -Level 'error'
    $result = [ordered]@{ probe = $script:Probe; question = $question; measurable = $false; branch = 'probe_threw'; error_class = $_.Exception.GetType().Name; error_message_class = $_.Exception.Message.GetType().Name }
} finally {
    Stop-A24AllSpawned
    Restore-A24VaultRegistry
    if ($vault -and (Test-Path -LiteralPath $vault)) {
        Remove-Item -LiteralPath $vault -Recurse -Force -ErrorAction SilentlyContinue
    }
}

$capturePath = Write-A24Capture -Name "chromium-content-$Label.json" -Content (ConvertTo-Json -InputObject $result -Depth 20)
Register-MandatoryPass -Capture $capturePath -Result $result

Assert-MandatoryMeasurement -Probe $script:Probe -Label $Label

Write-ProbeResult -Probe $script:Probe -Status 'ok' -Message 'Chromium/Electron content-staged rate, semantic-action and menu-attempt probe captured' -Data @{
    capture = Split-Path -Leaf $capturePath
}
exit 0
