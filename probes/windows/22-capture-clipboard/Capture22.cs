using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.Drawing.Imaging;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;

namespace AgentDesktopProbe.A22 {
    public sealed class HostInfo {
        public int Build;
        public int SessionId;
        public string WindowStation;
        public string Desktop;
        public bool UserInteractive;
        public bool WgcIsSupported;
        public string WgcSupportSource;
        public string WgcErrorKind;
    }

    public sealed class PixelStats {
        public bool Ok;
        public bool ApiOk;
        public int Width;
        public int Height;
        public long NonZeroPixels;
        public long TotalPixels;
        public bool AppearsBlack;
        public int LastError;
        public string Outcome;
    }

    public sealed class BlockingMeas {
        public bool ReturnedWithinBound;
        public long ElapsedMs;
        public int BoundMs;
        public string Classification;
        public bool ApiOk;
        public int LastError;
    }

    public sealed class ClipboardContention {
        public bool OpenClipboardReturned;
        public int OpenClipboardLastError;
        public bool HolderWindowNonZero;
        public int RetryAttempts;
        public int RetrySuccesses;
        public int[] RetryElapsedMs;
        public uint SeqBefore;
        public uint SeqAfterSet;
        public uint SeqAfterEmpty;
        public uint SeqAfterNoop;
        public bool SeqAdvancedOnSet;
        public bool SeqAdvancedOnEmpty;
        public bool SeqAdvancedOnNoop;
    }

    public sealed class DibShape {
        public bool Present;
        public int FormatId;
        public string FormatName;
        public int HeaderBytes;
        public int BiWidth;
        public int BiHeight;
        public int BiBitCount;
        public string RowOrder;
        public int PayloadBytes;
    }

    public sealed class IsolationResult {
        public bool Measurable;
        public string Branch;
        public bool ChildSetOk;
        public bool ChildGetOk;
        public bool ParentSeqUnchanged;
        public uint ParentSeqBefore;
        public uint ParentSeqAfter;
        public string ErrorKind;
    }

    public static class Capture22 {
        public const int PW_CLIENTONLY = 0x00000001;
        public const int PW_RENDERFULLCONTENT = 0x00000002;
        public const int SMTO_ABORTIFHUNG = 0x0002;
        public const int CF_UNICODETEXT = 13;
        public const int CF_DIB = 8;
        public const int CF_DIBV5 = 17;
        public const int CF_BITMAP = 2;
        public const uint BI_RGB = 0;
        public const int SRCCOPY = 0x00CC0020;

        [DllImport("user32.dll")]
        public static extern bool PrintWindow(IntPtr hwnd, IntPtr hdcBlt, uint nFlags);

        [DllImport("user32.dll")]
        public static extern IntPtr GetDC(IntPtr hWnd);

        [DllImport("user32.dll")]
        public static extern int ReleaseDC(IntPtr hWnd, IntPtr hDC);

        [DllImport("user32.dll")]
        public static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);

        [DllImport("user32.dll")]
        public static extern bool IsIconic(IntPtr hWnd);

