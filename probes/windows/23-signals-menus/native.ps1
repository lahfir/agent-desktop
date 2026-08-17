#Requires -Version 5.1
# Signals23 Win32 bindings for Area 23 (C# 5). Dot-sourced by probe.ps1 / measure-cost.ps1.
#
# Every fact this module reads was smoke-tested interactively against real
# Notepad, WPF and Obsidian processes on this host before being folded in here:
# GUITHREADINFO.flags needs a per-target-thread call (idThread=0 means the
# *foreground* thread, not the target), GetWindowThreadProcessId's return value
# is the thread id (not a bool), and a #32768 popup exposes itself to UIA as
# ControlType.Pane, never ControlType.Menu - so the UIA source has to search by
# ControlType, not trust the class-name source to agree with it.

function Initialize-Signals23Native {
    if ('AgentDesktopProbe.A23.Signals23' -as [type]) { return }
    $src = @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

namespace AgentDesktopProbe.A23 {
    [StructLayout(LayoutKind.Sequential)]
    public struct Rect { public int Left; public int Top; public int Right; public int Bottom; }

    [StructLayout(LayoutKind.Sequential)]
    public struct GuiThreadInfo {
        public uint cbSize;
        public uint flags;
        public IntPtr hwndActive;
        public IntPtr hwndFocus;
        public IntPtr hwndCapture;
        public IntPtr hwndMenuOwner;
        public IntPtr hwndMoveSize;
        public IntPtr hwndCaret;
        public Rect rcCaret;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct ThreadEntry32 {
        public uint dwSize;
        public uint cntUsage;
        public uint th32ThreadID;
        public uint th32OwnerProcessID;
        public int tpBasePri;
        public int tpDeltaPri;
        public uint dwFlags;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct Filetime { public uint dwLowDateTime; public uint dwHighDateTime; }

    [StructLayout(LayoutKind.Sequential)]
    public struct Input { public int type; public InputUnion U; }

    [StructLayout(LayoutKind.Explicit)]
    public struct InputUnion {
        [FieldOffset(0)] public KeybdInput ki;
        [FieldOffset(0)] public MouseInput mi;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct KeybdInput { public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }

    [StructLayout(LayoutKind.Sequential)]
    public struct MouseInput { public int dx; public int dy; public uint mouseData; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }

    public sealed class ThreadFlagsResult {
        public bool Ok;
        public uint Flags;
        public bool InMenuMode;
        public bool PopupMenuMode;
        public bool SystemMenuMode;
        public bool MenuOwnerNonZero;
    }

    public sealed class WindowFilterResult {
        public bool Visible;
        public bool NonZeroRect;
        public bool Cloaked;
        public bool ToolWindow;
        public bool PassesFilter;
        public bool PidNonZero;
        public bool TokenReadOk;
        public IntPtr Handle;
    }

    public sealed class PopupRecord {
        public bool Visible;
        public bool OwnedByTargetPid;
        public bool PassesFilter;
        public bool NonZeroRect;
        public bool Cloaked;
        public bool ToolWindow;
    }

    public static class Signals23 {
        public const uint TH32CS_SNAPTHREAD = 0x00000004;
        public const uint WM_SYSCOMMAND = 0x0112;
        public const uint WM_KEYDOWN = 0x0100;
        public const uint WM_KEYUP = 0x0101;
        public const uint WM_CONTEXTMENU = 0x007B;
        public const uint WM_CLOSE = 0x0010;
        public const uint SC_KEYMENU = 0xF100;
        public const int VK_DOWN = 0x28;
        public const int VK_ESCAPE = 0x1B;
        public const int VK_MENU = 0x12;
        public const int VK_SPACE = 0x20;
        public const uint KEYEVENTF_KEYUP = 0x0002;
        public const uint MOUSEEVENTF_RIGHTDOWN = 0x0008;
        public const uint MOUSEEVENTF_RIGHTUP = 0x0010;
        public const int GWL_EXSTYLE = -20;
        public const int WS_EX_TOOLWINDOW = 0x00000080;
        public const int WS_EX_DLGMODALFRAME = 0x00000001;
        public const uint GW_OWNER = 4;
        public const uint DWMWA_CLOAKED = 14;
        public const uint PROCESS_QUERY_LIMITED_INFORMATION = 0x1000;
        public const uint GUI_INMENUMODE = 0x0004;
        public const uint GUI_POPUPMENUMODE = 0x0010;
        public const uint GUI_SYSTEMMENUMODE = 0x0008;

        [DllImport("user32.dll")]
        public static extern bool GetGUIThreadInfo(uint idThread, ref GuiThreadInfo lpgui);

        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern IntPtr CreateToolhelp32Snapshot(uint dwFlags, uint th32ProcessID);

        [DllImport("kernel32.dll")]
        public static extern bool Thread32First(IntPtr hSnapshot, ref ThreadEntry32 lpte);

        [DllImport("kernel32.dll")]
        public static extern bool Thread32Next(IntPtr hSnapshot, ref ThreadEntry32 lpte);

        [DllImport("kernel32.dll")]
        public static extern bool CloseHandle(IntPtr h);

        [DllImport("user32.dll")]
        public static extern bool EnumWindows(EnumWindowsProc lpEnumFunc, IntPtr lParam);

        public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        public static extern int GetClassNameW(IntPtr hWnd, StringBuilder lpClassName, int nMaxCount);

        [DllImport("user32.dll")]
        public static extern bool IsWindowVisible(IntPtr hWnd);

        [DllImport("user32.dll")]
        public static extern bool GetWindowRect(IntPtr hWnd, out Rect lpRect);

        [DllImport("user32.dll")]
        public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint lpdwProcessId);

        [DllImport("user32.dll")]
        public static extern int GetWindowLongW(IntPtr hWnd, int nIndex);

        [DllImport("user32.dll")]
        public static extern IntPtr GetWindow(IntPtr hWnd, uint uCmd);

        [DllImport("dwmapi.dll")]
        public static extern int DwmGetWindowAttribute(IntPtr hwnd, uint dwAttribute, out uint pvAttribute, int cbAttribute);

        [DllImport("user32.dll", SetLastError = true)]
        public static extern bool PostMessageW(IntPtr hWnd, uint Msg, IntPtr wParam, IntPtr lParam);

        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        public static extern IntPtr FindWindowExW(IntPtr hwndParent, IntPtr hwndChildAfter, string lpszClass, string lpszWindow);

        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        public static extern int GetWindowTextLengthW(IntPtr hWnd);

        [DllImport("user32.dll")]
        public static extern bool SetForegroundWindow(IntPtr hWnd);

        [DllImport("user32.dll")]
        public static extern IntPtr GetForegroundWindow();

        [DllImport("user32.dll")]
        public static extern bool AttachThreadInput(uint idAttach, uint idAttachTo, bool fAttach);

        [DllImport("kernel32.dll")]
        public static extern uint GetCurrentThreadId();

        [DllImport("user32.dll")]
        public static extern uint SendInput(uint nInputs, Input[] pInputs, int cbSize);

        [DllImport("user32.dll")]
        public static extern bool SetCursorPos(int x, int y);

        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern IntPtr OpenProcess(uint dwDesiredAccess, bool bInheritHandle, int dwProcessId);

        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern bool GetProcessTimes(IntPtr hProcess, out Filetime lpCreationTime, out Filetime lpExitTime, out Filetime lpKernelTime, out Filetime lpUserTime);

        [DllImport("kernel32.dll")]
        public static extern uint GetLastError();

        public static uint[] ThreadsForPid(int pid) {
            List<uint> result = new List<uint>();
            IntPtr snap = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
            if (snap == IntPtr.Zero || snap == new IntPtr(-1)) { return result.ToArray(); }
            try {
                ThreadEntry32 te = new ThreadEntry32();
                te.dwSize = (uint)Marshal.SizeOf(typeof(ThreadEntry32));
                if (Thread32First(snap, ref te)) {
                    do {
                        if (te.th32OwnerProcessID == (uint)pid) { result.Add(te.th32ThreadID); }
                        te.dwSize = (uint)Marshal.SizeOf(typeof(ThreadEntry32));
                    } while (Thread32Next(snap, ref te));
                }
            } finally {
                CloseHandle(snap);
            }
            return result.ToArray();
        }

        public static ThreadFlagsResult ReadThreadFlags(uint tid) {
            ThreadFlagsResult result = new ThreadFlagsResult();
            GuiThreadInfo gti = new GuiThreadInfo();
            gti.cbSize = (uint)Marshal.SizeOf(typeof(GuiThreadInfo));
            result.Ok = GetGUIThreadInfo(tid, ref gti);
            if (!result.Ok) { return result; }
            result.Flags = gti.flags;
            result.InMenuMode = (gti.flags & GUI_INMENUMODE) != 0;
            result.PopupMenuMode = (gti.flags & GUI_POPUPMENUMODE) != 0;
            result.SystemMenuMode = (gti.flags & GUI_SYSTEMMENUMODE) != 0;
            result.MenuOwnerNonZero = gti.hwndMenuOwner != IntPtr.Zero;
            return result;
        }

        /// Any thread of `pid` reporting classic menu mode, aggregated so the
        /// caller never has to enumerate thread ids itself.
        public static bool AnyThreadInMenuMode(int pid, out int threadsRead, out int threadsTotal) {
            uint[] tids = ThreadsForPid(pid);
            threadsTotal = tids.Length;
            threadsRead = 0;
            bool any = false;
            foreach (uint tid in tids) {
                ThreadFlagsResult r = ReadThreadFlags(tid);
                if (r.Ok) {
                    threadsRead++;
                    if (r.InMenuMode || r.PopupMenuMode || r.SystemMenuMode) { any = true; }
                }
            }
            return any;
        }

        public static bool PassesAgentFacingFilter(IntPtr hwnd, out WindowFilterResult detail) {
            detail = new WindowFilterResult();
            detail.Visible = IsWindowVisible(hwnd);
            Rect r;
            GetWindowRect(hwnd, out r);
            detail.NonZeroRect = (r.Right - r.Left) > 0 && (r.Bottom - r.Top) > 0;
            uint cloakedVal;
            int hr = DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, out cloakedVal, sizeof(uint));
            detail.Cloaked = (hr == 0) && cloakedVal != 0;
            int exStyle = GetWindowLongW(hwnd, GWL_EXSTYLE);
            detail.ToolWindow = (exStyle & WS_EX_TOOLWINDOW) != 0;
            detail.PassesFilter = detail.Visible && detail.NonZeroRect && !detail.Cloaked && !detail.ToolWindow;
            return detail.PassesFilter;
        }

        public static List<PopupRecord> ClassPopupsForPid(int pid) {
            List<PopupRecord> found = new List<PopupRecord>();
            EnumWindowsProc cb = delegate(IntPtr hWnd, IntPtr lp) {
                StringBuilder sb = new StringBuilder(64);
                GetClassNameW(hWnd, sb, sb.Capacity);
                if (sb.ToString() == "#32768") {
                    uint owner;
                    GetWindowThreadProcessId(hWnd, out owner);
                    PopupRecord rec = new PopupRecord();
                    rec.OwnedByTargetPid = (int)owner == pid;
                    WindowFilterResult detail;
                    rec.PassesFilter = PassesAgentFacingFilter(hWnd, out detail);
                    rec.Visible = detail.Visible;
                    rec.NonZeroRect = detail.NonZeroRect;
                    rec.Cloaked = detail.Cloaked;
                    rec.ToolWindow = detail.ToolWindow;
                    found.Add(rec);
                }
                return true;
            };
            EnumWindows(cb, IntPtr.Zero);
            GC.KeepAlive(cb);
            return found;
        }

        public static IntPtr FindWindowByClassForPid(int pid, string className) {
            IntPtr result = IntPtr.Zero;
            EnumWindowsProc cb = delegate(IntPtr hWnd, IntPtr lp) {
                StringBuilder sb = new StringBuilder(64);
                GetClassNameW(hWnd, sb, sb.Capacity);
                if (sb.ToString() == className) {
                    uint owner;
                    GetWindowThreadProcessId(hWnd, out owner);
                    if ((int)owner == pid) { result = hWnd; return false; }
                }
                return true;
            };
            EnumWindows(cb, IntPtr.Zero);
            GC.KeepAlive(cb);
            return result;
        }

        public static IntPtr FindVisibleTitledTopLevelForPid(int pid) {
            IntPtr result = IntPtr.Zero;
            EnumWindowsProc cb = delegate(IntPtr hWnd, IntPtr lp) {
                if (IsWindowVisible(hWnd) && GetWindowTextLengthW(hWnd) > 0) {
                    uint owner;
                    GetWindowThreadProcessId(hWnd, out owner);
                    if ((int)owner == pid) { result = hWnd; return false; }
                }
                return true;
            };
            EnumWindows(cb, IntPtr.Zero);
            GC.KeepAlive(cb);
            return result;
        }

        /// Every top-level window the agent-facing filter would admit, read in
        /// one EnumWindows pass with the pid/token identity read folded in -
        /// the shape the mid-walk-race and durably-unidentifiable legs both
        /// replay. Handles and pids never leave this process; only the
        /// booleans below are ever serialized by the caller.
        public static List<WindowFilterResult> AgentFacingPass(out int windowCount) {
            List<WindowFilterResult> results = new List<WindowFilterResult>();
            int count = 0;
            EnumWindowsProc cb = delegate(IntPtr hWnd, IntPtr lp) {
                WindowFilterResult detail;
                bool passes = PassesAgentFacingFilter(hWnd, out detail);
                if (!passes) { return true; }
                count++;
                uint pid;
                GetWindowThreadProcessId(hWnd, out pid);
                detail.PidNonZero = pid != 0;
                detail.TokenReadOk = false;
                if (detail.PidNonZero) {
                    IntPtr proc = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, (int)pid);
                    if (proc != IntPtr.Zero) {
                        Filetime created, exit, kernel, user;
                        bool ok = GetProcessTimes(proc, out created, out exit, out kernel, out user);
                        detail.TokenReadOk = ok && (created.dwHighDateTime != 0 || created.dwLowDateTime != 0);
                        CloseHandle(proc);
                    }
                }
                detail.Handle = hWnd;
                results.Add(detail);
                return true;
            };
            EnumWindows(cb, IntPtr.Zero);
            GC.KeepAlive(cb);
            windowCount = count;
            return results;
        }

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

        public const uint TH32CS_SNAPPROCESS = 0x00000002;

        [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        public static extern bool Process32FirstW(IntPtr hSnapshot, ref ProcessEntry32 lppe);

        [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        public static extern bool Process32NextW(IntPtr hSnapshot, ref ProcessEntry32 lppe);

        public static int[] AllProcessIds() {
            List<int> ids = new List<int>();
            IntPtr snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if (snap == IntPtr.Zero || snap == new IntPtr(-1)) { return ids.ToArray(); }
            try {
                ProcessEntry32 pe = new ProcessEntry32();
                pe.dwSize = (uint)Marshal.SizeOf(typeof(ProcessEntry32));
                if (Process32FirstW(snap, ref pe)) {
                    do { ids.Add((int)pe.th32ProcessID); } while (Process32NextW(snap, ref pe));
                }
            } finally {
                CloseHandle(snap);
            }
            return ids.ToArray();
        }

        private static bool ProcessTokenReadOk(int pid) {
            IntPtr proc = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid);
            if (proc == IntPtr.Zero) { return false; }
            try {
                Filetime created, exit, kernel, user;
                bool ok = GetProcessTimes(proc, out created, out exit, out kernel, out user);
                return ok && (created.dwHighDateTime != 0 || created.dwLowDateTime != 0);
            } finally {
                CloseHandle(proc);
            }
        }

        /// R2's shipped shape, matched against `app_ops.rs::process_snapshot`
        /// and `list_apps_live` rather than assumed: `process_snapshot` reads
        /// pid+name only (one ToolHelp walk, zero `OpenProcess` calls), and a
        /// process handle is opened - via `token_for_pid` - only for a pid
        /// that actually owns an agent-facing window, once per distinct pid
        /// even when several windows share it. Pre-opening every system
        /// process (this crate's first draft of this method) does strictly
        /// more work than the shipped design and measures the wrong thing.
        public static List<WindowFilterResult> SingleWalkCachedPass(out int windowCount) {
            HashSet<int> knownPids = new HashSet<int>(AllProcessIds());
            Dictionary<int, bool> tokenCache = new Dictionary<int, bool>();
            List<WindowFilterResult> results = new List<WindowFilterResult>();
            int count = 0;
            EnumWindowsProc cb = delegate(IntPtr hWnd, IntPtr lp) {
                WindowFilterResult detail;
                bool passes = PassesAgentFacingFilter(hWnd, out detail);
                if (!passes) { return true; }
                count++;
                uint pid;
                GetWindowThreadProcessId(hWnd, out pid);
                detail.PidNonZero = pid != 0;
                detail.TokenReadOk = false;
                if (detail.PidNonZero) {
                    bool cached;
                    if (!tokenCache.TryGetValue((int)pid, out cached)) {
                        cached = knownPids.Contains((int)pid) && ProcessTokenReadOk((int)pid);
                        tokenCache[(int)pid] = cached;
                    }
                    detail.TokenReadOk = cached;
                }
                detail.Handle = hWnd;
                results.Add(detail);
                return true;
            };
            EnumWindows(cb, IntPtr.Zero);
            GC.KeepAlive(cb);
            windowCount = count;
            return results;
        }

        public static bool ActivateOnce(IntPtr hwnd) {
            uint unused;
            uint targetThread = GetWindowThreadProcessId(hwnd, out unused);
            uint current = GetCurrentThreadId();
            bool attached = false;
            if (targetThread != 0 && targetThread != current) { attached = AttachThreadInput(current, targetThread, true); }
            bool ok = SetForegroundWindow(hwnd);
            if (attached) { AttachThreadInput(current, targetThread, false); }
            return ok;
        }

        public static bool ForegroundIs(IntPtr hwnd) { return GetForegroundWindow() == hwnd; }

        public static bool PostSysKeyMenu(IntPtr hwnd, int lparamChar) {
            return PostMessageW(hwnd, WM_SYSCOMMAND, (IntPtr)SC_KEYMENU, (IntPtr)lparamChar);
        }

        public static void PostKeyTap(IntPtr hwnd, int vk) {
            PostMessageW(hwnd, WM_KEYDOWN, (IntPtr)vk, IntPtr.Zero);
            PostMessageW(hwnd, WM_KEYUP, (IntPtr)vk, IntPtr.Zero);
        }

        public static bool PostContextMenuKeyboardInvoked(IntPtr target) {
            return PostMessageW(target, WM_CONTEXTMENU, target, (IntPtr)(-1));
        }

        public static IntPtr FindEditChild(IntPtr parent) {
            return FindWindowExW(parent, IntPtr.Zero, "Edit", null);
        }

        public static void TapAlt() {
            Input[] inputs = new Input[2];
            inputs[0].type = 1;
            inputs[0].U.ki = new KeybdInput { wVk = VK_MENU, wScan = 0, dwFlags = 0, time = 0, dwExtraInfo = IntPtr.Zero };
            inputs[1].type = 1;
            inputs[1].U.ki = new KeybdInput { wVk = VK_MENU, wScan = 0, dwFlags = KEYEVENTF_KEYUP, time = 0, dwExtraInfo = IntPtr.Zero };
            SendInput(2, inputs, Marshal.SizeOf(typeof(Input)));
        }

        public static void KeyChord(ushort[] vks) {
            List<Input> seq = new List<Input>();
            foreach (ushort vk in vks) {
                Input i = new Input();
                i.type = 1;
                i.U.ki = new KeybdInput { wVk = vk, wScan = 0, dwFlags = 0, time = 0, dwExtraInfo = IntPtr.Zero };
                seq.Add(i);
            }
            for (int j = vks.Length - 1; j >= 0; j--) {
                Input i = new Input();
                i.type = 1;
                i.U.ki = new KeybdInput { wVk = vks[j], wScan = 0, dwFlags = KEYEVENTF_KEYUP, time = 0, dwExtraInfo = IntPtr.Zero };
                seq.Add(i);
            }
            Input[] arr = seq.ToArray();
            SendInput((uint)arr.Length, arr, Marshal.SizeOf(typeof(Input)));
        }

        public static void RightClickAt(int x, int y) {
            SetCursorPos(x, y);
            Input[] inputs = new Input[2];
            inputs[0].type = 0;
            inputs[0].U.mi = new MouseInput { dx = 0, dy = 0, mouseData = 0, dwFlags = MOUSEEVENTF_RIGHTDOWN, time = 0, dwExtraInfo = IntPtr.Zero };
            inputs[1].type = 0;
            inputs[1].U.mi = new MouseInput { dx = 0, dy = 0, mouseData = 0, dwFlags = MOUSEEVENTF_RIGHTUP, time = 0, dwExtraInfo = IntPtr.Zero };
            SendInput(2, inputs, Marshal.SizeOf(typeof(Input)));
        }

        public static Rect WindowRect(IntPtr hwnd) {
            Rect r;
            GetWindowRect(hwnd, out r);
            return r;
        }

        public static bool HasOwner(IntPtr hwnd) { return GetWindow(hwnd, GW_OWNER) != IntPtr.Zero; }

        /// Control-class token for a root-level window, used by the
        /// stateless-menu-predicate leg to classify every root-level UIA
        /// child it enumerates. A class name (e.g. "#32768", "Notepad",
        /// "HwndWrapper[...]") is a control-class token, not a window title -
        /// it never carries a file path, pid, or user/machine name, so it is
        /// safe under the probe corpus's redaction pipeline as-is.
        public static string ClassNameOf(IntPtr hwnd) {
            StringBuilder sb = new StringBuilder(128);
            GetClassNameW(hwnd, sb, sb.Capacity);
            return sb.ToString();
        }

        public static bool HasDlgModalFrame(IntPtr hwnd) {
            return (GetWindowLongW(hwnd, GWL_EXSTYLE) & WS_EX_DLGMODALFRAME) != 0;
        }

        public static void CloseWindow(IntPtr hwnd) { PostMessageW(hwnd, WM_CLOSE, IntPtr.Zero, IntPtr.Zero); }

        /// A pid deliberately manufactured with no GUI thread at all (a
        /// console-subsystem child with no window station surface), for the
        /// GetGUIThreadInfo-reach leg's negative case.
        public static uint ReadFlagsForFirstThreadOfPid(int pid, out bool anyThreadFound) {
            uint[] tids = ThreadsForPid(pid);
            anyThreadFound = tids.Length > 0;
            if (tids.Length == 0) { return 0; }
            ThreadFlagsResult r = ReadThreadFlags(tids[0]);
            if (!r.Ok) { return 0xFFFFFFFF; }
            return r.Flags;
        }
    }
}
'@
    Add-ProbeInlineCSharp -Source $src -AssemblyLeaf 'AgentDesktopProbeA23Signals23'
}
