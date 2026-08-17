using System;
using System.Drawing;
using System.Windows.Automation;
using System.Windows.Automation.Provider;
using System.Windows.Forms;

namespace AgentDeskFixtureApp
{
    /// <summary>
    /// Tracks a running vertical offset so each card is stacked below the
    /// last one, and hands the main form's constructor a final content
    /// height for <see cref="Form.AutoScrollMinSize"/> once every card has
    /// been built.
    /// </summary>
    public sealed class LayoutCursor
    {
        public LayoutCursor(int startY)
        {
            this.Bottom = startY;
        }

        public int Bottom { get; private set; }

        public int Advance(int height)
        {
            int y = this.Bottom;
            this.Bottom = this.Bottom + height + 16;
            return y;
        }
    }

    /// <summary>
    /// Shared card-shell layout so every card in every U5 file positions
    /// itself the same way: a titled <see cref="GroupBox"/> stacked at the
    /// cursor's next offset, wide enough for a two-column actor/readout
    /// layout.
    /// </summary>
    internal static class CardLayout
    {
        internal const int CardWidth = 840;
        internal const int Margin = 20;
        internal const int RowHeight = 34;
        internal const int RowStart = 24;

        internal static GroupBox AddCard(Form owner, LayoutCursor cursor, string title, int height)
        {
            GroupBox box = new GroupBox();
            box.Text = title;
            box.Location = new Point(Margin, cursor.Advance(height));
            box.Size = new Size(CardWidth, height);
            owner.Controls.Add(box);
            return box;
        }

        internal static int RowY(int rowIndex)
        {
            return RowStart + rowIndex * RowHeight;
        }
    }

    /// <summary>
    /// Subclasses an already-created control's window and answers the UIA
    /// root-object query (<c>WM_GETOBJECT</c>, <c>lParam == UiaRootObjectId</c>)
    /// with a custom provider, delegating every other message - including
    /// every other accessibility query - back to the control's own window
    /// procedure via <see cref="NativeWindow.DefWndProc"/>. This is the exact
    /// mechanism <c>probes/windows/24-fixture-e2e/06-cell-role-provider.ps1</c>
    /// proved compiles and reads back on this toolchain (A24-9), generalized
    /// so more than one card can reuse it instead of each hand-rolling its
    /// own <see cref="NativeWindow"/> subclass.
    ///
    /// Hooked from <see cref="Control.HandleCreated"/> rather than once at
    /// construction: WinForms recreates a control's native handle on some
    /// property changes, and a hook taken only at construction would go
    /// silently stale the first time that happens.
    /// </summary>
    internal abstract class UiaProviderHost : NativeWindow
    {
        private const int WmGetObject = 0x003D;
        private static readonly IntPtr UiaRootObjectId = new IntPtr(-25);

        private IRawElementProviderSimple cachedProvider;

        internal void Hook(Control control)
        {
            control.HandleCreated += delegate { this.AttachTo(control.Handle); };
            control.HandleDestroyed += delegate { this.DetachIfOwned(); };
            if (control.IsHandleCreated)
            {
                this.AttachTo(control.Handle);
            }
        }

        private void AttachTo(IntPtr handle)
        {
            this.DetachIfOwned();
            this.AssignHandle(handle);
            this.cachedProvider = null;
        }

        private void DetachIfOwned()
        {
            if (this.Handle != IntPtr.Zero)
            {
                this.ReleaseHandle();
            }
        }

        protected abstract IRawElementProviderSimple CreateProvider(IntPtr handle);

        protected override void WndProc(ref Message m)
        {
            if (m.Msg == WmGetObject && m.LParam == UiaRootObjectId)
            {
                if (this.cachedProvider == null)
                {
                    this.cachedProvider = this.CreateProvider(m.HWnd);
                }
                m.Result = AutomationInteropProvider.ReturnRawElementProvider(
                    m.HWnd, m.WParam, m.LParam, this.cachedProvider);
                return;
            }
            base.WndProc(ref m);
        }
    }

    /// <summary>
    /// Backs <c>switch-button</c>: a plain WinForms <see cref="Button"/> has
    /// no toggling shape at all, so <c>roles.rs</c>'s <c>button_role</c> never
    /// takes the <c>ToggleAvailable</c> arm against one. This host answers
    /// <c>IsTogglePatternAvailableProperty</c> and <see cref="ITogglePattern"/>
    /// - err, <see cref="IToggleProvider"/> - itself and leaves every other
    /// property (bounds, enabled, offscreen, name) to the host provider via
    /// <see cref="IRawElementProviderSimple.HostRawElementProvider"/>, so the
    /// control stays a fully functional, boundable, clickable Button that
    /// additionally advertises Toggle.
    ///
    /// <see cref="Toggle"/> can be called by UI Automation on a thread other
    /// than the UI thread (it is a direct COM call from the client process),
    /// so the state flip itself happens synchronously under a lock - the
    /// product reads the new state back immediately after issuing the
    /// command, and a deferred flip would race that read - while the visible
    /// button text update is marshalled onto the UI thread separately.
    /// </summary>
    internal sealed class ToggleButtonHost : UiaProviderHost
    {
        private readonly object gate = new object();
        private readonly Control owner;
        private readonly string automationId;
        private ToggleState state;

        internal ToggleButtonHost(Control owner, string automationId)
        {
            this.owner = owner;
            this.automationId = automationId;
            this.state = ToggleState.Off;
        }

        internal event EventHandler Toggled;

        internal ToggleState CurrentState
        {
            get { lock (this.gate) { return this.state; } }
        }

        internal void ToggleNow()
        {
            ToggleState next;
            lock (this.gate)
            {
                this.state = (this.state == ToggleState.On) ? ToggleState.Off : ToggleState.On;
                next = this.state;
            }
            if (this.owner.InvokeRequired)
            {
                this.owner.BeginInvoke((MethodInvoker)delegate { this.RaiseToggled(); });
            }
            else
            {
                this.RaiseToggled();
            }
        }

