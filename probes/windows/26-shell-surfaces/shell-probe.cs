// agent-desktop windows probe area 26: shell-surface helper on the UIA3 COM stack.
//
// Binding mechanism identical to probes/windows/08-uia3-com.cs: hand-declared
// [ComImport] interfaces in full vtable order bound to CUIAutomation8. The GAC
// managed System.Windows.Automation assembly is NEVER referenced here, so every
// number this helper produces comes from the client stack the Rust
// `uiautomation` crate wraps (KTD3). A managed cross-check for one specific row
// (A26-5) is taken PowerShell-side and labelled non-authoritative there.
//
// Compiled by probes/windows/26-shell-surfaces/lib.ps1 with the in-box
// pre-Roslyn csc.exe under /langversion:5 (C-1/A24-1 ceiling). One compact JSON
// document on stdout per mode; diagnostics to stderr.
//
// Capture-safety contract: this helper never prints a window title, an element
// Name, a pid number, or an untagged AutomationId. AutomationIds are tagged:
// machine-local GUIDs become <machine-local-guid>, anything outside the
// framework-stable ^[A-Za-z][A-Za-z0-9_.]{0,47}$ shape becomes <opaque>.
// Raw HWND values are emitted only under JSON keys the corpus normalizer masks.

using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.RegularExpressions;

namespace AgentDesktop.Probe.Shell26
{
    [StructLayout(LayoutKind.Sequential)]
    public struct UiaPoint { public int X; public int Y; }

    [StructLayout(LayoutKind.Sequential)]
    public struct UiaRect { public int Left; public int Top; public int Right; public int Bottom; }

    [StructLayout(LayoutKind.Sequential)]
    public struct KEYBDINPUT { public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }

    [StructLayout(LayoutKind.Sequential)]
    public struct MOUSEINPUT { public int dx; public int dy; public uint mouseData; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }

    [StructLayout(LayoutKind.Explicit)]
    public struct INPUTUNION { [FieldOffset(0)] public MOUSEINPUT mi; [FieldOffset(0)] public KEYBDINPUT ki; }

    [StructLayout(LayoutKind.Sequential)]
    public struct INPUT { public uint type; public INPUTUNION u; }

