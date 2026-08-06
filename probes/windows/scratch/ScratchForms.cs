using System;
using System.Collections.Generic;
using System.Drawing;
using System.Windows.Automation;
using System.Windows.Automation.Provider;
using System.Windows.Forms;

namespace AgentDesktop.Scratch
{
    public class AutomationIdProvider : IRawElementProviderSimple
    {
        private readonly IntPtr handle;
        private readonly string automationId;

        public AutomationIdProvider(IntPtr handle, string automationId)
        {
            this.handle = handle;
            this.automationId = automationId;
        }

        public ProviderOptions ProviderOptions
        {
            get { return ProviderOptions.ServerSideProvider; }
        }

        public object GetPatternProvider(int patternId)
        {
            return null;
        }

        public object GetPropertyValue(int propertyId)
        {
            if (propertyId == AutomationElementIdentifiers.AutomationIdProperty.Id)
            {
                return automationId;
            }
            return null;
        }

        public IRawElementProviderSimple HostRawElementProvider
        {
            get { return AutomationInteropProvider.HostProviderFromHandle(handle); }
        }
    }

    public class AutomationIdWindow : NativeWindow
    {
        private const int WmGetObject = 0x003D;
        private static readonly IntPtr UiaRootObjectId = new IntPtr(-25);

        private readonly string automationId;
        private AutomationIdProvider provider;

        public AutomationIdWindow(string automationId)
        {
            this.automationId = automationId;
        }

        protected override void WndProc(ref Message m)
        {
            if (m.Msg == WmGetObject && m.LParam == UiaRootObjectId)
            {
                if (provider == null)
                {
                    provider = new AutomationIdProvider(m.HWnd, automationId);
                }
                m.Result = AutomationInteropProvider.ReturnRawElementProvider(
                    m.HWnd, m.WParam, m.LParam, provider);
                return;
            }
            base.WndProc(ref m);
        }
    }

    public class ScratchOptions
    {
        public string Tag;
        public bool MutateList;
        public bool HostProviders;
        public int X;
        public int Y;

        public ScratchOptions()
        {
            Tag = "main";
            MutateList = false;
            HostProviders = false;
            X = 100;
            Y = 100;
        }

        public static ScratchOptions Parse(string[] args)
        {
            ScratchOptions options = new ScratchOptions();
            for (int i = 0; i < args.Length; i++)
            {
                string arg = args[i];
                if (arg == "--tag" && i + 1 < args.Length)
                {
                    options.Tag = args[i + 1];
                    i++;
                }
                else if (arg == "--mutate-list")
                {
                    options.MutateList = true;
                }
                else if (arg == "--host-providers")
                {
                    options.HostProviders = true;
                }
                else if (arg == "--pos" && i + 1 < args.Length)
                {
                    string[] parts = args[i + 1].Split(',');
                    int x;
                    int y;
                    if (parts.Length == 2 && int.TryParse(parts[0], out x) && int.TryParse(parts[1], out y))
                    {
                        options.X = x;
                        options.Y = y;
                    }
                    i++;
                }
            }
            return options;
        }
    }

    public class ScratchForm : Form
    {
        private const int GwlId = -12;

        [System.Runtime.InteropServices.DllImport("user32.dll", EntryPoint = "SetWindowLongPtrW")]
        private static extern IntPtr SetWindowLongPtr64(IntPtr handle, int index, IntPtr value);

        [System.Runtime.InteropServices.DllImport("user32.dll", EntryPoint = "SetWindowLongW")]
        private static extern int SetWindowLong32(IntPtr handle, int index, int value);

        private static readonly string[] ControlIdOrder = new string[]
        {
            "chkToggle", "lblValueCaption", "txtValue", "cboChoice", "btnAction",
            "btnMutateList", "btnZeroSize", "tbSlider", "trvNodes", "lstItems",
            "pnlScroll", "btnRow00", "btnRow01", "btnRow02", "btnRow03", "btnRow04",
            "btnRow05", "btnRow06", "btnRow07", "lblStatus", "txtStatusMirror",
            "lblScrollPos", "lblSliderValue", "lblInstance",
            "tabMain", "tpAlpha", "tpBravo", "tpCharlie", "nudCount", "dgvRows"
        };