        private void RaiseToggled()
        {
            if (this.Toggled != null)
            {
                this.Toggled(this, EventArgs.Empty);
            }
        }

        protected override IRawElementProviderSimple CreateProvider(IntPtr handle)
        {
            return new Provider(this, handle);
        }

        private sealed class Provider : IRawElementProviderSimple, IToggleProvider
        {
            private readonly ToggleButtonHost host;
            private readonly IntPtr handle;

            internal Provider(ToggleButtonHost host, IntPtr handle)
            {
                this.host = host;
                this.handle = handle;
            }

            public ProviderOptions ProviderOptions
            {
                get { return ProviderOptions.ServerSideProvider; }
            }

            public object GetPatternProvider(int patternId)
            {
                if (patternId == TogglePatternIdentifiers.Pattern.Id)
                {
                    return this;
                }
                return null;
            }

            public object GetPropertyValue(int propertyId)
            {
                if (propertyId == AutomationElementIdentifiers.IsTogglePatternAvailableProperty.Id)
                {
                    return true;
                }
                if (propertyId == AutomationElementIdentifiers.ControlTypeProperty.Id)
                {
                    return ControlType.Button.Id;
                }
                if (propertyId == AutomationElementIdentifiers.AutomationIdProperty.Id)
                {
                    return this.host.automationId;
                }
                return null;
            }

            public IRawElementProviderSimple HostRawElementProvider
            {
                get { return AutomationInteropProvider.HostProviderFromHandle(this.handle); }
            }

            public void Toggle()
            {
                this.host.ToggleNow();
            }

            public ToggleState ToggleState
            {
                get { return this.host.CurrentState; }
            }
        }
    }

    /// <summary>
    /// Backs both <c>menu-disclosure</c> (<c>ControlType.Button</c>) and
    /// <c>outline-parent</c> (<c>ControlType.TreeItem</c>): a control
    /// answering <c>IsExpandCollapsePatternAvailable</c> and
    /// <see cref="IExpandCollapseProvider"/> itself, parameterized on which
    /// <c>ControlType</c> to report so the same expand/collapse state
    /// machine and thread-safety discipline back both targets rather than
    /// being duplicated per role.
    ///
    /// <see cref="ToolStripSplitButton"/> was tried first for
    /// <c>menu-disclosure</c> as the zero-custom-code shape - it did not
    /// survive the read-back-while-building loop: <c>ToolStripItem.Name</c>
    /// does not surface as UIA <c>AutomationId</c> on this stock provider
    /// (measured empirically; confirmed by <c>--native-id menu-disclosure</c>
    /// returning zero matches against a real running fixture), unlike
    /// <c>Control.Name</c>, which A24-2 measured and which this class's own
    /// <c>owner</c> control still carries. This host is what actually ships.
    /// </summary>
    internal sealed class ExpandCollapseButtonHost : UiaProviderHost
    {
        private readonly object gate = new object();
        private readonly Control owner;
        private readonly string automationId;
        private readonly int controlTypeId;
        private ExpandCollapseState state;

        internal ExpandCollapseButtonHost(Control owner, string automationId)
            : this(owner, automationId, ControlType.Button.Id)
        {
        }

        internal ExpandCollapseButtonHost(Control owner, string automationId, int controlTypeId)
        {
            this.owner = owner;
            this.automationId = automationId;
            this.controlTypeId = controlTypeId;
            this.state = ExpandCollapseState.Collapsed;
        }

        internal event EventHandler StateChanged;

        internal ExpandCollapseState CurrentState
        {
            get { lock (this.gate) { return this.state; } }
        }

        internal void ExpandNow()
        {
            this.SetState(ExpandCollapseState.Expanded);
        }

        internal void CollapseNow()
        {
            this.SetState(ExpandCollapseState.Collapsed);
        }

        private void SetState(ExpandCollapseState next)
        {
            lock (this.gate)
            {
                this.state = next;
            }
            if (this.owner.InvokeRequired)
            {
                this.owner.BeginInvoke((MethodInvoker)delegate { this.RaiseStateChanged(); });
            }
            else
            {
                this.RaiseStateChanged();
            }
        }

        private void RaiseStateChanged()
        {
            if (this.StateChanged != null)
            {
                this.StateChanged(this, EventArgs.Empty);
            }
        }

        protected override IRawElementProviderSimple CreateProvider(IntPtr handle)
        {
            return new Provider(this, handle);
        }

        private sealed class Provider : IRawElementProviderSimple, IExpandCollapseProvider
        {
            private readonly ExpandCollapseButtonHost host;
            private readonly IntPtr handle;

            internal Provider(ExpandCollapseButtonHost host, IntPtr handle)
            {
                this.host = host;
                this.handle = handle;
            }

            public ProviderOptions ProviderOptions
            {
                get { return ProviderOptions.ServerSideProvider; }
            }

            public object GetPatternProvider(int patternId)
            {
                if (patternId == ExpandCollapsePatternIdentifiers.Pattern.Id)
                {
                    return this;
                }
                return null;
            }

            public object GetPropertyValue(int propertyId)
            {
                if (propertyId == AutomationElementIdentifiers.IsExpandCollapsePatternAvailableProperty.Id)
                {
                    return true;
                }
                if (propertyId == AutomationElementIdentifiers.ControlTypeProperty.Id)
                {
                    return this.host.controlTypeId;
                }
                if (propertyId == AutomationElementIdentifiers.AutomationIdProperty.Id)
                {
                    return this.host.automationId;
                }
                return null;
            }

