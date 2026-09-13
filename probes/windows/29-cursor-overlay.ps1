#Requires -Version 5.1
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path (Split-Path -Parent $MyInvocation.MyCommand.Path) 'common.ps1')

$probe = '29-cursor-overlay'
$captureDir = Get-CaptureDir -Probe $probe

$overlaySource = @'
using System;
using System.Runtime.InteropServices;

namespace AgentDesktopOverlayProbe {
    [StructLayout(LayoutKind.Sequential)]
    public struct POINT { public int x; public int y; }

    [StructLayout(LayoutKind.Sequential)]
    public struct SIZE { public int cx; public int cy; }

    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct DEVMODEW {
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmDeviceName;
        public ushort dmSpecVersion; public ushort dmDriverVersion; public ushort dmSize;
        public ushort dmDriverExtra; public uint dmFields;
        public int dmPositionX; public int dmPositionY; public uint dmDisplayOrientation;
        public uint dmDisplayFixedOutput; public short dmColor; public short dmDuplex;
        public short dmYResolution; public short dmTTOption; public short dmCollate;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmFormName;
        public ushort dmLogPixels; public uint dmBitsPerPel; public uint dmPelsWidth;
        public uint dmPelsHeight; public uint dmDisplayFlags; public uint dmDisplayFrequency;
        public uint dmICMMethod; public uint dmICMIntent; public uint dmMediaType;
        public uint dmDitherType; public uint dmReserved1; public uint dmReserved2;
        public uint dmPanningWidth; public uint dmPanningHeight;
    }

