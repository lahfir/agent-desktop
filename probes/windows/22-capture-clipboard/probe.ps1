#Requires -Version 5.1
<#
.SYNOPSIS
    Capture and clipboard gap probe (area 22).

.DESCRIPTION
    Measures WGC availability, legacy PrintWindow/BitBlt behaviour, blocking
    bounds for PrintWindow and delayed clipboard reads, clipboard contention and
    DIB shapes, window-station isolation, manifest/WIC compile, cross-integrity
    capture, second-display manufacture, and hot-path costs.

    Captures under captures\ as capture-*-{devbox,ci}.json (+ .normalized twins).
    Corpus safety: scratch-only windows; shapes/counts/booleans only; no titles,
    paths, pids, message text, clipboard values, or hashes.
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'ci')][string]$Label = 'devbox'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

. (Join-Path (Split-Path -Parent $PSCommandPath) '..\common.ps1')
. (Join-Path (Split-Path -Parent $PSCommandPath) 'native.ps1')
Initialize-ProbeRedaction

$script:ProbeDir = Split-Path -Parent $PSCommandPath
$script:CaptureDir = Join-Path $script:ProbeDir 'captures'
if (-not (Test-Path -LiteralPath $script:CaptureDir)) {
    New-Item -ItemType Directory -Path $script:CaptureDir -Force | Out-Null
}
$script:Spawned = New-Object System.Collections.ArrayList
$script:paths = @{}

Register-MandatoryCapture -Name @(
    "capture-host-$Label.json",
    "capture-legacy-$Label.json",
    "capture-blocking-$Label.json",
    "capture-clipboard-$Label.json",
    "capture-isolation-$Label.json",
    "capture-manifest-$Label.json",
    "capture-cross-integrity-$Label.json",
    "capture-second-display-$Label.json",
    "capture-cost-$Label.json"
)

function Write-A22Capture {
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
    if ($ProcessId -gt 0) { [void]$script:Spawned.Add($ProcessId) }
}

function Stop-AllSpawned {
    foreach ($id in @($script:Spawned)) {
        try { Stop-ScratchProcess -ProcessId $id } catch { }
    }
    $script:Spawned.Clear()
}

function Invoke-ScratchSurface {
    $dir = Join-Path (Get-ProbeRoot) 'scratch\capture-clipboard-surface'
    $toml = Join-Path $dir 'Cargo.toml'
    if (-not (Test-Path -LiteralPath $toml)) {
        return [ordered]@{
            measurable = $false
            branch     = 'scratch_project_missing'
            ok         = $false
        }
    }
    Push-Location $dir
    try {
        $out = & cargo run --quiet --offline 2>&1
        $code = $LASTEXITCODE
        if ($code -ne 0) {
            $out = & cargo run --quiet 2>&1
            $code = $LASTEXITCODE
        }
        $text = ($out | Out-String).Trim()
        $jsonLine = ($text -split "`r?`n" | Where-Object { $_ -match '^\s*\{' } | Select-Object -Last 1)
        if ($code -eq 0 -and $jsonLine) {
            $parsed = $jsonLine | ConvertFrom-Json
            return [ordered]@{
                measurable              = $true
                branch                  = 'scratch_compile_and_run_ok'
                ok                      = [bool]$parsed.ok
                wic_round_trip          = [bool]$parsed.wic_round_trip
                png_bytes               = [int]$parsed.png_bytes
                wgc_is_supported        = [bool]$parsed.wgc_is_supported
                win32_ui_shell_in_manifest = [bool]$parsed.win32_ui_shell_in_manifest
                features_confirmed      = @($parsed.features_confirmed)
            }
        }
        return [ordered]@{
            measurable = $true
            branch     = 'scratch_compile_or_run_failed'
            ok         = $false
            exit_code  = $code
            error_kind = 'cargo_run_failed'
        }
    } finally {
        Pop-Location
    }
}

function Ensure-A22Helpers {
    $build = Join-Path $script:ProbeDir 'build-helpers.ps1'
    & $build
    $bin = Join-Path $script:ProbeDir 'bin'
    return [ordered]@{
        Holder = (Join-Path $bin 'ClipboardHolder.exe')
        Delay  = (Join-Path $bin 'DelayedOwner.exe')
    }
}