            public IRawElementProviderSimple HostRawElementProvider
            {
                get { return AutomationInteropProvider.HostProviderFromHandle(this.handle); }
            }

            public void Expand()
            {
                this.host.ExpandNow();
            }

            public void Collapse()
            {
                this.host.CollapseNow();
            }

            public ExpandCollapseState ExpandCollapseState
            {
                get { return this.host.CurrentState; }
            }
        }
    }

    /// <summary>
    /// Overrides <c>ControlTypeProperty</c> and <c>AutomationIdProperty</c>,
    /// leaving every other query - bounds, enabled, offscreen, name - to the
    /// host control's own default provider. Used wherever a target needs a
    /// <c>ControlType</c> no stock control produces but no pattern-gated
    /// role refinement is involved (unlike <see cref="ToggleButtonHost"/> /
    /// <see cref="ExpandCollapseButtonHost"/>): <c>menu-fire-item</c>
    /// (<see cref="ControlType.MenuItem"/>, since <c>ToolStripItem.Name</c>
    /// does not surface as <c>AutomationId</c> the way
    /// <see cref="Control.Name"/> does - the same finding that moved
    /// <c>menu-disclosure</c> off <see cref="ToolStripSplitButton"/>) and the
    /// outline's own container plus its leaf items
    /// (<see cref="ControlType.Tree"/> / <see cref="ControlType.TreeItem"/>,
    /// since <c>TreeNode.Name</c> does not surface as <c>AutomationId</c>
    /// either - measured the same way, <c>--native-id outline-parent</c>
    /// returned zero matches against the stock <see cref="TreeView"/>
    /// shape this replaced).
    ///
    /// A registered custom raw provider does **not** transparently inherit
    /// the host's own <c>InvokePattern</c> availability the way bounds or
    /// enabled-state merge through <see cref="IRawElementProviderSimple.HostRawElementProvider"/>
    /// - measured live against <c>menu-fire-item</c>: with no explicit
    /// <see cref="IInvokeProvider"/>, <c>InvokePattern.Invoke</c> came back
    /// <c>skipped</c> and the click chain's <c>LegacyIAccessible.DoDefaultAction</c>
    /// fallback reported success without ever running the Click handler
    /// (<c>menu-status</c> stayed <c>idle</c>), where the identical click
    /// against an unmodified <see cref="Button"/> in the same process
    /// invoked correctly. Any override host whose target must still be
    /// clickable through the CLI therefore implements
    /// <see cref="IInvokeProvider"/> itself and delegates to the caller's
    /// own invoke callback, rather than relying on host delegation for
    /// click delivery.
    /// </summary>
    internal sealed class ControlTypeOverrideHost : UiaProviderHost
    {
        private readonly string automationId;
        private readonly int controlTypeId;
        private readonly Action invokeAction;

        internal ControlTypeOverrideHost(string automationId, int controlTypeId)
            : this(automationId, controlTypeId, null)
        {
        }

        internal ControlTypeOverrideHost(string automationId, int controlTypeId, Action invokeAction)
        {
            this.automationId = automationId;
            this.controlTypeId = controlTypeId;
            this.invokeAction = invokeAction;
        }

        protected override IRawElementProviderSimple CreateProvider(IntPtr handle)
        {
            return new Provider(this.automationId, this.controlTypeId, this.invokeAction, handle);
        }

        private sealed class Provider : IRawElementProviderSimple, IInvokeProvider
        {
            private readonly string automationId;
            private readonly int controlTypeId;
            private readonly Action invokeAction;
            private readonly IntPtr handle;

            internal Provider(string automationId, int controlTypeId, Action invokeAction, IntPtr handle)
            {
                this.automationId = automationId;
                this.controlTypeId = controlTypeId;
                this.invokeAction = invokeAction;
                this.handle = handle;
            }

            public ProviderOptions ProviderOptions
            {
                get { return ProviderOptions.ServerSideProvider; }
            }

            public object GetPatternProvider(int patternId)
            {
                if (this.invokeAction != null && patternId == InvokePatternIdentifiers.Pattern.Id)
                {
                    return this;
                }
                return null;
            }

            public object GetPropertyValue(int propertyId)
            {
                if (propertyId == AutomationElementIdentifiers.ControlTypeProperty.Id)
                {
                    return this.controlTypeId;
                }
                if (propertyId == AutomationElementIdentifiers.AutomationIdProperty.Id)
                {
                    return this.automationId;
                }
                if (this.invokeAction != null
                    && propertyId == AutomationElementIdentifiers.IsInvokePatternAvailableProperty.Id)
                {
                    return true;
                }
                return null;
            }

            public void Invoke()
            {
                if (this.invokeAction != null)
                {
                    this.invokeAction();
                }
            }

            public IRawElementProviderSimple HostRawElementProvider
            {
                get { return AutomationInteropProvider.HostProviderFromHandle(this.handle); }
            }
        }
    }

    /// <summary>
    /// Every resident target the plan's approach items 1-4, 5's
    /// non-provider members, 6, 9 and 12 describe, built eagerly into the
    /// main form so every id is present in the identity manifest
    /// (<c>AgentDeskFixture.ids.txt</c>) the moment <c>Main</c> writes it.
    /// </summary>
    internal static class FixtureCards
    {
        internal static void Build(Form owner, LayoutCursor cursor)
        {
            BuildClicksAndMouse(owner, cursor);
            BuildTextInput(owner, cursor);
            BuildStateControls(owner, cursor);
            BuildChoices(owner, cursor);
            BuildCollectionsAndDisclosure(owner, cursor);
            BuildScroll(owner, cursor);
            BuildZeroBounds(owner, cursor);
            BuildSurfaces(owner, cursor);
        }

