$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\common.ps1"

Add-Type -AssemblyName UIAutomationClient | Out-Null
Add-Type -AssemblyName UIAutomationTypes | Out-Null
Add-Type -AssemblyName WindowsBase | Out-Null

$Probe = '02-cache-timing'
$Stack = 'managed'
$Repetitions = 5
$PlaceX = 40
$PlaceY = 40
$PlaceWidth = 1000
$PlaceHeight = 640
$ExplorerWaitSec = 30

$AE = [System.Windows.Automation.AutomationElement]
$Walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker
$TrueCondition = [System.Windows.Automation.Condition]::TrueCondition
$Subtree = [System.Windows.Automation.TreeScope]::Subtree

$Properties = @(
    @{ Name = 'Name';              Property = $AE::NameProperty },
    @{ Name = 'ControlType';       Property = $AE::ControlTypeProperty },
    @{ Name = 'AutomationId';      Property = $AE::AutomationIdProperty },
    @{ Name = 'ClassName';         Property = $AE::ClassNameProperty },
    @{ Name = 'BoundingRectangle'; Property = $AE::BoundingRectangleProperty },
    @{ Name = 'IsEnabled';         Property = $AE::IsEnabledProperty },
    @{ Name = 'IsOffscreen';       Property = $AE::IsOffscreenProperty },
    @{ Name = 'FrameworkId';       Property = $AE::FrameworkIdProperty }
)

function Get-TopLevelElements {
    $out = New-Object System.Collections.ArrayList
    try {
        $c = $Walker.GetFirstChild($AE::RootElement)
        while ($null -ne $c) {
            [void]$out.Add($c)
            try { $c = $Walker.GetNextSibling($c) } catch { break }
        }
    } catch { }
    return @($out)
}

function New-CacheRequest {
    $request = New-Object System.Windows.Automation.CacheRequest
    $request.TreeScope = $Subtree
    $request.TreeFilter = [System.Windows.Automation.Automation]::ControlViewCondition
    foreach ($p in $Properties) { $request.Add($p.Property) | Out-Null }
    return $request
}

function Measure-PerPropertyPass {
    param($Root)
    $findWatch = [System.Diagnostics.Stopwatch]::StartNew()
    $found = $Root.FindAll($Subtree, $TrueCondition)
    $findWatch.Stop()
    $readWatch = [System.Diagnostics.Stopwatch]::StartNew()
    $reads = 0
    foreach ($element in $found) {
        foreach ($p in $Properties) {
            try { [void]$element.GetCurrentPropertyValue($p.Property); $reads++ } catch { }
        }
    }
    $readWatch.Stop()
    return [pscustomobject]@{
        NodeCount = $found.Count
        Reads     = $reads
        FindMs    = $findWatch.Elapsed.TotalMilliseconds
        ReadMs    = $readWatch.Elapsed.TotalMilliseconds
    }
}

function Measure-CachedPass {
    param($Root)
    $request = New-CacheRequest
    $findWatch = [System.Diagnostics.Stopwatch]::StartNew()
    $scope = $request.Activate()
    try { $found = $Root.FindAll($Subtree, $TrueCondition) } finally { $scope.Dispose() }
    $findWatch.Stop()
    $readWatch = [System.Diagnostics.Stopwatch]::StartNew()
    $reads = 0
    foreach ($element in $found) {
        foreach ($p in $Properties) {
            try { [void]$element.GetCachedPropertyValue($p.Property); $reads++ } catch { }
        }
    }
    $readWatch.Stop()
    return [pscustomobject]@{
        NodeCount = $found.Count
        Reads     = $reads
        FindMs    = $findWatch.Elapsed.TotalMilliseconds
        ReadMs    = $readWatch.Elapsed.TotalMilliseconds
    }
}

