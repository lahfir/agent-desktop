#Requires -Version 5.1
<#
.SYNOPSIS
    HWND uniqueness-counter wrap-rate probe under staged churn (area 24,
    sub-phase 2.12).

.DESCRIPTION
    phases.md owes 2.12.1 a FINDINGS row this corpus has never produced: what
    is the HWND uniqueness-counter wrap rate under realistic churn. A wrap
    unreachable under realistic churn leaves the fix purely structural; a
    reachable wrap adds a wrap-handling rule. No probe measured it before
    this one.

    A Win32 window handle packs a table index into its low-order 16 bits and
    a uniqueness/generation counter into its high-order 16 bits (the same
    index+counter shape every USER-object handle uses). Destroying a window
    frees its table slot for the very next CreateWindowEx call, so an index
    is expected to repeat almost immediately under churn - that alone is not
    a hazard, because the counter half distinguishes the new window's handle
    from the old one. The hazard 2.12.1 cares about is the FULL 32-bit value
    repeating: the counter itself wrapping after 65536 reuses of one slot,
    at which point a stale HWND a caller still holds could resolve to a
    different, unrelated live window instead of failing cleanly.

    This probe measures both, from ONE still-running process (this
    PowerShell process itself - no window is created by spawning a new
    process; every CreateWindowEx/DestroyWindow round-trip runs in-process
    via P/Invoke against a scratch window class this probe registers and
    unregisters itself), and reports:
      - total windows created and how many distinct HWND values were seen
      - how many HWND VALUES were reused (the same full 32-bit handle issued
        twice for two different windows - the actual hazard) and the
        creation count at the first such reuse
      - how many creations reused only the low-16-bit index while the
        high-16-bit counter had advanced (expected, benign slot recycling)
        and the creation count at the first such reuse
      - the highest counter value observed for any index (how close the
        churn got to the 16-bit wrap boundary even where no wrap landed)
      - elapsed wall time and which bound (iteration cap or wall-clock cap)
        the run hit

    Every window is created invisible (no WS_VISIBLE), WS_EX_NOACTIVATE |
    WS_EX_TOOLWINDOW, at an off-screen origin (2000,2000 on this box's
    1639x732 display), destroyed before the next one is created, and never
    shown or activated - this probe never changes the foreground, and
    verifies that by reading GetForegroundWindow's owning pid before and
    after the churn and failing loudly (PROBE-INTERFERENCE) if it moved.

    Captures under captures\ as hwnd-wrap-churn-{devbox,ci}.json (+
    .normalized twin). Corpus safety: only counts, booleans, elapsed
    milliseconds and symbolic branch/cap strings are ever handed to
    ConvertTo-Json - no window titles, file paths, pids, machine names, or
    user names. The raw HWND values themselves never leave the C# helper;
    only their derived counts and the two flagged creation-count offsets are
    read back into PowerShell.
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'ci')][string]$Label = 'devbox'
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

Register-MandatoryCapture -Name @("hwnd-wrap-churn-$Label.json")

function Write-A24Capture {
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

function Get-BoundedErrorText {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Text)
    if ([string]::IsNullOrEmpty($Text)) { return '' }
    $flat = ($Text -replace '[\r\n]+', ' ').Trim()
    if ($flat.Length -gt 300) { $flat = $flat.Substring(0, 300) + '...' }
    return $flat
}

<#
    Everything below runs in-process: register a scratch window class, then
    in a tight loop CreateWindowEx + record the returned handle's shape +
    DestroyWindow, until either the iteration cap or the wall-clock cap is
    hit. Doing the whole loop in C# rather than crossing the P/Invoke
    boundary once per line of PowerShell is what makes a six-figure
    iteration count finish in bounded time.