        private static Button NewButton(GroupBox card, string id, string text, int x, int y, int width)
        {
            Button button = new Button();
            FixtureIdentity.Assign(button, id);
            button.Text = text;
            button.Location = new Point(x, y);
            button.Size = new Size(width, 26);
            card.Controls.Add(button);
            return button;
        }

        private static TextBox NewStatus(GroupBox card, string id, int x, int y, int width)
        {
            TextBox status = FixtureStatus.Create(id);
            status.Location = new Point(x, y);
            status.Width = width;
            card.Controls.Add(status);
            return status;
        }

        // ---------------------------------------------------------------- item 1: clicks and mouse

        private static void BuildClicksAndMouse(Form owner, LayoutCursor cursor)
        {
            GroupBox card = CardLayout.AddCard(owner, cursor, "Clicks and mouse", 260);

            Button primary = NewButton(card, "primary-button", "Primary", 16, CardLayout.RowY(0), 110);
            primary.Click += delegate { FixtureStatus.SetValue("click-status", "clicked"); };
            NewStatus(card, "click-status", 140, CardLayout.RowY(0), 200);

            Button doubleTarget = NewButton(card, "double-target", "Double", 16, CardLayout.RowY(1), 110);
            doubleTarget.DoubleClick += delegate { FixtureStatus.SetValue("double-status", "double-clicked"); };
            NewStatus(card, "double-status", 140, CardLayout.RowY(1), 200);

            BuildTripleTarget(card, CardLayout.RowY(2));
            NewStatus(card, "triple-status", 140, CardLayout.RowY(2), 200);

            Button contextTarget = NewButton(card, "context-target", "Context", 16, CardLayout.RowY(3), 110);
            BuildContextChoicePopup(contextTarget);
            NewStatus(card, "context-status", 140, CardLayout.RowY(3), 200);

            Button hoverTarget = NewButton(card, "hover-target", "Hover", 16, CardLayout.RowY(4), 110);
            hoverTarget.MouseEnter += delegate { FixtureStatus.SetValue("hover-status", "hovered"); };
            NewStatus(card, "hover-status", 140, CardLayout.RowY(4), 200);

            BuildTwinControls(card, CardLayout.RowY(5));
        }

        /// <summary>
        /// A physical triple-click delivers three separate WM_LBUTTONDOWN/UP
        /// pairs (Windows only special-cases the second click as
        /// WM_LBUTTONDBLCLK; there is no native "triple" message), so this
        /// counts <see cref="Control.Click"/> events instead and resets the
        /// counter once the system double-click interval has elapsed since
        /// the first click in the run - a click arriving after that window
        /// starts a fresh count of one rather than accumulating forever.
        /// </summary>
        private static void BuildTripleTarget(GroupBox card, int y)
        {
            Button tripleTarget = NewButton(card, "triple-target", "Triple", 16, y, 110);
            int clickCount = 0;
            DateTime windowStart = DateTime.MinValue;
            tripleTarget.Click += delegate
            {
                DateTime now = DateTime.UtcNow;
                double windowMs = SystemInformation.DoubleClickTime * 2;
                if (clickCount == 0 || (now - windowStart).TotalMilliseconds > windowMs)
                {
                    clickCount = 1;
                    windowStart = now;
                }
                else
                {
                    clickCount = clickCount + 1;
                }
                if (clickCount >= 3)
                {
                    FixtureStatus.SetValue("triple-status", "triple-clicked");
                    clickCount = 0;
                }
            };
        }

        private static void BuildTwinControls(GroupBox card, int y)
        {
            Button twinA = new Button();
            FixtureIdentity.Assign(twinA, "twin-control");
            twinA.Text = "Twin";
            twinA.Location = new Point(16, y);
            twinA.Size = new Size(90, 26);
            card.Controls.Add(twinA);

            Button twinB = new Button();
            FixtureIdentity.Assign(twinB, "twin-control");
            twinB.Text = "Twin";
            twinB.Location = new Point(112, y);
            twinB.Size = new Size(90, 26);
            card.Controls.Add(twinB);

            Button moveTwins = NewButton(card, "move-twins", "Move twins", 210, y, 110);
            moveTwins.Click += delegate
            {
                int dx = 14;
                twinA.Location = new Point(twinA.Location.X + dx, twinA.Location.Y);
                twinB.Location = new Point(twinB.Location.X + dx, twinB.Location.Y);
            };
        }

        /// <summary>
        /// A real <see cref="ContextMenuStrip"/> was tried first - it is
        /// the obvious, zero-custom-code shape for "right-click, choose an
        /// item" - and did not survive the read-back loop: its item is a
        /// <see cref="ToolStripMenuItem"/>, and <c>ToolStripItem.Name</c>
        /// does not surface as <c>AutomationId</c> (the same finding that
        /// moved <c>menu-fire-item</c> and <c>menu-disclosure</c> off
        /// <see cref="ToolStripItem"/>-based shapes); measured live,
        /// <c>find --native-id context-choice</c> against a real running
        /// build returned zero matches even with the native context menu
        /// genuinely open (a real right-click delivered headed, confirmed
        /// by <c>right-click</c>'s own success). What ships instead is a
        /// borderless, non-modal <see cref="Form"/> - the same
        /// show/hide-on-<see cref="Form.FormClosing"/> reuse pattern as
        /// <c>fixture-overlay</c> and the duplicate windows - carrying a
        /// real <see cref="Button"/> for <c>context-choice</c>, appearing at
        /// the right-click point. It sacrifices genuine Win32 menu semantics
        /// (no <c>ControlType.Menu</c> family role on this surface) for the
        /// one property R2 makes non-negotiable: the staged id actually
        /// resolves by <c>--native-id</c> while its surface is up. A first
        /// build dismissed itself on <see cref="Form.Deactivate"/> to mimic
        /// a real context menu's click-elsewhere dismissal; measured live
        /// against the harness, a spawned CLI process's own right-click
        /// call reliably moves OS foreground away from this popup within
        /// its own process lifetime, so <c>Deactivate</c> fired and hid the
        /// window before the harness's own poll could observe it open -
        /// the same resolves-while-up property R2 asks for, defeated by a
        /// dismissal rule this surface never needed: both callers already
        /// dismiss it explicitly by clicking <c>context-choice</c>.
        /// </summary>
        private static void BuildContextChoicePopup(Control contextTarget)
        {
            Form popup = new Form();
            popup.FormBorderStyle = FormBorderStyle.None;
            popup.ShowInTaskbar = false;
            popup.StartPosition = FormStartPosition.Manual;
            popup.Size = new Size(120, 32);

            Button choice = new Button();
            FixtureIdentity.Assign(choice, "context-choice");
            choice.Text = "Choose";
            choice.Dock = DockStyle.Fill;
            popup.Controls.Add(choice);

            popup.FormClosing += delegate(object sender, FormClosingEventArgs e)
            {
                e.Cancel = true;
                popup.Hide();
            };
            choice.Click += delegate
            {
                FixtureStatus.SetValue("context-status", "chosen");
                popup.Hide();
            };

            contextTarget.MouseUp += delegate(object sender, MouseEventArgs e)
            {
                if (e.Button == MouseButtons.Right)
                {
                    popup.Location = contextTarget.PointToScreen(new Point(e.X, e.Y));
                    popup.Show();
                }
            };
        }

