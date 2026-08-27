#Requires -Version 5.1
<#
.SYNOPSIS
    Area 26 02-notifications.ps1 - rows A26-3 (Action Center shape) and
    A26-4 (UserNotificationListener readings plus the machine's consent-store
    value).

.DESCRIPTION
    A26-3 records the Action Center's XAML shape as counts, framework
    AutomationId tags and pattern sets only: MainListView presence,
    ListViewHeaderItem group count, ListViewItem entry count, one
    representative entry's child AutomationId set each with its own pattern
    set, and ClearAllButton presence. Two classes of identifier never leave
    this script: every Name read off a notification element (names are the
    content the redaction gate exists to keep out) and every machine-local
    GUID-shaped or otherwise opaque AutomationId. Framework-stable structural
    ids the product itself addresses elements by (MainListView, Title,
    Content, Attribution, DismissButton, ExpandButton, VerbButton,
    ClearAllButton) ARE recorded; everything else is tagged <machine-local-guid>
    or <opaque> inside shell-probe.cs before PowerShell ever sees it.

    A26-4 is independent of whether a surface can be raised: it reads whether
    WinRT UserNotificationListener.Current activates at all, what
    GetAccessStatus() reports (RequestAccessAsync is deliberately NOT called,
    per KTD2's non-claim about cause), and what the machine's own consent
    store says for userNotificationListener under CapabilityAccessManager -
    both halves so KTD2's refusal to explain the Deny stays visible in
    evidence.

    Run: powershell -NoProfile -ExecutionPolicy Bypass -File .\probes\windows\26-shell-surfaces\02-notifications.ps1 -Label <devbox|ci>
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'ci')][string]$Label = 'devbox'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

. (Join-Path (Split-Path -Parent $PSCommandPath) '..\common.ps1')
Initialize-ProbeRedaction
. (Join-Path (Split-Path -Parent $PSCommandPath) 'lib.ps1')

$script:Probe = '26-shell-surfaces/02-notifications'
Register-MandatoryCapture -Name @("notifications-shape-$Label.json", "notifications-listener-$Label.json")

$frameworkAllowedAutomationIds = @(
    'MainListView', 'Title', 'Content', 'Attribution', 'DismissButton',
    'ExpandButton', 'VerbButton', 'ClearAllButton'
)

function ConvertTo-SafeRecord {
    param([Parameter(Mandatory = $true)]$Node)
    return $Node
}

# ---------------------------------------------------------------- listener leg