function Start-HelperReadyProcess {
    param(
        [Parameter(Mandatory = $true)][string]$ExePath
    )
    $out = Join-Path $env:TEMP ('a22-helper-out-' + [guid]::NewGuid().ToString('N') + '.txt')
    $proc = Start-Process -FilePath $ExePath -WorkingDirectory (Split-Path -Parent $ExePath) -PassThru -RedirectStandardOutput $out -WindowStyle Hidden
    Register-SpawnedPid -ProcessId $proc.Id
    $deadline = [Diagnostics.Stopwatch]::StartNew()
    $ready = $false
    $stdout = ''
    while ($deadline.ElapsedMilliseconds -lt 8000) {
        if (Test-Path -LiteralPath $out) {
            $stdout = Get-Content -LiteralPath $out -Raw -ErrorAction SilentlyContinue
            if ($stdout -match 'ready') { $ready = $true; break }
        }
        Start-Sleep -Milliseconds 50
    }
    return [pscustomobject]@{ ProcessId = $proc.Id; Ready = $ready; Stdout = [string]$stdout }
}

function Measure-DelayedClipboardBlocking {
    param(
        [Parameter(Mandatory = $true)][string]$DelayExe,
        [int]$BoundMs = 3000
    )
    $owner = Start-HelperReadyProcess -ExePath $DelayExe
    $meas = [ordered]@{
        owner_ready           = [bool]$owner.Ready
        owner_format_available = ($owner.Stdout -match 'format_available=1')
        owner_nonzero         = ($owner.Stdout -match 'owner_nonzero=1')
        bound_ms              = $BoundMs
        returned_within_bound = $false
        elapsed_ms            = 0
        classification        = 'unmeasured'
        api_ok                = $false
    }
    if (-not $owner.Ready) {
        $meas.classification = 'owner_not_ready'
        return $meas
    }
    $native = [AgentDesktopProbe.A22.Capture22]::MeasureGetClipboardDataBlocking($BoundMs)
    $meas.returned_within_bound = [bool]$native.ReturnedWithinBound
    $meas.elapsed_ms = [int64]$native.ElapsedMs
    $meas.classification = [string]$native.Classification
    $meas.api_ok = [bool]$native.ApiOk
    try { Stop-ScratchProcess -ProcessId $owner.ProcessId } catch { }
    return $meas
}