        // ---------------------------------------------------------------- item 2: text input

        private static void BuildTextInput(Form owner, LayoutCursor cursor)
        {
            GroupBox card = CardLayout.AddCard(owner, cursor, "Text input", 130);

            TextBox textInput = new TextBox();
            FixtureIdentity.Assign(textInput, "text-input");
            textInput.Location = new Point(16, CardLayout.RowY(0));
            textInput.Width = 200;
            card.Controls.Add(textInput);
            textInput.TextChanged += delegate { FixtureStatus.SetValue("text-status", "changed"); };
            NewStatus(card, "text-status", 226, CardLayout.RowY(0), 200);

            TextBox secureInput = new TextBox();
            FixtureIdentity.Assign(secureInput, "secure-input");
            secureInput.UseSystemPasswordChar = true;
            secureInput.Location = new Point(16, CardLayout.RowY(1));
            secureInput.Width = 200;
            card.Controls.Add(secureInput);

            TextBox multilineInput = new TextBox();
            FixtureIdentity.Assign(multilineInput, "multiline-input");
            multilineInput.Multiline = true;
            multilineInput.Location = new Point(16, CardLayout.RowY(2));
            multilineInput.Size = new Size(400, 50);
            card.Controls.Add(multilineInput);
        }

        // ---------------------------------------------------------------- item 3: state controls

        private static void BuildStateControls(Form owner, LayoutCursor cursor)
        {
            GroupBox card = CardLayout.AddCard(owner, cursor, "State controls", 160);

            CheckBox toggleBox = new CheckBox();
            FixtureIdentity.Assign(toggleBox, "toggle-box");
            toggleBox.Text = "Toggle";
            toggleBox.Location = new Point(16, CardLayout.RowY(0));
            toggleBox.Size = new Size(110, 26);
            card.Controls.Add(toggleBox);
            toggleBox.CheckedChanged += delegate
            {
                FixtureStatus.SetValue("toggle-status", toggleBox.Checked ? "checked" : "unchecked");
            };
            NewStatus(card, "toggle-status", 140, CardLayout.RowY(0), 200);

            Button switchButton = NewButton(card, "switch-button", "Switch", 16, CardLayout.RowY(1), 110);
            ToggleButtonHost switchHost = new ToggleButtonHost(switchButton, "switch-button");
            switchHost.Hook(switchButton);
            switchHost.Toggled += delegate
            {
                switchButton.Text = (switchHost.CurrentState == ToggleState.On) ? "Switch: on" : "Switch: off";
            };
            switchButton.Click += delegate { switchHost.ToggleNow(); };

            TrackBar valueSlider = new TrackBar();
            FixtureIdentity.Assign(valueSlider, "value-slider");
            valueSlider.Minimum = 0;
            valueSlider.Maximum = 10;
            valueSlider.Location = new Point(140, CardLayout.RowY(1));
            valueSlider.Width = 200;
            card.Controls.Add(valueSlider);
            valueSlider.ValueChanged += delegate { FixtureStatus.SetValue("slider-status", "changed"); };
            NewStatus(card, "slider-status", 350, CardLayout.RowY(1), 160);

            BuildValueStepper(card, CardLayout.RowY(2));
        }

        /// <summary>
        /// No probe had measured whether <c>NumericUpDown</c> presents
        /// <c>ControlType.Spinner</c> before this fixture was read back
        /// live. It does: the up-down pair's UIA-visible child carries
        /// <c>ControlType.Spinner</c> (role <c>incrementor</c>) even though
        /// the composite control's own root reads as <c>ComboBox</c>, which
        /// satisfies R4's role-coverage requirement without a
        /// provider-based fallback.
        /// </summary>
        private static void BuildValueStepper(GroupBox card, int y)
        {
            NumericUpDown stepper = new NumericUpDown();
            FixtureIdentity.Assign(stepper, "value-stepper");
            stepper.Minimum = 0;
            stepper.Maximum = 100;
            stepper.Location = new Point(16, y);
            stepper.Width = 100;
            card.Controls.Add(stepper);
            stepper.ValueChanged += delegate { FixtureStatus.SetValue("stepper-status", "changed"); };
            NewStatus(card, "stepper-status", 140, y, 200);
        }

        // ---------------------------------------------------------------- item 4: choices

