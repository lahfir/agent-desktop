// agent-desktop windows probe 08: UIA3 COM shim (sub-phase 2.0, unit U7).
//
// Binding mechanism: hand-declared [ComImport] interfaces in full UIAutomationClient.h
// vtable order. The GAC UIAutomationClient assembly (managed System.Windows.Automation)
// is NEVER referenced -- it is a different client stack and cannot observe the six
// UIA3-only patterns the Rust `uiautomation` crate will see.
//
// Compiled by probes/windows/08-uia3-com.ps1 with the in-box pre-Roslyn csc.exe under
// /langversion:5. Emits one compact JSON document on stdout; diagnostics go to stderr.
//
// Modes (stable invocation contract -- U4 consumes `census`):
//   08-uia3-com.exe events --hwnd 0xNNNN [--reps 20]
//   08-uia3-com.exe cache  --hwnd 0xNNNN [--label L] [--max-nodes 1500] [--reps 5]
//   08-uia3-com.exe walker --hwnd 0xNNNN [--label L] [--max-nodes 20000] [--max-depth 60]
//   08-uia3-com.exe census --target LABEL=0xNNNN [--target ...] [--max-nodes 400] [--max-depth 30]

using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.RegularExpressions;
using System.Threading;

namespace AgentDesktop.Probe.Uia3
{
    [StructLayout(LayoutKind.Sequential)]
    public struct UiaPoint { public int X; public int Y; }

    [StructLayout(LayoutKind.Sequential)]
    public struct UiaRect { public int Left; public int Top; public int Right; public int Bottom; }