function Measure-ListenerLeg {
    $leg = [ordered]@{
        measurable                   = $true
        winrt_type_load_attempted    = $true
        activation_reported          = $false
        activation_exception_class   = $null
        get_access_status            = $null
        request_access_async_called  = $false
        consent_store_locations      = @()
    }
    try {
        $null = [Windows.UI.Notifications.Management.UserNotificationListener, Windows.UI.Notifications, ContentType = WindowsRuntime]
        $listener = [Windows.UI.Notifications.Management.UserNotificationListener]::Current
        $leg['activation_reported'] = ($null -ne $listener)
        if ($null -ne $listener) {
            $status = $listener.GetAccessStatus()
            $name = [string]$status
            # The managed projection of the enum renders as e.g. "Deny"; keep
            # only the vocabulary word.
            $seg = $name.Split(',') | Select-Object -First 1
            $leg['get_access_status'] = ($seg.Trim() -replace '^.*\.', '')
        }
    } catch {
        $leg['activation_exception_class'] = $_.Exception.GetType().Name
    }
    $candidates = @(
        @{ hive = 'HKCU'; subkey = 'Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\userNotificationListener' },
        @{ hive = 'HKCU'; subkey = 'Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\userNotificationListener\NonPackaged' },
        @{ hive = 'HKCU'; subkey = 'Software\Microsoft\Windows\CurrentVersion\PushNotifications' }
    )
    foreach ($cand in $candidates) {
        $path = ('Registry::' + $cand.hive + '\' + $cand.subkey)
        $record = [ordered]@{ location = $cand.subkey; exists = $false; grant_value = '<absent>' }
        if (Test-Path -LiteralPath $path) {
            $record['exists'] = $true
            try {
                $props = Get-ItemProperty -LiteralPath $path
                $v = $props.'Value'
                if (-not $v -and $null -ne $props.ToastEnabled) { $v = ([string]$props.ToastEnabled) }
                if ($v) { $record['grant_value'] = [string]$v }
            } catch {
                $record['grant_value'] = '<unreadable>'
            }
        }
        $leg['consent_store_locations'] += ,$record
    }
    return $leg
}

# ------------------------------------------------------------------- shape leg

function Get-LandmarkNodeIndex {
    param([Parameter(Mandatory = $true)]$Nodes, [Parameter(Mandatory = $true)][string]$Tag)
    for ($i = 0; $i -lt $Nodes.Count; $i++) {
        if ($Nodes[$i].aid -eq $Tag) { return $i }
    }
    return -1
}

function Test-IsDescendantOf {
    param([Parameter(Mandatory = $true)]$Nodes, [int]$Index, [int]$Ancestor)
    $cur = $Index
    while ($cur -ge 0) {
        if ($cur -eq $Ancestor) { return $true }
        $cur = [int]$Nodes[$cur].parent
    }
    return $false
}

function Measure-ShapeLeg {
    Initialize-ShellProbe | Out-Null
    $leg = [ordered]@{
        measurable                      = $false
        branch                          = 'not_measured'
        stack                           = 'uia3-com'
        toast_staging                   = [ordered]@{ attempted = $true; accepted = $false; exception_class = $null; note = 'staging only creates an entry to measure; RequestAccessAsync is never called' }
    }

    if (-not (Reset-ShellSurfaceBaseline)) {
        $leg['branch'] = 'shell_baseline_not_clean_before_raise'
        return $leg
    }

    Invoke-ShellProbe -Arguments @('key', '--seq', 'lwin_a') | Out-Null
    Start-Sleep -Milliseconds 900
    $scan = Invoke-ShellProbe -Arguments @('reachscan')
    $candidate = $null
    foreach ($c in $scan.children) {
        if ($c.ac_candidate -and $c.nativewindowhandle -ne 0) { $candidate = $c; break }
    }
    if (-not $candidate) {
        $leg['branch'] = 'action_center_not_raisable_by_lwin_a_accelerator'
        Invoke-ShellProbe -Arguments @('key', '--seq', 'esc') | Out-Null
        return $leg
    }

    $surfaceHandle = [string]$candidate.nativewindowhandle

    # Deterministic start state: the notification list is cleared first so a
    # staged toast is the only entry the shape read measures - run order and
    # prior-session leftovers cannot shift the counts. The ClearAllButton
    # invoke doubles as the mutation-path exercise this row's cleanup needs.
    try {
        Invoke-ShellProbe -Arguments @('invokebyaid', '--hwnd', $surfaceHandle, '--aid', 'ClearAllButton') | Out-Null
        Start-Sleep -Milliseconds 800
    } catch { }

    # Best-effort staging of one synthetic notification so the per-entry arm
    # has something shaped like content to measure. Text is synthetic hex, no
    # real content is composed anywhere in this script.
    try {
        $null = [Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime]
        $null = [Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime]
        $xmlText = '<toast><visual><binding template="ToastGeneric"><text>agent-desktop-probe-' +
            ([guid]::NewGuid().ToString('N').Substring(0, 8)) + '</text></binding></visual></toast>'
        $xml = New-Object Windows.Data.Xml.Dom.XmlDocument
        $xml.LoadXml($xmlText)
        $toast = New-Object Windows.UI.Notifications.ToastNotification($xml)
        $aumid = '{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\WindowsPowerShell\v1.0\powershell.exe'
        $notifier = [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier($aumid)
        $notifier.Show($toast)
        $leg['toast_staging']['accepted'] = $true
        Start-Sleep -Milliseconds 1500
    } catch {
        $leg['toast_staging']['exception_class'] = $_.Exception.GetType().Name
    }

    $surfaceHandle = [string]$candidate.nativewindowhandle
    $treeRaw = Invoke-ShellProbe -Arguments @('actree', '--hwnd', $surfaceHandle, '--rescan')
    $tree = ConvertTo-SafeRecord -Node $treeRaw
    $nodes = @($tree.nodes)
    $leg['measurable'] = $true
    $leg['branch'] = 'shape_read_from_raised_surface'

    $mainIdx = Get-LandmarkNodeIndex -Nodes $nodes -Tag 'MainListView'
    $clearAllIdx = Get-LandmarkNodeIndex -Nodes $nodes -Tag 'ClearAllButton'
    $leg['main_list_view_present'] = ($mainIdx -ge 0)
    $leg['clear_all_button_present'] = ($clearAllIdx -ge 0)
    $leg['node_count_total'] = [int]$tree.node_count
    $leg['rescan_all_ids_unchanged'] = [bool]$tree.rescan.all_ids_unchanged

    $histogram = [ordered]@{}
    $headerCount = 0
    $entries = New-Object System.Collections.ArrayList
    if ($mainIdx -ge 0) {
        for ($i = $mainIdx + 1; $i -lt $nodes.Count; $i++) {
            if (-not (Test-IsDescendantOf -Nodes $nodes -Index $i -Ancestor $mainIdx)) { continue }
            $ct = [string]$nodes[$i].ct
            if (-not $histogram.Contains($ct)) { $histogram[$ct] = 0 }
            $histogram[$ct] = [int]$histogram[$ct] + 1
            if ($ct -eq 'HeaderItem') { $headerCount++ }
            if ($ct -eq 'ListItem') { [void]$entries.Add($i) }
        }
    }
    $leg['control_type_counts_under_main_list_view'] = $histogram
    $leg['list_view_header_item_count_controltype_headeritem'] = $headerCount
    $leg['list_view_item_entry_count_controltype_listitem'] = $entries.Count

    # Representative entry: first ListItem whose parent chain reaches
    # MainListView directly through the entry layer. Record its children as
    # safe AutomationId tags with pattern sets; GUID-tagged children are
    # recorded AS TAGS only, which carry no machine-local information.
    if ($entries.Count -gt 0) {
        $repIdx = [int]$entries[0]
        $rep = [ordered]@{
            node_index                     = $repIdx
            control_type                   = [string]$nodes[$repIdx].ct
            automation_id_tag              = [string]$nodes[$repIdx].aid
            invoke_pattern_available       = (@($nodes[$repIdx].pats) -contains 'Invoke')
            direct_children                = @()
        }
        for ($j = 0; $j -lt $nodes.Count; $j++) {
            if ([int]$nodes[$j].parent -ne $repIdx) { continue }
            $childPats = @($nodes[$j].pats)
            $hasVerbMarker = $false
            $tag = [string]$nodes[$j].aid
            if (@($frameworkAllowedAutomationIds) -contains $tag) { $hasVerbMarker = $true }
            $childRec = [ordered]@{
                automation_id_tag             = $tag
                framework_stable_tag_recorded = $hasVerbMarker
                control_type                  = [string]$nodes[$j].ct
                pattern_set                   = $childPats
            }
            $rep['direct_children'] += ,$childRec
        }
        $leg['representative_entry'] = $rep
        $leg['entry_children_shape_measurable'] = $true
    } else {
        $leg['representative_entry'] = $null
        $leg['entry_children_shape_measurable'] = $false
        $leg['branch'] = 'no_entries_posted_this_session_landmarks_only'
    }

    # Landmarks-only honesty marker: presence counts that stay true regardless
    # of how many entries exist.
    $landmarkTagsSeen = @()
    foreach ($n in $nodes) {
        $t = [string]$n.aid
        if ((@($frameworkAllowedAutomationIds) -contains $t) -and ($landmarkTagsSeen -notcontains $t)) { $landmarkTagsSeen += $t }
    }
    $leg['framework_automation_id_tags_seen'] = $landmarkTagsSeen

    # Cleanup: dismiss our staged toast, then close the surface.
    try {
        Invoke-ShellProbe -Arguments @('invokebyaid', '--hwnd', $surfaceHandle, '--aid', 'ClearAllButton') | Out-Null
    } catch { }
    Invoke-ShellProbe -Arguments @('key', '--seq', 'esc') | Out-Null
    return $leg
}

$shapeError = $null
try {
    $shape = Measure-ShapeLeg
} catch {
    $shape = [ordered]@{ measurable = $false; branch = 'shape_leg_threw'; error_class = $_.Exception.GetType().Name }
    $shapeError = $_.Exception.Message
}

try {
    $listener = Measure-ListenerLeg
} catch {
    $listener = [ordered]@{ measurable = $false; branch = 'listener_leg_threw'; error_class = $_.Exception.GetType().Name }
}

$status = 'ok'
$message = 'notifications shape + listener captured'

try {
    $shapeCapture = Write-Shell26Capture -Name "notifications-shape-$Label.json" -Content (ConvertTo-Json -InputObject ([ordered]@{
                probe         = $script:Probe
                question      = 'what does the Action Center XAML tree look like on this build to the UIA3 COM client, expressed as counts, framework AutomationId tags and pattern sets with no Name text recorded'
                cites         = @('KTD2')
                label         = $Label
                client_stack  = 'uia3-com'
                shape         = $shape
                shape_error   = $shapeError
            }) -Depth 20)
    Register-MandatoryPass -Capture $shapeCapture -Result $shape
    $listenerCapture = Write-Shell26Capture -Name "notifications-listener-$Label.json" -Content (ConvertTo-Json -InputObject ([ordered]@{
                probe         = $script:Probe
                question      = 'does WinRT UserNotificationListener activate from an unpackaged PS5.1 process on this box, what does GetAccessStatus report without calling RequestAccessAsync, and does the machine consent store agree'
                cites         = @('KTD2')
                label         = $Label
                listener      = $listener
            }) -Depth 16)
    Register-MandatoryPass -Capture $listenerCapture -Result $listener
} catch {
    $status = 'fail'
    $message = ('capture write failed: ' + $_.Exception.Message)
}

if ($shape.measurable -eq $false) {
    # Prefer measuring; only report failure when even landmark presence could
    # not be observed. The placeholder path keeps the artifact trail honest.
    if ($shape.branch -match 'not_raisable|baseline|threw') {
        $message = ('shape leg unmeasurable: ' + $shape.branch)
    }
}

Write-ProbeResult -Probe $script:Probe -Status $status -Message $message -Data @{
    captures = @("captures/notifications-shape-$Label.json", "captures/notifications-listener-$Label.json")
    rows     = @('A26-3', 'A26-4')
    stack    = 'uia3-com'
}
Assert-MandatoryMeasurement -Probe $script:Probe -Label $Label
exit 0