    [StructLayout(LayoutKind.Sequential, Pack = 1)]
    public struct BLENDFUNCTION {
        public byte BlendOp; public byte BlendFlags;
        public byte SourceConstantAlpha; public byte AlphaFormat;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct BITMAPINFOHEADER {
        public uint biSize; public int biWidth; public int biHeight;
        public ushort biPlanes; public ushort biBitCount; public uint biCompression;
        public uint biSizeImage; public int biXPelsPerMeter; public int biYPelsPerMeter;
        public uint biClrUsed; public uint biClrImportant;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct MSG {
        public IntPtr hwnd; public uint message; public IntPtr wParam; public IntPtr lParam;
        public uint time; public POINT pt;
    }

    public delegate IntPtr WndProcDelegate(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct WNDCLASSEXW {
        public uint cbSize; public uint style; public WndProcDelegate lpfnWndProc;
        public int cbClsExtra; public int cbWndExtra; public IntPtr hInstance;
        public IntPtr hIcon; public IntPtr hCursor; public IntPtr hbrBackground;
        [MarshalAs(UnmanagedType.LPWStr)] public string lpszMenuName;
        [MarshalAs(UnmanagedType.LPWStr)] public string lpszClassName;
        public IntPtr hIconSm;
    }

    public static class Overlay {
        public const int WS_EX_LAYERED = 0x00080000;
        public const int WS_EX_TRANSPARENT = 0x00000020;
        public const int WS_EX_TOOLWINDOW = 0x00000080;
        public const int WS_EX_NOACTIVATE = 0x08000000;
        public const int WS_EX_TOPMOST = 0x00000008;
        public const int WS_POPUP = unchecked((int)0x80000000);
        public const int SW_SHOWNOACTIVATE = 4;
        public const int SW_SHOW = 5;
        public const uint SWP_NOACTIVATE = 0x0010;
        public const uint SWP_NOSIZE = 0x0001;
        public const uint SWP_NOMOVE = 0x0002;
        public const uint SWP_SHOWWINDOW = 0x0040;
        public const uint ULW_ALPHA = 0x00000002;
        public const uint SRCCOPY = 0x00CC0020;
        public const uint CAPTUREBLT = 0x40000000;

        static WndProcDelegate keepAlive;
        static bool registered;
        const string ClassName = "AgentDesktopOverlayProbeClass";

        [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        static extern ushort RegisterClassExW(ref WNDCLASSEXW cls);
        [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        static extern IntPtr CreateWindowExW(int exStyle, string cls, string name, int style,
            int x, int y, int w, int h, IntPtr parent, IntPtr menu, IntPtr inst, IntPtr param);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        static extern IntPtr DefWindowProcW(IntPtr h, uint m, IntPtr w, IntPtr l);
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode)]
        static extern IntPtr GetModuleHandleW(string name);
        [DllImport("user32.dll", SetLastError = true)]
        public static extern bool DestroyWindow(IntPtr h);
        [DllImport("user32.dll")]
        public static extern bool ShowWindow(IntPtr h, int cmd);
        [DllImport("user32.dll", SetLastError = true)]
        public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int w, int hh, uint flags);
        [DllImport("user32.dll")]
        public static extern IntPtr GetForegroundWindow();
        [DllImport("user32.dll")]
        public static extern IntPtr WindowFromPoint(POINT p);
        [DllImport("user32.dll")]
        public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        public static extern IntPtr FindWindowW(string cls, string name);
        [DllImport("user32.dll", SetLastError = true)]
        public static extern bool GetWindowRect(IntPtr h, out RECT rect);
        [DllImport("user32.dll")]
        static extern bool PeekMessageW(out MSG msg, IntPtr h, uint min, uint max, uint remove);
        [DllImport("user32.dll")]
        static extern bool TranslateMessage(ref MSG msg);
        [DllImport("user32.dll")]
        static extern IntPtr DispatchMessageW(ref MSG msg);
        [DllImport("user32.dll")]
        public static extern IntPtr GetDC(IntPtr h);
        [DllImport("user32.dll")]
        public static extern int ReleaseDC(IntPtr h, IntPtr dc);
        [DllImport("user32.dll", SetLastError = true)]
        public static extern bool UpdateLayeredWindow(IntPtr h, IntPtr dst, ref POINT pos, ref SIZE size,
            IntPtr src, ref POINT srcPos, uint key, ref BLENDFUNCTION blend, uint flags);
        [DllImport("gdi32.dll")]
        public static extern IntPtr CreateCompatibleDC(IntPtr dc);
        [DllImport("gdi32.dll")]
        public static extern bool DeleteDC(IntPtr dc);
        [DllImport("gdi32.dll")]
        public static extern IntPtr SelectObject(IntPtr dc, IntPtr obj);
        [DllImport("gdi32.dll")]
        public static extern bool DeleteObject(IntPtr obj);
        [DllImport("gdi32.dll", SetLastError = true)]
        public static extern IntPtr CreateDIBSection(IntPtr dc, ref BITMAPINFOHEADER header, uint usage,
            out IntPtr bits, IntPtr section, uint offset);
        [DllImport("gdi32.dll")]
        public static extern IntPtr CreateCompatibleBitmap(IntPtr dc, int w, int h);
        [DllImport("gdi32.dll", SetLastError = true)]
        public static extern bool BitBlt(IntPtr dst, int x, int y, int w, int h, IntPtr src, int sx, int sy, uint rop);
        [DllImport("gdi32.dll")]
        public static extern uint GetPixel(IntPtr dc, int x, int y);
        [DllImport("user32.dll")]
        public static extern int GetSystemMetrics(int index);
        [DllImport("gdi32.dll")]
        public static extern int GetDeviceCaps(IntPtr dc, int index);
        [DllImport("user32.dll", SetLastError = true)]
        public static extern bool SystemParametersInfoW(uint action, uint param, ref int result, uint winIni);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        public static extern bool EnumDisplaySettingsW(string device, int mode, ref DEVMODEW dev);

        public static void EnsureClass() {
            if (registered) { return; }
            keepAlive = new WndProcDelegate(DefWindowProcW);
            WNDCLASSEXW cls = new WNDCLASSEXW();
            cls.cbSize = (uint)Marshal.SizeOf(typeof(WNDCLASSEXW));
            cls.lpfnWndProc = keepAlive;
            cls.hInstance = GetModuleHandleW(null);
            cls.lpszClassName = ClassName;
            if (RegisterClassExW(ref cls) == 0) {
                int err = Marshal.GetLastWin32Error();
                if (err != 1410) { throw new InvalidOperationException("RegisterClassExW failed: " + err); }
            }
            registered = true;
        }

        public static IntPtr Create(bool noActivate, bool transparent, int x, int y, int w, int h) {
            EnsureClass();
            int ex = WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST;
            if (noActivate) { ex |= WS_EX_NOACTIVATE; }
            if (transparent) { ex |= WS_EX_TRANSPARENT; }
            IntPtr hwnd = CreateWindowExW(ex, ClassName, "agent-desktop overlay probe", WS_POPUP,
                x, y, w, h, IntPtr.Zero, IntPtr.Zero, GetModuleHandleW(null), IntPtr.Zero);
            if (hwnd == IntPtr.Zero) {
                throw new InvalidOperationException("CreateWindowExW failed: " + Marshal.GetLastWin32Error());
            }
            return hwnd;
        }

        public static void Pump() {
            MSG msg;
            int guard = 0;
            while (guard < 256 && PeekMessageW(out msg, IntPtr.Zero, 0, 0, 1)) {
                TranslateMessage(ref msg);
                DispatchMessageW(ref msg);
                guard++;
            }
        }

        public static long PaintTicks(IntPtr hwnd, int x, int y, int w, int h, uint bgra) {
            IntPtr screen = GetDC(IntPtr.Zero);
            IntPtr mem = CreateCompatibleDC(screen);
            BITMAPINFOHEADER header = new BITMAPINFOHEADER();
            header.biSize = (uint)Marshal.SizeOf(typeof(BITMAPINFOHEADER));
            header.biWidth = w;
            header.biHeight = -h;
            header.biPlanes = 1;
            header.biBitCount = 32;
            header.biCompression = 0;
            IntPtr bits;
            IntPtr dib = CreateDIBSection(mem, ref header, 0, out bits, IntPtr.Zero, 0);
            if (dib == IntPtr.Zero) {
                DeleteDC(mem); ReleaseDC(IntPtr.Zero, screen);
                throw new InvalidOperationException("CreateDIBSection failed: " + Marshal.GetLastWin32Error());
            }
            IntPtr previous = SelectObject(mem, dib);
            int count = w * h;
            int[] pixels = new int[count];
            for (int i = 0; i < count; i++) { pixels[i] = unchecked((int)bgra); }
            Marshal.Copy(pixels, 0, bits, count);

            POINT pos = new POINT(); pos.x = x; pos.y = y;
            POINT src = new POINT(); src.x = 0; src.y = 0;
            SIZE size = new SIZE(); size.cx = w; size.cy = h;
            BLENDFUNCTION blend = new BLENDFUNCTION();
            blend.BlendOp = 0; blend.BlendFlags = 0;
            blend.SourceConstantAlpha = 255; blend.AlphaFormat = 1;

            System.Diagnostics.Stopwatch watch = System.Diagnostics.Stopwatch.StartNew();
            bool ok = UpdateLayeredWindow(hwnd, screen, ref pos, ref size, mem, ref src, 0, ref blend, ULW_ALPHA);
            watch.Stop();
            int lastError = Marshal.GetLastWin32Error();

            SelectObject(mem, previous);
            DeleteObject(dib);
            DeleteDC(mem);
            ReleaseDC(IntPtr.Zero, screen);
            if (!ok) { throw new InvalidOperationException("UpdateLayeredWindow failed: " + lastError); }
            return watch.ElapsedTicks;
        }

        public static uint SampleScreenPixel(int x, int y) {
            IntPtr screen = GetDC(IntPtr.Zero);
            IntPtr mem = CreateCompatibleDC(screen);
            IntPtr bitmap = CreateCompatibleBitmap(screen, 1, 1);
            IntPtr previous = SelectObject(mem, bitmap);
            BitBlt(mem, 0, 0, 1, 1, screen, x, y, SRCCOPY | CAPTUREBLT);
            uint value = GetPixel(mem, 0, 0);
            SelectObject(mem, previous);
            DeleteObject(bitmap);
            DeleteDC(mem);
            ReleaseDC(IntPtr.Zero, screen);
            return value;
        }
    }
}
'@

Add-ProbeInlineCSharp -Source $overlaySource -AssemblyLeaf 'AgentDesktopOverlayProbe'
Add-Type -AssemblyName System.Windows.Forms
Initialize-ProbeNative

$O = [AgentDesktopOverlayProbe.Overlay]

# PowerShell parses a hex literal above 0x7FFFFFFF as a negative Int32, so the
# premultiplied BGRA fills are parsed as unsigned rather than written as literals.
$accentBgra = [uint32]::Parse('FF4299FF', [System.Globalization.NumberStyles]::HexNumber)
$markerBgra = [uint32]::Parse('FF00FF00', [System.Globalization.NumberStyles]::HexNumber)

function New-ProbePoint {
    param([int]$X, [int]$Y)
    $point = New-Object AgentDesktopOverlayProbe.POINT
    $point.x = $X
    $point.y = $Y
    return $point
}

function Get-MinOfSevenMs {
    param([Parameter(Mandatory = $true)][scriptblock]$Measure)
    $samples = New-Object System.Collections.ArrayList
    for ($i = 0; $i -lt 8; $i++) {
        $ticks = & $Measure
        if ($i -eq 0) { continue }
        [void]$samples.Add(($ticks / [double][System.Diagnostics.Stopwatch]::Frequency) * 1000.0)
    }
    $sorted = $samples | Sort-Object
    return [pscustomobject]@{
        min_ms    = [math]::Round($sorted[0], 3)
        median_ms = [math]::Round($sorted[3], 3)
        max_ms    = [math]::Round($sorted[6], 3)
        samples   = 7
    }
}

$results = [ordered]@{}
$scratchPid = 0

try {
    # A target that holds the foreground for the whole run, so every foreground
    # claim is measured against a window this probe does not own.
    $scratch = Start-ScratchProcess -FilePath 'notepad.exe'
    $scratchPid = $scratch.ProcessId
    $targetHandle = $scratch.MainWindowHandle
    if ($targetHandle -eq [IntPtr]::Zero) {
        throw 'PROBE-HARNESS: the scratch target never presented a window'
    }
    Start-Sleep -Milliseconds 600
    $foregroundBeforeAnything = $O::GetForegroundWindow()

    # --- Foreground steal, measured in both directions -------------------------
    $steal = [ordered]@{}
    foreach ($case in @(
        @{ Name = 'no_activate'; NoActivate = $true;  Show = $O::SW_SHOWNOACTIVATE },
        @{ Name = 'activating';  NoActivate = $false; Show = $O::SW_SHOW }
    )) {
        $before = $O::GetForegroundWindow()
        $hwnd = $O::Create($case.NoActivate, $true, 400, 300, 256, 256)
        $afterCreate = $O::GetForegroundWindow()
        [void]$O::ShowWindow($hwnd, $case.Show)
        $O::Pump()
        Start-Sleep -Milliseconds 120
        $afterShow = $O::GetForegroundWindow()
        [void]$O::PaintTicks($hwnd, 400, 300, 256, 256, $accentBgra)
        $O::Pump()
        Start-Sleep -Milliseconds 120
        $afterPaint = $O::GetForegroundWindow()
        [void]$O::SetWindowPos($hwnd, [IntPtr](-1), 500, 400, 0, 0, ($O::SWP_NOACTIVATE -bor $O::SWP_NOSIZE))
        $O::Pump()
        Start-Sleep -Milliseconds 120
        $afterMove = $O::GetForegroundWindow()
        [void]$O::DestroyWindow($hwnd)
        $O::Pump()
        Start-Sleep -Milliseconds 200

        $steal[$case.Name] = [ordered]@{
            foreground_was_target_before = ($before -eq $targetHandle)
            overlay_took_foreground      = @(
                ($afterCreate -eq $hwnd), ($afterShow -eq $hwnd),
                ($afterPaint -eq $hwnd), ($afterMove -eq $hwnd)
            )
            target_kept_foreground       = @(
                ($afterCreate -eq $targetHandle), ($afterShow -eq $targetHandle),
                ($afterPaint -eq $targetHandle), ($afterMove -eq $targetHandle)
            )
            stages                       = @('create', 'show', 'paint', 'move')
        }
    }
    $results['foreground_steal'] = $steal

    # --- UpdateLayeredWindow cost: follower window versus virtual screen -------
    $virtualWidth = $O::GetSystemMetrics(78)
    $virtualHeight = $O::GetSystemMetrics(79)
    $virtualX = $O::GetSystemMetrics(76)
    $virtualY = $O::GetSystemMetrics(77)

    $paintCosts = [ordered]@{}
    foreach ($shape in @(
        @{ Name = 'follower_256'; W = 256; H = 256 },
        @{ Name = 'follower_512'; W = 512; H = 512 },
        @{ Name = 'virtual_screen'; W = $virtualWidth; H = $virtualHeight }
    )) {
        $hwnd = $O::Create($true, $true, $virtualX, $virtualY, $shape.W, $shape.H)
        [void]$O::ShowWindow($hwnd, $O::SW_SHOWNOACTIVATE)
        $O::Pump()
        $width = $shape.W
        $height = $shape.H
        $cost = Get-MinOfSevenMs -Measure { $O::PaintTicks($hwnd, $virtualX, $virtualY, $width, $height, $accentBgra) }
        [void]$O::DestroyWindow($hwnd)
        $O::Pump()
        $paintCosts[$shape.Name] = [ordered]@{
            width    = $shape.W
            height   = $shape.H
            pixels   = ([int64]$shape.W * [int64]$shape.H)
            cost_ms  = $cost
        }
    }
    $results['update_layered_window_cost'] = $paintCosts

    # --- Z-order over the shell's own topmost chrome, by pixel ----------------
    $taskbar = $O::FindWindowW('Shell_TrayWnd', $null)
    $taskbarZ = [ordered]@{ taskbar_found = ($taskbar -ne [IntPtr]::Zero) }
    if ($taskbar -ne [IntPtr]::Zero) {
        $rect = New-Object AgentDesktopOverlayProbe.RECT
        [void]$O::GetWindowRect($taskbar, [ref]$rect)
        $sampleX = [int](($rect.Left + $rect.Right) / 2)
        $sampleY = [int](($rect.Top + $rect.Bottom) / 2)
        $before = $O::SampleScreenPixel($sampleX, $sampleY)
        $hwnd = $O::Create($true, $true, ($sampleX - 64), ($sampleY - 16), 128, 32)
        [void]$O::ShowWindow($hwnd, $O::SW_SHOWNOACTIVATE)
        [void]$O::SetWindowPos($hwnd, [IntPtr](-1), 0, 0, 0, 0,
            ($O::SWP_NOACTIVATE -bor $O::SWP_NOSIZE -bor $O::SWP_NOMOVE))
        [void]$O::PaintTicks($hwnd, ($sampleX - 64), ($sampleY - 16), 128, 32, $markerBgra)
        $O::Pump()
        Start-Sleep -Milliseconds 400
        $over = $O::SampleScreenPixel($sampleX, $sampleY)
        $foregroundDuring = $O::GetForegroundWindow()
        [void]$O::DestroyWindow($hwnd)
        $O::Pump()
        Start-Sleep -Milliseconds 300
        $after = $O::SampleScreenPixel($sampleX, $sampleY)
        $taskbarZ['pixel_before'] = ('0x{0:X6}' -f $before)
        $taskbarZ['pixel_with_overlay'] = ('0x{0:X6}' -f $over)
        $taskbarZ['pixel_after_teardown'] = ('0x{0:X6}' -f $after)
        $taskbarZ['overlay_paints_over_taskbar'] = ($over -eq 0x00FF00 -and $before -ne 0x00FF00)
        $taskbarZ['teardown_restores_pixel'] = ($after -eq $before)
        $taskbarZ['overlay_took_foreground'] = ($foregroundDuring -eq $hwnd)
    }
    $results['taskbar_z_order'] = $taskbarZ

    # --- Click-through, measured in both directions ---------------------------
    $clickThrough = [ordered]@{}
    foreach ($case in @(
        @{ Name = 'transparent'; Transparent = $true },
        @{ Name = 'opaque_to_hit_test'; Transparent = $false }
    )) {
        $hwnd = $O::Create($true, $case.Transparent, 700, 500, 200, 200)
        [void]$O::ShowWindow($hwnd, $O::SW_SHOWNOACTIVATE)
        [void]$O::PaintTicks($hwnd, 700, 500, 200, 200, $accentBgra)
        $O::Pump()
        Start-Sleep -Milliseconds 200
        $hit = $O::WindowFromPoint((New-ProbePoint -X 800 -Y 600))
        [void]$O::DestroyWindow($hwnd)
        $O::Pump()
        Start-Sleep -Milliseconds 150
        $clickThrough[$case.Name] = [ordered]@{
            hit_test_returned_overlay = ($hit -eq $hwnd)
        }
    }
    $results['click_through'] = $clickThrough

    # --- Named-pipe control channel roundtrip, from an already-running process -
    $pipeName = 'agent-desktop-cursor-probe-' + [guid]::NewGuid().ToString('N')
    $server = New-Object System.IO.Pipes.NamedPipeServerStream(
        $pipeName, [System.IO.Pipes.PipeDirection]::InOut, 1,
        [System.IO.Pipes.PipeTransmissionMode]::Byte,
        [System.IO.Pipes.PipeOptions]::Asynchronous)
    $serverLoop = [powershell]::Create()
    [void]$serverLoop.AddScript({
            param($stream, $rounds)
            for ($i = 0; $i -lt $rounds; $i++) {
                $stream.WaitForConnection()
                $buffer = New-Object byte[] 4096
                $read = $stream.Read($buffer, 0, $buffer.Length)
                if ($read -gt 0) { $stream.Write([byte[]]@(1), 0, 1); $stream.Flush() }
                $stream.Disconnect()
            }
            $stream.Dispose()
        }).AddArgument($server).AddArgument(8)
    $serverHandle = $serverLoop.BeginInvoke()

    $payload = [System.Text.Encoding]::UTF8.GetBytes(
        '{"action":"present","session_id":"s0000000","instruction":{"destination":{"x":100.0,"y":100.0},"click":true,"phase":"travel"}}')
    $roundtrip = Get-MinOfSevenMs -Measure {
        $watch = [System.Diagnostics.Stopwatch]::StartNew()
        $client = New-Object System.IO.Pipes.NamedPipeClientStream(
            '.', $pipeName, [System.IO.Pipes.PipeDirection]::InOut)
        $client.Connect(2000)
        $client.Write($payload, 0, $payload.Length)
        $client.Flush()
        $acknowledgement = New-Object byte[] 1
        [void]$client.Read($acknowledgement, 0, 1)
        $watch.Stop()
        $client.Dispose()
        $watch.ElapsedTicks
    }
    [void]$serverLoop.EndInvoke($serverHandle)
    $serverLoop.Dispose()
    $results['named_pipe_roundtrip'] = [ordered]@{
        transport = 'NamedPipeClientStream connect + write + one-byte ack read'
        cost_ms   = $roundtrip
    }

    # --- Coordinate space and per-monitor DPI --------------------------------
    $monitors = New-Object System.Collections.ArrayList
    foreach ($screen in [System.Windows.Forms.Screen]::AllScreens) {
        [void]$monitors.Add([ordered]@{
                primary = $screen.Primary
                width   = $screen.Bounds.Width
                height  = $screen.Bounds.Height
            })
    }
    # A false answer here would be indistinguishable from a failed call, so the
    # call's own return is recorded beside the value it wrote.
    $animationsOn = 0
    $animationsRead = $O::SystemParametersInfoW(0x1042, 0, [ref]$animationsOn, 0)
    $uiEffectsOn = 0
    $uiEffectsRead = $O::SystemParametersInfoW(0x103E, 0, [ref]$uiEffectsOn, 0)
    # Two refresh-rate sources are read so the working one is chosen by
    # measurement: the device-name-free EnumDisplaySettings call is the obvious
    # one and the screen DC's VREFRESH is the alternative.
    $mode = New-Object AgentDesktopOverlayProbe.DEVMODEW
    $mode.dmSize = [uint16][System.Runtime.InteropServices.Marshal]::SizeOf([type]'AgentDesktopOverlayProbe.DEVMODEW')
    $enumRead = $O::EnumDisplaySettingsW($null, -1, [ref]$mode)
    $screenDc = $O::GetDC([IntPtr]::Zero)
    $deviceCapsHz = $O::GetDeviceCaps($screenDc, 116)
    [void]$O::ReleaseDC([IntPtr]::Zero, $screenDc)
    # A remote session would explain an animations-disabled reading as a bandwidth
    # artifact rather than a configured preference, so the session kind is read
    # rather than inferred from the fact that this desktop arrives over RDP.
    $remoteSession = $O::GetSystemMetrics(4096)
    $results['session_kind'] = [ordered]@{
        remote_session       = ($remoteSession -ne 0)
        monitors_by_metric   = $O::GetSystemMetrics(80)
    }

    $results['coordinate_space'] = [ordered]@{
        virtual_screen         = [ordered]@{ x = $virtualX; y = $virtualY; width = $virtualWidth; height = $virtualHeight }
        monitor_count          = $monitors.Count
        monitors               = $monitors
        client_area_animations = [ordered]@{ call_succeeded = $animationsRead; enabled = ($animationsOn -ne 0) }
        ui_effects             = [ordered]@{ call_succeeded = $uiEffectsRead; enabled = ($uiEffectsOn -ne 0) }
        refresh_hz             = [ordered]@{
            enum_display_settings_null_device = [ordered]@{ call_succeeded = $enumRead; value = $mode.dmDisplayFrequency }
            screen_dc_vrefresh                = $deviceCapsHz
        }
        mixed_dpi_measurable   = ($monitors.Count -gt 1)
    }

    $results['foreground_at_start_was_target'] = ($foregroundBeforeAnything -eq $targetHandle)

    [void](Write-ProbeJson -Probe $probe -Name 'cursor-overlay.json' -InputObject $results)
    Write-ProbeResult -Probe $probe -Status 'ok' -Message 'cursor overlay renderer feasibility measured' -Data $results
} finally {
    if ($scratchPid -ne 0) { Stop-ScratchProcess -ProcessId $scratchPid }
}