    [ComImport, Guid("352ffba8-0973-437c-a61f-f64cafd81df9"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IUIAutomationCondition
    {
    }

    [ComImport, Guid("14314595-b4bc-4055-95f2-58f2e42c9855"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IUIAutomationElementArray
    {
        int GetLength();
        IUIAutomationElement GetElement(int index);
    }

    [ComImport, Guid("b32a92b5-bc25-4078-9c08-d7ee95c48e03"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IUIAutomationCacheRequest
    {
        void AddProperty(int propertyId);
        void AddPattern(int patternId);
        IUIAutomationCacheRequest Clone();
        int GetTreeScope();
        void SetTreeScope(int scope);
        IUIAutomationCondition GetTreeFilter();
        void SetTreeFilter(IUIAutomationCondition filter);
        int GetAutomationElementMode();
        void SetAutomationElementMode(int mode);
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
        IUIAutomationElementArray FindAllBuildCache(int scope, IUIAutomationCondition condition, IUIAutomationCacheRequest cacheRequest);
        IUIAutomationElement BuildUpdatedCache(IUIAutomationCacheRequest cacheRequest);
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

    [ComImport, Guid("146c3c17-f12e-4e22-8c27-f894b9b79c69"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IUIAutomationEventHandler
    {
        void HandleAutomationEvent(IUIAutomationElement sender, int eventId);
    }

    [ComImport, Guid("40cd37d4-c756-4b0c-8c6f-bddfeeb13b50"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IUIAutomationPropertyChangedEventHandler
    {
        void HandlePropertyChangedEvent(IUIAutomationElement sender, int propertyId, [MarshalAs(UnmanagedType.Struct)] object newValue);
    }

    [ComImport, Guid("c270f6b5-5c69-4290-9745-7a7f97169468"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IUIAutomationFocusChangedEventHandler
    {
        void HandleFocusChangedEvent(IUIAutomationElement sender);
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
        IUIAutomationCacheRequest CreateCacheRequest();
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
        void AddAutomationEventHandler(int eventId, IUIAutomationElement element, int scope, IUIAutomationCacheRequest cacheRequest, IUIAutomationEventHandler handler);
        void RemoveAutomationEventHandler(int eventId, IUIAutomationElement element, IUIAutomationEventHandler handler);
        void Slot32AddPropertyChangedNativeArray();
        void AddPropertyChangedEventHandler(IUIAutomationElement element, int scope, IUIAutomationCacheRequest cacheRequest, IUIAutomationPropertyChangedEventHandler handler, [MarshalAs(UnmanagedType.SafeArray, SafeArraySubType = VarEnum.VT_I4)] int[] propertyArray);
        void RemovePropertyChangedEventHandler(IUIAutomationElement element, IUIAutomationPropertyChangedEventHandler handler);
        void Slot35AddStructureChanged();
        void Slot36RemoveStructureChanged();
        void AddFocusChangedEventHandler(IUIAutomationCacheRequest cacheRequest, IUIAutomationFocusChangedEventHandler handler);
        void RemoveFocusChangedEventHandler(IUIAutomationFocusChangedEventHandler handler);
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
        [DllImport("user32.dll", SetLastError = true)]
        public static extern IntPtr GetDlgItem(IntPtr hDlg, int nIDDlgItem);
        [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode, EntryPoint = "SendMessageW")]
        public static extern IntPtr SendMessageText(IntPtr hWnd, uint msg, IntPtr wParam, string lParam);
        [DllImport("user32.dll", SetLastError = true, EntryPoint = "SendMessageW")]
        public static extern IntPtr SendMessage(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);
        [DllImport("user32.dll", SetLastError = true)]
        public static extern bool PostMessage(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);
        [DllImport("user32.dll", SetLastError = true)]
        public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
        [DllImport("user32.dll", SetLastError = true)]
        public static extern bool AttachThreadInput(uint attachTo, uint attachFrom, bool attach);
        [DllImport("user32.dll", SetLastError = true)]
        public static extern IntPtr SetFocus(IntPtr hWnd);
        [DllImport("user32.dll")]
        public static extern bool ShowWindow(IntPtr hWnd, int cmd);
        [DllImport("user32.dll")]
        public static extern bool IsWindow(IntPtr hWnd);
        [DllImport("kernel32.dll")]
        public static extern uint GetCurrentThreadId();
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

        public static string Num(double v) { return v.ToString("0.####", CultureInfo.InvariantCulture); }
        public static string Int(long v) { return v.ToString(CultureInfo.InvariantCulture); }
        public static string Bool(bool v) { return v ? "true" : "false"; }
        public static string P(string key, string rawValue) { return Str(key) + ":" + rawValue; }
        public static string Obj(List<string> parts) { return "{" + string.Join(",", parts.ToArray()) + "}"; }
        public static string Arr(List<string> parts) { return "[" + string.Join(",", parts.ToArray()) + "]"; }
    }

    public sealed class EventRecord
    {
        public int Sequence;
        public int Repetition;
        public string Kind;
        public int EventId;
        public int PropertyId;
        public int ThreadId;
        public double OffsetMs;
        public string SenderAutomationId;
        public int SenderControlTypeId;
        public bool SenderIsTarget;
        public string NewValue;
    }

    public sealed class EventLog
    {
        private readonly List<EventRecord> records = new List<EventRecord>();
        private readonly object gate = new object();
        private readonly Stopwatch clock = Stopwatch.StartNew();
        public int Repetition;
        public int TargetProcessId;
        public bool ReadSenderProperties = true;

        public void Add(string kind, int eventId, int propertyId, IUIAutomationElement sender, string newValue)
        {
            EventRecord r = new EventRecord();
            r.Kind = kind;
            r.EventId = eventId;
            r.PropertyId = propertyId;
            r.ThreadId = (int)Native.GetCurrentThreadId();
            r.NewValue = newValue;
            r.SenderControlTypeId = -1;
            r.SenderAutomationId = "<unreadable>";
            try
            {
                if (sender != null && ReadSenderProperties)
                {
                    r.SenderAutomationId = sender.GetCurrentAutomationId();
                    r.SenderControlTypeId = sender.GetCurrentControlType();
                    r.SenderIsTarget = sender.GetCurrentProcessId() == TargetProcessId;
                }
            }
            catch (Exception ex) { r.SenderAutomationId = "<error:" + ex.GetType().Name + ">"; }
            lock (gate)
            {
                r.Repetition = Repetition;
                r.OffsetMs = clock.Elapsed.TotalMilliseconds;
                r.Sequence = records.Count;
                records.Add(r);
            }
        }

        public List<EventRecord> Snapshot()
        {
            lock (gate) { return new List<EventRecord>(records); }
        }

        public int Count { get { lock (gate) { return records.Count; } } }
    }

    public sealed class AutomationSink : IUIAutomationEventHandler
    {
        private readonly EventLog log;
        public AutomationSink(EventLog l) { log = l; }
        public void HandleAutomationEvent(IUIAutomationElement sender, int eventId)
        {
            log.Add("AutomationEvent", eventId, 0, sender, null);
        }
    }

    public sealed class PropertySink : IUIAutomationPropertyChangedEventHandler
    {
        private readonly EventLog log;
        public PropertySink(EventLog l) { log = l; }
        public void HandlePropertyChangedEvent(IUIAutomationElement sender, int propertyId, object newValue)
        {
            string v = null;
            try { if (newValue != null) { v = Convert.ToString(newValue, CultureInfo.InvariantCulture); } }
            catch (Exception) { v = "<unconvertible>"; }
            if (v != null && v.Length > 64) { v = v.Substring(0, 64); }
            log.Add("PropertyChanged", 0, propertyId, sender, v);
        }
    }

    public sealed class FocusSink : IUIAutomationFocusChangedEventHandler
    {
        private readonly EventLog log;
        public FocusSink(EventLog l) { log = l; }
        public void HandleFocusChangedEvent(IUIAutomationElement sender)
        {
            log.Add("FocusChanged", 0, 0, sender, null);
        }
    }

    public static class Uia
    {
        public const int TreeScopeElement = 1;
        public const int TreeScopeChildren = 2;
        public const int TreeScopeSubtree = 7;

        public const int EventToolTipOpened = 20000;
        public const int EventStructureChanged = 20002;
        public const int EventMenuOpened = 20003;
        public const int EventInvoked = 20009;
        public const int EventElementSelected = 20012;
        public const int EventTextChanged = 20015;
        public const int EventWindowOpened = 20016;
        public const int EventWindowClosed = 20017;

        private static readonly int[] ControlTypeIds = new int[]
        {
            50000, 50001, 50002, 50003, 50004, 50005, 50006, 50007, 50008, 50009,
            50010, 50011, 50012, 50013, 50014, 50015, 50016, 50017, 50018, 50019,
            50020, 50021, 50022, 50023, 50024, 50025, 50026, 50027, 50028, 50029,
            50030, 50031, 50032, 50033, 50034, 50035, 50036, 50037, 50038, 50039, 50040
        };

        private static readonly string[] ControlTypeNames = new string[]
        {
            "Button", "Calendar", "CheckBox", "ComboBox", "Edit", "Hyperlink", "Image", "ListItem", "List", "Menu",
            "MenuBar", "MenuItem", "ProgressBar", "RadioButton", "ScrollBar", "Slider", "Spinner", "StatusBar", "Tab", "TabItem",
            "Text", "ToolBar", "ToolTip", "Tree", "TreeItem", "Custom", "Group", "Thumb", "DataGrid", "DataItem",
            "Document", "SplitButton", "Window", "Pane", "Header", "HeaderItem", "Table", "TitleBar", "Separator", "SemanticZoom", "AppBar"
        };

        public static string ControlTypeName(int id)
        {
            for (int i = 0; i < ControlTypeIds.Length; i++)
            {
                if (ControlTypeIds[i] == id) { return ControlTypeNames[i]; }
            }
            return "Unknown";
        }

        public static string EventName(int id)
        {
            switch (id)
            {
                case 20000: return "ToolTipOpened";
                case 20002: return "StructureChanged";
                case 20003: return "MenuOpened";
                case 20009: return "Invoke_Invoked";
                case 20012: return "SelectionItem_ElementSelected";
                case 20015: return "Text_TextChanged";
                case 20016: return "Window_WindowOpened";
                case 20017: return "Window_WindowClosed";
                default: return "Event" + id.ToString(CultureInfo.InvariantCulture);
            }
        }

        public static string CleanProviderDescription(string d)
        {
            if (string.IsNullOrEmpty(d)) { return d; }
            string s = Regex.Replace(d, "pid:\\d+", "pid:<pid>");
            s = Regex.Replace(s, "providerId:0x[0-9A-Fa-f]+", "providerId:<providerid>");
            s = Regex.Replace(s, "hwnd:0x[0-9A-Fa-f]+", "hwnd:<hwnd>");
            if (s.Length > 240) { s = s.Substring(0, 240) + "..."; }
            return s;
        }
    }

    // Runtime-discovered UIA property and pattern identifier tables. Nothing here is
    // hardcoded from memory: GetPropertyProgrammaticName / GetPatternProgrammaticName
    // are asked what each numeric id means, so a wrong constant is impossible.
    public sealed class IdTables
    {
        public readonly Dictionary<string, int> PropertyByName = new Dictionary<string, int>(StringComparer.OrdinalIgnoreCase);
        public readonly Dictionary<int, string> PropertyById = new Dictionary<int, string>();
        public readonly List<string> PatternNames = new List<string>();
        public readonly Dictionary<string, int> PatternAvailabilityProperty = new Dictionary<string, int>(StringComparer.Ordinal);
        public readonly Dictionary<string, int> PatternIdByName = new Dictionary<string, int>(StringComparer.Ordinal);

        public static IdTables Discover(IUIAutomation uia)
        {
            IdTables t = new IdTables();
            for (int id = 30000; id <= 30300; id++)
            {
                string name = null;
                try { name = uia.GetPropertyProgrammaticName(id); }
                catch (Exception) { continue; }
                if (string.IsNullOrEmpty(name)) { continue; }
                t.PropertyById[id] = name;
                if (!t.PropertyByName.ContainsKey(name)) { t.PropertyByName[name] = id; }
                Match m = Regex.Match(name, "^Is(?<p>.+?)Pattern(?<n>\\d?)Available$");
                if (m.Success) { t.PatternAvailabilityProperty[m.Groups["p"].Value + m.Groups["n"].Value] = id; }
            }
            for (int id = 10000; id <= 10060; id++)
            {
                string name = null;
                try { name = uia.GetPatternProgrammaticName(id); }
                catch (Exception) { continue; }
                if (string.IsNullOrEmpty(name)) { continue; }
                t.PatternNames.Add(id.ToString(CultureInfo.InvariantCulture) + "=" + name);
                string bare = Regex.Replace(name, "Pattern$", "");
                if (!t.PatternIdByName.ContainsKey(bare)) { t.PatternIdByName[bare] = id; }
            }
            return t;
        }

        public int Prop(string programmaticName)
        {
            int id;
            if (PropertyByName.TryGetValue(programmaticName, out id)) { return id; }
            return -1;
        }
    }

    public sealed class NodeInfo
    {
        public int Depth;
        public int ControlTypeId;
        public string AutomationId;
        public string ClassName;
        public int NameLength;
        public string FrameworkId;
        public string ProviderDescription;
        public bool InControlView;
        public List<string> Patterns = new List<string>();
        public string RuntimeId;
    }

    public static class Tree
    {
        public static string RuntimeIdOf(IUIAutomationElement e)
        {
            try
            {
                int[] rid = e.GetRuntimeId();
                if (rid == null) { return ""; }
                StringBuilder sb = new StringBuilder();
                for (int i = 0; i < rid.Length; i++)
                {
                    if (i > 0) { sb.Append('.'); }
                    sb.Append(rid[i].ToString(CultureInfo.InvariantCulture));
                }
                return sb.ToString();
            }
            catch (Exception) { return ""; }
        }

        public static int CountNodes(IUIAutomationTreeWalker walker, IUIAutomationElement root, int maxNodes, int maxDepth, out bool truncated, out int deepest)
        {
            int count = 0;
            int deep = 0;
            bool trunc = false;
            Stack<KeyValuePair<IUIAutomationElement, int>> stack = new Stack<KeyValuePair<IUIAutomationElement, int>>();
            stack.Push(new KeyValuePair<IUIAutomationElement, int>(root, 0));
            while (stack.Count > 0)
            {
                KeyValuePair<IUIAutomationElement, int> item = stack.Pop();
                count++;
                if (item.Value > deep) { deep = item.Value; }
                if (count >= maxNodes) { trunc = true; break; }
                if (item.Value >= maxDepth) { trunc = true; continue; }
                IUIAutomationElement child = null;
                try { child = walker.GetFirstChildElement(item.Key); }
                catch (Exception) { child = null; }
                while (child != null)
                {
                    stack.Push(new KeyValuePair<IUIAutomationElement, int>(child, item.Value + 1));
                    IUIAutomationElement next = null;
                    try { next = walker.GetNextSiblingElement(child); }
                    catch (Exception) { next = null; }
                    child = next;
                }
            }
            truncated = trunc;
            deepest = deep;
            return count;
        }

        public static void Collect(IUIAutomationTreeWalker walker, IUIAutomationElement root, int depth, int maxDepth,
            List<IUIAutomationElement> sink, List<int> depths, int maxNodes)
        {
            if (sink.Count >= maxNodes) { return; }
            sink.Add(root);
            depths.Add(depth);
            if (depth >= maxDepth) { return; }
            IUIAutomationElement child = null;
            try { child = walker.GetFirstChildElement(root); }
            catch (Exception) { return; }
            while (child != null && sink.Count < maxNodes)
            {
                Collect(walker, child, depth + 1, maxDepth, sink, depths, maxNodes);
                IUIAutomationElement next = null;
                try { next = walker.GetNextSiblingElement(child); }
                catch (Exception) { next = null; }
                child = next;
            }
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

        public static List<string> GetAll(string[] a, string name)
        {
            List<string> r = new List<string>();
            for (int i = 0; i < a.Length - 1; i++)
            {
                if (string.Equals(a[i], name, StringComparison.Ordinal)) { r.Add(a[i + 1]); }
            }
            return r;
        }

        public static int GetInt(string[] a, string name, int fallback)
        {
            string s = Get(a, name, null);
            int v;
            if (s != null && int.TryParse(s, NumberStyles.Integer, CultureInfo.InvariantCulture, out v)) { return v; }
            return fallback;
        }

        public static IntPtr ParseHandle(string s)
        {
            if (string.IsNullOrEmpty(s)) { return IntPtr.Zero; }
            string t = s.Trim();
            long v;
            if (t.StartsWith("0x", StringComparison.OrdinalIgnoreCase))
            {
                if (long.TryParse(t.Substring(2), NumberStyles.HexNumber, CultureInfo.InvariantCulture, out v)) { return new IntPtr(v); }
                return IntPtr.Zero;
            }
            if (long.TryParse(t, NumberStyles.Integer, CultureInfo.InvariantCulture, out v)) { return new IntPtr(v); }
            return IntPtr.Zero;
        }

        public static string Hex(IntPtr h)
        {
            return "0x" + h.ToInt64().ToString("x", CultureInfo.InvariantCulture);
        }
    }

    public static class Modes
    {
        private const uint WmSetText = 0x000C;
        private const uint BmClick = 0x00F5;
        private const int SwHide = 0;
        private const int SwShowNoActivate = 4;

        private static readonly string[] Uia3OnlyPatterns = new string[]
        {
            "LegacyIAccessible", "Drag", "DropTarget", "Annotation", "Styles", "TextChild"
        };

        private static List<KeyValuePair<string, int>> SortedAvailability(IdTables t)
        {
            List<KeyValuePair<string, int>> list = new List<KeyValuePair<string, int>>();
            foreach (KeyValuePair<string, int> kv in t.PatternAvailabilityProperty) { list.Add(kv); }
            list.Sort(delegate(KeyValuePair<string, int> a, KeyValuePair<string, int> b)
            {
                return string.CompareOrdinal(a.Key, b.Key);
            });
            return list;
        }

        private static List<string> ReadPatterns(IUIAutomationElement e, List<KeyValuePair<string, int>> avail)
        {
            List<string> found = new List<string>();
            for (int i = 0; i < avail.Count; i++)
            {
                object v = null;
                try { v = e.GetCurrentPropertyValue(avail[i].Value); }
                catch (Exception) { continue; }
                if (v is bool && (bool)v) { found.Add(avail[i].Key); }
            }
            return found;
        }

        private static string DictJson(Dictionary<string, int> d)
        {
            List<string> keys = new List<string>(d.Keys);
            keys.Sort(StringComparer.Ordinal);
            List<string> parts = new List<string>();
            for (int i = 0; i < keys.Count; i++) { parts.Add(Js.P(keys[i], Js.Int(d[keys[i]]))); }
            return Js.Obj(parts);
        }

        private static void Bump(Dictionary<string, int> d, string key)
        {
            int v;
            if (d.TryGetValue(key, out v)) { d[key] = v + 1; } else { d[key] = 1; }
        }

        private static double Median(List<double> values)
        {
            List<double> copy = new List<double>(values);
            copy.Sort();
            if (copy.Count == 0) { return 0; }
            return copy[copy.Count / 2];
        }

        public static string RunEvents(IUIAutomation uia, IdTables t, string[] args)
        {
            IntPtr form = Args.ParseHandle(Args.Get(args, "--hwnd", "0"));
            int reps = Args.GetInt(args, "--reps", 20);
            if (form == IntPtr.Zero || !Native.IsWindow(form)) { throw new InvalidOperationException("events mode requires a live --hwnd"); }

            uint targetPid;
            uint targetThread = Native.GetWindowThreadProcessId(form, out targetPid);
            IntPtr hEdit = Native.GetDlgItem(form, 1003);
            IntPtr hAction = Native.GetDlgItem(form, 1005);
            IntPtr hToggle = Native.GetDlgItem(form, 1001);

            IUIAutomationElement formElement = uia.ElementFromHandle(form);
            IUIAutomationElement root = uia.GetRootElement();

            bool cheapCallbacks = Array.IndexOf(args, "--cheap-callbacks") >= 0;
            string drive = Args.Get(args, "--drive", "vcfw");
            EventLog log = new EventLog();
            log.TargetProcessId = (int)targetPid;
            log.ReadSenderProperties = !cheapCallbacks;
            AutomationSink autoSink = new AutomationSink(log);
            PropertySink propSink = new PropertySink(log);
            FocusSink focusSink = new FocusSink(log);

            List<int> watchedProps = new List<int>();
            string[] wanted = new string[] { "Name", "ValuePattern.Value", "TogglePattern.ToggleState", "HasKeyboardFocus", "RangeValuePattern.Value" };
            List<string> watchedNames = new List<string>();
            for (int i = 0; i < wanted.Length; i++)
            {
                int id = t.Prop(wanted[i]);
                if (id > 0) { watchedProps.Add(id); watchedNames.Add(wanted[i] + "=" + id.ToString(CultureInfo.InvariantCulture)); }
            }

            ManualResetEvent ready = new ManualResetEvent(false);
            ManualResetEvent quietTeardown = new ManualResetEvent(false);
            ManualResetEvent quietDone = new ManualResetEvent(false);
            ManualResetEvent reRegister = new ManualResetEvent(false);
            ManualResetEvent reRegistered = new ManualResetEvent(false);
            ManualResetEvent racyTeardown = new ManualResetEvent(false);
            ManualResetEvent racyDone = new ManualResetEvent(false);
            List<string> workerErrors = new List<string>();
            int[] workerThreadId = new int[1];
            string[] workerApartment = new string[1];
            double[] quietRemoveMs = new double[1];
            double[] racyRemoveMs = new double[1];
            List<string> quietTimings = new List<string>();
            List<string> racyTimings = new List<string>();

            ThreadStart register = delegate()
            {
                try
                {
                    uia.AddAutomationEventHandler(Uia.EventInvoked, formElement, Uia.TreeScopeSubtree, null, autoSink);
                    uia.AddAutomationEventHandler(Uia.EventTextChanged, formElement, Uia.TreeScopeSubtree, null, autoSink);
                    uia.AddAutomationEventHandler(Uia.EventElementSelected, formElement, Uia.TreeScopeSubtree, null, autoSink);
                    uia.AddAutomationEventHandler(Uia.EventWindowOpened, root, Uia.TreeScopeSubtree, null, autoSink);
                    uia.AddAutomationEventHandler(Uia.EventWindowClosed, root, Uia.TreeScopeSubtree, null, autoSink);
                    uia.AddPropertyChangedEventHandler(formElement, Uia.TreeScopeSubtree, null, propSink, watchedProps.ToArray());
                    uia.AddFocusChangedEventHandler(null, focusSink);
                }
                catch (Exception ex) { workerErrors.Add("register: " + ex.GetType().Name + ": " + ex.Message); }
            };
            Action<List<string>> remove = delegate(List<string> timings)
            {
                string[] labels = new string[]
                {
                    "Invoke_Invoked", "Text_TextChanged", "SelectionItem_ElementSelected",
                    "Window_WindowOpened", "Window_WindowClosed", "PropertyChanged", "FocusChanged"
                };
                for (int step = 0; step < labels.Length; step++)
                {
                    Stopwatch one = Stopwatch.StartNew();
                    try
                    {
                        if (step == 0) { uia.RemoveAutomationEventHandler(Uia.EventInvoked, formElement, autoSink); }
                        else if (step == 1) { uia.RemoveAutomationEventHandler(Uia.EventTextChanged, formElement, autoSink); }
                        else if (step == 2) { uia.RemoveAutomationEventHandler(Uia.EventElementSelected, formElement, autoSink); }
                        else if (step == 3) { uia.RemoveAutomationEventHandler(Uia.EventWindowOpened, root, autoSink); }
                        else if (step == 4) { uia.RemoveAutomationEventHandler(Uia.EventWindowClosed, root, autoSink); }
                        else if (step == 5) { uia.RemovePropertyChangedEventHandler(formElement, propSink); }
                        else { uia.RemoveFocusChangedEventHandler(focusSink); }
                    }
                    catch (Exception ex) { workerErrors.Add("remove " + labels[step] + ": " + ex.GetType().Name + ": " + ex.Message); }
                    one.Stop();
                    timings.Add(Js.Obj(new List<string>(new string[]
                    {
                        Js.P("handler", Js.Str(labels[step])), Js.P("elapsedMs", Js.Num(one.Elapsed.TotalMilliseconds))
                    })));
                }
            };

            Thread worker = new Thread(delegate()
            {
                workerThreadId[0] = (int)Native.GetCurrentThreadId();
                workerApartment[0] = Thread.CurrentThread.GetApartmentState().ToString();
                register();
                ready.Set();
                quietTeardown.WaitOne();
                Stopwatch sw = Stopwatch.StartNew();
                remove(quietTimings);
                sw.Stop();
                quietRemoveMs[0] = sw.Elapsed.TotalMilliseconds;
                quietDone.Set();
                reRegister.WaitOne();
                register();
                reRegistered.Set();
                racyTeardown.WaitOne();
                Stopwatch sw2 = Stopwatch.StartNew();
                remove(racyTimings);
                sw2.Stop();
                racyRemoveMs[0] = sw2.Elapsed.TotalMilliseconds;
                racyDone.Set();
            });
            worker.IsBackground = true;
            worker.SetApartmentState(ApartmentState.MTA);
            worker.Start();
            ready.WaitOne(30000);
            Thread.Sleep(400);

            List<int> repStart = new List<int>();
            for (int rep = 0; rep < reps; rep++)
            {
                log.Repetition = rep;
                repStart.Add(log.Count);
                if (drive.IndexOf('v') >= 0)
                {
                    Native.SendMessageText(hEdit, WmSetText, IntPtr.Zero, "probe-value-" + rep.ToString(CultureInfo.InvariantCulture));
                    Thread.Sleep(120);
                }
                if (drive.IndexOf('c') >= 0)
                {
                    Native.PostMessage(hAction, BmClick, IntPtr.Zero, IntPtr.Zero);
                    Thread.Sleep(120);
                    Native.PostMessage(hToggle, BmClick, IntPtr.Zero, IntPtr.Zero);
                    Thread.Sleep(120);
                }
                if (drive.IndexOf('f') >= 0)
                {
                    bool attached = Native.AttachThreadInput(Native.GetCurrentThreadId(), targetThread, true);
                    Native.SetFocus(hEdit);
                    Thread.Sleep(80);
                    Native.SetFocus(hAction);
                    if (attached) { Native.AttachThreadInput(Native.GetCurrentThreadId(), targetThread, false); }
                    Thread.Sleep(120);
                }
                if (drive.IndexOf('w') >= 0)
                {
                    Native.ShowWindow(form, SwHide);
                    Thread.Sleep(150);
                    Native.ShowWindow(form, SwShowNoActivate);
                    Thread.Sleep(250);
                }
            }
            log.Repetition = -1;
            Thread.Sleep(1000);
            int eventsBeforeTeardown = log.Count;

            quietTeardown.Set();
            bool quietCompleted = quietDone.WaitOne(150000);
            reRegister.Set();
            reRegistered.WaitOne(30000);

            bool[] driverRunning = new bool[] { true };
            Thread driver = new Thread(delegate()
            {
                while (driverRunning[0])
                {
                    Native.PostMessage(hAction, BmClick, IntPtr.Zero, IntPtr.Zero);
                    Native.SendMessageText(hEdit, WmSetText, IntPtr.Zero, "inflight");
                    Thread.Sleep(5);
                }
            });
            driver.IsBackground = true;
            driver.Start();
            Thread.Sleep(1500);
            int eventsBeforeRacyTeardown = log.Count;
            racyTeardown.Set();
            bool racyCompletedUnderLoad = racyDone.WaitOne(20000);
            bool racyCompletedAfterLoadStopped = false;
            if (!racyCompletedUnderLoad)
            {
                driverRunning[0] = false;
                racyCompletedAfterLoadStopped = racyDone.WaitOne(120000);
            }
            driverRunning[0] = false;
            driver.Join(5000);
            Thread.Sleep(500);
            int eventsAfterTeardown = log.Count;

            List<EventRecord> records = log.Snapshot();
            Dictionary<int, List<string>> perRep = new Dictionary<int, List<string>>();
            Dictionary<string, int> kindTotals = new Dictionary<string, int>();
            HashSet<int> eventThreads = new HashSet<int>();
            for (int i = 0; i < records.Count; i++)
            {
                EventRecord r = records[i];
                string label = Describe(r, t);
                if (!perRep.ContainsKey(r.Repetition)) { perRep[r.Repetition] = new List<string>(); }
                if (r.Sequence < eventsBeforeTeardown && r.SenderIsTarget) { perRep[r.Repetition].Add(label); }
                Bump(kindTotals, label);
                eventThreads.Add(r.ThreadId);
            }

            List<string> signatures = new List<string>();
            List<string> coreSignatures = new List<string>();
            for (int rep = 0; rep < reps; rep++)
            {
                List<string> seq = perRep.ContainsKey(rep) ? perRep[rep] : new List<string>();
                signatures.Add(string.Join(">", seq.ToArray()));
                List<string> core = new List<string>();
                for (int i = 0; i < seq.Count; i++)
                {
                    if (seq[i].IndexOf("Window_Window", StringComparison.Ordinal) >= 0) { continue; }
                    core.Add(seq[i]);
                }
                coreSignatures.Add(string.Join(">", core.ToArray()));
            }
            Dictionary<string, int> signatureCounts = new Dictionary<string, int>();
            for (int i = 0; i < signatures.Count; i++) { Bump(signatureCounts, signatures[i]); }
            Dictionary<string, int> coreSignatureCounts = new Dictionary<string, int>();
            for (int i = 0; i < coreSignatures.Count; i++) { Bump(coreSignatureCounts, coreSignatures[i]); }
            bool stable = signatureCounts.Count <= 1;
            bool coreStable = coreSignatureCounts.Count <= 1;

            int mainThread = (int)Native.GetCurrentThreadId();
            bool distinctFromMain = true;
            bool distinctFromWorker = true;
            foreach (int tid in eventThreads)
            {
                if (tid == mainThread) { distinctFromMain = false; }
                if (tid == workerThreadId[0]) { distinctFromWorker = false; }
            }

            List<string> sampleParts = new List<string>();
            int sampleLimit = Math.Min(records.Count, 60);
            for (int i = 0; i < sampleLimit; i++)
            {
                EventRecord r = records[i];
                List<string> p = new List<string>();
                p.Add(Js.P("seq", Js.Int(r.Sequence)));
                p.Add(Js.P("rep", Js.Int(r.Repetition)));
                p.Add(Js.P("label", Js.Str(Describe(r, t))));
                p.Add(Js.P("threadId", Js.Int(r.ThreadId)));
                p.Add(Js.P("elapsedMs", Js.Num(r.OffsetMs)));
                p.Add(Js.P("senderAutomationId", Js.Str(r.SenderAutomationId)));
                p.Add(Js.P("senderControlType", Js.Str(Uia.ControlTypeName(r.SenderControlTypeId))));
                p.Add(Js.P("senderIsTarget", Js.Bool(r.SenderIsTarget)));
                if (r.NewValue != null) { p.Add(Js.P("newValue", Js.Str(r.NewValue))); }
                sampleParts.Add(Js.Obj(p));
            }

            List<string> sigParts = new List<string>();
            for (int i = 0; i < signatures.Count; i++)
            {
                sigParts.Add(Js.Obj(new List<string>(new string[] { Js.P("rep", Js.Int(i)), Js.P("sequence", Js.Str(signatures[i])) })));
            }

            List<string> o = new List<string>();
            o.Add(Js.P("mode", Js.Str("events")));
            o.Add(Js.P("repetitions", Js.Int(reps)));
            o.Add(Js.P("cheapCallbacks", Js.Bool(cheapCallbacks)));
            o.Add(Js.P("driveActions", Js.Str(drive)));
            o.Add(Js.P("callbackWork", Js.Str(cheapCallbacks ? "log only, no property reads on sender" : "reads AutomationId+ControlType+ProcessId on sender inside the callback")));
            o.Add(Js.P("targetHwnd", Js.Str(Args.Hex(form))));
            o.Add(Js.P("targetProcessId", Js.Int((long)targetPid)));
            o.Add(Js.P("workerThreadId", Js.Int(workerThreadId[0])));
            o.Add(Js.P("workerApartment", Js.Str(workerApartment[0] == null ? "unknown" : workerApartment[0])));
            o.Add(Js.P("watchedProperties", Js.Str(string.Join(",", watchedNames.ToArray()))));
            o.Add(Js.P("totalEvents", Js.Int(records.Count)));
            o.Add(Js.P("distinctEventThreadCount", Js.Int(eventThreads.Count)));
            o.Add(Js.P("eventThreadsDistinctFromMain", Js.Bool(distinctFromMain)));
            o.Add(Js.P("eventThreadsDistinctFromWorker", Js.Bool(distinctFromWorker)));
            o.Add(Js.P("kindTotals", DictJson(kindTotals)));
            o.Add(Js.P("orderingStableAcrossReps", Js.Bool(stable)));
            o.Add(Js.P("distinctOrderingSignatures", Js.Int(signatureCounts.Count)));
            o.Add(Js.P("orderingStableExcludingWindowEvents", Js.Bool(coreStable)));
            o.Add(Js.P("distinctCoreOrderingSignatures", Js.Int(coreSignatureCounts.Count)));
            o.Add(Js.P("orderingPerRep", Js.Arr(sigParts)));
            o.Add(Js.P("quiescentTeardownCompleted", Js.Bool(quietCompleted)));
            o.Add(Js.P("quiescentTeardownElapsedMs", Js.Num(quietRemoveMs[0])));
            o.Add(Js.P("inflightTeardownCompletedUnderLoad", Js.Bool(racyCompletedUnderLoad)));
            o.Add(Js.P("inflightTeardownCompletedOnlyAfterLoadStopped", Js.Bool(racyCompletedAfterLoadStopped)));
            o.Add(Js.P("inflightTeardownElapsedMs", Js.Num(racyRemoveMs[0])));
            o.Add(Js.P("quiescentTeardownPerHandler", Js.Arr(quietTimings)));
            o.Add(Js.P("inflightTeardownPerHandler", Js.Arr(racyTimings)));
            o.Add(Js.P("inflightEventBacklogAtTeardown", Js.Int(eventsBeforeRacyTeardown - eventsBeforeTeardown)));
            o.Add(Js.P("eventsDuringTeardownWindow", Js.Int(eventsAfterTeardown - eventsBeforeTeardown)));
            o.Add(Js.P("workerErrors", Js.Str(string.Join(" | ", workerErrors.ToArray()))));
            o.Add(Js.P("sample", Js.Arr(sampleParts)));
            return Js.Obj(o);
        }

        private static string Describe(EventRecord r, IdTables t)
        {
            if (r.Kind == "AutomationEvent") { return "AutomationEvent:" + Uia.EventName(r.EventId); }
            if (r.Kind == "PropertyChanged")
            {
                string n;
                if (!t.PropertyById.TryGetValue(r.PropertyId, out n)) { n = r.PropertyId.ToString(CultureInfo.InvariantCulture); }
                return "PropertyChanged:" + n;
            }
            return r.Kind;
        }

        public static string RunIds(IUIAutomation uia, IdTables t, string[] args)
        {
            List<string> props = new List<string>();
            List<int> ids = new List<int>(t.PropertyById.Keys);
            ids.Sort();
            for (int i = 0; i < ids.Count; i++)
            {
                props.Add(Js.Obj(new List<string>(new string[]
                {
                    Js.P("id", Js.Int(ids[i])), Js.P("programmaticName", Js.Str(t.PropertyById[ids[i]]))
                })));
            }
            List<string> pats = new List<string>();
            for (int i = 0; i < t.PatternNames.Count; i++) { pats.Add(Js.Str(t.PatternNames[i])); }
            List<KeyValuePair<string, int>> avail = SortedAvailability(t);
            List<string> availJson = new List<string>();
            for (int i = 0; i < avail.Count; i++)
            {
                availJson.Add(Js.Obj(new List<string>(new string[]
                {
                    Js.P("pattern", Js.Str(avail[i].Key)), Js.P("availabilityPropertyId", Js.Int(avail[i].Value))
                })));
            }
            List<string> o = new List<string>();
            o.Add(Js.P("mode", Js.Str("ids")));
            o.Add(Js.P("propertyCount", Js.Int(props.Count)));
            o.Add(Js.P("properties", Js.Arr(props)));
            o.Add(Js.P("patterns", Js.Arr(pats)));
            o.Add(Js.P("patternAvailabilityProperties", Js.Arr(availJson)));
            return Js.Obj(o);
        }

        public static string RunCache(IUIAutomation uia, IdTables t, string[] args)
        {
            IntPtr hwnd = Args.ParseHandle(Args.Get(args, "--hwnd", "0"));
            string label = Args.Get(args, "--label", "target");
            int maxNodes = Args.GetInt(args, "--max-nodes", 1500);
            int reps = Args.GetInt(args, "--reps", 5);
            IUIAutomationElement root = uia.ElementFromHandle(hwnd);
            IUIAutomationCondition all = uia.CreateTrueCondition();

            string[] wanted = new string[]
            {
                "Name", "ControlType", "AutomationId", "ClassName",
                "BoundingRectangle", "IsEnabled", "IsOffscreen", "FrameworkId"
            };
            List<int> props = new List<int>();
            List<string> propNames = new List<string>();
            for (int i = 0; i < wanted.Length; i++)
            {
                int id = t.Prop(wanted[i]);
                if (id > 0) { props.Add(id); propNames.Add(wanted[i]); }
            }

            IUIAutomationElementArray warm = root.FindAll(Uia.TreeScopeSubtree, all);
            int nodeCount = Math.Min(warm.GetLength(), maxNodes);

            IUIAutomationCacheRequest cache = uia.CreateCacheRequest();
            for (int i = 0; i < props.Count; i++) { cache.AddProperty(props[i]); }

            List<double> uncached = new List<double>();
            List<double> cached = new List<double>();
            List<double> uncachedFind = new List<double>();
            List<double> cachedFind = new List<double>();

            for (int rep = 0; rep < reps; rep++)
            {
                Stopwatch sw = Stopwatch.StartNew();
                IUIAutomationElementArray arr = root.FindAll(Uia.TreeScopeSubtree, all);
                double find = sw.Elapsed.TotalMilliseconds;
                int n = Math.Min(arr.GetLength(), maxNodes);
                for (int i = 0; i < n; i++)
                {
                    IUIAutomationElement e = arr.GetElement(i);
                    for (int p = 0; p < props.Count; p++)
                    {
                        try { object v = e.GetCurrentPropertyValue(props[p]); GC.KeepAlive(v); }
                        catch (Exception) { }
                    }
                }
                sw.Stop();
                uncachedFind.Add(find);
                uncached.Add(sw.Elapsed.TotalMilliseconds);

                Stopwatch sw2 = Stopwatch.StartNew();
                IUIAutomationElementArray arr2 = root.FindAllBuildCache(Uia.TreeScopeSubtree, all, cache);
                double find2 = sw2.Elapsed.TotalMilliseconds;
                int n2 = Math.Min(arr2.GetLength(), maxNodes);
                for (int i = 0; i < n2; i++)
                {
                    IUIAutomationElement e = arr2.GetElement(i);
                    for (int p = 0; p < props.Count; p++)
                    {
                        try { object v = e.GetCachedPropertyValue(props[p]); GC.KeepAlive(v); }
                        catch (Exception) { }
                    }
                }
                sw2.Stop();
                cachedFind.Add(find2);
                cached.Add(sw2.Elapsed.TotalMilliseconds);
            }

            double medianUncached = Median(uncached);
            double medianCached = Median(cached);
            double medianUncachedRead = Median(uncached) - Median(uncachedFind);
            double medianCachedRead = Median(cached) - Median(cachedFind);
            double readMultiplier = medianCachedRead > 0 ? medianUncachedRead / medianCachedRead : 0;
            double multiplier = medianCached > 0 ? medianUncached / medianCached : 0;
            string verdict;
            if (multiplier < 1.0) { verdict = "cache-slower-than-per-property"; }
            else if (multiplier < 3.0) { verdict = "below-3x-claim"; }
            else if (multiplier <= 5.0) { verdict = "within-3-5x-claim"; }
            else { verdict = "above-5x-claim"; }

            List<string> o = new List<string>();
            o.Add(Js.P("mode", Js.Str("cache")));
            o.Add(Js.P("label", Js.Str(label)));
            o.Add(Js.P("hwnd", Js.Str(Args.Hex(hwnd))));
            o.Add(Js.P("nodeCount", Js.Int(nodeCount)));
            o.Add(Js.P("propertiesPerNode", Js.Int(props.Count)));
            o.Add(Js.P("propertyNames", Js.Str(string.Join(",", propNames.ToArray()))));
            o.Add(Js.P("repetitions", Js.Int(reps)));
            o.Add(Js.P("perPropertyMs", Js.Num(medianUncached)));
            o.Add(Js.P("cachedMs", Js.Num(medianCached)));
            o.Add(Js.P("timingPerPropertyFindMs", Js.Num(Median(uncachedFind))));
            o.Add(Js.P("timingCachedFindMs", Js.Num(Median(cachedFind))));
            o.Add(Js.P("timingPerPropertyReadMs", Js.Num(medianUncachedRead)));
            o.Add(Js.P("timingCachedReadMs", Js.Num(medianCachedRead)));
            o.Add(Js.P("timingReadPhaseMultiplier", Js.Num(readMultiplier)));
            o.Add(Js.P("timingMultiplier", Js.Num(multiplier)));
            o.Add(Js.P("claimVerdict", Js.Str(verdict)));
            o.Add(Js.P("claimChecked", Js.Str("phases.md 3-5x batched-read speedup")));
            return Js.Obj(o);
        }

        public static string RunWalker(IUIAutomation uia, IdTables t, string[] args)
        {
            IntPtr hwnd = Args.ParseHandle(Args.Get(args, "--hwnd", "0"));
            string label = Args.Get(args, "--label", "target");
            int maxNodes = Args.GetInt(args, "--max-nodes", 20000);
            int maxDepth = Args.GetInt(args, "--max-depth", 60);
            IUIAutomationElement root = uia.ElementFromHandle(hwnd);

            int pid = 0;
            string processName = "";
            string frameworkId = "";
            try { pid = root.GetCurrentProcessId(); } catch (Exception) { }
            try { processName = Process.GetProcessById(pid).ProcessName; } catch (Exception) { processName = "<unavailable>"; }
            try { frameworkId = root.GetCurrentFrameworkId(); } catch (Exception) { }

            int settleMs = Args.GetInt(args, "--settle-ms", 8000);
            string[] names = new string[] { "RawView", "ControlView", "ContentView" };
            IUIAutomationTreeWalker[] walkers = new IUIAutomationTreeWalker[]
            {
                uia.GetRawViewWalker(), uia.GetControlViewWalker(), uia.GetContentViewWalker()
            };
            List<string> firstPass = new List<string>();
            List<string> views = new List<string>();
            int rawCount = 0;
            int controlCount = 0;
            for (int pass = 0; pass < 2; pass++)
            {
                if (pass == 1) { Thread.Sleep(settleMs); }
                for (int i = 0; i < names.Length; i++)
                {
                    bool truncated;
                    int deepest;
                    Stopwatch sw = Stopwatch.StartNew();
                    int count = Tree.CountNodes(walkers[i], root, maxNodes, maxDepth, out truncated, out deepest);
                    sw.Stop();
                    if (pass == 1 && i == 0) { rawCount = count; }
                    if (pass == 1 && i == 1) { controlCount = count; }
                    List<string> p = new List<string>();
                    p.Add(Js.P("view", Js.Str(names[i])));
                    p.Add(Js.P("nodeCount", Js.Int(count)));
                    p.Add(Js.P("deepestDepth", Js.Int(deepest)));
                    p.Add(Js.P("truncated", Js.Bool(truncated)));
                    p.Add(Js.P("elapsedMs", Js.Num(sw.Elapsed.TotalMilliseconds)));
                    if (pass == 0) { firstPass.Add(Js.Obj(p)); } else { views.Add(Js.Obj(p)); }
                }
            }

            List<string> o = new List<string>();
            o.Add(Js.P("settleMs", Js.Int(settleMs)));
            o.Add(Js.P("viewsFirstContact", Js.Arr(firstPass)));
            o.Add(Js.P("mode", Js.Str("walker")));
            o.Add(Js.P("label", Js.Str(label)));
            o.Add(Js.P("hwnd", Js.Str(Args.Hex(hwnd))));
            o.Add(Js.P("processId", Js.Int(pid)));
            o.Add(Js.P("processName", Js.Str(processName)));
            o.Add(Js.P("frameworkId", Js.Str(frameworkId)));
            o.Add(Js.P("maxNodes", Js.Int(maxNodes)));
            o.Add(Js.P("maxDepth", Js.Int(maxDepth)));
            o.Add(Js.P("viewsAfterSettle", Js.Arr(views)));
            o.Add(Js.P("rawMinusControl", Js.Int(rawCount - controlCount)));
            return Js.Obj(o);
        }

        public static string RunCensus(IUIAutomation uia, IdTables t, string[] args)
        {
            List<string> targets = Args.GetAll(args, "--target");
            int maxNodes = Args.GetInt(args, "--max-nodes", 400);
            int maxDepth = Args.GetInt(args, "--max-depth", 30);
            List<KeyValuePair<string, int>> avail = SortedAvailability(t);
            IUIAutomationTreeWalker rawWalker = uia.GetRawViewWalker();
            IUIAutomationTreeWalker controlWalker = uia.GetControlViewWalker();

            List<string> uia3Only = new List<string>();
            for (int i = 0; i < Uia3OnlyPatterns.Length; i++)
            {
                int id;
                bool present = t.PatternAvailabilityProperty.TryGetValue(Uia3OnlyPatterns[i], out id);
                List<string> p = new List<string>();
                p.Add(Js.P("pattern", Js.Str(Uia3OnlyPatterns[i])));
                p.Add(Js.P("availabilityPropertyDiscovered", Js.Bool(present)));
                p.Add(Js.P("availabilityPropertyId", Js.Int(present ? id : -1)));
                uia3Only.Add(Js.Obj(p));
            }

            List<string> targetJson = new List<string>();
            for (int ti = 0; ti < targets.Count; ti++)
            {
                string spec = targets[ti];
                int eq = spec.IndexOf('=');
                string label = eq > 0 ? spec.Substring(0, eq) : "target" + ti.ToString(CultureInfo.InvariantCulture);
                IntPtr hwnd = Args.ParseHandle(eq > 0 ? spec.Substring(eq + 1) : spec);
                targetJson.Add(CensusTarget(uia, t, label, hwnd, rawWalker, controlWalker, avail, maxNodes, maxDepth));
            }

            List<string> o = new List<string>();
            o.Add(Js.P("mode", Js.Str("census")));
            o.Add(Js.P("maxNodes", Js.Int(maxNodes)));
            o.Add(Js.P("maxDepth", Js.Int(maxDepth)));
            o.Add(Js.P("patternAvailabilityPropertyCount", Js.Int(avail.Count)));
            o.Add(Js.P("uia3OnlyPatternProperties", Js.Arr(uia3Only)));
            o.Add(Js.P("targets", Js.Arr(targetJson)));
            return Js.Obj(o);
        }

        private static string CensusTarget(IUIAutomation uia, IdTables t, string label, IntPtr hwnd,
            IUIAutomationTreeWalker rawWalker, IUIAutomationTreeWalker controlWalker,
            List<KeyValuePair<string, int>> avail, int maxNodes, int maxDepth)
        {
            List<string> o = new List<string>();
            o.Add(Js.P("label", Js.Str(label)));
            o.Add(Js.P("hwnd", Js.Str(Args.Hex(hwnd))));
            if (hwnd == IntPtr.Zero || !Native.IsWindow(hwnd))
            {
                o.Add(Js.P("error", Js.Str("hwnd is not a live window")));
                return Js.Obj(o);
            }
            IUIAutomationElement root = uia.ElementFromHandle(hwnd);
            int pid = 0;
            try { pid = root.GetCurrentProcessId(); } catch (Exception) { }
            string processName = "<unavailable>";
            try { processName = Process.GetProcessById(pid).ProcessName; } catch (Exception) { }
            o.Add(Js.P("processId", Js.Int(pid)));
            o.Add(Js.P("processName", Js.Str(processName)));
            try { o.Add(Js.P("rootFrameworkId", Js.Str(root.GetCurrentFrameworkId()))); } catch (Exception) { }
            try { o.Add(Js.P("rootControlType", Js.Str(Uia.ControlTypeName(root.GetCurrentControlType())))); } catch (Exception) { }
            try { o.Add(Js.P("rootProviderDescription", Js.Str(Uia.CleanProviderDescription(root.GetCurrentProviderDescription())))); } catch (Exception) { }

            List<IUIAutomationElement> controlNodes = new List<IUIAutomationElement>();
            List<int> controlDepths = new List<int>();
            Tree.Collect(controlWalker, root, 0, maxDepth, controlNodes, controlDepths, maxNodes);
            HashSet<string> controlIds = new HashSet<string>();
            for (int i = 0; i < controlNodes.Count; i++) { controlIds.Add(Tree.RuntimeIdOf(controlNodes[i])); }

            List<IUIAutomationElement> rawNodes = new List<IUIAutomationElement>();
            List<int> rawDepths = new List<int>();
            Tree.Collect(rawWalker, root, 0, maxDepth, rawNodes, rawDepths, maxNodes);

            Dictionary<string, int> patternTotals = new Dictionary<string, int>();
            Dictionary<string, int> providerDescriptions = new Dictionary<string, int>();
            Dictionary<string, Dictionary<string, int>> byType = new Dictionary<string, Dictionary<string, int>>();
            Dictionary<string, int> typeCounts = new Dictionary<string, int>();
            Dictionary<string, int> typeWithPattern = new Dictionary<string, int>();
            List<string> rawOnly = new List<string>();
            List<string> elementJson = new List<string>();
            int withAnyPattern = 0;
            int withAnyPatternExcludingLegacy = 0;

            for (int i = 0; i < rawNodes.Count; i++)
            {
                IUIAutomationElement e = rawNodes[i];
                NodeInfo n = new NodeInfo();
                n.Depth = rawDepths[i];
                n.RuntimeId = Tree.RuntimeIdOf(e);
                n.InControlView = controlIds.Contains(n.RuntimeId);
                try { n.ControlTypeId = e.GetCurrentControlType(); } catch (Exception) { n.ControlTypeId = -1; }
                try { n.AutomationId = e.GetCurrentAutomationId(); } catch (Exception) { n.AutomationId = ""; }
                try { n.ClassName = e.GetCurrentClassName(); } catch (Exception) { n.ClassName = ""; }
                try { string nm = e.GetCurrentName(); n.NameLength = nm == null ? 0 : nm.Length; } catch (Exception) { n.NameLength = -1; }
                try { n.FrameworkId = e.GetCurrentFrameworkId(); } catch (Exception) { n.FrameworkId = ""; }
                try { n.ProviderDescription = Uia.CleanProviderDescription(e.GetCurrentProviderDescription()); } catch (Exception) { n.ProviderDescription = ""; }
                n.Patterns = ReadPatterns(e, avail);

                if (!n.InControlView && !string.IsNullOrEmpty(n.AutomationId)) { rawOnly.Add(Js.Str(n.AutomationId)); }
                if (!string.IsNullOrEmpty(n.ProviderDescription)) { Bump(providerDescriptions, n.ProviderDescription); }
                string typeName = Uia.ControlTypeName(n.ControlTypeId);
                Bump(typeCounts, typeName);
                if (n.Patterns.Count > 0)
                {
                    withAnyPattern++;
                    Bump(typeWithPattern, typeName);
                }
                bool nonLegacy = false;
                for (int p = 0; p < n.Patterns.Count; p++)
                {
                    Bump(patternTotals, n.Patterns[p]);
                    if (!byType.ContainsKey(typeName)) { byType[typeName] = new Dictionary<string, int>(); }
                    Bump(byType[typeName], n.Patterns[p]);
                    if (n.Patterns[p] != "LegacyIAccessible") { nonLegacy = true; }
                }
                if (nonLegacy) { withAnyPatternExcludingLegacy++; }

                if (elementJson.Count < 60)
                {
                    List<string> ep = new List<string>();
                    ep.Add(Js.P("depth", Js.Int(n.Depth)));
                    ep.Add(Js.P("controlType", Js.Str(typeName)));
                    ep.Add(Js.P("controlTypeId", Js.Int(n.ControlTypeId)));
                    ep.Add(Js.P("automationId", Js.Str(n.AutomationId)));
                    ep.Add(Js.P("className", Js.Str(n.ClassName)));
                    ep.Add(Js.P("nameLength", Js.Int(n.NameLength)));
                    ep.Add(Js.P("frameworkId", Js.Str(n.FrameworkId)));
                    ep.Add(Js.P("inControlView", Js.Bool(n.InControlView)));
                    ep.Add(Js.P("patterns", Js.Str(string.Join(",", n.Patterns.ToArray()))));
                    elementJson.Add(Js.Obj(ep));
                }
            }

            List<string> byTypeJson = new List<string>();
            List<string> typeKeys = new List<string>(typeCounts.Keys);
            typeKeys.Sort(StringComparer.Ordinal);
            for (int i = 0; i < typeKeys.Count; i++)
            {
                string k = typeKeys[i];
                List<string> p = new List<string>();
                p.Add(Js.P("controlType", Js.Str(k)));
                p.Add(Js.P("count", Js.Int(typeCounts[k])));
                p.Add(Js.P("withAnyPattern", Js.Int(typeWithPattern.ContainsKey(k) ? typeWithPattern[k] : 0)));
                p.Add(Js.P("patterns", byType.ContainsKey(k) ? DictJson(byType[k]) : "{}"));
                byTypeJson.Add(Js.Obj(p));
            }

            List<string> pollParts = new List<string>();
            try
            {
                int[] ids;
                string[] names;
                uia.PollForPotentialSupportedPatterns(root, out ids, out names);
                pollParts.Add(Js.P("ok", Js.Bool(true)));
                pollParts.Add(Js.P("names", Js.Str(names == null ? "" : string.Join(",", names))));
            }
            catch (Exception ex)
            {
                pollParts.Add(Js.P("ok", Js.Bool(false)));
                pollParts.Add(Js.P("error", Js.Str(ex.GetType().Name + ": " + ex.Message)));
            }

            o.Add(Js.P("controlViewNodes", Js.Int(controlNodes.Count)));
            o.Add(Js.P("rawViewNodes", Js.Int(rawNodes.Count)));
            o.Add(Js.P("truncated", Js.Bool(rawNodes.Count >= maxNodes || controlNodes.Count >= maxNodes)));
            o.Add(Js.P("elementsWithAnyPattern", Js.Int(withAnyPattern)));
            o.Add(Js.P("elementsWithAnyPatternExcludingLegacy", Js.Int(withAnyPatternExcludingLegacy)));
            o.Add(Js.P("rawOnlyAutomationIds", Js.Arr(rawOnly)));
            o.Add(Js.P("patternTotals", DictJson(patternTotals)));
            o.Add(Js.P("providerDescriptionsByNodeCount", DictJson(providerDescriptions)));
            o.Add(Js.P("byControlType", Js.Arr(byTypeJson)));
            o.Add(Js.P("rootPollForPotentialSupportedPatterns", Js.Obj(pollParts)));
            o.Add(Js.P("elements", Js.Arr(elementJson)));
            return Js.Obj(o);
        }
    }

    public static class Program
    {
        private static IUIAutomation uia;
        private static IdTables tables;
        private static string clsidUsed = "";

        [MTAThread]
        public static int Main(string[] args)
        {
            if (args.Length == 0)
            {
                Console.Error.WriteLine("usage: 08-uia3-com.exe <events|cache|walker|census> [options]");
                return 2;
            }
            Thread watchdog = new Thread(WatchdogLoop);
            watchdog.IsBackground = true;
            watchdog.Start();
            try
            {
                CreateAutomation();
                tables = IdTables.Discover(uia);
                string mode = args[0];
                string json;
                if (mode == "events") { json = Modes.RunEvents(uia, tables, args); }
                else if (mode == "cache") { json = Modes.RunCache(uia, tables, args); }
                else if (mode == "walker") { json = Modes.RunWalker(uia, tables, args); }
                else if (mode == "census") { json = Modes.RunCensus(uia, tables, args); }
                else if (mode == "ids") { json = Modes.RunIds(uia, tables, args); }
                else { Console.Error.WriteLine("unknown mode " + mode); return 2; }
                List<string> parts = new List<string>();
                parts.Add(Js.P("stack", Js.Str("uia3-com")));
                parts.Add(Js.P("binding", Js.Str("hand-declared ComImport, no GAC UIAutomationClient reference")));
                parts.Add(Js.P("coclass", Js.Str(clsidUsed)));
                parts.Add(Js.P("mainThreadId", Js.Int((long)Native.GetCurrentThreadId())));
                parts.Add(Js.P("mainApartment", Js.Str(Thread.CurrentThread.GetApartmentState().ToString())));
                parts.Add(Js.P("discoveredPatternAvailabilityProperties", Js.Int(tables.PatternAvailabilityProperty.Count)));
                parts.Add(Js.P("result", json));
                Console.Out.Write(Js.Obj(parts));
                Console.Out.Flush();
                return 0;
            }
            catch (COMException cex)
            {
                Console.Error.WriteLine("COM failure hresult=0x" + cex.ErrorCode.ToString("x8", CultureInfo.InvariantCulture) + " " + cex.Message);
                Console.Error.WriteLine(cex.StackTrace);
                return 3;
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine("failure " + ex.GetType().Name + ": " + ex.Message);
                Console.Error.WriteLine(ex.StackTrace);
                return 4;
            }
        }

        private static void WatchdogLoop()
        {
            Thread.Sleep(600000);
            Console.Error.WriteLine("watchdog: 600s elapsed, forcing exit 5");
            Environment.Exit(5);
        }

        private static void CreateAutomation()
        {
            Guid cuia8 = new Guid("e22ad333-b25f-460c-83d0-0581107395c9");
            Guid cuia = new Guid("ff48dba4-60ef-4201-aa87-54103eef594e");
            try
            {
                object o = Activator.CreateInstance(Type.GetTypeFromCLSID(cuia8, true));
                uia = (IUIAutomation)o;
                clsidUsed = "CUIAutomation8";
                return;
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine("CUIAutomation8 unavailable (" + ex.GetType().Name + ": " + ex.Message + "), falling back to CUIAutomation");
            }
            object o2 = Activator.CreateInstance(Type.GetTypeFromCLSID(cuia, true));
            uia = (IUIAutomation)o2;
            clsidUsed = "CUIAutomation";
        }
    }
}
