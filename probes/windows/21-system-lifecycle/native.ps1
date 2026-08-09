#Requires -Version 5.1
# Lifecycle21 Win32 bindings for Area 21 (C# 5). Dot-sourced by probe.ps1.

function Initialize-LifecycleNative {
    if ('AgentDesktopProbe.A21.Lifecycle21' -as [type]) { return }
    $src = @'
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;

namespace AgentDesktopProbe.A21 {
    [StructLayout(LayoutKind.Sequential)]
    public struct Rect { public int Left; public int Top; public int Right; public int Bottom; }

    [StructLayout(LayoutKind.Sequential)]
    public struct Point { public int X; public int Y; }

    [StructLayout(LayoutKind.Sequential)]
    public struct ProcessInformation {
        public IntPtr hProcess;
        public IntPtr hThread;
        public int dwProcessId;
        public int dwThreadId;
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct StartupInfo {
        public int cb;
        public string lpReserved;
        public string lpDesktop;
        public string lpTitle;
        public int dwX;
        public int dwY;
        public int dwXSize;
        public int dwYSize;
        public int dwXCountChars;
        public int dwYCountChars;
        public int dwFillAttribute;
        public int dwFlags;
        public short wShowWindow;
        public short cbReserved2;
        public IntPtr lpReserved2;
        public IntPtr hStdInput;
        public IntPtr hStdOutput;
        public IntPtr hStdError;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct WindowPlacement {
        public int length;
        public int flags;
        public int showCmd;
        public Point ptMinPosition;
        public Point ptMaxPosition;
        public Rect rcNormalPosition;
    }

    public sealed class LaunchResult {
        public bool Ok;
        public int ProcessId;
        public bool ProcessHandleNonZero;
        public bool ThreadHandleNonZero;
        public int LastError;
        public IntPtr ProcessHandle;
        public IntPtr ThreadHandle;
    }

    public sealed class ExitRead {
        public bool WaitSignaled;
        public uint ExitCode;
        public bool HighNibbleIsC;
        public string Classification;
    }

    public sealed class HangProbe {
        public bool IsHung;
        public long IsHungElapsedMs;
        public bool TimeoutTimedOut;
        public long TimeoutElapsedMs;
        public bool AgreeHung;
    }

    public sealed class PlacementSnap {
        public int ShowCmd;
        public int Left;
        public int Top;
        public int Width;
        public int Height;
        public bool MinimizedSentinel;
    }

    public static class Lifecycle21 {
        public const uint CREATE_NEW_CONSOLE = 0x00000010;
        public const uint CREATE_NO_WINDOW = 0x08000000;
        public const uint WAIT_OBJECT_0 = 0;
        public const uint WAIT_TIMEOUT = 0x00000102;
        public const uint STILL_ACTIVE = 259;
        public const uint WM_CLOSE = 0x0010;
        public const uint WM_NULL = 0x0000;
        public const uint SMTO_ABORTIFHUNG = 0x0002;
        public const int SW_HIDE = 0;
        public const int SW_SHOWNORMAL = 1;
        public const int SW_SHOWMINIMIZED = 2;
        public const int SW_SHOWMAXIMIZED = 3;
        public const int SW_SHOWNOACTIVATE = 4;
        public const int SW_RESTORE = 9;
        public const int SW_MINIMIZE = 6;
        public const int SW_MAXIMIZE = 3;
        public const uint SWP_NOZORDER = 0x0004;
        public const uint SWP_NOACTIVATE = 0x0010;
        public const uint SWP_SHOWWINDOW = 0x0040;
        public const uint PROCESS_QUERY_INFORMATION = 0x0400;
        public const uint PROCESS_QUERY_LIMITED_INFORMATION = 0x1000;
        public const uint PROCESS_TERMINATE = 0x0001;
        public const uint SYNCHRONIZE = 0x00100000;
        public const int ERROR_ELEVATION_REQUIRED = 740;
        public const uint TH32CS_SNAPPROCESS = 0x00000002;

        [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        public static extern bool CreateProcessW(
            string lpApplicationName,
            StringBuilder lpCommandLine,
            IntPtr lpProcessAttributes,
            IntPtr lpThreadAttributes,
            bool bInheritHandles,
            uint dwCreationFlags,
            IntPtr lpEnvironment,
            string lpCurrentDirectory,
            ref StartupInfo lpStartupInfo,
            out ProcessInformation lpProcessInformation);

        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern bool TerminateProcess(IntPtr hProcess, uint uExitCode);

        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern bool GetExitCodeProcess(IntPtr hProcess, out uint lpExitCode);

        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern uint WaitForSingleObject(IntPtr hHandle, uint dwMilliseconds);

        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern bool CloseHandle(IntPtr hObject);

        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern IntPtr OpenProcess(uint dwDesiredAccess, bool bInheritHandle, int dwProcessId);

        [DllImport("user32.dll", SetLastError = true)]
        public static extern bool PostMessageW(IntPtr hWnd, uint Msg, IntPtr wParam, IntPtr lParam);

        [DllImport("user32.dll")]
        public static extern bool IsHungAppWindow(IntPtr hwnd);

        [DllImport("user32.dll", SetLastError = true)]
        public static extern IntPtr SendMessageTimeoutW(
            IntPtr hWnd, uint Msg, IntPtr wParam, IntPtr lParam,
            uint fuFlags, uint uTimeout, out IntPtr lpdwResult);

        [DllImport("user32.dll", SetLastError = true)]
        public static extern bool SetWindowPos(IntPtr hWnd, IntPtr hWndInsertAfter, int X, int Y, int cx, int cy, uint uFlags);

        [DllImport("user32.dll")]
        public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);

        [DllImport("user32.dll", SetLastError = true)]
        public static extern bool GetWindowRect(IntPtr hWnd, out Rect lpRect);

        [DllImport("user32.dll", SetLastError = true)]
        public static extern bool GetWindowPlacement(IntPtr hWnd, ref WindowPlacement lpwndpl);

        [DllImport("user32.dll")]
        public static extern bool SetForegroundWindow(IntPtr hWnd);

        [DllImport("user32.dll")]
        public static extern IntPtr GetForegroundWindow();

        [DllImport("user32.dll")]
        public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint lpdwProcessId);

        [DllImport("user32.dll")]
        public static extern bool AttachThreadInput(uint idAttach, uint idAttachTo, bool fAttach);

        [DllImport("kernel32.dll")]
        public static extern uint GetCurrentThreadId();

        [DllImport("user32.dll")]
        public static extern bool EnumWindows(EnumWindowsProc lpEnumFunc, IntPtr lParam);

        public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

        [DllImport("user32.dll")]
        public static extern bool IsWindowVisible(IntPtr hWnd);

        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern IntPtr CreateToolhelp32Snapshot(uint dwFlags, uint th32ProcessID);

        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
        public struct ProcessEntry32 {
            public uint dwSize;
            public uint cntUsage;
            public uint th32ProcessID;
            public IntPtr th32DefaultHeapID;
            public uint th32ModuleID;
            public uint cntThreads;
            public uint th32ParentProcessID;
            public int pcPriClassBase;
            public uint dwFlags;
            [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 260)]
            public string szExeFile;
        }

        [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        public static extern bool Process32FirstW(IntPtr hSnapshot, ref ProcessEntry32 lppe);

        [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        public static extern bool Process32NextW(IntPtr hSnapshot, ref ProcessEntry32 lppe);

        [DllImport("kernel32.dll")]
        public static extern uint GetLastError();

        public static LaunchResult Launch(string applicationName, string commandLine, string cwd, uint flags) {
            LaunchResult result = new LaunchResult();
            StartupInfo si = new StartupInfo();
            si.cb = Marshal.SizeOf(typeof(StartupInfo));
            ProcessInformation pi;
            StringBuilder cmd = null;
            if (!string.IsNullOrEmpty(commandLine)) {
                cmd = new StringBuilder(commandLine);
            }
            bool ok = CreateProcessW(
                applicationName,
                cmd,
                IntPtr.Zero,
                IntPtr.Zero,
                false,
                flags,
                IntPtr.Zero,
                cwd,
                ref si,
                out pi);
            result.Ok = ok;
            result.LastError = ok ? 0 : Marshal.GetLastWin32Error();
            if (ok) {
                result.ProcessId = pi.dwProcessId;
                result.ProcessHandle = pi.hProcess;
                result.ThreadHandle = pi.hThread;
                result.ProcessHandleNonZero = pi.hProcess != IntPtr.Zero;
                result.ThreadHandleNonZero = pi.hThread != IntPtr.Zero;
            }
            return result;
        }

        public static void CloseLaunchHandles(LaunchResult launch) {
            if (launch == null) { return; }
            if (launch.ThreadHandle != IntPtr.Zero) { CloseHandle(launch.ThreadHandle); launch.ThreadHandle = IntPtr.Zero; }
            if (launch.ProcessHandle != IntPtr.Zero) { CloseHandle(launch.ProcessHandle); launch.ProcessHandle = IntPtr.Zero; }
        }

        public static int CountRunningImage(string imageLeaf) {
            int count = 0;
            IntPtr snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if (snap == IntPtr.Zero || snap == new IntPtr(-1)) { return -1; }
            try {
                ProcessEntry32 pe = new ProcessEntry32();
                pe.dwSize = (uint)Marshal.SizeOf(typeof(ProcessEntry32));
                if (!Process32FirstW(snap, ref pe)) { return 0; }
                do {
                    if (!string.IsNullOrEmpty(pe.szExeFile) &&
                        string.Equals(pe.szExeFile, imageLeaf, StringComparison.OrdinalIgnoreCase)) {
                        count++;
                    }
                } while (Process32NextW(snap, ref pe));
            } finally {
                CloseHandle(snap);
            }
            return count;
        }

        public static IntPtr FindVisibleWindowForPid(int pid) {
            return FindWindowForPid(pid, true);
        }

        public static IntPtr FindAnyWindowForPid(int pid) {
            return FindWindowForPid(pid, false);
        }

        private static IntPtr FindWindowForPid(int pid, bool visibleOnly) {
            IntPtr found = IntPtr.Zero;
            EnumWindows(delegate(IntPtr hWnd, IntPtr lParam) {
                if (visibleOnly && !IsWindowVisible(hWnd)) { return true; }
                uint owner = 0;
                GetWindowThreadProcessId(hWnd, out owner);
                if ((int)owner == pid) {
                    found = hWnd;
                    return false;
                }
                return true;
            }, IntPtr.Zero);
            return found;
        }

        public static int FindChildProcessId(int parentPid) {
            int child = 0;
            IntPtr snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if (snap == IntPtr.Zero || snap == new IntPtr(-1)) { return 0; }
            try {
                ProcessEntry32 pe = new ProcessEntry32();
                pe.dwSize = (uint)Marshal.SizeOf(typeof(ProcessEntry32));
                if (!Process32FirstW(snap, ref pe)) { return 0; }
                do {
                    if ((int)pe.th32ParentProcessID == parentPid) {
                        child = (int)pe.th32ProcessID;
                        break;
                    }
                } while (Process32NextW(snap, ref pe));
            } finally {
                CloseHandle(snap);
            }
            return child;
        }

        public static bool PostClose(IntPtr hwnd) {
            return PostMessageW(hwnd, WM_CLOSE, IntPtr.Zero, IntPtr.Zero);
        }

        public static ExitRead ReadExit(IntPtr processHandle, uint waitMs) {
            ExitRead read = new ExitRead();
            uint wait = WaitForSingleObject(processHandle, waitMs);
            read.WaitSignaled = (wait == WAIT_OBJECT_0);
            uint code = 0;
            GetExitCodeProcess(processHandle, out code);
            read.ExitCode = code;
            read.HighNibbleIsC = ((code & 0xF0000000u) == 0xC0000000u);
            if (!read.WaitSignaled) {
                read.Classification = "running_or_wait_timeout";
            } else if (read.HighNibbleIsC) {
                read.Classification = "crashed";
            } else {
                read.Classification = "exited";
            }
            return read;
        }

        public static HangProbe ProbeHang(IntPtr hwnd, uint timeoutMs) {
            HangProbe probe = new HangProbe();
            Stopwatch sw = Stopwatch.StartNew();
            probe.IsHung = IsHungAppWindow(hwnd);
            sw.Stop();
            probe.IsHungElapsedMs = sw.ElapsedMilliseconds;

            IntPtr result;
            sw.Reset();
            sw.Start();
            IntPtr sent = SendMessageTimeoutW(hwnd, WM_NULL, IntPtr.Zero, IntPtr.Zero, SMTO_ABORTIFHUNG, timeoutMs, out result);
            sw.Stop();
            probe.TimeoutElapsedMs = sw.ElapsedMilliseconds;
            probe.TimeoutTimedOut = (sent == IntPtr.Zero);
            probe.AgreeHung = (probe.IsHung == probe.TimeoutTimedOut);
            return probe;
        }

        public static PlacementSnap SnapPlacement(IntPtr hwnd) {
            PlacementSnap snap = new PlacementSnap();
            WindowPlacement wp = new WindowPlacement();
            wp.length = Marshal.SizeOf(typeof(WindowPlacement));
            GetWindowPlacement(hwnd, ref wp);
            snap.ShowCmd = wp.showCmd;
            Rect r;
            GetWindowRect(hwnd, out r);
            snap.Left = r.Left;
            snap.Top = r.Top;
            snap.Width = r.Right - r.Left;
            snap.Height = r.Bottom - r.Top;
            snap.MinimizedSentinel = (r.Left <= -32000) || (r.Top <= -32000);
            return snap;
        }

        public static bool MoveResize(IntPtr hwnd, int x, int y, int w, int h) {
            return SetWindowPos(hwnd, IntPtr.Zero, x, y, w, h, SWP_NOZORDER | SWP_NOACTIVATE | SWP_SHOWWINDOW);
        }

        public static bool ForegroundOwned(int expectedPid) {
            IntPtr fg = GetForegroundWindow();
            if (fg == IntPtr.Zero) { return false; }
            uint pid = 0;
            GetWindowThreadProcessId(fg, out pid);
            return (int)pid == expectedPid;
        }

        public static bool ActivateOnce(IntPtr hwnd) {
            uint unusedPid = 0;
            uint targetThread = GetWindowThreadProcessId(hwnd, out unusedPid);
            uint current = GetCurrentThreadId();
            bool attached = false;
            if (targetThread != 0 && targetThread != current) {
                attached = AttachThreadInput(current, targetThread, true);
            }
            bool ok = SetForegroundWindow(hwnd);
            if (attached) {
                AttachThreadInput(current, targetThread, false);
            }
            return ok;
        }

        public static uint HresultFromWin32(uint win32) {
            if (win32 <= 0) { return 0; }
            return 0x80070000u | (win32 & 0xFFFFu);
        }
    }
}
'@
    Add-ProbeInlineCSharp -Source $src -AssemblyLeaf 'AgentDesktopProbeA21Lifecycle21'
}