        private static readonly string[] BaselineItems = new string[]
        {
            "Item-Alpha", "Item-Bravo", "Item-Charlie", "Item-Delta", "Item-Echo"
        };

        private static readonly string[] MutatedItems = new string[]
        {
            "Item-Alpha", "Item-Charlie", "Item-Delta", "Item-Echo", "Item-Foxtrot", "Item-Golf"
        };

        private readonly Label statusLabel = new Label();
        private readonly TextBox statusMirror = new TextBox();
        private readonly Label scrollLabel = new Label();
        private readonly Label sliderLabel = new Label();
        private readonly Label instanceLabel = new Label();
        private readonly ListBox itemList = new ListBox();
        private readonly Panel scrollPanel = new Panel();
        private readonly TrackBar slider = new TrackBar();
        private readonly Timer scrollWatch = new Timer();
        private readonly List<AutomationIdWindow> automationIdWindows = new List<AutomationIdWindow>();

        private readonly bool hostProviders;

        private bool listMutated;
        private int actionCount;
        private int lastScrollY;

        public ScratchForm(ScratchOptions options)
        {
            hostProviders = options.HostProviders;
            Name = "frmScratchMain";
            Text = "AgentDesktop Scratch WinForms [" + options.Tag + "]";
            ClientSize = new Size(760, 680);
            FormBorderStyle = FormBorderStyle.Sizable;
            StartPosition = FormStartPosition.Manual;
            Location = new Point(options.X, options.Y);
            MinimizeBox = true;
            MaximizeBox = true;

            BuildLeftColumn();
            BuildRightColumn();
            BuildScrollPanel();
            BuildStatusStrip(options.Tag);
            BuildDataControls();

            listMutated = options.MutateList;
            ApplyListContents(listMutated);
            SetStatus("status:ready");

            scrollWatch.Interval = 100;
            scrollWatch.Tick += OnScrollWatchTick;
            scrollWatch.Start();
        }

        protected override bool ShowWithoutActivation
        {
            get { return true; }
        }

        protected override void OnLoad(EventArgs e)
        {
            base.OnLoad(e);
            if (!hostProviders)
            {
                InstallAutomationIdProvider(this);
            }
            RegisterDescendants(this);
        }

        private void RegisterDescendants(Control parent)
        {
            foreach (Control control in parent.Controls)
            {
                AssignControlId(control);
                if (!hostProviders)
                {
                    InstallAutomationIdProvider(control);
                }
                RegisterDescendants(control);
            }
        }

        private void InstallAutomationIdProvider(Control control)
        {
            if (string.IsNullOrEmpty(control.Name))
            {
                return;
            }
            AutomationIdWindow window = new AutomationIdWindow(control.Name);
            window.AssignHandle(control.Handle);
            automationIdWindows.Add(window);
        }

        private static void AssignControlId(Control control)
        {
            int index = Array.IndexOf(ControlIdOrder, control.Name);
            if (index < 0)
            {
                return;
            }
            IntPtr handle = control.Handle;
            if (IntPtr.Size == 8)
            {
                SetWindowLongPtr64(handle, GwlId, new IntPtr(1001 + index));
            }
            else
            {
                SetWindowLong32(handle, GwlId, 1001 + index);
            }
        }

        private static T Place<T>(Control parent, T control, string name, int x, int y, int w, int h)
            where T : Control
        {
            control.Name = name;
            control.SetBounds(x, y, w, h);
            parent.Controls.Add(control);
            return control;
        }