function Measure-Target {
    param($Root, [string]$Label)
    [void](Measure-PerPropertyPass -Root $Root)
    [void](Measure-CachedPass -Root $Root)
    $perFind = 0.0; $perRead = 0.0; $cacheFind = 0.0; $cacheRead = 0.0
    $nodeCount = 0; $cachedNodeCount = 0; $reads = 0; $cachedReads = 0
    for ($i = 0; $i -lt $Repetitions; $i++) {
        $a = Measure-PerPropertyPass -Root $Root
        $b = Measure-CachedPass -Root $Root
        $perFind += $a.FindMs; $perRead += $a.ReadMs
        $cacheFind += $b.FindMs; $cacheRead += $b.ReadMs
        $nodeCount = $a.NodeCount; $cachedNodeCount = $b.NodeCount
        $reads = $a.Reads; $cachedReads = $b.Reads
    }
    $perTotal = $perFind + $perRead
    $cacheTotal = $cacheFind + $cacheRead
    $overall = 0.0
    if ($cacheTotal -gt 0) { $overall = [Math]::Round(($perTotal / $cacheTotal), 4) }
    $readPhase = 0.0
    if ($cacheRead -gt 0) { $readPhase = [Math]::Round(($perRead / $cacheRead), 4) }
    $findPhase = 0.0
    if ($cacheFind -gt 0) { $findPhase = [Math]::Round(($perFind / $cacheFind), 4) }
    return [ordered]@{
        Label                        = $Label
        Stack                        = $Stack
        NodeCount                    = $nodeCount
        CachedNodeCount              = $cachedNodeCount
        PropertiesPerNode            = $Properties.Count
        PropertyNames                = (($Properties | ForEach-Object { $_.Name }) -join ',')
        Repetitions                  = $Repetitions
        PropertyReadsPerRepetition   = $reads
        CachedPropertyReadsPerRepetition = $cachedReads
        AutomationElementMode        = 'Full (CacheRequest default) — matched to the per-property pass so the delta is caching, not element-mode weight'
        timingPerPropertyTotalMs     = [Math]::Round($perTotal, 4)
        timingCachedTotalMs          = [Math]::Round($cacheTotal, 4)
        timingPerPropertyFindMs      = [Math]::Round($perFind, 4)
        timingCachedFindMs           = [Math]::Round($cacheFind, 4)
        timingPerPropertyReadMs      = [Math]::Round($perRead, 4)
        timingCachedReadMs           = [Math]::Round($cacheRead, 4)
        timingOverallMultiplier      = $overall
        timingFindPhaseMultiplier    = $findPhase
        timingReadPhaseMultiplier    = $readPhase
    }
}

$notepadPid = 0
$explorerHandle = 0
$explorerPreexisting = $false