        private static void BuildChoices(Form owner, LayoutCursor cursor)
        {
            GroupBox card = CardLayout.AddCard(owner, cursor, "Choices", 200);

            ComboBox optionPicker = new ComboBox();
            FixtureIdentity.Assign(optionPicker, "option-picker");
            optionPicker.DropDownStyle = ComboBoxStyle.DropDownList;
            optionPicker.Items.Add("one");
            optionPicker.Items.Add("two");
            optionPicker.Items.Add("three");
            optionPicker.Location = new Point(16, CardLayout.RowY(0));
            optionPicker.Width = 150;
            card.Controls.Add(optionPicker);
            optionPicker.SelectedIndexChanged += delegate { FixtureStatus.SetValue("picker-status", "changed"); };
            NewStatus(card, "picker-status", 176, CardLayout.RowY(0), 200);

            BuildRadioGroup(card, CardLayout.RowY(1));
            NewStatus(card, "radio-status", 400, CardLayout.RowY(1), 200);

            TabControl tabView = new TabControl();
            FixtureIdentity.Assign(tabView, "tab-view");
            tabView.Location = new Point(16, CardLayout.RowY(2) + 6);
            tabView.Size = new Size(300, 60);
            TabPage tabOne = new TabPage("One");
            FixtureIdentity.Assign(tabOne, "tab-one");
            TabPage tabTwo = new TabPage("Two");
            FixtureIdentity.Assign(tabTwo, "tab-two");
            tabView.TabPages.Add(tabOne);
            tabView.TabPages.Add(tabTwo);
            tabView.SelectedIndexChanged += delegate { FixtureStatus.SetValue("tab-status", "changed"); };
            card.Controls.Add(tabView);
            NewStatus(card, "tab-status", 330, CardLayout.RowY(2) + 6, 200);

            LinkLabel exampleLink = new LinkLabel();
            FixtureIdentity.Assign(exampleLink, "example-link");
            exampleLink.Text = "Example link";
            exampleLink.Location = new Point(16, CardLayout.RowY(4) + 14);
            exampleLink.AutoSize = true;
            card.Controls.Add(exampleLink);
        }

        private static void BuildRadioGroup(GroupBox card, int y)
        {
            RadioButton radioOne = new RadioButton();
            FixtureIdentity.Assign(radioOne, "radio-one");
            radioOne.Text = "One";
            radioOne.Location = new Point(16, y);
            radioOne.Size = new Size(80, 24);
            card.Controls.Add(radioOne);

            RadioButton radioTwo = new RadioButton();
            FixtureIdentity.Assign(radioTwo, "radio-two");
            radioTwo.Text = "Two";
            radioTwo.Location = new Point(100, y);
            radioTwo.Size = new Size(80, 24);
            card.Controls.Add(radioTwo);

            RadioButton radioThree = new RadioButton();
            FixtureIdentity.Assign(radioThree, "radio-three");
            radioThree.Text = "Three";
            radioThree.Location = new Point(184, y);
            radioThree.Size = new Size(80, 24);
            card.Controls.Add(radioThree);

            EventHandler onChanged = delegate { FixtureStatus.SetValue("radio-status", "changed"); };
            radioOne.CheckedChanged += onChanged;
            radioTwo.CheckedChanged += onChanged;
            radioThree.CheckedChanged += onChanged;
        }

        // ---------------------------------------------------------------- item 5: collections and disclosure

        private static void BuildCollectionsAndDisclosure(Form owner, LayoutCursor cursor)
        {
            GroupBox card = CardLayout.AddCard(owner, cursor, "Collections and disclosure", 300);

            BuildOutlineTree(card, CardLayout.RowY(0));
            BuildMenuDisclosure(card, 260, CardLayout.RowY(0));
            BuildItemList(card, 16, CardLayout.RowY(0) + 130);
            BuildMenuStrip(card, 400, CardLayout.RowY(0) + 130);
        }

        /// <summary>
        /// <c>TreeView</c>'s stock provider gives the <em>container</em> a
        /// working <c>AutomationId</c> from <c>Control.Name</c> (measured
        /// live), but <c>TreeNode.Name</c> does not surface as
        /// <c>AutomationId</c> at all - a <see cref="TreeNode"/> is not a
        /// <see cref="Control"/>, so it never reaches the identity path
        /// A24-2 measured. Confirmed by driving a real build of this
        /// fixture through <c>agent-desktop find --native-id
        /// outline-parent</c>: zero matches against the stock
        /// <see cref="TreeView"/> shape. So the whole outline, container and
        /// items alike, is built from real <see cref="Control"/>s wearing
        /// <see cref="ControlTypeOverrideHost"/> / <see cref="ExpandCollapseButtonHost"/>
        /// overrides - the same mechanism already proven for
        /// <c>switch-button</c> and the cell targets - arranged with
        /// indentation to read as a tree, rather than a real
        /// <see cref="TreeView"/>. All three items stay resident and
        /// visible regardless of <c>outline-parent</c>'s expand/collapse
        /// state (the plan's resident/staged split places them as
        /// resident), so KTD8's expand/collapse assertion is carried purely
        /// by <c>outline-parent</c>'s own reported
        /// <c>ExpandCollapseState</c>, read back through <c>get</c>.
        /// </summary>
        private static void BuildOutlineTree(GroupBox card, int y)
        {
            Panel outlineTree = new Panel();
            FixtureIdentity.Assign(outlineTree, "outline-tree");
            outlineTree.Location = new Point(16, y);
            outlineTree.Size = new Size(220, 110);
            outlineTree.BorderStyle = BorderStyle.FixedSingle;
            card.Controls.Add(outlineTree);
            new ControlTypeOverrideHost("outline-tree", ControlType.Tree.Id).Hook(outlineTree);

            Label parentLabel = new Label();
            FixtureIdentity.Assign(parentLabel, "outline-parent");
            parentLabel.Text = "Parent";
            parentLabel.Location = new Point(4, 4);
            parentLabel.AutoSize = true;
            outlineTree.Controls.Add(parentLabel);
            ExpandCollapseButtonHost parentHost = new ExpandCollapseButtonHost(
                parentLabel, "outline-parent", ControlType.TreeItem.Id);
            parentHost.Hook(parentLabel);
            parentHost.StateChanged += delegate
            {
                parentLabel.Text = (parentHost.CurrentState == ExpandCollapseState.Expanded)
                    ? "Parent (expanded)"
                    : "Parent";
            };

            Label childA = new Label();
            FixtureIdentity.Assign(childA, "outline-child-a");
            childA.Text = "Child A";
            childA.Location = new Point(20, 28);
            childA.AutoSize = true;
            outlineTree.Controls.Add(childA);
            new ControlTypeOverrideHost("outline-child-a", ControlType.TreeItem.Id).Hook(childA);

            Label childB = new Label();
            FixtureIdentity.Assign(childB, "outline-child-b");
            childB.Text = "Child B";
            childB.Location = new Point(20, 48);
            childB.AutoSize = true;
            outlineTree.Controls.Add(childB);
            new ControlTypeOverrideHost("outline-child-b", ControlType.TreeItem.Id).Hook(childB);
        }