        private void BuildLeftColumn()
        {
            CheckBox toggle = Place(this, new CheckBox(), "chkToggle", 16, 16, 200, 24);
            toggle.Text = "Enable feature";
            toggle.CheckedChanged += OnToggleChanged;

            Place(this, new Label(), "lblValueCaption", 16, 48, 200, 20).Text = "Value field";
            Place(this, new TextBox(), "txtValue", 16, 70, 320, 24).Text = "seed-value";

            ComboBox choice = Place(this, new ComboBox(), "cboChoice", 16, 104, 200, 24);
            choice.DropDownStyle = ComboBoxStyle.DropDownList;
            choice.Items.AddRange(new object[] { "Choice-One", "Choice-Two", "Choice-Three", "Choice-Four" });
            choice.SelectedIndex = 0;
            choice.SelectedIndexChanged += OnChoiceChanged;

            Button action = Place(this, new Button(), "btnAction", 232, 104, 104, 26);
            action.Text = "Do Action";
            action.Click += OnActionClick;

            Button mutate = Place(this, new Button(), "btnMutateList", 16, 140, 104, 26);
            mutate.Text = "Mutate List";
            mutate.Click += OnMutateClick;

            Place(this, new Button(), "btnZeroSize", 140, 140, 0, 0).Text = "zero";

            Place(this, new Button(), "btnDupPair", 16, 300, 90, 26).Text = "dup-a";
            Place(this, new Button(), "btnDupPair", 112, 300, 90, 26).Text = "dup-b";

            Place(this, slider, "tbSlider", 16, 176, 320, 45);
            slider.Minimum = 0;
            slider.Maximum = 100;
            slider.TickFrequency = 10;
            slider.ValueChanged += OnSliderChanged;
        }

        private void BuildRightColumn()
        {
            TreeView tree = Place(this, new TreeView(), "trvNodes", 392, 16, 340, 150);
            TreeNode parent = new TreeNode("Node-Parent");
            parent.Nodes.Add(new TreeNode("Node-Child-1"));
            parent.Nodes.Add(new TreeNode("Node-Child-2"));
            parent.Nodes.Add(new TreeNode("Node-Child-3"));
            TreeNode second = new TreeNode("Node-Sibling");
            second.Nodes.Add(new TreeNode("Node-Sibling-Child"));
            tree.Nodes.Add(parent);
            tree.Nodes.Add(second);
            tree.AfterExpand += OnTreeExpanded;
            tree.AfterCollapse += OnTreeCollapsed;

            Place(this, itemList, "lstItems", 392, 176, 340, 108);
            itemList.SelectedIndexChanged += OnListSelectionChanged;
        }

        private void BuildScrollPanel()
        {
            Place(this, scrollPanel, "pnlScroll", 16, 232, 340, 180);
            scrollPanel.AutoScroll = true;
            scrollPanel.BorderStyle = BorderStyle.FixedSingle;
            for (int i = 0; i < 8; i++)
            {
                Button row = Place(scrollPanel, new Button(),
                    "btnRow" + i.ToString("00"), 8, 8 + (i * 40), 260, 30);
                row.Text = "Row " + i.ToString("00");
                row.Click += OnRowClick;
            }
        }

        private void BuildStatusStrip(string tag)
        {
            Place(this, statusLabel, "lblStatus", 16, 424, 340, 24);
            statusLabel.AutoSize = false;
            statusLabel.BorderStyle = BorderStyle.FixedSingle;

            Place(this, statusMirror, "txtStatusMirror", 392, 424, 340, 24);
            statusMirror.ReadOnly = true;

            Place(this, scrollLabel, "lblScrollPos", 16, 456, 160, 20).Text = "scroll:0";
            Place(this, sliderLabel, "lblSliderValue", 196, 456, 160, 20).Text = "slider:0";
            Place(this, instanceLabel, "lblInstance", 392, 456, 340, 20).Text = "instance:" + tag
                + " pid:" + System.Diagnostics.Process.GetCurrentProcess().Id.ToString();
        }