#>
function Initialize-A24HwndChurnNative {
    if ('AgentDesktopProbe.A24.HwndChurn05' -as [type]) { return }
    $src = @'
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;

namespace AgentDesktopProbe.A24 {
    public class HwndChurnResult {
        public bool Success;
        public string Branch;
        public string ErrorDetail;
        public int TotalCreated;
        public int CreationFailures;
        public int DistinctFullValues;
        public int DistinctFullValuesReused;
        public int FullValueReuseEvents;
        public int FirstFullValueReuseAtCreationCount = -1;
        public int DistinctIndices;
        public int IndexOnlyReuseEvents;
        public int FirstIndexOnlyReuseAtCreationCount = -1;
        public int MaxGenerationObservedForAnyIndex;
        public long ElapsedMs;
        public string CapHit;
        public int MaxIterationsRequested;
        public int MaxWallClockMsRequested;
    }

    public static class HwndChurn05 {
        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
        private struct WNDCLASSEX {
            public int cbSize;
            public uint style;
            public IntPtr lpfnWndProc;
            public int cbClsExtra;
            public int cbWndExtra;
            public IntPtr hInstance;
            public IntPtr hIcon;
            public IntPtr hCursor;
            public IntPtr hbrBackground;
            [MarshalAs(UnmanagedType.LPWStr)] public string lpszMenuName;
            [MarshalAs(UnmanagedType.LPWStr)] public string lpszClassName;
            public IntPtr hIconSm;
        }

        private delegate IntPtr WndProcDelegate(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);
        private static readonly WndProcDelegate _wndProc = ScratchWndProc;
        private static IntPtr ScratchWndProc(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam) {
            return DefWindowProc(hWnd, msg, wParam, lParam);
        }

        [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        private static extern ushort RegisterClassEx(ref WNDCLASSEX lpwcx);
        [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        private static extern bool UnregisterClass(string lpClassName, IntPtr hInstance);
        [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        private static extern IntPtr CreateWindowEx(uint dwExStyle, string lpClassName, string lpWindowName, uint dwStyle, int x, int y, int nWidth, int nHeight, IntPtr hWndParent, IntPtr hMenu, IntPtr hInstance, IntPtr lpParam);
        [DllImport("user32.dll", SetLastError = true)]
        private static extern bool DestroyWindow(IntPtr hWnd);
        [DllImport("user32.dll")]
        private static extern IntPtr DefWindowProc(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode)]
        private static extern IntPtr GetModuleHandle(string lpModuleName);

        private const uint WS_EX_TOOLWINDOW = 0x00000080;
        private const uint WS_EX_NOACTIVATE = 0x08000000;

        public static HwndChurnResult RunChurn(int maxIterations, int maxWallClockMs, int originX, int originY) {
            var result = new HwndChurnResult();
            result.MaxIterationsRequested = maxIterations;
            result.MaxWallClockMsRequested = maxWallClockMs;

            string className = "ADP24HwndChurn05_" + Guid.NewGuid().ToString("N");
            IntPtr hInstance = GetModuleHandle(null);

            WNDCLASSEX wc = new WNDCLASSEX();
            wc.cbSize = Marshal.SizeOf(typeof(WNDCLASSEX));
            wc.style = 0;
            wc.lpfnWndProc = Marshal.GetFunctionPointerForDelegate(_wndProc);
            wc.cbClsExtra = 0;
            wc.cbWndExtra = 0;
            wc.hInstance = hInstance;
            wc.hIcon = IntPtr.Zero;
            wc.hCursor = IntPtr.Zero;
            wc.hbrBackground = IntPtr.Zero;
            wc.lpszMenuName = null;
            wc.lpszClassName = className;
            wc.hIconSm = IntPtr.Zero;

            ushort atom = RegisterClassEx(ref wc);
            if (atom == 0) {
                result.Success = false;
                result.Branch = "register_class_failed";
                result.ErrorDetail = "RegisterClassEx failed, Win32 error " + Marshal.GetLastWin32Error();
                return result;
            }

            uint dwStyle = 0x00000000;
            uint dwExStyle = WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;

            var fullCounts = new Dictionary<long, int>();
            var indexCounts = new Dictionary<int, int>();
            int total = 0, creationFailures = 0, fullReuseEvents = 0, indexOnlyReuseEvents = 0;
            int firstFullReuseAt = -1, firstIndexOnlyReuseAt = -1, maxGen = 0;
            string capHit = "neither";
            var sw = Stopwatch.StartNew();

            try {
                for (int i = 0; i < maxIterations; i++) {
                    if (sw.ElapsedMilliseconds >= maxWallClockMs) { capHit = "wall_clock_cap"; break; }

                    IntPtr h = IntPtr.Zero;
                    try {
                        h = CreateWindowEx(dwExStyle, className, "", dwStyle, originX, originY, 10, 10, IntPtr.Zero, IntPtr.Zero, hInstance, IntPtr.Zero);
                        if (h == IntPtr.Zero) {
                            creationFailures++;
                            if (creationFailures > 20) { capHit = "creation_failure"; break; }
                            continue;
                        }

                        total++;
                        long val = h.ToInt64();
                        int idx = (int)(val & 0xFFFF);
                        int gen = (int)((val >> 16) & 0xFFFF);
                        if (gen > maxGen) { maxGen = gen; }

                        int prevFullCount;
                        bool fullSeenBefore = fullCounts.TryGetValue(val, out prevFullCount);
                        if (fullSeenBefore) {
                            fullReuseEvents++;
                            if (firstFullReuseAt < 0) { firstFullReuseAt = total; }
                            fullCounts[val] = prevFullCount + 1;
                        } else {
                            fullCounts[val] = 1;
                        }

                        int prevIdxCount;
                        bool idxSeenBefore = indexCounts.TryGetValue(idx, out prevIdxCount);
                        if (idxSeenBefore) {
                            if (!fullSeenBefore) {
                                indexOnlyReuseEvents++;
                                if (firstIndexOnlyReuseAt < 0) { firstIndexOnlyReuseAt = total; }
                            }
                            indexCounts[idx] = prevIdxCount + 1;
                        } else {
                            indexCounts[idx] = 1;
                        }
                    } finally {
                        if (h != IntPtr.Zero) { DestroyWindow(h); }
                    }
                }
            } finally {
                UnregisterClass(className, hInstance);
            }

            sw.Stop();
            if (capHit == "neither") { capHit = "iteration_cap"; }

            int distinctFullValuesReused = 0;
            foreach (var kv in fullCounts) {
                if (kv.Value > 1) { distinctFullValuesReused++; }
            }

            result.Success = true;
            result.Branch = "in_process_create_destroy_loop";
            result.TotalCreated = total;
            result.CreationFailures = creationFailures;
            result.DistinctFullValues = fullCounts.Count;
            result.DistinctFullValuesReused = distinctFullValuesReused;
            result.FullValueReuseEvents = fullReuseEvents;
            result.FirstFullValueReuseAtCreationCount = firstFullReuseAt;
            result.DistinctIndices = indexCounts.Count;
            result.IndexOnlyReuseEvents = indexOnlyReuseEvents;
            result.FirstIndexOnlyReuseAtCreationCount = firstIndexOnlyReuseAt;
            result.MaxGenerationObservedForAnyIndex = maxGen;
            result.ElapsedMs = sw.ElapsedMilliseconds;
            result.CapHit = capHit;
            return result;
        }
    }
}
'@
    Add-ProbeInlineCSharp -Source $src -AssemblyLeaf 'AgentDesktopProbeA24HwndChurn'
}

<#
    2000,2000 is genuinely off-screen for this box's 1639x732 display. No
    window is ever shown (WS_VISIBLE is never set) or activated (WS_EX_
    NOACTIVATE), so the origin is belt-and-suspenders on top of that, not
    the only thing keeping the screen and the foreground undisturbed.
#>
function Measure-HwndWrapChurn {
    param(
        [int]$MaxIterations = 300000,
        [int]$MaxWallClockMs = 90000,
        [int]$OriginX = 2000,
        [int]$OriginY = 2000
    )

    Initialize-ProbeNative
    Initialize-A24HwndChurnNative

    $fgBefore = [AgentDesktopProbe.Native]::GetForegroundProcessId()
    $native = [AgentDesktopProbe.A24.HwndChurn05]::RunChurn($MaxIterations, $MaxWallClockMs, $OriginX, $OriginY)
    $fgAfter = [AgentDesktopProbe.Native]::GetForegroundProcessId()
    $foregroundStable = ($fgBefore -eq $fgAfter)
    if (-not $foregroundStable -and $fgBefore -ne 0 -and $fgAfter -ne 0) {
        throw ('PROBE-INTERFERENCE: foreground pid changed during hwnd churn (before and after disagree)')
    }

    if (-not $native.Success) {
        return [ordered]@{
            measurable         = $false
            branch             = [string]$native.Branch
            error_detail       = (Get-BoundedErrorText -Text ([string]$native.ErrorDetail))
            foreground_stable  = $foregroundStable
        }
    }

    if ($native.TotalCreated -eq 0) {
        return [ordered]@{
            measurable         = $false
            branch             = 'zero_windows_created_before_cap'
            creation_failures  = $native.CreationFailures
            cap_hit            = [string]$native.CapHit
            foreground_stable  = $foregroundStable
        }
    }

    $fullValueReuseObserved = ($native.FullValueReuseEvents -gt 0)
    $indexOnlyReuseObserved = ($native.IndexOnlyReuseEvents -gt 0)

    $branch = if ($fullValueReuseObserved) {
        'full_value_reuse_observed'
    } elseif ($indexOnlyReuseObserved) {
        'index_only_reuse_observed_no_full_value_reuse'
    } else {
        'no_reuse_observed_at_all'
    }

    return [ordered]@{
        measurable                                    = $true
        branch                                         = $branch
        cap_hit                                         = [string]$native.CapHit
        max_iterations_requested                        = $native.MaxIterationsRequested
        max_wall_clock_ms_requested                     = $native.MaxWallClockMsRequested
        total_windows_created                           = $native.TotalCreated
        creation_failures                               = $native.CreationFailures
        elapsed_ms                                      = $native.ElapsedMs
        distinct_full_hwnd_values_seen                  = $native.DistinctFullValues
        full_value_reuse_events                         = $native.FullValueReuseEvents
        full_value_reuse_observed                       = $fullValueReuseObserved
        first_full_value_reuse_at_creation_count        = $(if ($native.FirstFullValueReuseAtCreationCount -ge 0) { $native.FirstFullValueReuseAtCreationCount } else { $null })
        distinct_hwnd_indices_seen                      = $native.DistinctIndices
        index_only_reuse_events                         = $native.IndexOnlyReuseEvents
        index_only_reuse_observed                       = $indexOnlyReuseObserved
        first_index_only_reuse_at_creation_count        = $(if ($native.FirstIndexOnlyReuseAtCreationCount -ge 0) { $native.FirstIndexOnlyReuseAtCreationCount } else { $null })
        max_generation_counter_observed_for_any_index   = $native.MaxGenerationObservedForAnyIndex
        generation_field_width_bits                     = 16
        foreground_stable                                = $foregroundStable
    }
}

function Measure-HwndWrapChurnReport {
    return [ordered]@{
        probe       = '24-fixture-e2e/05-hwnd-wrap-churn'
        question    = 'what is the HWND uniqueness-counter wrap rate under staged churn from one still-running process: total windows created, distinct HWND values seen, how many HWND VALUES were reused (full-value collision, the wrap hazard) versus how many creations only reused the low-16-bit index while the high-16-bit counter advanced (benign slot recycling), the creation count at first full-value reuse, the highest counter value observed for any index, and elapsed wall time'
        methodology = 'CreateWindowEx/DestroyWindow round-trips synthesized entirely in-process via P/Invoke against a scratch window class this probe registers and unregisters itself - no external process is spawned per window, the whole burst runs inside this one still-running PowerShell process; every window is created invisible (no WS_VISIBLE), WS_EX_NOACTIVATE|WS_EX_TOOLWINDOW, at an off-screen origin, and destroyed before the next is created; a full HWND value is the 32-bit handle CreateWindowEx returns; index = low 16 bits, generation counter = high 16 bits, per the documented USER-handle index+uniqueness-counter encoding; capped by BOTH an iteration count and a wall-clock budget, whichever is hit first'
        result      = Measure-HwndWrapChurn
    }
}

try {
    $measurement = Measure-HwndWrapChurnReport
    $script:CapturePath = Write-A24Capture -Name "hwnd-wrap-churn-$Label.json" -Content (ConvertTo-Json -InputObject $measurement -Depth 10)
    Register-MandatoryPass -Capture $script:CapturePath -Result $measurement
} finally {
    # No external process is ever spawned by this probe and every scratch
    # window is destroyed by RunChurn's own try/finally before it returns,
    # so there is nothing left to clean up on the PowerShell side.
}

Assert-MandatoryMeasurement -Probe '24-fixture-e2e/05-hwnd-wrap-churn' -Label $Label

Write-ProbeResult -Probe '24-fixture-e2e/05-hwnd-wrap-churn' -Status 'ok' -Message 'hwnd uniqueness-counter wrap-rate under churn measured' -Data @{
    capture = if ($script:CapturePath) { Split-Path -Leaf $script:CapturePath } else { '<none>' }
    branch  = [string]$measurement.result.branch
    total   = if ($measurement.result.Contains('total_windows_created')) { $measurement.result.total_windows_created } else { 0 }
}
exit 0
