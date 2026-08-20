using System;
using System.Drawing;
using System.Windows.Forms;

namespace AgentDeskFixtureApp
{
    /// <summary>
    /// Approach item 1: clicks and mouse.
    /// </summary>
    internal static partial class FixtureCards
    {
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
    }
}