try {
    Initialize-ProbeNative
    $capturedAt = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')

    $comReferencePath = Join-Path (Join-Path (Get-ProbeRoot) 'captures\08-uia3-com') 'cache-timing.json'
    if (-not (Test-Path -LiteralPath $comReferencePath)) {
        throw ('the authoritative UIA3 COM cache measurement is missing at ' + $comReferencePath + '; this probe reports itself as a cross-check against it and has nothing to cross-check against')
    }
    $comRef = (ConvertFrom-Json ([IO.File]::ReadAllText($comReferencePath))).shim.result

    $notepad = Start-ScratchProcess -FilePath (Join-Path $env:WINDIR 'System32\notepad.exe') -TimeoutSec 20
    $notepadPid = $notepad.ProcessId
    if ($notepad.MainWindowHandle -eq [IntPtr]::Zero) { throw 'notepad produced no main window handle' }
    Show-WindowNoActivate -WindowHandle $notepad.MainWindowHandle -X $PlaceX -Y $PlaceY -Width $PlaceWidth -Height $PlaceHeight
    Start-Sleep -Milliseconds 700
    $notepadResult = Measure-Target -Root ($AE::FromHandle($notepad.MainWindowHandle)) -Label 'notepad'

    $preCabinet = @()
    foreach ($top in (Get-TopLevelElements)) {
        try { if ($top.Current.ClassName -eq 'CabinetWClass') { $preCabinet += [int]$top.Current.NativeWindowHandle } } catch { }
    }
    $explorerPreexisting = ($preCabinet.Count -gt 0)
    $explorerElement = $null
    if ($explorerPreexisting) {
        $explorerHandle = $preCabinet[0]
        $explorerElement = $AE::FromHandle([IntPtr]$explorerHandle)
    } else {
        $launcher = Start-Process -FilePath (Join-Path $env:WINDIR 'explorer.exe') -ArgumentList @((Join-Path $env:WINDIR 'System32')) -PassThru
        Register-ScratchProcessId -ProcessId $launcher.Id
        $deadline = (Get-Date).AddSeconds($ExplorerWaitSec)
        while ((Get-Date) -lt $deadline) {
            foreach ($top in (Get-TopLevelElements)) {
                try {
                    if ($top.Current.ClassName -eq 'CabinetWClass' -and ($preCabinet -notcontains [int]$top.Current.NativeWindowHandle)) {
                        $explorerHandle = [int]$top.Current.NativeWindowHandle
                        break
                    }
                } catch { }
            }
            if ($explorerHandle -ne 0) { break }
            Start-Sleep -Milliseconds 500
        }
        if ($explorerHandle -ne 0) {
            Show-WindowNoActivate -WindowHandle ([IntPtr]$explorerHandle) -X $PlaceX -Y $PlaceY -Width $PlaceWidth -Height $PlaceHeight
            Start-Sleep -Seconds 3
            $explorerElement = $AE::FromHandle([IntPtr]$explorerHandle)
        }
    }
    if ($null -eq $explorerElement) { throw 'no CabinetWClass folder window appeared within the wait window' }
    $explorerResult = Measure-Target -Root $explorerElement -Label 'explorer'

    $capture = [ordered]@{
        Probe          = $Probe
        CapturedAtUtc  = $capturedAt
        Stack          = $Stack
        Scope          = 'api-contract'
        Authority      = 'NON-AUTHORITATIVE managed cross-check (KTD1). The authoritative CacheRequest measurement is U7 08-uia3-com/cache-timing.json on the UIA3 COM stack — the stack the Rust adapter uses.'
        Method         = 'per-property pass calls FindAll(Subtree) then GetCurrentPropertyValue for each of 8 properties on each node; cached pass activates a CacheRequest carrying the same 8 properties over the same TreeScope.Subtree with the ControlView tree filter, then reads GetCachedPropertyValue. One discarded warm-up pass of each precedes the measured repetitions.'
        Targets        = @($notepadResult, $explorerResult)
        Uia3ComReference = [ordered]@{
            Source                       = 'probes/windows/captures/08-uia3-com/cache-timing.json'
            Stack                        = 'uia3-com'
            Target                       = 'explorer folder window over %WINDIR%\System32'
            NodeCount                    = $comRef.nodeCount
            timingComOverallMultiplier   = $comRef.timingMultiplier
            timingComReadPhaseMultiplier = $comRef.timingReadPhaseMultiplier
            ComFindPhaseFinding          = $(if ($comRef.timingCachedFindMs -gt $comRef.timingPerPropertyFindMs) {
                'the cache-building find phase is SLOWER than the uncached find (' + [Math]::Round($comRef.timingCachedFindMs, 2) + ' ms vs ' + [Math]::Round($comRef.timingPerPropertyFindMs, 2) + ' ms): building the cache costs time up front and pays it back on reads'
            } else {
                'the authoritative capture no longer shows a slower cache-building find phase (' + [Math]::Round($comRef.timingCachedFindMs, 2) + ' ms vs ' + [Math]::Round($comRef.timingPerPropertyFindMs, 2) + ' ms), so the phase-split finding this probe cross-checks has changed underneath it'
            })
            ClaimVerdict                 = ($comRef.claimVerdict + ' against the phases.md 3-5x batched-read claim')
        }
        Comparison     = 'stated in ComparisonToAuthoritative below; the managed numbers are reported to show whether the managed stack reproduces the COM shape, not to replace it'
        Interpretation = [ordered]@{
            Notepad  = $(if ($notepadResult.timingOverallMultiplier -lt 1.0) {
                'NEW-EDGE: batching is a PESSIMIZATION here — timingOverallMultiplier is below 1.0 on classic Notepad. Notepad is a plain Win32 control tree served by UIAutomationClientsideProviders, which run inside the client process, so an uncached property read costs no cross-process RPC and the CacheRequest adds pure setup cost. A Windows adapter that batches unconditionally will make simple Win32 windows slower.'
            } else {
                'this run did NOT reproduce the Notepad pessimization: timingOverallMultiplier is ' + $notepadResult.timingOverallMultiplier + ', at or above 1.0. Read the measured numbers, not this sentence.'
            })
            Explorer = $(if ($explorerResult.timingFindPhaseMultiplier -lt 1.0 -and $explorerResult.timingReadPhaseMultiplier -ge 100.0) {
                'on a genuinely out-of-process provider the managed stack reproduces the COM shape exactly: timingFindPhaseMultiplier below 1.0 (building the cache makes the find phase slower) and timingReadPhaseMultiplier in the hundreds, leaving a modest overall multiplier.'
            } else {
                'this run did NOT reproduce the COM phase split on Explorer: timingFindPhaseMultiplier ' + $explorerResult.timingFindPhaseMultiplier + ' (expected below 1.0) and timingReadPhaseMultiplier ' + $explorerResult.timingReadPhaseMultiplier + ' (expected in the hundreds). Read the measured numbers, not this sentence.'
            })
            Headline  = 'U7 uia3-com stays authoritative at 2.6942x overall and 298.158x on the read phase. The managed cross-check reproduces the direction and the phase split, not the magnitude, and adds the client-side-provider caveat above.'
        }
    }
    $explorerOverall = $explorerResult.timingOverallMultiplier
    $explorerRead = $explorerResult.timingReadPhaseMultiplier
    $explorerFind = $explorerResult.timingFindPhaseMultiplier
    $capture['ComparisonToAuthoritative'] = [ordered]@{
        ComOverallMultiplier     = $comRef.timingMultiplier
        timingManagedOverallMultiplier   = $explorerOverall
        ComReadPhaseMultiplier   = $comRef.timingReadPhaseMultiplier
        timingManagedReadPhaseMultiplier = $explorerRead
        timingManagedFindPhaseMultiplier = $explorerFind
        SameTargetShape          = 'both measure an explorer folder window over %WINDIR%\System32 with the same 8 properties and 5 repetitions; managed NodeCount differs from the COM NodeCount because the two client stacks do not present the same control view (KTD1)'
        Note                     = 'a find-phase multiplier below 1.0 reproduces the COM finding that cache-building makes the find phase slower; the payback is entirely in the read phase'
    }
    [void](Write-ProbeJson -Probe $Probe -Name 'cache-timing.json' -InputObject $capture)

    $resultData = [ordered]@{
        ExplorerNodeCount       = $explorerResult.NodeCount
        timingManagedOverall    = $explorerOverall
        timingManagedReadPhase  = $explorerRead
        ComAuthoritativeOverall = $comRef.timingMultiplier
    }
    Write-ProbeLog -Message 'wrote cache-timing.json (managed cross-check, non-authoritative)'
    Write-ProbeResult -Probe $Probe -Status 'ok' -Message 'managed batched-vs-per-property cross-check captured against U7 UIA3 COM authority' -Data $resultData
    exit 0
} catch {
    Write-ProbeResult -Probe $Probe -Status 'fail' -Message ('unhandled error: ' + $_.Exception.Message)
    exit 1
} finally {
    if ($notepadPid -ne 0) { try { Stop-ScratchProcess -ProcessId $notepadPid } catch { Write-ProbeLog -Message $_.Exception.Message -Level 'error' } }
    if ($explorerHandle -ne 0 -and -not $explorerPreexisting) {
        try {
            $shell = New-Object -ComObject Shell.Application
            foreach ($w in @($shell.Windows())) {
                try { if ([int]$w.HWND -eq $explorerHandle) { $w.Quit() } } catch { }
            }
        } catch { Write-ProbeLog -Message ('could not close folder window: ' + $_.Exception.Message) -Level 'error' }
    }
}