    [ComImport, Guid("352ffba8-0973-437c-a61f-f64cafd81df9"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IUIAutomationCondition { }

    [ComImport, Guid("14314595-b4bc-4055-95f2-58f2e42c9855"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IUIAutomationElementArray
    {
        int GetLength();
        IUIAutomationElement GetElement(int index);
    }

    [ComImport, Guid("4042c624-389c-4afc-a630-9df854a541fc"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IUIAutomationTreeWalker
    {
        IUIAutomationElement GetParentElement(IUIAutomationElement element);
        IUIAutomationElement GetFirstChildElement(IUIAutomationElement element);
        IUIAutomationElement GetLastChildElement(IUIAutomationElement element);
        IUIAutomationElement GetNextSiblingElement(IUIAutomationElement element);
        IUIAutomationElement GetPreviousSiblingElement(IUIAutomationElement element);
        void Slot6ParentBuildCache();
        void Slot7FirstChildBuildCache();
        void Slot8LastChildBuildCache();
        void Slot9NextSiblingBuildCache();
        void Slot10PreviousSiblingBuildCache();
        IUIAutomationElement NormalizeElement(IUIAutomationElement element);
        void Slot12NormalizeBuildCache();
        IUIAutomationCondition GetCondition();
    }

    [ComImport, Guid("d22108aa-8ac5-49a5-837b-37bbb3d7591e"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IUIAutomationElement
    {
        void SetFocus();
        [return: MarshalAs(UnmanagedType.SafeArray, SafeArraySubType = VarEnum.VT_I4)]
        int[] GetRuntimeId();
        IUIAutomationElement FindFirst(int scope, IUIAutomationCondition condition);
        IUIAutomationElementArray FindAll(int scope, IUIAutomationCondition condition);
        void Slot5FindFirstBuildCache();
        IUIAutomationElementArray FindAllBuildCache(int scope, IUIAutomationCondition condition, object cacheRequest);
        IUIAutomationElement BuildUpdatedCache(object cacheRequest);
        [return: MarshalAs(UnmanagedType.Struct)]
        object GetCurrentPropertyValue(int propertyId);
        void Slot9GetCurrentPropertyValueEx();
        [return: MarshalAs(UnmanagedType.Struct)]
        object GetCachedPropertyValue(int propertyId);
        void Slot11GetCachedPropertyValueEx();
        void Slot12GetCurrentPatternAs();
        void Slot13GetCachedPatternAs();
        [return: MarshalAs(UnmanagedType.IUnknown)]
        object GetCurrentPattern(int patternId);
        void Slot15GetCachedPattern();
        void Slot16GetCachedParent();
        IUIAutomationElementArray GetCachedChildren();
        int GetCurrentProcessId();
        int GetCurrentControlType();
        [return: MarshalAs(UnmanagedType.BStr)]
        string GetCurrentLocalizedControlType();
        [return: MarshalAs(UnmanagedType.BStr)]
        string GetCurrentName();
        void Slot22AcceleratorKey();
        void Slot23AccessKey();
        int GetCurrentHasKeyboardFocus();
        int GetCurrentIsKeyboardFocusable();
        int GetCurrentIsEnabled();
        [return: MarshalAs(UnmanagedType.BStr)]
        string GetCurrentAutomationId();
        [return: MarshalAs(UnmanagedType.BStr)]
        string GetCurrentClassName();
        void Slot29HelpText();
        void Slot30Culture();
        int GetCurrentIsControlElement();
        int GetCurrentIsContentElement();
        void Slot33IsPassword();
        IntPtr GetCurrentNativeWindowHandle();
        void Slot35ItemType();
        int GetCurrentIsOffscreen();
        void Slot37Orientation();
        [return: MarshalAs(UnmanagedType.BStr)]
        string GetCurrentFrameworkId();
        void Slot39IsRequiredForForm();
        void Slot40ItemStatus();
        UiaRect GetCurrentBoundingRectangle();
        void Slot42LabeledBy();
        void Slot43AriaRole();
        void Slot44AriaProperties();
        void Slot45IsDataValidForForm();
        void Slot46ControllerFor();
        void Slot47DescribedBy();
        void Slot48FlowsTo();
        [return: MarshalAs(UnmanagedType.BStr)]
        string GetCurrentProviderDescription();
    }

    // Notification verb-button invocation. IID verified against the shipped
    // Windows SDK UIAutomationClient.h (MIDL_INTERFACE fb377fbe-...); a failed
    // QI fails benignly with null and the caller records invoked:false.
    [ComImport, Guid("FB377FBE-8EA6-46D5-9C73-6499642D3059"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IUIAutomationInvokePattern
    {
        void Invoke();
    }

    [ComImport, Guid("30cbe57d-d9d0-452a-ab13-7ac5ac4825ee"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IUIAutomation
    {
        int CompareElements(IUIAutomationElement el1, IUIAutomationElement el2);
        void Slot2CompareRuntimeIds();
        IUIAutomationElement GetRootElement();
        IUIAutomationElement ElementFromHandle(IntPtr hwnd);
        IUIAutomationElement ElementFromPoint(UiaPoint pt);
        IUIAutomationElement GetFocusedElement();
        void Slot7RootBuildCache();
        void Slot8FromHandleBuildCache();
        void Slot9FromPointBuildCache();
        void Slot10FocusedBuildCache();
        IUIAutomationTreeWalker CreateTreeWalker(IUIAutomationCondition condition);
        IUIAutomationTreeWalker GetControlViewWalker();
        IUIAutomationTreeWalker GetContentViewWalker();
        IUIAutomationTreeWalker GetRawViewWalker();
        IUIAutomationCondition GetRawViewCondition();
        IUIAutomationCondition GetControlViewCondition();
        IUIAutomationCondition GetContentViewCondition();
        object CreateCacheRequest();
        IUIAutomationCondition CreateTrueCondition();
        void Slot20CreateFalseCondition();
        void Slot21CreatePropertyCondition();
        void Slot22CreatePropertyConditionEx();
        void Slot23CreateAndCondition();
        void Slot24CreateAndConditionFromArray();
        void Slot25CreateAndConditionFromNativeArray();
        void Slot26CreateOrCondition();
        void Slot27CreateOrConditionFromArray();
        void Slot28CreateOrConditionFromNativeArray();
        void Slot29CreateNotCondition();
        void AddAutomationEventHandler(int eventId, IUIAutomationElement element, int scope, object cacheRequest, object handler);
        void RemoveAutomationEventHandler(int eventId, IUIAutomationElement element, object handler);
        void Slot32AddPropertyChangedNativeArray();
        void AddPropertyChangedEventHandler(IUIAutomationElement element, int scope, object cacheRequest, object handler, [MarshalAs(UnmanagedType.SafeArray, SafeArraySubType = VarEnum.VT_I4)] int[] propertyArray);
        void RemovePropertyChangedEventHandler(IUIAutomationElement element, object handler);
        void Slot35AddStructureChanged();
        void Slot36RemoveStructureChanged();
        void AddFocusChangedEventHandler(object cacheRequest, object handler);
        void RemoveFocusChangedEventHandler(object handler);
        void RemoveAllEventHandlers();
        void Slot40IntNativeArrayToSafeArray();
        void Slot41IntSafeArrayToNativeArray();
        void Slot42RectToVariant();
        void Slot43VariantToRect();
        void Slot44SafeArrayToRectNativeArray();
        void Slot45CreateProxyFactoryEntry();
        void Slot46ProxyFactoryMapping();
        [return: MarshalAs(UnmanagedType.BStr)]
        string GetPropertyProgrammaticName(int property);
        [return: MarshalAs(UnmanagedType.BStr)]
        string GetPatternProgrammaticName(int pattern);
        void PollForPotentialSupportedPatterns(IUIAutomationElement element,
            [MarshalAs(UnmanagedType.SafeArray, SafeArraySubType = VarEnum.VT_I4)] out int[] patternIds,
            [MarshalAs(UnmanagedType.SafeArray, SafeArraySubType = VarEnum.VT_BSTR)] out string[] patternNames);
    }

    public static class Native
    {
        public delegate bool EnumProc(IntPtr hWnd, IntPtr lParam);

        [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr lParam);
        [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr hWnd, EnumProc cb, IntPtr lParam);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassNameW(IntPtr hWnd, StringBuilder buf, int max);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr hWnd, StringBuilder buf, int max);
        [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
        [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr hWnd);
        [DllImport("user32.dll")] public static extern IntPtr GetParent(IntPtr hWnd);
        [DllImport("user32.dll", EntryPoint = "GetWindowLongW")] public static extern int GetWindowLong32(IntPtr hWnd, int nIndex);
        [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
        [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
        [DllImport("user32.dll", SetLastError = true)] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
        [DllImport("user32.dll", SetLastError = true)] public static extern uint SendInput(uint nInputs, INPUT[] pInputs, int cbSize);
        [DllImport("kernel32.dll", SetLastError = true)] public static extern IntPtr OpenProcess(uint access, bool inherit, int pid);
        [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)] public static extern bool QueryFullProcessImageNameW(IntPtr hProcess, uint flags, StringBuilder exeName, ref uint size);
        [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr hObject);
        [DllImport("user32.dll", SetLastError = true)] public static extern bool PostMessageW(IntPtr hWnd, uint msg, IntPtr w, IntPtr l);
        [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr hwnd, int attr, out int value, int size);

        public const int GwlExStyle = -20;
        public const int WsExToolWindow = 0x00000080;
        public const int DwmwaCloaked = 14;
        public const ushort VkLWin = 0x5B;
        public const ushort VkA = 0x41;
        public const ushort VkEscape = 0x1B;
        public const ushort VkControl = 0x11;
        public const ushort VkReturn = 0x0D;
        public const uint KeyeventfKeyup = 0x0002;
        public const uint KeyeventfUnicode = 0x0004;
        public const uint ProcessQueryLimitedInformation = 0x1000;

        public static string ClassOf(IntPtr h) { StringBuilder b = new StringBuilder(256); GetClassNameW(h, b, 256); return b.ToString(); }
        public static bool TitleEquals(IntPtr h, string wanted)
        {
            StringBuilder b = new StringBuilder(512);
            GetWindowTextW(h, b, 512);
            return string.Equals(b.ToString(), wanted, StringComparison.OrdinalIgnoreCase);
        }
        public static int PidOf(IntPtr h) { uint p; GetWindowThreadProcessId(h, out p); return (int)p; }
        public static bool IsToolWindow(IntPtr h) { return (GetWindowLong32(h, GwlExStyle) & WsExToolWindow) != 0; }

        // 0 none; 1 DWM_CLOAKED_SHELL; 2 DWM_CLOAKED_APP; -1 call failed.
        public static int CloakState(IntPtr h) { int v; if (DwmGetWindowAttribute(h, DwmwaCloaked, out v, 4) != 0) { return -1; } return v; }

        [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr FindWindowW(string cls, string title);
        [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int cmd);

        public static bool ShowWindowMode(IntPtr h, int cmd) { return ShowWindow(h, cmd); }

        public static string HostTokenOf(IntPtr h)
        {
            int pid = PidOf(h);
            if (pid == 0) { return "unknown"; }
            IntPtr proc = OpenProcess(ProcessQueryLimitedInformation, false, pid);
            if (proc == IntPtr.Zero) { return "unreadable"; }
            try
            {
                StringBuilder sb = new StringBuilder(1024);
                uint size = 1024;
                if (!QueryFullProcessImageNameW(proc, 0, sb, ref size)) { return "unreadable"; }
                string leaf = sb.ToString();
                int slash = leaf.LastIndexOf('\\');
                if (slash >= 0) { leaf = leaf.Substring(slash + 1); }
                leaf = leaf.ToLowerInvariant();
                if (leaf.EndsWith(".exe")) { leaf = leaf.Substring(0, leaf.Length - 4); }
                switch (leaf)
                {
                    case "applicationframehost": return "frame_host";
                    case "shellexperiencehost": return "shell_experience_host";
                    case "searchhost":
                    case "searchui":
                    case "searchapp": return "search_host";
                    case "startmenuexperiencehost": return "start_menu_experience_host";
                    case "systemsettings": return "system_settings";
                    case "textinputhost": return "text_input_host";
                    case "explorer": return "explorer";
                    default: return "other";
                }
            }
            finally { CloseHandle(proc); }
        }

        public static IntPtr[] AllTopLevelWindows()
        {
            List<IntPtr> found = new List<IntPtr>();
            EnumWindows(delegate(IntPtr h, IntPtr l) { found.Add(h); return true; }, IntPtr.Zero);
            return found.ToArray();
        }

        public static IntPtr[] ChildrenOf(IntPtr h)
        {
            List<IntPtr> found = new List<IntPtr>();
            EnumChildWindows(h, delegate(IntPtr c, IntPtr l) { found.Add(c); return true; }, IntPtr.Zero);
            return found.ToArray();
        }

        public static IntPtr FindTopLevelByClass(string className)
        {
            foreach (IntPtr h in AllTopLevelWindows())
            {
                if (string.Equals(ClassOf(h), className, StringComparison.Ordinal)) { return h; }
            }
            return FindWindowW(className, null);
        }

        public static INPUT[] KeyChord(ushort modVk, ushort keyVk)
        {
            List<INPUT> seq = new List<INPUT>();
            Action<ushort, bool> add = delegate(ushort vk, bool up)
            {
                INPUT i = new INPUT();
                i.type = 1;
                i.u.ki.wVk = vk;
                i.u.ki.dwFlags = up ? KeyeventfKeyup : 0;
                i.u.ki.time = 0;
                i.u.ki.dwExtraInfo = IntPtr.Zero;
                seq.Add(i);
            };
            if (modVk != 0) { add(modVk, false); }
            add(keyVk, false);
            add(keyVk, true);
            if (modVk != 0) { add(modVk, true); }
            return seq.ToArray();
        }

        // One UTF-16 unit per KEYEVENTF_UNICODE down/up pair - a surrogate
        // pair arrives as two separate events, exactly the product's chunking
        // (A4-1). Synthetic probe text only.
        public static INPUT[] TypeText(string text)
        {
            List<INPUT> seq = new List<INPUT>();
            foreach (char c in text)
            {
                INPUT down = new INPUT();
                down.type = 1;
                down.u.ki.wScan = c;
                down.u.ki.dwFlags = KeyeventfUnicode;
                seq.Add(down);
                INPUT up = new INPUT();
                up.type = 1;
                up.u.ki.wScan = c;
                up.u.ki.dwFlags = KeyeventfUnicode | KeyeventfKeyup;
                seq.Add(up);
            }
            return seq.ToArray();
        }

        public static uint SendSequence(INPUT[] inputs)
        {
            return SendInput((uint)inputs.Length, inputs, Marshal.SizeOf(typeof(INPUT)));
        }
    }

    public static class Js
    {
        public static string Str(string v)
        {
            if (v == null) { return "null"; }
            StringBuilder sb = new StringBuilder("\"");
            for (int i = 0; i < v.Length; i++)
            {
                char c = v[i];
                if (c == '"' || c == '\\') { sb.Append('\\'); sb.Append(c); }
                else if (c == '\n') { sb.Append("\\n"); }
                else if (c == '\r') { sb.Append("\\r"); }
                else if (c == '\t') { sb.Append("\\t"); }
                else if (c < ' ' || c > '~') { sb.Append("\\u"); sb.Append(((int)c).ToString("x4", CultureInfo.InvariantCulture)); }
                else { sb.Append(c); }
            }
            sb.Append('"');
            return sb.ToString();
        }
        public static string Num(double v) { return Math.Round(v, 1).ToString("0.###", CultureInfo.InvariantCulture); }
        public static string Int(long v) { return v.ToString(CultureInfo.InvariantCulture); }
        public static string Bool(bool v) { return v ? "true" : "false"; }
        public static string P(string key, string rawValue) { return Str(key) + ":" + rawValue; }
        public static string Obj(List<string> parts) { return "{" + string.Join(",", parts.ToArray()) + "}"; }
        public static string Arr(List<string> parts) { return "[" + string.Join(",", parts.ToArray()) + "]"; }
        public static string StrArr(IEnumerable<string> items)
        {
            List<string> parts = new List<string>();
            foreach (string s in items) { parts.Add(Str(s)); }
            return Arr(parts);
        }
    }

    public static class Tags
    {
        private static readonly Regex GuidShape = new Regex(
            "^[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}$", RegexOptions.Compiled);
        private static readonly Regex FrameworkShape = new Regex("^[A-Za-z][A-Za-z0-9_.]{0,47}$", RegexOptions.Compiled);

        public static string AutomationId(string raw)
        {
            if (raw == null) { return "-"; }
            string trimmed = raw.Trim().Trim('{', '}');
            if (trimmed.Length == 0) { return "-"; }
            if (GuidShape.IsMatch(trimmed)) { return "<machine-local-guid>"; }
            if (FrameworkShape.IsMatch(raw)) { return raw; }
            return "<opaque>";
        }
    }

    public sealed class IdTables
    {
        public readonly Dictionary<int, string> PatternAvailabilityProperty = new Dictionary<int, string>();
        public readonly Dictionary<string, int> PatternIdByName = new Dictionary<string, int>(StringComparer.OrdinalIgnoreCase);

        public static IdTables Discover(IUIAutomation uia)
        {
            IdTables t = new IdTables();
            for (int id = 30000; id <= 30300; id++)
            {
                string name = null;
                try { name = uia.GetPropertyProgrammaticName(id); } catch (Exception) { continue; }
                if (string.IsNullOrEmpty(name)) { continue; }
                Match m = Regex.Match(name, "^Is(?<p>.+?)Pattern(?<n>\\d?)Available$");
                if (m.Success) { t.PatternAvailabilityProperty[id] = m.Groups["p"].Value + m.Groups["n"].Value; }
            }
            for (int id = 10000; id <= 10060; id++)
            {
                string name = null;
                try { name = uia.GetPatternProgrammaticName(id); } catch (Exception) { continue; }
                if (string.IsNullOrEmpty(name)) { continue; }
                t.PatternIdByName[Regex.Replace(name, "Pattern$", "")] = id;
            }
            return t;
        }

        public List<string> PatternsOf(IUIAutomationElement e)
        {
            List<string> found = new List<string>();
            foreach (KeyValuePair<int, string> kv in PatternAvailabilityProperty)
            {
                object v = null;
                try { v = e.GetCurrentPropertyValue(kv.Key); } catch (Exception) { continue; }
                if (v is bool && (bool)v) { found.Add(kv.Value); }
            }
            found.Sort(StringComparer.Ordinal);
            return found;
        }
    }

    public sealed class NodeRec
    {
        public int Index;
        public int Depth;
        public int Parent;
        public int ControlTypeId;
        public string AutomationIdRaw;
        public List<string> Patterns;
        public bool BoundsPositive;
        public int OffscreenFlag;
        public bool NamePresent;
    }

    public static class Probe
    {
        public const int TreeScopeChildren = 2;
        public const int TreeScopeSubtree = 7;
        private static readonly string[] ControlTypeNames = new string[]
        {
            "Button", "Calendar", "CheckBox", "ComboBox", "Edit", "Hyperlink", "Image", "ListItem", "List", "Menu",
            "MenuBar", "MenuItem", "ProgressBar", "RadioButton", "ScrollBar", "Slider", "Spinner", "StatusBar", "Tab", "TabItem",
            "Text", "ToolBar", "ToolTip", "Tree", "TreeItem", "Custom", "Group", "Thumb", "DataGrid", "DataItem",
            "Document", "SplitButton", "Window", "Pane", "Header", "HeaderItem", "Table", "TitleBar", "Separator", "SemanticZoom", "AppBar"
        };

        public static string ControlTypeName(int id)
        {
            if (id >= 50000 && id <= 50040) { return ControlTypeNames[id - 50000]; }
            return "Unknown" + id.ToString(CultureInfo.InvariantCulture);
        }

        public static Dictionary<int, IUIAutomationElement> RootChildrenByIndex(IUIAutomation uia)
        {
            Dictionary<int, IUIAutomationElement> byIndex = new Dictionary<int, IUIAutomationElement>();
            IUIAutomationElement root = uia.GetRootElement();
            IUIAutomationTreeWalker walker = uia.GetControlViewWalker();
            int idx = 0;
            IUIAutomationElement child = null;
            try { child = walker.GetFirstChildElement(root); } catch (Exception) { child = null; }
            while (child != null && idx < 200)
            {
                byIndex[idx++] = child;
                IUIAutomationElement next = null;
                try { next = walker.GetNextSiblingElement(child); } catch (Exception) { next = null; }
                child = next;
            }
            return byIndex;
        }

        // The Action Center's CoreWindow, identified without reading any title
        // into output: class Windows.UI.Core.CoreWindow owned by
        // shell_experience_host, preferring the uncloaked instance when open.
        // Returns IntPtr.Zero when absent; candidateCount reports how many host-
        // matching CoreWindows were seen for ambiguity honesty.
        public static IntPtr FindActionCenterCoreWindow(IUIAutomation uia, out int candidateCount)
        {
            HashSet<long> seen = new HashSet<long>();
            List<IntPtr> candidates = new List<IntPtr>();
            foreach (KeyValuePair<int, IUIAutomationElement> kv in RootChildrenByIndex(uia))
            {
                IntPtr h = IntPtr.Zero;
                try { h = kv.Value.GetCurrentNativeWindowHandle(); } catch (Exception) { h = IntPtr.Zero; }
                string cls = null;
                try { cls = kv.Value.GetCurrentClassName(); } catch (Exception) { cls = ""; }
                if (h == IntPtr.Zero || cls != "Windows.UI.Core.CoreWindow") { continue; }
                if (Native.HostTokenOf(h) == "shell_experience_host") { candidates.Add(h); seen.Add(h.ToInt64()); }
            }
            foreach (IntPtr h in Native.AllTopLevelWindows())
            {
                if (!seen.Add(h.ToInt64())) { continue; }
                if (Native.ClassOf(h) == "Windows.UI.Core.CoreWindow" && Native.HostTokenOf(h) == "shell_experience_host")
                {
                    candidates.Add(h);
                }
            }
            candidateCount = candidates.Count;
            IntPtr best = IntPtr.Zero;
            int bestCloak = -2;
            foreach (IntPtr h in candidates)
            {
                int cloak = Native.CloakState(h);
                if (best == IntPtr.Zero || (cloak == 0 && bestCloak != 0)) { best = h; bestCloak = cloak; }
            }
            return best;
        }

        public static string CloakWord(int state)
        {
            if (state == -1) { return "dwm_call_failed"; }
            if (state == 0) { return "none"; }
            if (state == 1) { return "cloaked_shell"; }
            if (state == 2) { return "cloaked_app"; }
            return "value_" + state.ToString(CultureInfo.InvariantCulture);
        }

        public static NodeRec ReadNode(IUIAutomationElement e, IdTables t, int index, int depth, int parent)
        {
            NodeRec r = new NodeRec();
            r.Index = index;
            r.Depth = depth;
            r.Parent = parent;
            r.AutomationIdRaw = "";
            r.Patterns = new List<string>();
            r.BoundsPositive = false;
            r.OffscreenFlag = -1;
            r.NamePresent = false;
            try { r.ControlTypeId = e.GetCurrentControlType(); } catch (Exception) { r.ControlTypeId = 0; }
            try { r.AutomationIdRaw = e.GetCurrentAutomationId(); } catch (Exception) { r.AutomationIdRaw = ""; }
            try { r.Patterns = t.PatternsOf(e); } catch (Exception) { }
            try { UiaRect rc = e.GetCurrentBoundingRectangle(); r.BoundsPositive = (rc.Right > rc.Left && rc.Bottom > rc.Top); } catch (Exception) { }
            try { r.OffscreenFlag = e.GetCurrentIsOffscreen(); } catch (Exception) { r.OffscreenFlag = -1; }
            try { string nm = e.GetCurrentName(); r.NamePresent = !string.IsNullOrEmpty(nm); } catch (Exception) { r.NamePresent = false; }
            return r;
        }

        // Bounded control-view flattening. Raw automation ids stay in memory for
        // within-run stability comparison; only tagged values are ever printed.
        public static List<NodeRec> CollectFlat(IUIAutomation uia, IUIAutomationElement root, IdTables t, int maxNodes, int maxDepth)
        {
            List<NodeRec> list = new List<NodeRec>();
            Queue<KeyValuePair<IUIAutomationElement, int[]>> pending = new Queue<KeyValuePair<IUIAutomationElement, int[]>>();
            IUIAutomationTreeWalker walker = uia.GetControlViewWalker();
            pending.Enqueue(new KeyValuePair<IUIAutomationElement, int[]>(root, new int[] { 0, -1 }));
            while (pending.Count > 0 && list.Count < maxNodes)
            {
                KeyValuePair<IUIAutomationElement, int[]> head = pending.Dequeue();
                int myIndex = list.Count;
                NodeRec rec = ReadNode(head.Key, t, myIndex, head.Value[0], head.Value[1]);
                list.Add(rec);
                if (head.Value[0] >= maxDepth) { continue; }
                IUIAutomationElement child = null;
                try { child = walker.GetFirstChildElement(head.Key); } catch (Exception) { child = null; }
                while (child != null && list.Count + pending.Count < maxNodes + pending.Count)
                {
                    pending.Enqueue(new KeyValuePair<IUIAutomationElement, int[]>(child, new int[] { head.Value[0] + 1, myIndex }));
                    IUIAutomationElement next = null;
                    try { next = walker.GetNextSiblingElement(child); } catch (Exception) { next = null; }
                    child = next;
                }
            }
            return list;
        }

        public static string NodesJson(List<NodeRec> nodes)
        {
            List<string> parts = new List<string>();
            foreach (NodeRec n in nodes)
            {
                parts.Add(Js.Obj(new List<string>(new string[]
                {
                    Js.P("i", Js.Int(n.Index)),
                    Js.P("d", Js.Int(n.Depth)),
                    Js.P("parent", Js.Int(n.Parent)),
                    Js.P("ct", Js.Str(ControlTypeName(n.ControlTypeId))),
                    Js.P("aid", Js.Str(Tags.AutomationId(n.AutomationIdRaw))),
                    Js.P("nm", Js.Bool(n.NamePresent)),
                    Js.P("pats", Js.StrArr(n.Patterns)),
                    Js.P("pos", Js.Bool(n.BoundsPositive)),
                    Js.P("off", Js.Int(n.OffscreenFlag))
                })));
            }
            return Js.Arr(parts);
        }

        public static string RescanEqualityJson(List<NodeRec> first, List<NodeRec> second)
        {
            int n = Math.Min(first.Count, second.Count);
            List<string> aidSame = new List<string>();
            List<string> ctSame = new List<string>();
            List<string> patSame = new List<string>();
            for (int i = 0; i < n; i++)
            {
                aidSame.Add(Js.Bool(string.Equals(first[i].AutomationIdRaw, second[i].AutomationIdRaw, StringComparison.Ordinal)));
                ctSame.Add(Js.Bool(first[i].ControlTypeId == second[i].ControlTypeId));
                patSame.Add(Js.Bool(first[i].Patterns.Count == second[i].Patterns.Count));
            }
            List<string> o = new List<string>();
            o.Add(Js.P("compared_pairs", Js.Int(n)));
            o.Add(Js.P("first_count", Js.Int(first.Count)));
            o.Add(Js.P("second_count", Js.Int(second.Count)));
            o.Add(Js.P("aid_unchanged_by_index", Js.Arr(aidSame)));
            o.Add(Js.P("ct_unchanged_by_index", Js.Arr(ctSame)));
            o.Add(Js.P("pattern_set_size_unchanged_by_index", Js.Arr(patSame)));
            o.Add(Js.P("all_ids_unchanged", Js.Bool(ArrayTrue(aidSame))));
            return Js.Obj(o);
        }

        private static bool ArrayTrue(List<string> boolParts)
        {
            if (boolParts.Count == 0) { return false; }
            foreach (string p in boolParts) { if (p != "true") { return false; } }
            return true;
        }
    }

    public static class Modes
    {
        private const string AcCoreClass = "Windows.UI.Core.CoreWindow";

        public static string RunReachScan(IUIAutomation uia, IdTables t)
        {
            IntPtr[] topLevel = Native.AllTopLevelWindows();
            bool shellTray = false;
            bool shellTrayVisible = false;
            bool progman = false;
            bool overflowClass = false;
            int overflowVisible = 0;
            HashSet<long> enumHandles = new HashSet<long>();
            foreach (IntPtr h in topLevel)
            {
                enumHandles.Add(h.ToInt64());
                string cls = Native.ClassOf(h);
                if (cls == "Shell_TrayWnd") { shellTray = true; if (Native.IsWindowVisible(h)) { shellTrayVisible = true; } }
                if (cls == "Progman") { progman = true; }
                if (cls == "NotifyIconOverflowWindow") { overflowClass = true; if (Native.IsWindowVisible(h)) { overflowVisible++; } }
            }

            Dictionary<int, IUIAutomationElement> children = Probe.RootChildrenByIndex(uia);
            int childCount = children.Count;
            List<string> childRows = new List<string>();
            int coreWindowCandidates = 0;
            IntPtr acHwnd = IntPtr.Zero;
            foreach (KeyValuePair<int, IUIAutomationElement> kv in children)
            {
                IUIAutomationElement e = kv.Value;
                string cls = "";
                long handleVal = 0;
                try { cls = e.GetCurrentClassName() ?? ""; } catch (Exception) { }
                try { handleVal = e.GetCurrentNativeWindowHandle().ToInt64(); } catch (Exception) { }
                bool isCandidateRaw = cls == AcCoreClass && handleVal != 0 &&
                    Native.HostTokenOf(new IntPtr(handleVal)) == "shell_experience_host";
                bool mainListViewPresent = false;
                bool startMenuFramePresent = false;
                bool quickActionsPresent = false;
                if (isCandidateRaw)
                {
                    // Landmark disambiguation inside the candidate itself: this
                    // build hosts several cloakable shell CoreWindows under the
                    // same owner process, so the uncloaked one must be told
                    // apart by its own framework AutomationId landmarks. The
                    // Action Center swaps its layout by content - MainListView
                    // only exists when notifications are present; the empty
                    // center shows the Microsoft.QuickAction.* pane instead.
                    foreach (NodeRec n in Probe.CollectFlat(uia, kv.Value, t, 150, 14))
                    {
                        string tag = Tags.AutomationId(n.AutomationIdRaw);
                        if (tag == "MainListView") { mainListViewPresent = true; }
                        if (tag == "SplitViewFrameXAMLWindow") { startMenuFramePresent = true; }
                        if (tag.StartsWith("Microsoft.QuickAction.", StringComparison.Ordinal)) { quickActionsPresent = true; }
                    }
                }
                bool isCandidate = isCandidateRaw && (mainListViewPresent || quickActionsPresent);
                if (isCandidate)
                {
                    coreWindowCandidates++;
                    if (acHwnd == IntPtr.Zero && Native.CloakState(new IntPtr(handleVal)) == 0)
                    {
                        acHwnd = new IntPtr(handleVal);
                    }
                }
                childRows.Add(Js.Obj(new List<string>(new string[]
                {
                    Js.P("ct", Js.Str(Probe.ControlTypeName(SafeControlType(e)))),
                    Js.P("cls", Js.Str(cls)),
                    Js.P("host_token", Js.Str(Native.HostTokenOf(new IntPtr(handleVal)))),
                    Js.P("ac_candidate", Js.Bool(isCandidate)),
                    Js.P("landmark_main_list_view_present", Js.Bool(mainListViewPresent)),
                    Js.P("landmark_quick_actions_present", Js.Bool(quickActionsPresent)),
                    Js.P("landmark_start_menu_frame_present", Js.Bool(startMenuFramePresent)),
                    Js.P("nativewindowhandle", Js.Int(handleVal))
                })));
            }

            bool acYieldedByEnum = false;
            if (acHwnd != IntPtr.Zero) { acYieldedByEnum = enumHandles.Contains(acHwnd.ToInt64()); }

            List<string> o = new List<string>();
            o.Add(Js.P("mode", Js.Str("reachscan")));
            o.Add(Js.P("client_stack", Js.Str("uia3-com")));
            o.Add(Js.P("binding", Js.Str(Program.Coclass)));
            o.Add(Js.P("enum_walk_yields_shell_tray_wnd", Js.Bool(shellTray)));
            o.Add(Js.P("enum_walk_shell_tray_visible", Js.Bool(shellTrayVisible)));
            o.Add(Js.P("enum_walk_yields_progman", Js.Bool(progman)));
            o.Add(Js.P("enum_walk_yields_notify_icon_overflow_class", Js.Bool(overflowClass)));
            o.Add(Js.P("notify_icon_overflow_visible_instances", Js.Int(overflowVisible)));
            o.Add(Js.P("surface_present_in_enum_walk", Js.Bool(acYieldedByEnum)));
            o.Add(Js.P("uia_root_child_count", Js.Int(childCount)));
            o.Add(Js.P("action_center_candidate_corewindows", Js.Int(coreWindowCandidates)));
            o.Add(Js.P("surface_open_handle_found", Js.Bool(acHwnd != IntPtr.Zero)));
            o.Add(Js.P("find_window_route_finds_shell_tray_wnd", Js.Bool(Native.FindWindowW("Shell_TrayWnd", null) != IntPtr.Zero)));
            o.Add(Js.P("find_window_route_finds_notify_icon_overflow_window", Js.Bool(Native.FindWindowW("NotifyIconOverflowWindow", null) != IntPtr.Zero)));
            o.Add(Js.P("children", Js.Arr(childRows)));
            return Js.Obj(o);
        }

        private static int SafeControlType(IUIAutomationElement e)
        {
            try { return e.GetCurrentControlType(); } catch (Exception) { return 0; }
        }

        public static string RunPredicate(IUIAutomation uia, IdTables t, string hwndArg)
        {
            IntPtr h = Args.ParseHandle(hwndArg);
            if (h == IntPtr.Zero || !Native.IsWindow(h)) { throw new InvalidOperationException("predicate requires a live --hwnd"); }
            IntPtr parent = Native.GetParent(h);
            string parentClass = "-";
            bool hasParent = parent != IntPtr.Zero;
            if (hasParent) { parentClass = Native.ClassOf(parent); if (parentClass.Length == 0) { parentClass = "<unreadable>"; } }
            List<string> o = new List<string>();
            o.Add(Js.P("ws_ex_tool_window", Js.Bool(Native.IsToolWindow(h))));
            o.Add(Js.P("cloak_state", Js.Str(Probe.CloakWord(Native.CloakState(h)))));
            o.Add(Js.P("is_window_visible", Js.Bool(Native.IsWindowVisible(h))));
            o.Add(Js.P("has_parent", Js.Bool(hasParent)));
            o.Add(Js.P("parent_class", Js.Str(parentClass)));
            o.Add(Js.P("host_token", Js.Str(Native.HostTokenOf(h))));
            return Js.Obj(o);
        }

        public static string RunActionCenterTree(IUIAutomation uia, IdTables t, string hwndArg, bool rescan, int maxNodes, int maxDepth)
        {
            IntPtr h = Args.ParseHandle(hwndArg);
            if (h == IntPtr.Zero || !Native.IsWindow(h)) { throw new InvalidOperationException("actree requires a live --hwnd"); }
            IUIAutomationElement root = uia.ElementFromHandle(h);
            List<NodeRec> first = Probe.CollectFlat(uia, root, t, maxNodes, maxDepth);
            List<string> o = new List<string>();
            o.Add(Js.P("mode", Js.Str("actree")));
            o.Add(Js.P("client_stack", Js.Str("uia3-com")));
            o.Add(Js.P("binding", Js.Str(Program.Coclass)));
            o.Add(Js.P("node_count", Js.Int(first.Count)));
            o.Add(Js.P("node_cap", Js.Int(maxNodes)));
            o.Add(Js.P("nodes", Probe.NodesJson(first)));
            if (rescan)
            {
                List<NodeRec> second = Probe.CollectFlat(uia, root, t, maxNodes, maxDepth);
                o.Add(Js.P("rescan", Probe.RescanEqualityJson(first, second)));
            }
            return Js.Obj(o);
        }

        public static string RunTrayScan(IUIAutomation uia, IdTables t)
        {
            IntPtr taskbar = Native.FindTopLevelByClass("Shell_TrayWnd");
            if (taskbar == IntPtr.Zero) { throw new InvalidOperationException("no Shell_TrayWnd top-level window"); }

            List<IntPtr> promotedToolbars = new List<IntPtr>();
            foreach (IntPtr c in Native.ChildrenOf(taskbar))
            {
                if (Native.ClassOf(c) == "ToolbarWindow32") { promotedToolbars.Add(c); }
            }
            // The notification-area toolbar is the one nested inside TrayNotifyWnd;
            // older shells park other Toolbars directly on the taskbar.
            IntPtr notifyHost = IntPtr.Zero;
            foreach (IntPtr c in Native.ChildrenOf(taskbar))
            {
                if (Native.ClassOf(c) == "TrayNotifyWnd") { notifyHost = c; break; }
            }
            IntPtr promotedViaTrayNotify = IntPtr.Zero;
            if (notifyHost != IntPtr.Zero)
            {
                foreach (IntPtr d in Native.ChildrenOf(notifyHost))
                {
                    if (Native.ClassOf(d) == "ToolbarWindow32") { promotedViaTrayNotify = d; break; }
                    foreach (IntPtr e in Native.ChildrenOf(d))
                    {
                        if (Native.ClassOf(e) == "ToolbarWindow32") { promotedViaTrayNotify = e; break; }
                    }
                    if (promotedViaTrayNotify != IntPtr.Zero) { break; }
                }
            }

            IntPtr overflowWindow = Native.FindTopLevelByClass("NotifyIconOverflowWindow");
            IntPtr overflowToolbar = IntPtr.Zero;
            bool overflowPresent = overflowWindow != IntPtr.Zero;
            if (overflowPresent)
            {
                foreach (IntPtr c in Native.ChildrenOf(overflowWindow))
                {
                    if (Native.ClassOf(c) == "ToolbarWindow32") { overflowToolbar = c; break; }
                }
            }

            List<string> toolbars = new List<string>();
            AppendToolbarReport(t, toolbars, "promoted_via_tray_notify_wnd", promotedViaTrayNotify, false);
            for (int i = 0; i < promotedToolbars.Count && i < 4; i++)
            {
                AppendToolbarReport(t, toolbars, "taskbar_toolbar_" + i.ToString(CultureInfo.InvariantCulture), promotedToolbars[i], false);
            }
            AppendToolbarReport(t, toolbars, "overflow", overflowToolbar, false);

            List<string> o = new List<string>();
            o.Add(Js.P("mode", Js.Str("trayscan")));
            o.Add(Js.P("client_stack", Js.Str("uia3-com")));
            o.Add(Js.P("binding", Js.Str(Program.Coclass)));
            o.Add(Js.P("taskbar_found", Js.Bool(true)));
            o.Add(Js.P("tray_notify_wnd_child_present", Js.Bool(notifyHost != IntPtr.Zero)));
            o.Add(Js.P("promoted_toolbar_windows_of_taskbar", Js.Int(promotedToolbars.Count)));
            o.Add(Js.P("promoted_toolbar_via_tray_notify_found", Js.Bool(promotedViaTrayNotify != IntPtr.Zero)));
            o.Add(Js.P("overflow_window_class_notify_icon_overflow_window_present", Js.Bool(overflowPresent)));
            o.Add(Js.P("overflow_window_visible", Js.Bool(overflowPresent && Native.IsWindowVisible(overflowWindow))));
            o.Add(Js.P("overflow_inner_toolbar_found", Js.Bool(overflowToolbar != IntPtr.Zero)));
            o.Add(Js.P("toolbars", Js.Arr(toolbars)));
            return Js.Obj(o);
        }

        private static void AppendToolbarReport(IdTables t, List<string> sink, string label, IntPtr toolbarHwnd, bool additionalInstances)
        {
            if (toolbarHwnd == IntPtr.Zero || !Native.IsWindow(toolbarHwnd))
            {
                sink.Add(Js.Obj(new List<string>(new string[]
                {
                    Js.P("label", Js.Str(label)),
                    Js.P("found", Js.Bool(false))
                })));
                return;
            }
            IUIAutomationElement toolbarElement = null;
            try { toolbarElement = Program.Current.ElementFromHandle(toolbarHwnd); } catch (Exception) { toolbarElement = null; }
            int comChildren = 0;
            List<NodeRec> buttons = new List<NodeRec>();
            if (toolbarElement != null)
            {
                try
                {
                    IUIAutomationElementArray arr = toolbarElement.FindAll(Probe.TreeScopeChildren, Program.TrueCondition());
                    comChildren = arr.GetLength();
                }
                catch (Exception) { comChildren = -1; }
                buttons = DirectChildrenShapes(Program.Current, toolbarElement, Program.Tables(), 40);
            }
            List<string> buttonRows = new List<string>();
            foreach (NodeRec b in buttons)
            {
                buttonRows.Add(Js.Obj(new List<string>(new string[]
                {
                    Js.P("ct", Js.Str(Probe.ControlTypeName(b.ControlTypeId))),
                    Js.P("automation_id_present_nonempty", Js.Bool(!string.IsNullOrEmpty(b.AutomationIdRaw))),
                    Js.P("pats", Js.StrArr(b.Patterns)),
                    Js.P("bounds_positive_area", Js.Bool(b.BoundsPositive))
                })));
            }
            // Stability: re-read once in the same session and compare raw values
            // in memory. Only booleans cross the boundary.
            List<string> stableRows = new List<string>();
            if (buttons.Count > 0)
            {
                List<NodeRec> again = DirectChildrenShapes(Program.Current, toolbarElement, Program.Tables(), 40);
                int n = Math.Min(buttons.Count, again.Count);
                for (int i = 0; i < n; i++)
                {
                    stableRows.Add(Js.Obj(new List<string>(new string[]
                    {
                        Js.P("index", Js.Int(i)),
                        Js.P("automation_id_value_unchanged", Js.Bool(string.Equals(buttons[i].AutomationIdRaw, again[i].AutomationIdRaw, StringComparison.Ordinal))),
                        Js.P("control_type_unchanged", Js.Bool(buttons[i].ControlTypeId == again[i].ControlTypeId)),
                        Js.P("pattern_set_size_unchanged", Js.Bool(buttons[i].Patterns.Count == again[i].Patterns.Count)),
                        Js.P("bounds_positive_unchanged", Js.Bool(buttons[i].BoundsPositive == again[i].BoundsPositive))
                    })));
                }
            }
            sink.Add(Js.Obj(new List<string>(new string[]
            {
                Js.P("label", Js.Str(label)),
                Js.P("found", Js.Bool(true)),
                Js.P("additional_unreported_instances", Js.Bool(additionalInstances)),
                Js.P("nativewindowhandle", Js.Int(toolbarHwnd.ToInt64())),
                Js.P("com_direct_children", Js.Int(comChildren)),
                Js.P("button_shapes_recorded", Js.Int(buttons.Count)),
                Js.P("buttons", Js.Arr(buttonRows)),
                Js.P("stability_reread", Js.Arr(stableRows))
            })));
        }

        private static List<NodeRec> DirectChildrenShapes(IUIAutomation uia, IUIAutomationElement parent, IdTables t, int cap)
        {
            List<NodeRec> result = new List<NodeRec>();
            IUIAutomationTreeWalker walker = uia.GetControlViewWalker();
            IUIAutomationElement child = null;
            try { child = walker.GetFirstChildElement(parent); } catch (Exception) { child = null; }
            while (child != null && result.Count < cap)
            {
                result.Add(Probe.ReadNode(child, t, result.Count, 1, 0));
                IUIAutomationElement next = null;
                try { next = walker.GetNextSiblingElement(child); } catch (Exception) { next = null; }
                child = next;
            }
            return result;
        }

        public static string RunInvokeByAid(IUIAutomation uia, IdTables t, string hwndArg, string automationId)
        {
            IntPtr h = Args.ParseHandle(hwndArg);
            if (h == IntPtr.Zero || !Native.IsWindow(h)) { throw new InvalidOperationException("invokebyaid requires a live --hwnd"); }
            IUIAutomationElement root = uia.ElementFromHandle(h);
            IUIAutomationTreeWalker walker = uia.GetControlViewWalker();
            IUIAutomationElement found = null;
            Stack<IUIAutomationElement> pending = new Stack<IUIAutomationElement>();
            pending.Push(root);
            int visited = 0;
            while (pending.Count > 0 && found == null && visited < 600)
            {
                IUIAutomationElement node = pending.Pop();
                visited++;
                string aid = null;
                try { aid = node.GetCurrentAutomationId(); } catch (Exception) { aid = null; }
                if (aid != null && string.Equals(aid, automationId, StringComparison.Ordinal)) { found = node; break; }
                try
                {
                    IUIAutomationElement last = walker.GetLastChildElement(node);
                    while (last != null) { pending.Push(last); last = walker.GetPreviousSiblingElement(last); }
                }
                catch (Exception) { }
            }
            bool invoked = false;
            if (found != null)
            {
                try
                {
                    int invokeId = t.PatternIdByName.ContainsKey("Invoke") ? t.PatternIdByName["Invoke"] : 10000;
                    object pattern = found.GetCurrentPattern(invokeId);
                    IUIAutomationInvokePattern invoke = pattern as IUIAutomationInvokePattern;
                    if (invoke != null) { invoke.Invoke(); invoked = true; }
                }
                catch (Exception) { invoked = false; }
            }
            List<string> o = new List<string>();
            o.Add(Js.P("target_found", Js.Bool(found != null)));
            o.Add(Js.P("invoked", Js.Bool(invoked)));
            o.Add(Js.P("nodes_visited", Js.Int(visited)));
            return Js.Obj(o);
        }

        public static string RunFrameWalk(IUIAutomation uia, IdTables t, string frameArg)
        {
            IntPtr frame = Args.ParseHandle(frameArg);
            if (frame == IntPtr.Zero || !Native.IsWindow(frame)) { throw new InvalidOperationException("framewalk requires a live --hwnd"); }
            string frameClass = Native.ClassOf(frame);
            int framePid = Native.PidOf(frame);
            List<KeyValuePair<string, bool>> childRows = new List<KeyValuePair<string, bool>>();
            foreach (IntPtr c in Native.ChildrenOf(frame))
            {
                string cls = Native.ClassOf(c);
                int pid = Native.PidOf(c);
                childRows.Add(new KeyValuePair<string, bool>(cls.Length > 0 ? cls : "<unreadable>", pid == framePid));
            }
            // Aggregate per class so no pid is ever implied by position.
            SortedDictionary<string, int[]> byClass = new SortedDictionary<string, int[]>(StringComparer.Ordinal);
            foreach (KeyValuePair<string, bool> row in childRows)
            {
                int[] counts;
                if (!byClass.TryGetValue(row.Key, out counts)) { counts = new int[] { 0, 0 }; byClass[row.Key] = counts; }
                counts[0]++;
                if (row.Value) { counts[1]++; }
            }
            List<string> classRows = new List<string>();
            foreach (KeyValuePair<string, int[]> kv in byClass)
            {
                classRows.Add(Js.Obj(new List<string>(new string[]
                {
                    Js.P("class", Js.Str(kv.Key)),
                    Js.P("instances", Js.Int(kv.Value[0])),
                    Js.P("owner_pid_equals_frame_pid_count", Js.Int(kv.Value[1])),
                    Js.P("any_owner_differs_from_frame", Js.Bool(kv.Value[1] < kv.Value[0]))
                })));
            }
            List<string> o = new List<string>();
            o.Add(Js.P("mode", Js.Str("framewalk")));
            o.Add(Js.P("client_stack", Js.Str("n/a")));
            o.Add(Js.P("frame_class", Js.Str(frameClass)));
            o.Add(Js.P("frame_class_is_application_frame_window", Js.Bool(frameClass == "ApplicationFrameWindow")));
            o.Add(Js.P("child_instances_by_class", Js.Arr(classRows)));
            o.Add(Js.P("distinct_child_classes", Js.Int(byClass.Count)));
            return Js.Obj(o);
        }

        public static string RunForegroundInfo(IUIAutomation uia, IdTables t)
        {
            IntPtr fg = Native.GetForegroundWindow();
            List<string> o = new List<string>();
            if (fg == IntPtr.Zero)
            {
                o.Add(Js.P("foreground_present", Js.Bool(false)));
                return Js.Obj(o);
            }
            o.Add(Js.P("foreground_present", Js.Bool(true)));
            o.Add(Js.P("foreground_class", Js.Str(Native.ClassOf(fg))));
            o.Add(Js.P("foreground_host_token", Js.Str(Native.HostTokenOf(fg))));
            o.Add(Js.P("nativewindowhandle", Js.Int(fg.ToInt64())));
            o.Add(Js.P("foreground_tool_window", Js.Bool(Native.IsToolWindow(fg))));
            o.Add(Js.P("foreground_cloak_state", Js.Str(Probe.CloakWord(Native.CloakState(fg)))));
            return Js.Obj(o);
        }

        public static string RunActivate(IUIAutomation uia, IdTables t, string hwndArg)
        {            IntPtr h = Args.ParseHandle(hwndArg);
            if (h == IntPtr.Zero || !Native.IsWindow(h)) { throw new InvalidOperationException("activate requires a live --hwnd"); }
            List<string> o = new List<string>();
            // ShowWindow(SW_SHOW=5) then SetForegroundWindow; no SendInput here,
            // so the foreground-lock heuristic may refuse - the caller records
            // whether activation actually landed via a foreground re-read.
            bool shown = Native.ShowWindowMode(h, 5);
            bool activated = Native.SetForegroundWindow(h);
            o.Add(Js.P("show_window_ok", Js.Bool(shown)));
            o.Add(Js.P("set_foreground_window_accepted", Js.Bool(activated)));
            return Js.Obj(o);
        }

        public static string RunFindByClass(IUIAutomation uia, IdTables t, string className)
        {
            List<long> handles = new List<long>();
            foreach (IntPtr h in Native.AllTopLevelWindows())
            {
                if (string.Equals(Native.ClassOf(h), className, StringComparison.Ordinal) && !handles.Contains(h.ToInt64()))
                {
                    handles.Add(h.ToInt64());
                }
            }
            IntPtr viaFindWindow = Native.FindWindowW(className, null);
            if (viaFindWindow != IntPtr.Zero && !handles.Contains(viaFindWindow.ToInt64())) { handles.Add(viaFindWindow.ToInt64()); }
            List<string> parts = new List<string>();
            foreach (long v in handles) { parts.Add(Js.Int(v)); }
            List<string> o = new List<string>();
            o.Add(Js.P("class_name", Js.Str(className)));
            o.Add(Js.P("match_count", Js.Int(handles.Count)));
            o.Add(Js.P("handles", Js.Arr(parts)));
            return Js.Obj(o);
        }

        public static string RunCloseWindow(IUIAutomation uia, IdTables t, string hwndArg)
        {
            IntPtr h = Args.ParseHandle(hwndArg);
            List<string> o = new List<string>();
            if (h == IntPtr.Zero || !Native.IsWindow(h))
            {
                o.Add(Js.P("window_existed", Js.Bool(false)));
                return Js.Obj(o);
            }
            bool posted = Native.PostMessageW(h, 0x0010, IntPtr.Zero, IntPtr.Zero);
            o.Add(Js.P("window_existed", Js.Bool(true)));
            o.Add(Js.P("wm_close_posted", Js.Bool(posted)));
            return Js.Obj(o);
        }

        public static string RunSurfaceRootIds(IUIAutomation uia, IdTables t, string hwndArg)
        {
            IntPtr h = Args.ParseHandle(hwndArg);
            if (h == IntPtr.Zero || !Native.IsWindow(h)) { throw new InvalidOperationException("surfacerootids requires a live --hwnd"); }
            IUIAutomationElement root = uia.ElementFromHandle(h);
            string rootAid = "";
            int ct = 0;
            string cls = "";
            try { rootAid = root.GetCurrentAutomationId() ?? ""; } catch (Exception) { }
            try { ct = root.GetCurrentControlType(); } catch (Exception) { }
            try { cls = root.GetCurrentClassName() ?? ""; } catch (Exception) { }
            SortedDictionary<string, bool> childTags = new SortedDictionary<string, bool>(StringComparer.Ordinal);
            IUIAutomationTreeWalker walker = uia.GetControlViewWalker();
            IUIAutomationElement child = null;
            try { child = walker.GetFirstChildElement(root); } catch (Exception) { child = null; }
            int visitedChildren = 0;
            while (child != null && visitedChildren < 40)
            {
                visitedChildren++;
                try
                {
                    string aid = child.GetCurrentAutomationId();
                    childTags[Tags.AutomationId(aid)] = true;
                }
                catch (Exception) { }
                IUIAutomationElement next = null;
                try { next = walker.GetNextSiblingElement(child); } catch (Exception) { next = null; }
                child = next;
            }
            List<string> tags = new List<string>(childTags.Keys);
            List<string> o = new List<string>();
            o.Add(Js.P("mode", Js.Str("surfacerootids")));
            o.Add(Js.P("client_stack", Js.Str("uia3-com")));
            o.Add(Js.P("binding", Js.Str(Program.Coclass)));
            o.Add(Js.P("root_automation_id_tag", Js.Str(Tags.AutomationId(rootAid))));
            o.Add(Js.P("root_control_type", Js.Str(Probe.ControlTypeName(ct))));
            o.Add(Js.P("root_class", Js.Str(cls)));
            o.Add(Js.P("direct_children_visited", Js.Int(visitedChildren)));
            o.Add(Js.P("direct_children_automation_id_tags", Js.StrArr(tags)));
            return Js.Obj(o);
        }

        private sealed class Stats
        {
            public int Count;
            public double Warmup;
            public double Min;
            public double Median;
            public double Max;
        }

        // Samples INCLUDING the warm-up: index 0 is the discarded warm-up run.
        private static Stats MakeStats(List<double> samplesIncludingWarmup)
        {
            Stats s = new Stats();
            if (samplesIncludingWarmup.Count == 0)
            {
                s.Warmup = -1; s.Min = -1; s.Median = -1; s.Max = -1;
                return s;
            }
            double warmup = samplesIncludingWarmup[0];
            List<double> timed = new List<double>();
            for (int i = 1; i < samplesIncludingWarmup.Count; i++) { timed.Add(samplesIncludingWarmup[i]); }
            if (timed.Count == 0) { timed.Add(warmup); }
            timed.Sort();
            s.Count = timed.Count;
            s.Warmup = Math.Round(warmup, 1);
            s.Min = Math.Round(timed[0], 1);
            s.Median = Math.Round(timed[timed.Count / 2], 1);
            s.Max = Math.Round(timed[timed.Count - 1], 1);
            return s;
        }

        // Light per-poll probe of the UIA root's direct children: returns the
        // handle of any uncloaked shell-owned CoreWindow (kind unchecked),
        // read by walking the control-view first level directly.
        private static IntPtr QuickUncloakedShellCoreWindow(IUIAutomation uia)
        {
            IUIAutomationElement root = uia.GetRootElement();
            IUIAutomationTreeWalker walker = uia.GetControlViewWalker();
            IUIAutomationElement child = null;
            try { child = walker.GetFirstChildElement(root); } catch (Exception) { child = null; }
            while (child != null)
            {
                string cls = "";
                try { cls = child.GetCurrentClassName() ?? ""; } catch (Exception) { }
                if (cls == AcCoreClass)
                {
                    IntPtr h = IntPtr.Zero;
                    try { h = child.GetCurrentNativeWindowHandle(); } catch (Exception) { }
                    if (h != IntPtr.Zero && Native.CloakState(h) == 0 && Native.HostTokenOf(h) == "shell_experience_host")
                    {
                        return h;
                    }
                }
                IUIAutomationElement next = null;
                try { next = walker.GetNextSiblingElement(child); } catch (Exception) { next = null; }
                child = next;
            }
            return IntPtr.Zero;
        }

        private static bool HasLandmarkUnder(IUIAutomation uia, IdTables t, IntPtr h, string wantedAutomationId, int maxNodes, int maxDepth)
        {
            try
            {
                IUIAutomationElement root = uia.ElementFromHandle(h);
                foreach (NodeRec n in Probe.CollectFlat(uia, root, t, maxNodes, maxDepth))
                {
                    string tag = Tags.AutomationId(n.AutomationIdRaw);
                    if (tag == wantedAutomationId) { return true; }
                    // The empty Action Center swaps MainListView for the
                    // quick-actions pane; either landmark identifies it.
                    if (wantedAutomationId == "MainListView" &&
                        (tag.StartsWith("Microsoft.QuickAction.", StringComparison.Ordinal) ||
                         tag == "NoNotificationsTextBlock"))
                    {
                        return true;
                    }
                }
            }
            catch (Exception) { }
            return false;
        }

        private static void SendCloseSequence(IUIAutomation uia)
        {
            Native.SendSequence(Native.KeyChord(0, Native.VkEscape));
            System.Threading.Thread.Sleep(500);
            if (QuickUncloakedShellCoreWindow(uia) != IntPtr.Zero)
            {
                System.Threading.Thread.Sleep(400);
            }
            if (QuickUncloakedShellCoreWindow(uia) != IntPtr.Zero)
            {
                // ESC did not dismiss it; the accelerator itself is the toggle
                // on this build. Re-checked first so a working ESC cannot
                // re-open what it just closed.
                Native.SendSequence(Native.KeyChord(Native.VkLWin, Native.VkA));
                System.Threading.Thread.Sleep(500);
            }
        }

        // One raise-observe-close cycle. Detection is the reach mechanism the
        // resolver will use - UIA root children matched to an uncloaked shell-
        // owned CoreWindow - verified by the Action Center's MainListView
        // landmark so another open immersive surface cannot stand in. Returns
        // -1 for a detection deadline miss.
        private static void OneCycle(IUIAutomation uia, IdTables t, double detectDeadlineMs, int pollMs, out double openDetectMs, out double closeDetectMs)
        {
            Native.SendSequence(Native.KeyChord(Native.VkLWin, Native.VkA));
            Stopwatch swOpen = Stopwatch.StartNew();
            IntPtr acHwnd = IntPtr.Zero;
            while (swOpen.Elapsed.TotalMilliseconds < detectDeadlineMs)
            {
                IntPtr raw = QuickUncloakedShellCoreWindow(uia);
                if (raw != IntPtr.Zero && HasLandmarkUnder(uia, t, raw, "MainListView", 200, 16))
                {
                    acHwnd = raw;
                    break;
                }
                System.Threading.Thread.Sleep(pollMs);
            }
            swOpen.Stop();
            openDetectMs = acHwnd != IntPtr.Zero ? swOpen.Elapsed.TotalMilliseconds : -1;

            if (acHwnd != IntPtr.Zero)
            {
                SendCloseSequence(uia);
                Stopwatch swClose = Stopwatch.StartNew();
                bool closedSeen = false;
                while (swClose.Elapsed.TotalMilliseconds < detectDeadlineMs)
                {
                    if (QuickUncloakedShellCoreWindow(uia) == IntPtr.Zero) { closedSeen = true; break; }
                    System.Threading.Thread.Sleep(pollMs);
                }
                swClose.Stop();
                closeDetectMs = closedSeen ? swClose.Elapsed.TotalMilliseconds : -1;
            }
            else
            {
                closeDetectMs = -1;
            }
        }

        public static string RunCost(IUIAutomation uia, IdTables t, int cycles)
        {
            IntPtr taskbar = Native.FindTopLevelByClass("Shell_TrayWnd");
            List<double> raiseSamples = new List<double>();
            List<double> closeSamples = new List<double>();
            double warmOpen = 0;
            double warmClose = 0;
            OneCycle(uia, t, 4000, 25, out warmOpen, out warmClose);
            System.Threading.Thread.Sleep(300);

            // Tray subtree enumeration, warm-up plus seven timed reps.
            List<double> traySamples = new List<double>();
            if (taskbar != IntPtr.Zero)
            {
                IUIAutomationElement tb = uia.ElementFromHandle(taskbar);
                for (int r = 0; r <= 7; r++)
                {
                    Stopwatch sw = Stopwatch.StartNew();
                    IUIAutomationTreeWalker walker = uia.GetRawViewWalker();
                    int seen = 0;
                    Stack<IUIAutomationElement> pending = new Stack<IUIAutomationElement>();
                    pending.Push(tb);
                    while (pending.Count > 0 && seen < 500)
                    {
                        IUIAutomationElement node = pending.Pop();
                        seen++;
                        try
                        {
                            IUIAutomationElement last = walker.GetLastChildElement(node);
                            while (last != null) { pending.Push(last); last = walker.GetPreviousSiblingElement(last); }
                        }
                        catch (Exception) { }
                    }
                    sw.Stop();
                    traySamples.Add(sw.Elapsed.TotalMilliseconds);
                }
            }

            // UIA-root-children resolution, warm-up plus seven timed reps.
            List<double> rootSamples = new List<double>();
            for (int r = 0; r <= 7; r++)
            {
                Stopwatch sw = Stopwatch.StartNew();
                Dictionary<int, IUIAutomationElement> kids = Probe.RootChildrenByIndex(uia);
                foreach (KeyValuePair<int, IUIAutomationElement> kv in kids)
                {
                    try { GC.KeepAlive(kv.Value.GetCurrentNativeWindowHandle()); } catch (Exception) { }
                    try { GC.KeepAlive(kv.Value.GetCurrentControlType()); } catch (Exception) { }
                }
                sw.Stop();
                rootSamples.Add(sw.Elapsed.TotalMilliseconds);
            }

            // Raise/close cycles: one discarded warm-up then seven timed runs.
            int openFailures = 0;
            int closeFailures = 0;
            for (int c = 0; c < cycles; c++)
            {
                double od;
                double cd;
                OneCycle(uia, t, 4000, 25, out od, out cd);
                if (od < 0) { openFailures++; }
                if (cd < 0) { closeFailures++; }
                if (od < 0 || cd < 0) { SendCloseSequence(uia); System.Threading.Thread.Sleep(400); continue; }
                raiseSamples.Add(od);
                closeSamples.Add(cd);
            }

            // Action Center tree read: one extra raise kept open for the read,
            // then closed again.
            List<double> actreeSamples = new List<double>();
            int lastReadNodeCount = -1;
            Native.SendSequence(Native.KeyChord(Native.VkLWin, Native.VkA));
            System.Threading.Thread.Sleep(700);
            int candCount = 0;
            IntPtr acForRead = Probe.FindActionCenterCoreWindow(uia, out candCount);
            if (acForRead != IntPtr.Zero && Native.CloakState(acForRead) == 0 && HasLandmarkUnder(uia, t, acForRead, "MainListView", 200, 16))
            {
                IUIAutomationElement acRoot = uia.ElementFromHandle(acForRead);
                for (int r = 0; r <= 7; r++)
                {
                    Stopwatch sw = Stopwatch.StartNew();
                    List<NodeRec> nodes = Probe.CollectFlat(uia, acRoot, t, 400, 24);
                    sw.Stop();
                    actreeSamples.Add(sw.Elapsed.TotalMilliseconds);
                    lastReadNodeCount = nodes.Count;
                }
                Native.SendSequence(Native.KeyChord(0, Native.VkEscape));
            }
            System.Threading.Thread.Sleep(400);

            Stats raiseS = MakeStats(raiseSamples);
            Stats closeS = MakeStats(closeSamples);
            Stats trayS = MakeStats(traySamples);
            Stats rootS = MakeStats(rootSamples);
            Stats acReadS = MakeStats(actreeSamples);

            List<string> o = new List<string>();
            o.Add(Js.P("mode", Js.Str("cost")));
            o.Add(Js.P("client_stack", Js.Str("uia3-com")));
            o.Add(Js.P("binding", Js.Str(Program.Coclass)));
            o.Add(Js.P("cycles_attempted", Js.Int(cycles)));
            o.Add(Js.P("open_detection_failures", Js.Int(openFailures)));
            o.Add(Js.P("close_detection_failures", Js.Int(closeFailures)));
            o.Add(Js.P("raise_open_detect", StatsJson(raiseS)));
            o.Add(Js.P("close_detect", StatsJson(closeS)));
            o.Add(Js.P("tray_subtree_enum", StatsJson(trayS)));
            o.Add(Js.P("root_children_resolution", StatsJson(rootS)));
            o.Add(Js.P("action_center_tree_read", StatsJson(acReadS)));
            o.Add(Js.P("action_center_tree_read_nodes_last_read", Js.Int(lastReadNodeCount)));
            return Js.Obj(o);
        }

        private static string StatsJson(Stats s)
        {
            return Js.Obj(new List<string>(new string[]
            {
                Js.P("warmup_discarded", Js.Bool(true)),
                Js.P("timed_runs", Js.Int(s.Count)),
                Js.P("elapsed_warmup_ms", Js.Num(s.Warmup)),
                Js.P("elapsed_min_ms", Js.Num(s.Min)),
                Js.P("elapsed_median_ms", Js.Num(s.Median)),
                Js.P("elapsed_max_ms", Js.Num(s.Max))
            }));
        }
    }
    public static class Args
    {
        public static string Get(string[] a, string name, string fallback)
        {
            for (int i = 0; i < a.Length - 1; i++)
            {
                if (string.Equals(a[i], name, StringComparison.Ordinal)) { return a[i + 1]; }
            }
            return fallback;
        }

        public static IntPtr ParseHandle(string s)
        {
            if (string.IsNullOrEmpty(s)) { return IntPtr.Zero; }
            string txt = s.Trim();
            long v;
            if (txt.StartsWith("0x", StringComparison.OrdinalIgnoreCase))
            {
                if (long.TryParse(txt.Substring(2), NumberStyles.HexNumber, CultureInfo.InvariantCulture, out v)) { return new IntPtr(v); }
                return IntPtr.Zero;
            }
            if (long.TryParse(txt, NumberStyles.Integer, CultureInfo.InvariantCulture, out v)) { return new IntPtr(v); }
            return IntPtr.Zero;
        }
    }

    public static class Program
    {
        internal static IUIAutomation Current;
        internal static string Coclass { get { return Program.coclass; } }
        private static string coclass = "unknown";
        internal static IdTables Tables()
        { return Program.tables; }
        private static IdTables tables;
        internal static IUIAutomationCondition TrueCondition()
        { return Program.trueCondition; }
        private static IUIAutomationCondition trueCondition;

        public static int Main(string[] args)
        {
            if (args.Length == 0) { Console.Error.WriteLine("usage: <mode> [--options]"); return 2; }
            string clsidUsed;
            Guid cuia8 = new Guid("e22ad333-b25f-460c-83d0-0581107395c9");
            try
            {
                object o = Activator.CreateInstance(Type.GetTypeFromCLSID(cuia8, true));
                Current = (IUIAutomation)o;
                clsidUsed = "CUIAutomation8";
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine("CUIAutomation8 unavailable (" + ex.GetType().Name + ": " + ex.Message + "), falling back to CUIAutomation");
                Guid cuia = new Guid("ff48dba4-60ef-4201-aa87-54103eef594e");
                object o2 = Activator.CreateInstance(Type.GetTypeFromCLSID(cuia, true));
                Current = (IUIAutomation)o2;
                clsidUsed = "CUIAutomation";
            }
            Program.coclass = clsidUsed;
            tables = IdTables.Discover(Current);
            trueCondition = Current.CreateTrueCondition();
            string mode = args[0];
            string result;
            try
            {
                switch (mode)
                {
                    case "reachscan": result = Modes.RunReachScan(Current, tables); break;
                    case "predicate": result = Modes.RunPredicate(Current, tables, Args.Get(args, "--hwnd", "0")); break;
                    case "actree": result = Modes.RunActionCenterTree(Current, tables, Args.Get(args, "--hwnd", "0"), Array.IndexOf(args, "--rescan") >= 0,
                        int.Parse(Args.Get(args, "--maxnodes", "400"), CultureInfo.InvariantCulture),
                        int.Parse(Args.Get(args, "--maxdepth", "24"), CultureInfo.InvariantCulture)); break;
                    case "trayscan": result = Modes.RunTrayScan(Current, tables); break;
                    case "invokebyaid": result = Modes.RunInvokeByAid(Current, tables, Args.Get(args, "--hwnd", "0"), Args.Get(args, "--aid", "")); break;
                    case "framewalk": result = Modes.RunFrameWalk(Current, tables, Args.Get(args, "--frame", "0")); break;
                    case "foregroundinfo": result = Modes.RunForegroundInfo(Current, tables); break;
                    case "surfacerootids": result = Modes.RunSurfaceRootIds(Current, tables, Args.Get(args, "--hwnd", "0")); break;
                    case "activate": result = Modes.RunActivate(Current, tables, Args.Get(args, "--hwnd", "0")); break;
                    case "findbyclass": result = Modes.RunFindByClass(Current, tables, Args.Get(args, "--cls", "")); break;
                    case "closewindow": result = Modes.RunCloseWindow(Current, tables, Args.Get(args, "--hwnd", "0")); break;
                    case "cost": result = Modes.RunCost(Current, tables, int.Parse(Args.Get(args, "--cycles", "7"), CultureInfo.InvariantCulture)); break;
                    case "key": result = RunKey(args); break;
                    default:
                        Console.Error.WriteLine("unknown mode " + mode);
                        return 2;
                }
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine(ex.GetType().FullName + ": " + ex.Message);
                return 1;
            }
            Console.Out.Write(result);
            Console.Out.Flush();
            return 0;
        }

        private static string RunKey(string[] args)
        {
            string seq = Args.Get(args, "--seq", "");
            uint sent;
            if (seq == "lwin_a")
            {
                sent = Native.SendSequence(Native.KeyChord(Native.VkLWin, Native.VkA));
            }
            else if (seq == "lwin")
            {
                sent = Native.SendSequence(Native.KeyChord(0, Native.VkLWin));
            }
            else if (seq == "esc")
            {
                sent = Native.SendSequence(Native.KeyChord(0, Native.VkEscape));
            }
            else if (seq == "ctrl_o")
            {
                sent = Native.SendSequence(Native.KeyChord(Native.VkControl, (ushort)0x4F));
            }
            else if (seq == "return")
            {
                sent = Native.SendSequence(Native.KeyChord(0, Native.VkReturn));
            }
            else if (seq == "type")
            {
                sent = Native.SendSequence(Native.TypeText(Args.Get(args, "--text", "")));
            }
            else
            {
                throw new InvalidOperationException("key requires --seq of lwin_a|lwin|esc|ctrl_o|return|type");
            }
            List<string> o = new List<string>();
            o.Add(Js.P("sequence", Js.Str(seq)));
            o.Add(Js.P("events_reported_sent", Js.Int(sent)));
            return Js.Obj(o);
        }
    }
}