        private void BuildDataControls()
        {
            TabControl tabs = Place(this, new TabControl(), "tabMain", 16, 486, 340, 130);
            TabPage tabAlpha = new TabPage("Tab-Alpha");
            tabAlpha.Name = "tpAlpha";
            TabPage tabBravo = new TabPage("Tab-Bravo");
            tabBravo.Name = "tpBravo";
            TabPage tabCharlie = new TabPage("Tab-Charlie");
            tabCharlie.Name = "tpCharlie";
            tabs.TabPages.Add(tabAlpha);
            tabs.TabPages.Add(tabBravo);
            tabs.TabPages.Add(tabCharlie);

            NumericUpDown spinner = Place(this, new NumericUpDown(), "nudCount", 16, 626, 120, 24);
            spinner.Minimum = 0;
            spinner.Maximum = 100;
            spinner.Value = 0;

            DataGridView grid = Place(this, new DataGridView(), "dgvRows", 392, 290, 340, 124);
            grid.ColumnCount = 2;
            grid.Columns[0].Name = "Column-Label";
            grid.Columns[1].Name = "Column-Value";
            grid.RowHeadersVisible = false;
            grid.AllowUserToAddRows = false;
            grid.Rows.Add("Row-Alpha", "Value-Alpha");
            grid.Rows.Add("Row-Bravo", "Value-Bravo");
            grid.Rows.Add("Row-Charlie", "Value-Charlie");
        }

        private void ApplyListContents(bool mutated)
        {
            string[] source = mutated ? MutatedItems : BaselineItems;
            itemList.BeginUpdate();
            itemList.Items.Clear();
            for (int i = 0; i < source.Length; i++)
            {
                itemList.Items.Add(source[i]);
            }
            itemList.EndUpdate();
            listMutated = mutated;
        }

        private void SetStatus(string value)
        {
            statusLabel.Text = value;
            statusMirror.Text = value;
        }

        private void OnActionClick(object sender, EventArgs e)
        {
            actionCount++;
            SetStatus("action:" + actionCount.ToString());
        }

        private void OnMutateClick(object sender, EventArgs e)
        {
            ApplyListContents(!listMutated);
            SetStatus(listMutated ? "list:mutated" : "list:baseline");
        }

        private void OnToggleChanged(object sender, EventArgs e)
        {
            CheckBox box = (CheckBox)sender;
            SetStatus("check:" + (box.Checked ? "on" : "off"));
        }

        private void OnChoiceChanged(object sender, EventArgs e)
        {
            ComboBox box = (ComboBox)sender;
            SetStatus("combo:" + Convert.ToString(box.SelectedItem));
        }

        private void OnListSelectionChanged(object sender, EventArgs e)
        {
            SetStatus("list-sel:" + Convert.ToString(itemList.SelectedItem));
        }

        private void OnSliderChanged(object sender, EventArgs e)
        {
            sliderLabel.Text = "slider:" + slider.Value.ToString();
            SetStatus("slider:" + slider.Value.ToString());
        }

        private void OnTreeExpanded(object sender, TreeViewEventArgs e)
        {
            SetStatus("tree-expanded:" + e.Node.Text);
        }

        private void OnTreeCollapsed(object sender, TreeViewEventArgs e)
        {
            SetStatus("tree-collapsed:" + e.Node.Text);
        }

        private void OnRowClick(object sender, EventArgs e)
        {
            Control row = (Control)sender;
            SetStatus("row:" + row.Name);
        }

        private void OnScrollWatchTick(object sender, EventArgs e)
        {
            int current = -scrollPanel.AutoScrollPosition.Y;
            if (current != lastScrollY)
            {
                lastScrollY = current;
                scrollLabel.Text = "scroll:" + current.ToString();
            }
        }
    }

    public static class Program
    {
        [STAThread]
        public static void Main(string[] args)
        {
            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);
            Application.Run(new ScratchForm(ScratchOptions.Parse(args)));
        }
    }
}