        /// <summary>
        /// <see cref="ToolStripSplitButton"/> was tried first for the
        /// zero-custom-code <c>ControlType.SplitButton</c> shape and did not
        /// survive the read-back loop for the same reason
        /// <c>menu-fire-item</c> did not: <c>ToolStripItem.Name</c> does not
        /// surface as <c>AutomationId</c>. What ships is the
        /// <see cref="ExpandCollapseButtonHost"/> shape on a real
        /// <see cref="Button"/>.
        /// </summary>
        private static void BuildMenuDisclosure(GroupBox card, int x, int y)
        {
            Button menuDisclosure = NewButton(card, "menu-disclosure", "Disclosure", x, y, 120);
            ExpandCollapseButtonHost host = new ExpandCollapseButtonHost(menuDisclosure, "menu-disclosure");
            host.Hook(menuDisclosure);
            host.StateChanged += delegate
            {
                menuDisclosure.Text = (host.CurrentState == ExpandCollapseState.Expanded)
                    ? "Disclosure: open"
                    : "Disclosure: closed";
            };
            menuDisclosure.Click += delegate
            {
                if (host.CurrentState == ExpandCollapseState.Expanded)
                {
                    host.CollapseNow();
                }
                else
                {
                    host.ExpandNow();
                }
            };
        }

        private static void BuildItemList(GroupBox card, int x, int y)
        {
            ListBox itemList = new ListBox();
            FixtureIdentity.Assign(itemList, "item-list");
            itemList.Location = new Point(x, y);
            itemList.Size = new Size(200, 110);
            for (int i = 1; i <= 6; i++)
            {
                itemList.Items.Add("Item " + i.ToString());
            }
            card.Controls.Add(itemList);
        }

        /// <summary>
        /// A top-level <see cref="MenuStrip"/> item was the plan's original
        /// design ("a provider at rest, resolvable without opening
        /// anything"), but <c>ToolStripMenuItem.Name</c> does not surface as
        /// <c>AutomationId</c> either - measured the same way, zero matches
        /// on <c>--native-id menu-fire-item</c> against a real running
        /// build. <c>ControlType.MenuItem</c> requires no pattern-gated
        /// refinement (<c>roles.rs</c> maps it unconditionally), so a plain
        /// <see cref="Button"/> wearing a <see cref="ControlTypeOverrideHost"/>
        /// keeps the "resident, no menu-mode needed, dispatchable through
        /// the semantic tier" property the plan wanted from the MenuStrip
        /// shape while actually resolving by id.
        /// </summary>
        private static void BuildMenuStrip(GroupBox card, int x, int y)
        {
            Button fireItem = NewButton(card, "menu-fire-item", "Fire", x, y, 100);
            EventHandler fire = delegate { FixtureStatus.SetValue("menu-status", "fired"); };
            ControlTypeOverrideHost fireHost = new ControlTypeOverrideHost(
                "menu-fire-item", ControlType.MenuItem.Id, delegate { fire(fireItem, EventArgs.Empty); });
            fireHost.Hook(fireItem);
            fireItem.Click += fire;
            NewStatus(card, "menu-status", x + 110, y, 200);
        }

        // ---------------------------------------------------------------- item 6: scroll

        private static void BuildScroll(Form owner, LayoutCursor cursor)
        {
            GroupBox card = CardLayout.AddCard(owner, cursor, "Scroll", 220);

            Panel scrollArea = new Panel();
            FixtureIdentity.Assign(scrollArea, "scroll-area");
            scrollArea.Location = new Point(16, CardLayout.RowY(0));
            scrollArea.Size = new Size(400, 150);
            scrollArea.AutoScroll = true;
            card.Controls.Add(scrollArea);

            int rowHeight = 24;
            int rowCount = 60;
            for (int i = 0; i < rowCount; i++)
            {
                Label row = new Label();
                FixtureIdentity.Assign(row, "scroll-row-" + (i + 1).ToString());
                row.Text = "Row " + (i + 1).ToString();
                row.Location = new Point(4, i * rowHeight);
                row.Size = new Size(360, rowHeight - 2);
                scrollArea.Controls.Add(row);
            }

            // Forces the AutoScroll panel's real scrollbar styles to be
            // present at construction, which is what makes the stock UIA
            // provider synthesize ScrollPattern for it (R5's ancestor-scroll
            // ladder route) - deferring to the natural extent computed from
            // the 60 realized rows alone left this unconfirmed until
            // verified live.
            scrollArea.AutoScrollMinSize = new Size(360, rowCount * rowHeight);

            TextBox scrollOffset = FixtureStatus.Create("scroll-offset");
            scrollOffset.Location = new Point(432, CardLayout.RowY(0));
            scrollOffset.Width = 200;
            card.Controls.Add(scrollOffset);
            scrollArea.Scroll += delegate
            {
                scrollOffset.Text = (-scrollArea.AutoScrollPosition.Y).ToString();
            };
            scrollOffset.Text = "0";
        }