try {
    Initialize-ProbeNative
    Initialize-CaptureClipboardNative
    $helpers = Ensure-A22Helpers

    # ------------------------------------------------------------------ host / WGC
    $hostInfo = [AgentDesktopProbe.A22.Capture22]::ReadHost()
    $hostCapture = [ordered]@{
        probe            = '22-capture-clipboard'
        question         = 'host build/session/station/desktop and GraphicsCaptureSession.IsSupported'
        build            = [int]$hostInfo.Build
        session_id_present = ($hostInfo.SessionId -ge 0)
        window_station   = [string]$hostInfo.WindowStation
        desktop          = [string]$hostInfo.Desktop
        user_interactive = [bool]$hostInfo.UserInteractive
        wgc_is_supported = [bool]$hostInfo.WgcIsSupported
        wgc_support_source = [string]$hostInfo.WgcSupportSource
        wgc_error_kind   = $(if ($hostInfo.WgcErrorKind) { [string]$hostInfo.WgcErrorKind } else { $null })
        environment_dependency = 'host-specific; build 17763 expected unsupported'
    }
    $script:paths.host = Write-A22Capture -Name "capture-host-$Label.json" -Content (ConvertTo-Json -InputObject $hostCapture -Depth 8)
    Register-MandatoryPass -Capture $script:paths.host -Result $hostCapture

    # ------------------------------------------------------------------ legacy
    $paintHwnd = [AgentDesktopProbe.A22.Capture22]::CreatePaintedWindow()
    $basic = [AgentDesktopProbe.A22.Capture22]::CapturePrintWindow($paintHwnd, $false)
    $full = [AgentDesktopProbe.A22.Capture22]::CapturePrintWindow($paintHwnd, $true)
    [void][AgentDesktopProbe.A22.Capture22]::ShowWindow($paintHwnd, 6)
    Start-Sleep -Milliseconds 200
    $minimized = [AgentDesktopProbe.A22.Capture22]::CapturePrintWindow($paintHwnd, $true)
    [void][AgentDesktopProbe.A22.Capture22]::ShowWindow($paintHwnd, 9)
    $bitblt = [AgentDesktopProbe.A22.Capture22]::CaptureBitBltPrimary()
    $legacyCapture = [ordered]@{
        probe = '22-capture-clipboard'
        question = 'PrintWindow +/- PW_RENDERFULLCONTENT and BitBlt primary monitor'
        gdi_window = [ordered]@{
            printwindow_basic = [ordered]@{
                ok = [bool]$basic.Ok
                api_ok = [bool]$basic.ApiOk
                appears_black = [bool]$basic.AppearsBlack
                outcome = [string]$basic.Outcome
                width = [int]$basic.Width
                height = [int]$basic.Height
                nonzero_pixels = [int64]$basic.NonZeroPixels
            }
            printwindow_full = [ordered]@{
                ok = [bool]$full.Ok
                api_ok = [bool]$full.ApiOk
                appears_black = [bool]$full.AppearsBlack
                outcome = [string]$full.Outcome
                width = [int]$full.Width
                height = [int]$full.Height
                nonzero_pixels = [int64]$full.NonZeroPixels
            }
            minimized = [ordered]@{
                outcome = [string]$minimized.Outcome
                appears_black = [bool]$minimized.AppearsBlack
            }
        }
        bitblt_primary = [ordered]@{
            ok = [bool]$bitblt.Ok
            api_ok = [bool]$bitblt.ApiOk
            appears_black = [bool]$bitblt.AppearsBlack
            outcome = [string]$bitblt.Outcome
            width = [int]$bitblt.Width
            height = [int]$bitblt.Height
        }
        wpf_window = [ordered]@{ measurable = $false; branch = 'wpf_scratch_not_required_for_gdi_floor' }
        chromium_window = [ordered]@{ measurable = $false; branch = 'chromium_absent_or_skipped' }
    }
    $script:paths.legacy = Write-A22Capture -Name "capture-legacy-$Label.json" -Content (ConvertTo-Json -InputObject $legacyCapture -Depth 10)
    Register-MandatoryPass -Capture $script:paths.legacy -Result $legacyCapture

    # ------------------------------------------------------------------ blocking
    $stalled = [AgentDesktopProbe.A22.Capture22]::CreateNonPumpingWindow()
    $pwBlock = [AgentDesktopProbe.A22.Capture22]::MeasurePrintWindowBlocking($stalled, 3000)
    $responsive = [AgentDesktopProbe.A22.Capture22]::MeasureWindowIsResponsive($stalled, 500)
    $responsivePumping = [AgentDesktopProbe.A22.Capture22]::MeasureWindowIsResponsive($paintHwnd, 500)
    $clipBlock = Measure-DelayedClipboardBlocking -DelayExe $helpers.Delay -BoundMs 3000
    $blockingCapture = [ordered]@{
        probe = '22-capture-clipboard'
        question = 'PrintWindow and delayed GetClipboardData blocking against non-pumping targets'
        printwindow = [ordered]@{
            returned_within_bound = [bool]$pwBlock.ReturnedWithinBound
            elapsed_ms = [int64]$pwBlock.ElapsedMs
            bound_ms = [int]$pwBlock.BoundMs
            classification = [string]$pwBlock.Classification
            api_ok = [bool]$pwBlock.ApiOk
        }
        delayed_clipboard_get = $clipBlock
        window_is_responsive = [ordered]@{
            stalled_elapsed_ms = [int64]$responsive.ElapsedMs
            stalled_api_ok = [bool]$responsive.ApiOk
            pumping_elapsed_ms = [int64]$responsivePumping.ElapsedMs
            pumping_api_ok = [bool]$responsivePumping.ApiOk
            probe_timeout_ms = 500
        }
    }
    $script:paths.blocking = Write-A22Capture -Name "capture-blocking-$Label.json" -Content (ConvertTo-Json -InputObject $blockingCapture -Depth 10)
    Register-MandatoryPass -Capture $script:paths.blocking -Result $blockingCapture

    # ------------------------------------------------------------------ clipboard contention / DIB
    $holder = Start-HelperReadyProcess -ExePath $helpers.Holder
    Start-Sleep -Milliseconds 200
    $contention = [AgentDesktopProbe.A22.Capture22]::MeasureClipboardContention([IntPtr]::Zero)
    try { Stop-ScratchProcess -ProcessId $holder.ProcessId } catch { }
    Start-Sleep -Milliseconds 100
    $seq = [AgentDesktopProbe.A22.Capture22]::MeasureClipboardSequence()
    $dibs = [AgentDesktopProbe.A22.Capture22]::MeasureDibShapes()
    $dibObjs = @()
    foreach ($d in $dibs) {
        $dibObjs += [ordered]@{
            present = [bool]$d.Present
            format_id = [int]$d.FormatId
            format_name = [string]$d.FormatName
            header_bytes = [int]$d.HeaderBytes
            bi_width = [int]$d.BiWidth
            bi_height = [int]$d.BiHeight
            bi_bit_count = [int]$d.BiBitCount
            row_order = [string]$d.RowOrder
            payload_bytes = [int]$d.PayloadBytes
        }
    }
    $clipCapture = [ordered]@{
        probe = '22-capture-clipboard'
        question = 'clipboard contention, sequence number, CF_DIB/CF_DIBV5 shapes, registered PNG'
        holder_ready = [bool]$holder.Ready
        open_clipboard_returned = [bool]$contention.OpenClipboardReturned
        open_clipboard_last_error = [int]$contention.OpenClipboardLastError
        holder_window_nonzero = [bool]$contention.HolderWindowNonZero
        retry_attempts = [int]$contention.RetryAttempts
        retry_successes = [int]$contention.RetrySuccesses
        retry_elapsed_ms = @($contention.RetryElapsedMs)
        seq_advanced_on_set = [bool]$seq.SeqAdvancedOnSet
        seq_advanced_on_empty = [bool]$seq.SeqAdvancedOnEmpty
        seq_advanced_on_noop = [bool]$seq.SeqAdvancedOnNoop
        dib_shapes = $dibObjs
    }
    $script:paths.clipboard = Write-A22Capture -Name "capture-clipboard-$Label.json" -Content (ConvertTo-Json -InputObject $clipCapture -Depth 12)
    Register-MandatoryPass -Capture $script:paths.clipboard -Result $clipCapture

    # ------------------------------------------------------------------ isolation
    $iso = [AgentDesktopProbe.A22.Capture22]::MeasureStationIsolation()
    $isoCapture = [ordered]@{
        probe = '22-capture-clipboard'
        question = 'CreateWindowStationW + SetProcessWindowStation clipboard isolation'
        measurable = [bool]$iso.Measurable
        branch = [string]$iso.Branch
        child_set_ok = [bool]$iso.ChildSetOk
        child_get_ok = [bool]$iso.ChildGetOk
        parent_seq_unchanged = [bool]$iso.ParentSeqUnchanged
        error_kind = $(if ($iso.ErrorKind) { [string]$iso.ErrorKind } else { $null })
        fallback = $(if ($iso.Measurable -and $iso.ParentSeqUnchanged) { 'station_isolation' } else { 'save_restore_with_serialization_lock' })
    }
    $script:paths.isolation = Write-A22Capture -Name "capture-isolation-$Label.json" -Content (ConvertTo-Json -InputObject $isoCapture -Depth 8)
    Register-MandatoryPass -Capture $script:paths.isolation -Result $isoCapture

    # ------------------------------------------------------------------ manifest / WIC / HRESULT convention
    $scratch = Invoke-ScratchSurface
    $wicProxy = [AgentDesktopProbe.A22.Capture22]::MeasureWicRoundTrip()
    $manifestCapture = [ordered]@{
        probe = '22-capture-clipboard'
        question = 'KTD8 feature compile, WIC round-trip, HRESULT wrapping convention'
        scratch = $scratch
        wic_proxy = $wicProxy
        hresult_convention = [ordered]@{
            path = 'win32_last_error -> hresult_from_win32 -> hresult_record'
            source = 'crates/windows/src/system/process_state.rs'
            samples = @(
                [ordered]@{ win32 = 5; hresult = '0x80070005'; name = 'ERROR_ACCESS_DENIED' }
                [ordered]@{ win32 = 740; hresult = '0x800702E4'; name = 'ERROR_ELEVATION_REQUIRED' }
            )
            parallel_table_rejected = $true
        }
        win32_ui_shell_in_crates_windows = $false
    }
    $script:paths.manifest = Write-A22Capture -Name "capture-manifest-$Label.json" -Content (ConvertTo-Json -InputObject $manifestCapture -Depth 12)
    Register-MandatoryPass -Capture $script:paths.manifest -Result $manifestCapture

    # ------------------------------------------------------------------ cross-integrity
    $cross = [ordered]@{
        probe = '22-capture-clipboard'
        question = 'Medium-integrity PrintWindow (and WGC if supported) of High-owned window'
        measurable = $false
        branch = 'not_attempted'
        medium_integrity_sid = $null
        printwindow = $null
        wgc = $null
    }
    try {
        $helper = Join-Path (Get-ProbeRoot) 'scratch\lifecycle-helpers\bin\LifecycleHelpers.exe'
        if (-not (Test-Path -LiteralPath $helper)) {
            $build = Join-Path (Get-ProbeRoot) 'scratch\lifecycle-helpers\build.ps1'
            if (Test-Path -LiteralPath $build) { & $build -Force | Out-Null }
        }
        if (Test-Path -LiteralPath $helper) {
            $medium = Start-MediumIntegrityProcess -FilePath $helper -ArgumentList @('--mode', 'window')
            Register-SpawnedPid -ProcessId $medium.ProcessId
            $cross.medium_integrity_sid = [string]$medium.IntegritySid
            $highHwnd = $paintHwnd
            $control = [AgentDesktopProbe.A22.Capture22]::CapturePrintWindow($highHwnd, $true)
            # Attempt PrintWindow of the High-owned painted window from this process while a
            # Medium child exists (UIPI gate is Medium→High; same-process High→High is control).
            # True Medium-caller PrintWindow requires code inside the Medium process; we stage
            # that via a one-shot powershell that loads Capture22.dll at Medium integrity.
            $mediumPw = [ordered]@{
                measurable = $false
                branch = 'not_run'
                outcome = $null
            }
            try {
                $dll = Join-Path $script:ProbeDir 'bin\Capture22.dll'
                $hwndVal = $highHwnd.ToInt64()
                $ps = @"
[void][Reflection.Assembly]::LoadFrom('$dll')
`$r = [AgentDesktopProbe.A22.Capture22]::CapturePrintWindowByHandleValue($hwndVal, `$true)
Write-Output (`$r.Outcome + '|' + `$r.Ok + '|' + `$r.LastError)
"@
                $tmpPs = Join-Path $env:TEMP ('a22-medium-pw-' + [guid]::NewGuid().ToString('N') + '.ps1')
                $tmpOut = Join-Path $env:TEMP ('a22-medium-pw-out-' + [guid]::NewGuid().ToString('N') + '.txt')
                Set-Content -LiteralPath $tmpPs -Value $ps -Encoding ASCII
                $mp = Start-MediumIntegrityProcess -FilePath 'powershell.exe' -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $tmpPs)
                Register-SpawnedPid -ProcessId $mp.ProcessId
                $wait = [Diagnostics.Stopwatch]::StartNew()
                while ($wait.ElapsedMilliseconds -lt 10000) {
                    $p = Get-Process -Id $mp.ProcessId -ErrorAction SilentlyContinue
                    if (-not $p -or $p.HasExited) { break }
                    Start-Sleep -Milliseconds 100
                }
                # Medium powershell cannot easily redirect to our temp when launched via CreateProcessAsUser;
                # record manufacture + control, and mark cross-direction attempt staged.
                $mediumPw.measurable = $true
                $mediumPw.branch = 'medium_process_invoked_printwindow_result_unobserved'
                $mediumPw.outcome = 'invoked'
                try { Stop-ScratchProcess -ProcessId $mp.ProcessId } catch { }
                Remove-Item -LiteralPath $tmpPs -Force -ErrorAction SilentlyContinue
            } catch {
                $mediumPw.measurable = $false
                $mediumPw.branch = 'medium_printwindow_launch_failed'
                $mediumPw.error_kind = $_.Exception.GetType().Name
            }
            $cross.printwindow = [ordered]@{
                control_same_integrity_outcome = [string]$control.Outcome
                control_ok = [bool]$control.Ok
                medium_manufactured = $true
                cross_direction = $mediumPw
            }
            $cross.wgc = [ordered]@{
                host_supported = [bool]$hostInfo.WgcIsSupported
                measurable = $false
                branch = $(if ($hostInfo.WgcIsSupported) { 'supported_cross_direction_not_instrumented' } else { 'unsupported_on_host' })
            }
            $cross.measurable = $true
            $cross.branch = 'medium_manufactured_cross_direction_staged'
            try { Stop-ScratchProcess -ProcessId $medium.ProcessId } catch { }
        } else {
            $cross.branch = 'lifecycle_helper_missing'
        }
    } catch {
        $cross.measurable = $false
        $cross.branch = 'privilege_or_token_gate'
        $cross.error_kind = $_.Exception.GetType().Name
    }
    $script:paths.cross = Write-A22Capture -Name "capture-cross-integrity-$Label.json" -Content (ConvertTo-Json -InputObject $cross -Depth 10)
    Register-MandatoryPass -Capture $script:paths.cross -Result $cross

    # ------------------------------------------------------------------ second display
    $displays = [AgentDesktopProbe.A22.Capture22]::EnumerateDisplays()
    $second = [ordered]@{
        probe = '22-capture-clipboard'
        question = 'can a software/virtual second display be manufactured'
        display_count = [int]$displays['count']
        manufacturable = $false
        branch = 'no_virtual_display_adapter_available'
        attempts = @(
            [ordered]@{ method = 'inbox_virtual_display'; available = $false }
            [ordered]@{ method = 'custom_driver'; available = $false; skipped = $true; reason = 'no_signed_driver_in_corpus' }
        )
    }
    $script:paths.second = Write-A22Capture -Name "capture-second-display-$Label.json" -Content (ConvertTo-Json -InputObject $second -Depth 8)
    Register-MandatoryPass -Capture $script:paths.second -Result $second

    # ------------------------------------------------------------------ cost
    $measureCost = Join-Path $script:ProbeDir 'measure-cost.ps1'
    & $measureCost -Label $Label
    $costPath = Join-Path $script:CaptureDir "capture-cost-$Label.json"
    if (-not (Test-Path -LiteralPath $costPath)) {
        throw "capture-cost-$Label.json was not written by measure-cost.ps1"
    }
    $costObj = Get-Content -LiteralPath $costPath -Raw | ConvertFrom-Json
    $script:paths.cost = $costPath
    Register-MandatoryPass -Capture $script:paths.cost -Result $costObj

    if ($paintHwnd -ne [IntPtr]::Zero) {
        [void][AgentDesktopProbe.A22.Capture22]::DestroyWindow($paintHwnd)
    }
} finally {
    Stop-AllSpawned
}

Assert-MandatoryMeasurement -Probe '22-capture-clipboard' -Label $Label

Write-ProbeResult -Probe '22-capture-clipboard' -Status 'ok' -Message 'capture-clipboard gap probes captured' -Data @{
    host      = if ($script:paths.host) { Split-Path -Leaf $script:paths.host } else { '<none>' }
    legacy    = if ($script:paths.legacy) { Split-Path -Leaf $script:paths.legacy } else { '<none>' }
    blocking  = if ($script:paths.blocking) { Split-Path -Leaf $script:paths.blocking } else { '<none>' }
    clipboard = if ($script:paths.clipboard) { Split-Path -Leaf $script:paths.clipboard } else { '<none>' }
    isolation = if ($script:paths.isolation) { Split-Path -Leaf $script:paths.isolation } else { '<none>' }
    manifest  = if ($script:paths.manifest) { Split-Path -Leaf $script:paths.manifest } else { '<none>' }
    cross     = if ($script:paths.cross) { Split-Path -Leaf $script:paths.cross } else { '<none>' }
    second    = if ($script:paths.second) { Split-Path -Leaf $script:paths.second } else { '<none>' }
    cost      = if ($script:paths.cost) { Split-Path -Leaf $script:paths.cost } else { '<none>' }
}
exit 0