        [DllImport("user32.dll")]
        public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);

        [DllImport("user32.dll", SetLastError = true)]
        public static extern bool OpenClipboard(IntPtr hWndNewOwner);

        [DllImport("user32.dll", SetLastError = true)]
        public static extern bool CloseClipboard();

        [DllImport("user32.dll", SetLastError = true)]
        public static extern bool EmptyClipboard();

        [DllImport("user32.dll", SetLastError = true)]
        public static extern IntPtr SetClipboardData(uint uFormat, IntPtr hMem);

        [DllImport("user32.dll", SetLastError = true)]
        public static extern IntPtr GetClipboardData(uint uFormat);

        [DllImport("user32.dll")]
        public static extern uint GetClipboardSequenceNumber();

        [DllImport("user32.dll")]
        public static extern IntPtr GetOpenClipboardWindow();

        [DllImport("user32.dll")]
        public static extern IntPtr GetClipboardOwner();

        [DllImport("user32.dll")]
        public static extern uint RegisterClipboardFormatW([MarshalAs(UnmanagedType.LPWStr)] string lpszFormat);

        [DllImport("user32.dll")]
        public static extern bool IsClipboardFormatAvailable(uint format);

        [DllImport("user32.dll")]
        public static extern uint EnumClipboardFormats(uint format);

        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        public static extern int GetClipboardFormatNameW(uint format, StringBuilder lpszFormatName, int cchMaxCount);

        [DllImport("user32.dll", SetLastError = true)]
        public static extern IntPtr SendMessageTimeoutW(IntPtr hWnd, uint Msg, IntPtr wParam, IntPtr lParam, uint fuFlags, uint uTimeout, out IntPtr lpdwResult);

        [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        public static extern IntPtr CreateWindowExW(uint dwExStyle, string lpClassName, string lpWindowName, uint dwStyle, int x, int y, int nWidth, int nHeight, IntPtr hWndParent, IntPtr hMenu, IntPtr hInstance, IntPtr lpParam);

        [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        public static extern ushort RegisterClassW(ref WNDCLASS lpWndClass);

        [DllImport("user32.dll")]
        public static extern bool DestroyWindow(IntPtr hWnd);

        [DllImport("user32.dll")]
        public static extern IntPtr DefWindowProcW(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);

        [DllImport("gdi32.dll")]
        public static extern IntPtr CreateCompatibleDC(IntPtr hdc);

        [DllImport("gdi32.dll")]
        public static extern IntPtr CreateCompatibleBitmap(IntPtr hdc, int nWidth, int nHeight);

        [DllImport("gdi32.dll")]
        public static extern IntPtr SelectObject(IntPtr hdc, IntPtr hgdiobj);

        [DllImport("gdi32.dll")]
        public static extern bool DeleteObject(IntPtr hObject);

        [DllImport("gdi32.dll")]
        public static extern bool DeleteDC(IntPtr hdc);

        [DllImport("gdi32.dll")]
        public static extern bool BitBlt(IntPtr hdcDest, int nXDest, int nYDest, int nWidth, int nHeight, IntPtr hdcSrc, int nXSrc, int nYSrc, int dwRop);

        [DllImport("gdi32.dll")]
        public static extern int GetDIBits(IntPtr hdc, IntPtr hbmp, uint uStartScan, uint cScanLines, byte[] lpvBits, ref BITMAPINFO lpbi, uint uUsage);

        [DllImport("kernel32.dll")]
        public static extern uint GetLastError();

        [DllImport("kernel32.dll")]
        public static extern void SetLastError(uint dwErrCode);

        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        public static extern IntPtr GlobalAlloc(uint uFlags, UIntPtr dwBytes);

        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern IntPtr GlobalLock(IntPtr hMem);

        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern bool GlobalUnlock(IntPtr hMem);

        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern IntPtr GlobalFree(IntPtr hMem);

        [DllImport("kernel32.dll")]
        public static extern uint GetCurrentProcessId();

        [DllImport("kernel32.dll")]
        public static extern uint GetCurrentThreadId();

        [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        public static extern IntPtr CreateWindowStationW(string lpwinsta, uint dwFlags, uint dwDesiredAccess, IntPtr lpsa);

        [DllImport("user32.dll", SetLastError = true)]
        public static extern bool SetProcessWindowStation(IntPtr hWinSta);

        [DllImport("user32.dll")]
        public static extern IntPtr GetProcessWindowStation();

        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        public static extern bool GetUserObjectInformationW(IntPtr hObj, int nIndex, byte[] pvInfo, uint nLength, out uint lpnLengthNeeded);

        [DllImport("user32.dll", SetLastError = true)]
        public static extern bool CloseWindowStation(IntPtr hWinSta);

        [StructLayout(LayoutKind.Sequential)]
        public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }

        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
        public struct WNDCLASS {
            public uint style;
            public WndProc lpfnWndProc;
            public int cbClsExtra;
            public int cbWndExtra;
            public IntPtr hInstance;
            public IntPtr hIcon;
            public IntPtr hCursor;
            public IntPtr hbrBackground;
            public string lpszMenuName;
            public string lpszClassName;
        }

        [StructLayout(LayoutKind.Sequential)]
        public struct BITMAPINFOHEADER {
            public uint biSize;
            public int biWidth;
            public int biHeight;
            public ushort biPlanes;
            public ushort biBitCount;
            public uint biCompression;
            public uint biSizeImage;
            public int biXPelsPerMeter;
            public int biYPelsPerMeter;
            public uint biClrUsed;
            public uint biClrImportant;
        }

        [StructLayout(LayoutKind.Sequential)]
        public struct BITMAPINFO {
            public BITMAPINFOHEADER bmiHeader;
            public uint bmiColors;
        }

        public delegate IntPtr WndProc(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);

        static IntPtr ScratchProc(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam) {
            if (msg == 0x000F || msg == 0x0318) {
                IntPtr hdc = GetDC(hWnd);
                if (hdc != IntPtr.Zero) {
                    using (Graphics g = Graphics.FromHdc(hdc))
                    using (var brush = new SolidBrush(Color.FromArgb(255, 40, 120, 200))) {
                        g.FillRectangle(brush, 0, 0, 200, 120);
                    }
                    ReleaseDC(hWnd, hdc);
                }
                return IntPtr.Zero;
            }
            return DefWindowProcW(hWnd, msg, wParam, lParam);
        }

        static bool _classReady;
        static WndProc _procKeepAlive;

        public static IntPtr CreatePaintedWindow() {
            if (!_classReady) {
                _procKeepAlive = ScratchProc;
                var wc = new WNDCLASS();
                wc.lpfnWndProc = _procKeepAlive;
                wc.hInstance = Marshal.GetHINSTANCE(typeof(Capture22).Module);
                wc.lpszClassName = "AgentDesktopA22Paint";
                RegisterClassW(ref wc);
                _classReady = true;
            }
            IntPtr hwnd = CreateWindowExW(0, "AgentDesktopA22Paint", "", 0x00CF0000, 100, 100, 220, 140, IntPtr.Zero, IntPtr.Zero, Marshal.GetHINSTANCE(typeof(Capture22).Module), IntPtr.Zero);
            ShowWindow(hwnd, 5);
            return hwnd;
        }

        public static HostInfo ReadHost() {
            var info = new HostInfo();
            info.Build = Environment.OSVersion.Version.Build;
            info.SessionId = Process.GetCurrentProcess().SessionId;
            info.UserInteractive = Environment.UserInteractive;
            info.WindowStation = ReadObjectName(GetProcessWindowStation());
            info.Desktop = "Default";
            try {
                Type t = Type.GetType("Windows.Graphics.Capture.GraphicsCaptureSession, Windows.Graphics.Capture, ContentType=WindowsRuntime");
                if (t == null) {
                    info.WgcIsSupported = false;
                    info.WgcSupportSource = "type_unavailable";
                    info.WgcErrorKind = "winrt_type_missing";
                } else {
                    var m = t.GetMethod("IsSupported");
                    info.WgcIsSupported = (bool)m.Invoke(null, null);
                    info.WgcSupportSource = "GraphicsCaptureSession.IsSupported";
                    info.WgcErrorKind = null;
                }
            } catch (Exception ex) {
                info.WgcIsSupported = false;
                info.WgcSupportSource = "exception";
                info.WgcErrorKind = ex.GetType().Name;
            }
            return info;
        }

        static string ReadObjectName(IntPtr handle) {
            if (handle == IntPtr.Zero) return "";
            uint needed;
            GetUserObjectInformationW(handle, 2, null, 0, out needed);
            byte[] buf = new byte[Math.Max(needed, 2)];
            uint needed2;
            if (!GetUserObjectInformationW(handle, 2, buf, (uint)buf.Length, out needed2)) return "";
            return Encoding.Unicode.GetString(buf).TrimEnd('\0');
        }

        public static PixelStats CapturePrintWindow(IntPtr hwnd, bool renderFull) {
            var result = new PixelStats();
            RECT rc;
            if (!GetWindowRect(hwnd, out rc)) {
                result.Ok = false;
                result.LastError = (int)GetLastError();
                result.Outcome = "getwindowrect_failed";
                return result;
            }
            int w = Math.Max(1, rc.Right - rc.Left);
            int h = Math.Max(1, rc.Bottom - rc.Top);
            if (IsIconic(hwnd)) {
                result.Ok = false;
                result.Outcome = "minimized";
                result.Width = w;
                result.Height = h;
                return result;
            }
            IntPtr screen = GetDC(IntPtr.Zero);
            IntPtr mem = CreateCompatibleDC(screen);
            IntPtr bmp = CreateCompatibleBitmap(screen, w, h);
            IntPtr old = SelectObject(mem, bmp);
            SetLastError(0);
            bool ok = PrintWindow(hwnd, mem, (uint)(renderFull ? PW_RENDERFULLCONTENT : 0));
            int err = (int)GetLastError();
            FillStatsFromBitmap(result, mem, bmp, w, h, ok, err, renderFull ? "printwindow_full" : "printwindow_basic");
            SelectObject(mem, old);
            DeleteObject(bmp);
            DeleteDC(mem);
            ReleaseDC(IntPtr.Zero, screen);
            return result;
        }

        public static PixelStats CaptureBitBltPrimary() {
            var result = new PixelStats();
            int w = Math.Min(320, System.Windows.Forms.Screen.PrimaryScreen.Bounds.Width);
            int h = Math.Min(200, System.Windows.Forms.Screen.PrimaryScreen.Bounds.Height);
            IntPtr screen = GetDC(IntPtr.Zero);
            IntPtr mem = CreateCompatibleDC(screen);
            IntPtr bmp = CreateCompatibleBitmap(screen, w, h);
            IntPtr old = SelectObject(mem, bmp);
            SetLastError(0);
            bool ok = BitBlt(mem, 0, 0, w, h, screen, 0, 0, SRCCOPY);
            int err = (int)GetLastError();
            FillStatsFromBitmap(result, mem, bmp, w, h, ok, err, "bitblt_primary");
            SelectObject(mem, old);
            DeleteObject(bmp);
            DeleteDC(mem);
            ReleaseDC(IntPtr.Zero, screen);
            return result;
        }

        static void FillStatsFromBitmap(PixelStats result, IntPtr hdc, IntPtr bmp, int w, int h, bool ok, int err, string tag) {
            result.Width = w;
            result.Height = h;
            result.ApiOk = ok;
            result.LastError = err;
            result.TotalPixels = (long)w * h;
            var bi = new BITMAPINFO();
            bi.bmiHeader.biSize = (uint)Marshal.SizeOf(typeof(BITMAPINFOHEADER));
            bi.bmiHeader.biWidth = w;
            bi.bmiHeader.biHeight = -h;
            bi.bmiHeader.biPlanes = 1;
            bi.bmiHeader.biBitCount = 32;
            bi.bmiHeader.biCompression = BI_RGB;
            byte[] bits = new byte[w * h * 4];
            int got = GetDIBits(hdc, bmp, 0, (uint)h, bits, ref bi, 0);
            if (got <= 0) {
                result.Ok = false;
                result.Outcome = tag + "_getdibits_failed";
                return;
            }
            long nz = 0;
            for (int i = 0; i < bits.Length; i += 4) {
                if (bits[i] != 0 || bits[i + 1] != 0 || bits[i + 2] != 0) nz++;
            }
            result.NonZeroPixels = nz;
            result.AppearsBlack = nz == 0;
            result.Ok = ok && !result.AppearsBlack;
            result.Outcome = result.AppearsBlack ? (tag + "_black") : (tag + "_ok");
        }

        public static BlockingMeas MeasurePrintWindowBlocking(IntPtr hwnd, int boundMs) {
            var meas = new BlockingMeas();
            meas.BoundMs = boundMs;
            var sw = Stopwatch.StartNew();
            bool finished = false;
            bool apiOk = false;
            int lastErr = 0;
            var t = new Thread(() => {
                RECT rc;
                GetWindowRect(hwnd, out rc);
                int w = Math.Max(1, rc.Right - rc.Left);
                int h = Math.Max(1, rc.Bottom - rc.Top);
                IntPtr screen = GetDC(IntPtr.Zero);
                IntPtr mem = CreateCompatibleDC(screen);
                IntPtr bmp = CreateCompatibleBitmap(screen, w, h);
                IntPtr old = SelectObject(mem, bmp);
                SetLastError(0);
                apiOk = PrintWindow(hwnd, mem, PW_RENDERFULLCONTENT);
                lastErr = (int)GetLastError();
                SelectObject(mem, old);
                DeleteObject(bmp);
                DeleteDC(mem);
                ReleaseDC(IntPtr.Zero, screen);
                finished = true;
            });
            t.IsBackground = true;
            t.Start();
            bool joined = t.Join(boundMs);
            sw.Stop();
            meas.ElapsedMs = sw.ElapsedMilliseconds;
            meas.ReturnedWithinBound = joined && finished;
            meas.ApiOk = apiOk;
            meas.LastError = lastErr;
            meas.Classification = meas.ReturnedWithinBound ? "bounded" : "unbounded";
            return meas;
        }

        public static BlockingMeas MeasureWindowIsResponsive(IntPtr hwnd, uint timeoutMs) {
            var meas = new BlockingMeas();
            meas.BoundMs = (int)timeoutMs;
            var sw = Stopwatch.StartNew();
            IntPtr result;
            SetLastError(0);
            IntPtr ret = SendMessageTimeoutW(hwnd, 0, IntPtr.Zero, IntPtr.Zero, SMTO_ABORTIFHUNG, timeoutMs, out result);
            sw.Stop();
            meas.ElapsedMs = sw.ElapsedMilliseconds;
            meas.ApiOk = ret != IntPtr.Zero;
            meas.LastError = (int)GetLastError();
            meas.ReturnedWithinBound = true;
            meas.Classification = "bounded";
            return meas;
        }

        public static IntPtr CreateNonPumpingWindow() {
            var ready = new ManualResetEvent(false);
            IntPtr hwnd = IntPtr.Zero;
            var t = new Thread(() => {
                if (!_classReady) {
                    _procKeepAlive = ScratchProc;
                    var wc = new WNDCLASS();
                    wc.lpfnWndProc = _procKeepAlive;
                    wc.hInstance = Marshal.GetHINSTANCE(typeof(Capture22).Module);
                    wc.lpszClassName = "AgentDesktopA22Paint";
                    RegisterClassW(ref wc);
                    _classReady = true;
                }
                hwnd = CreateWindowExW(0, "AgentDesktopA22Paint", "", 0x00CF0000, 40, 40, 180, 100, IntPtr.Zero, IntPtr.Zero, Marshal.GetHINSTANCE(typeof(Capture22).Module), IntPtr.Zero);
                ShowWindow(hwnd, 5);
                ready.Set();
                Thread.Sleep(Timeout.Infinite);
            });
            t.IsBackground = true;
            t.SetApartmentState(ApartmentState.STA);
            t.Start();
            ready.WaitOne(5000);
            return hwnd;
        }

        public static ClipboardContention MeasureClipboardContention(IntPtr holderHwnd) {
            var c = new ClipboardContention();
            c.SeqBefore = GetClipboardSequenceNumber();
            // Holder already has clipboard open in another process; try open here.
            SetLastError(0);
            c.OpenClipboardReturned = OpenClipboard(IntPtr.Zero);
            c.OpenClipboardLastError = (int)GetLastError();
            c.HolderWindowNonZero = GetOpenClipboardWindow() != IntPtr.Zero;
            if (c.OpenClipboardReturned) CloseClipboard();

            var elapsed = new List<int>();
            int successes = 0;
            for (int i = 0; i < 5; i++) {
                var sw = Stopwatch.StartNew();
                bool ok = OpenClipboard(IntPtr.Zero);
                sw.Stop();
                elapsed.Add((int)sw.ElapsedMilliseconds);
                if (ok) {
                    successes++;
                    CloseClipboard();
                    break;
                }
                Thread.Sleep(5);
            }
            c.RetryAttempts = elapsed.Count;
            c.RetrySuccesses = successes;
            c.RetryElapsedMs = elapsed.ToArray();
            return c;
        }

        public static ClipboardContention MeasureClipboardSequence() {
            var c = new ClipboardContention();
            c.SeqBefore = GetClipboardSequenceNumber();
            if (!OpenClipboard(IntPtr.Zero)) {
                c.OpenClipboardReturned = false;
                c.OpenClipboardLastError = (int)GetLastError();
                return c;
            }
            c.OpenClipboardReturned = true;
            try {
                EmptyClipboard();
                c.SeqAfterEmpty = GetClipboardSequenceNumber();
                IntPtr h = GlobalAlloc(0x0002, (UIntPtr)4);
                IntPtr p = GlobalLock(h);
                Marshal.WriteInt16(p, 0);
                GlobalUnlock(h);
                SetClipboardData(CF_UNICODETEXT, h);
                c.SeqAfterSet = GetClipboardSequenceNumber();
                c.SeqAfterNoop = GetClipboardSequenceNumber();
            } finally {
                CloseClipboard();
            }
            c.SeqAdvancedOnEmpty = c.SeqAfterEmpty != c.SeqBefore;
            c.SeqAdvancedOnSet = c.SeqAfterSet != c.SeqAfterEmpty;
            c.SeqAdvancedOnNoop = c.SeqAfterNoop != c.SeqAfterSet;
            return c;
        }

        public static DibShape[] MeasureDibShapes() {
            var list = new List<DibShape>();
            using (var bmp = new Bitmap(8, 6, PixelFormat.Format32bppArgb))
            using (var g = Graphics.FromImage(bmp)) {
                g.Clear(Color.FromArgb(255, 10, 20, 30));
                g.FillRectangle(Brushes.Red, 0, 0, 4, 3);
                if (OpenClipboard(IntPtr.Zero)) {
                    EmptyClipboard();
                    IntPtr hBmp = bmp.GetHbitmap();
                    SetClipboardData(CF_BITMAP, hBmp);
                    CloseClipboard();
                }
            }
            Thread.Sleep(50);
            if (!OpenClipboard(IntPtr.Zero)) return list.ToArray();
            try {
                list.Add(ReadDib(CF_DIB, "CF_DIB"));
                list.Add(ReadDib(CF_DIBV5, "CF_DIBV5"));
                uint pngId = RegisterClipboardFormatW("PNG");
                var png = new DibShape();
                png.FormatId = (int)pngId;
                png.FormatName = "PNG";
                png.Present = IsClipboardFormatAvailable(pngId);
                list.Add(png);
            } finally {
                CloseClipboard();
            }
            return list.ToArray();
        }

        static DibShape ReadDib(uint format, string name) {
            var s = new DibShape();
            s.FormatId = (int)format;
            s.FormatName = name;
            s.Present = IsClipboardFormatAvailable(format);
            if (!s.Present) return s;
            IntPtr h = GetClipboardData(format);
            if (h == IntPtr.Zero) return s;
            IntPtr p = GlobalLock(h);
            if (p == IntPtr.Zero) return s;
            try {
                var hdr = (BITMAPINFOHEADER)Marshal.PtrToStructure(p, typeof(BITMAPINFOHEADER));
                s.HeaderBytes = (int)hdr.biSize;
                s.BiWidth = hdr.biWidth;
                s.BiHeight = hdr.biHeight;
                s.BiBitCount = hdr.biBitCount;
                s.RowOrder = hdr.biHeight >= 0 ? "bottom_up" : "top_down";
                s.PayloadBytes = Math.Abs(hdr.biHeight) * ((((hdr.biWidth * hdr.biBitCount) + 31) / 32) * 4);
            } finally {
                GlobalUnlock(h);
            }
            return s;
        }

        public static object MeasureWicRoundTrip() {
            using (var bmp = new Bitmap(4, 3, PixelFormat.Format32bppArgb)) {
                for (int y = 0; y < 3; y++)
                    for (int x = 0; x < 4; x++)
                        bmp.SetPixel(x, y, Color.FromArgb(255, x * 40, y * 50, 90));
                byte[] png;
                using (var ms = new MemoryStream()) {
                    bmp.Save(ms, ImageFormat.Png);
                    png = ms.ToArray();
                }
                using (var ms2 = new MemoryStream(png))
                using (var round = new Bitmap(ms2)) {
                    bool match = true;
                    for (int y = 0; y < 3 && match; y++)
                        for (int x = 0; x < 4 && match; x++)
                            if (round.GetPixel(x, y).ToArgb() != bmp.GetPixel(x, y).ToArgb()) match = false;
                    return new Dictionary<string, object> {
                        { "ok", match },
                        { "png_bytes", png.Length },
                        { "width", 4 },
                        { "height", 3 },
                        { "codec", "gdiplus_png_proxy_for_wic_shape" },
                        { "note", "rust_scratch_owns_true_wic_leg" }
                    };
                }
            }
        }

        public static IsolationResult MeasureStationIsolation() {
            var r = new IsolationResult();
            IntPtr original = GetProcessWindowStation();
            SetLastError(0);
            IntPtr station = CreateWindowStationW("AgentDesktopA22Sta", 0, 0x01FF0000u /* WINSTA_ALL_ACCESS */, IntPtr.Zero);
            if (station == IntPtr.Zero) {
                r.Measurable = false;
                r.Branch = "create_window_station_failed";
                r.ErrorKind = "CreateWindowStationW_" + GetLastError();
                return r;
            }
            try {
                r.ParentSeqBefore = GetClipboardSequenceNumber();
                if (!SetProcessWindowStation(station)) {
                    r.Measurable = false;
                    r.Branch = "set_process_window_station_failed";
                    r.ErrorKind = "SetProcessWindowStation_" + GetLastError();
                    return r;
                }
                bool setOk = false;
                bool getOk = false;
                if (OpenClipboard(IntPtr.Zero)) {
                    EmptyClipboard();
                    IntPtr h = GlobalAlloc(0x0002, (UIntPtr)8);
                    IntPtr p = GlobalLock(h);
                    Marshal.WriteInt16(p, (short)'x');
                    Marshal.WriteInt16(p, 2, 0);
                    GlobalUnlock(h);
                    setOk = SetClipboardData(CF_UNICODETEXT, h) != IntPtr.Zero;
                    CloseClipboard();
                }
                if (OpenClipboard(IntPtr.Zero)) {
                    IntPtr data = GetClipboardData(CF_UNICODETEXT);
                    getOk = data != IntPtr.Zero;
                    CloseClipboard();
                }
                SetProcessWindowStation(original);
                r.ParentSeqAfter = GetClipboardSequenceNumber();
                r.ChildSetOk = setOk;
                r.ChildGetOk = getOk;
                r.ParentSeqUnchanged = r.ParentSeqBefore == r.ParentSeqAfter;
                r.Measurable = setOk && getOk;
                r.Branch = r.Measurable
                    ? (r.ParentSeqUnchanged ? "station_isolation_works" : "station_isolation_parent_touched")
                    : "station_clipboard_unusable";
                return r;
            } finally {
                try { SetProcessWindowStation(original); } catch { }
                CloseWindowStation(station);
            }
        }

        public static object EnumerateDisplays() {
            var screens = System.Windows.Forms.Screen.AllScreens;
            return new Dictionary<string, object> {
                { "count", screens.Length },
                { "primary_bounds_w", screens[0].Bounds.Width },
                { "primary_bounds_h", screens[0].Bounds.Height }
            };
        }

        /// <summary>Entry for a child process: hold OpenClipboard until killed.</summary>
        public static int RunClipboardHolder() {
            IntPtr hwnd = CreateWindowExW(0, "STATIC", "", 0, 0, 0, 0, 0, new IntPtr(-3), IntPtr.Zero, IntPtr.Zero, IntPtr.Zero);
            if (!OpenClipboard(hwnd)) return 2;
            Console.WriteLine("ready");
            Console.Out.Flush();
            Thread.Sleep(Timeout.Infinite);
            return 0;
        }

        /// <summary>Entry for a child process: advertise delay-rendered CF_UNICODETEXT then stop pumping.</summary>
        public static int RunDelayedClipboardOwner() {
            // Real top-level window on this thread; we never pump after advertising.
            if (!_classReady) {
                _procKeepAlive = ScratchProc;
                var wc = new WNDCLASS();
                wc.lpfnWndProc = _procKeepAlive;
                wc.hInstance = Marshal.GetHINSTANCE(typeof(Capture22).Module);
                wc.lpszClassName = "AgentDesktopA22Paint";
                RegisterClassW(ref wc);
                _classReady = true;
            }
            IntPtr hwnd = CreateWindowExW(0, "AgentDesktopA22Paint", "", 0x00CF0000, 10, 10, 120, 80, IntPtr.Zero, IntPtr.Zero, Marshal.GetHINSTANCE(typeof(Capture22).Module), IntPtr.Zero);
            ShowWindow(hwnd, 5);
            if (!OpenClipboard(hwnd)) return 2;
            EmptyClipboard();
            if (SetClipboardData(CF_UNICODETEXT, IntPtr.Zero) == IntPtr.Zero && GetLastError() != 0) {
                // SetClipboardData(NULL) succeeds for delayed render; treat nonzero only as hard failure when unexpected.
            }
            CloseClipboard();
            bool available = IsClipboardFormatAvailable(CF_UNICODETEXT);
            IntPtr owner = GetClipboardOwner();
            Console.WriteLine("ready format_available=" + (available ? "1" : "0") + " owner_nonzero=" + (owner != IntPtr.Zero ? "1" : "0"));
            Console.Out.Flush();
            Thread.Sleep(Timeout.Infinite);
            return 0;
        }

        public static PixelStats CapturePrintWindowByHandleValue(long hwndValue, bool renderFull) {
            return CapturePrintWindow(new IntPtr(hwndValue), renderFull);
        }

        public static BlockingMeas MeasureGetClipboardDataBlocking(int boundMs) {
            var meas = new BlockingMeas();
            meas.BoundMs = boundMs;
            var sw = Stopwatch.StartNew();
            bool finished = false;
            bool apiOk = false;
            int lastErr = 0;
            var t = new Thread(() => {
                SetLastError(0);
                if (!OpenClipboard(IntPtr.Zero)) {
                    lastErr = (int)GetLastError();
                    finished = true;
                    return;
                }
                try {
                    SetLastError(0);
                    IntPtr data = GetClipboardData(CF_UNICODETEXT);
                    apiOk = data != IntPtr.Zero;
                    lastErr = (int)GetLastError();
                } finally {
                    CloseClipboard();
                }
                finished = true;
            });
            t.IsBackground = true;
            t.SetApartmentState(ApartmentState.STA);
            t.Start();
            bool joined = t.Join(boundMs);
            sw.Stop();
            meas.ElapsedMs = sw.ElapsedMilliseconds;
            meas.ReturnedWithinBound = joined && finished;
            meas.ApiOk = apiOk;
            meas.LastError = lastErr;
            meas.Classification = meas.ReturnedWithinBound ? "bounded" : "unbounded";
            return meas;
        }
    }
}