        // ---------------------------------------------------------------- item 9: zero bounds

        /// <summary>
        /// The plan names two mechanisms for reporting a zero
        /// <c>BoundingRectangle</c> - <c>GetPropertyValue</c> and
        /// <c>IRawElementProviderFragment</c> - citing A24-9 as having
        /// proved the route; A24-9 in fact never measured
        /// <c>BoundingRectangle</c> at all, only <c>ControlType</c> and the
        /// grid patterns, so the citation was an extrapolation. Measured
        /// live against a running build of this fixture, in three separate
        /// configurations: (1) a normal-size real <see cref="Button"/> with
        /// <c>GetPropertyValue(BoundingRectangleProperty)</c> overridden to
        /// zero - <c>get --property bounds</c> still read the button's
        /// genuine on-screen rectangle; (2) the same button with
        /// <c>IRawElementProviderFragment.BoundingRectangle</c> overridden
        /// instead - identical result; (3) the override combined with a
        /// real, degenerate 1x1 control size - <c>bounds</c> still read the
        /// real 1x1 rectangle, never the overridden zero. UI Automation
        /// evidently sources <c>BoundingRectangle</c> for the root provider
        /// of a real HWND from the window's own geometry regardless of what
        /// either provider interface reports; neither mechanism this
        /// toolchain offers can force it to zero. A genuinely 0x0-sized
        /// real control was also tried and is excluded from the UIA tree
        /// entirely - <c>find --native-id</c> against it returns zero
        /// matches - so that is not a route either. What ships is the
        /// closest achievable, honestly-reported approximation: a real
        /// <see cref="Button"/> at the smallest size that stays resolvable
        /// (1x1, not 0x0), reporting exactly what it is - a 1x1 rectangle,
        /// not a fabricated zero one.
        /// </summary>
        private static void BuildZeroBounds(Form owner, LayoutCursor cursor)
        {
            GroupBox card = CardLayout.AddCard(owner, cursor, "Zero bounds", 90);

            Button zeroBoundsButton = NewButton(card, "zero-bounds-button", "Zero bounds", 16, CardLayout.RowY(0), 130);
            zeroBoundsButton.Size = new Size(1, 1);
        }

        // ---------------------------------------------------------------- item 12: surfaces

        private static void BuildSurfaces(Form owner, LayoutCursor cursor)
        {
            GroupBox card = CardLayout.AddCard(owner, cursor, "Surfaces", 90);

            Button openSheet = NewButton(card, "open-sheet", "Open sheet", 16, CardLayout.RowY(0), 130);
            NewStatus(card, "sheet-status", 160, CardLayout.RowY(0), 200);

            FixtureSheet sheet = new FixtureSheet();
            openSheet.Click += delegate
            {
                // Deferred: a modal ShowDialog() called synchronously from
                // this handler would not return until the dialog closes,
                // and this handler runs on a blocking cross-process COM call
                // (InvokePattern.Invoke) - blocking it here would hang the
                // click command itself rather than merely the UI thread.
                owner.BeginInvoke((MethodInvoker)delegate { sheet.ShowDialog(owner); });
            };
        }
    }

    /// <summary>
    /// The modal dialog <c>open-sheet</c> shows. Built once and reused for
    /// the lifetime of the process: <see cref="Form.ShowDialog(IWin32Window)"/>
    /// does not dispose the form on <see cref="Form.Close"/>, so no
    /// cancel-close/hide trick is needed the way the duplicate-title windows
    /// and the occlusion overlay (shown non-modally via <c>Show</c>) require.
    /// </summary>
    internal sealed class FixtureSheet : Form
    {
        internal FixtureSheet()
        {
            this.Text = "Sheet";
            this.FormBorderStyle = FormBorderStyle.FixedDialog;
            this.StartPosition = FormStartPosition.CenterParent;
            this.ClientSize = new Size(320, 160);
            this.MinimizeBox = false;
            this.MaximizeBox = false;

            Label title = new Label();
            FixtureIdentity.Assign(title, "sheet-title");
            title.Text = "Sheet dialog";
            title.Location = new Point(16, 16);
            title.AutoSize = true;
            this.Controls.Add(title);

            TextBox field = new TextBox();
            FixtureIdentity.Assign(field, "sheet-field");
            field.Location = new Point(16, 44);
            field.Width = 260;
            this.Controls.Add(field);

            Button confirm = new Button();
            FixtureIdentity.Assign(confirm, "confirm-sheet");
            confirm.Text = "Confirm";
            confirm.Location = new Point(16, 90);
            confirm.Size = new Size(110, 28);
            confirm.Click += delegate
            {
                FixtureStatus.SetValue("sheet-status", "confirmed");
                this.Close();
            };
            this.Controls.Add(confirm);

            Button cancel = new Button();
            FixtureIdentity.Assign(cancel, "cancel-sheet");
            cancel.Text = "Cancel";
            cancel.Location = new Point(140, 90);
            cancel.Size = new Size(110, 28);
            cancel.Click += delegate
            {
                FixtureStatus.SetValue("sheet-status", "cancelled");
                this.Close();
            };
            this.Controls.Add(cancel);
        }

        protected override bool ShowWithoutActivation
        {
            get { return true; }
        }
    }
}
